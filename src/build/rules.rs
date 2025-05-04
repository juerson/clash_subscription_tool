use crate::build::{constants, download, ini as MyIni, mathrule, patterns, sort as MySort};
use futures::future::join_all;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    sync::{Arc, Mutex},
};

const NO_RESOLVE: &str = ",no-resolve";

#[derive(Serialize, Deserialize, Debug)]
struct Rules {
    rules: Vec<String>,
}

#[derive(Debug)]
struct RuleSets {
    name: String,
    rule: String,
}

pub async fn build_rules(
    ruleset: Vec<MyIni::RuleSet>, // 节点名称
    save_rules_dir: String,       // 用于存储下载的规则文件
    chunk: usize,
) -> (String, usize) {
    let down_rules_vec: Vec<RuleSets> = ruleset
        .iter()
        .map(|item| RuleSets {
            name: item.rule_name.clone(),
            rule: item.net_rule_path.clone(),
        })
        .collect();
    let local_rules_vec: Vec<RuleSets> = ruleset
        .iter()
        .map(|item| RuleSets {
            name: item.rule_name.clone(),
            rule: item.local_rule_path.clone(),
        })
        .collect();
    let final_rule_vec: Vec<RuleSets> = ruleset
        .iter()
        .map(|item| RuleSets {
            name: item.rule_name.clone(),
            rule: item.final_rule.clone(),
        })
        .collect();

    let mut down_rules: Vec<String> = Vec::new();
    if !down_rules_vec.is_empty() {
        down_rules = process_download_rules(down_rules_vec, save_rules_dir, chunk).await;
    }
    let local_rules: Vec<String> = process_local_rules(local_rules_vec);
    let final_rules: Vec<String> = process_final_rules(final_rule_vec);

    // 合并到down_rules中
    down_rules.extend(local_rules.into_iter());

    // 排序和去重
    let mut sorted_and_unique: Vec<String> = MySort::sort_rules(down_rules);

    // 合并到unique_rules中
    sorted_and_unique.extend(final_rules.into_iter());

    // 规则（已经Ok）
    let all_rules = Rules {
        rules: sorted_and_unique.clone(),
    };

    // 转换为YAML字符串
    let rules_string = serde_yaml::to_string(&all_rules).unwrap();

    // 处理yaml字符串中的缩进问题（该方法处理速度比较快）
    let combined = patterns::RE_DASH_LINE
        .replace_all(&rules_string, "  - ")
        .to_string();

    (combined, sorted_and_unique.len())
}

// 处理下载的规则
async fn process_download_rules(
    down_urls: Vec<RuleSets>,
    save_rules_dir: String,
    chunk: usize,
) -> Vec<String> {
    if down_urls.is_empty() {
        return Vec::new();
    }
    let download_tasks = down_urls
        .iter()
        .map(|item| {
            let name = item.name.clone();
            let url_clone = item.rule.clone();
            let save_pth = save_rules_dir.clone();
            tokio::spawn(async move {
                let data = download::download_multi_threaded(&url_clone, chunk)
                    .await
                    .unwrap_or_default();

                let file_name = Path::new(&url_clone)
                    .file_name()
                    .unwrap_or_else(|| OsStr::new("unknown"))
                    .to_string_lossy()
                    .into_owned();
                let path = format!("{}/{}", save_pth, file_name);

                // 计算hash值跟本地文件的hash值是否相等，不同就写入操作
                let _write_state = download::save_net_file(data.clone(), &path);

                RuleSets {
                    name,
                    rule: String::from_utf8(data).unwrap_or_default(),
                }
            })
        })
        .collect::<Vec<_>>();

    // 等待所有下载任务完成
    let results = join_all(download_tasks).await;

    let line_rules = Arc::new(Mutex::new(Vec::new()));

    // 遍历下载结果，将规则添加到规则列表中
    results.into_par_iter().for_each(|result| {
        if let Ok(item) = result {
            let name_str = item.name;
            let rule_str: String = item.rule;
            rule_str.lines().for_each(|line| {
                let mut rules_lock = line_rules.lock().unwrap();
                let rule_str = format_rules(line.to_string(), &name_str);
                if !rule_str.is_empty() {
                    rules_lock.push(rule_str);
                }
            });
        }
    });

    // 合并所有线程的结果
    let rules = Arc::try_unwrap(line_rules).unwrap().into_inner().unwrap();

    rules
}

// 处理本地的规则
fn process_local_rules(rulesets: Vec<RuleSets>) -> Vec<String> {
    rulesets
        .into_par_iter()
        .flat_map(|item| {
            let name_str = item.name;
            let rule_path = item.rule;

            if rule_path.is_empty() {
                return Vec::new();
            }

            let file = File::open(rule_path);
            if file.is_err() {
                return Vec::new();
            }

            let reader = BufReader::new(file.unwrap());

            reader
                .lines()
                .filter_map(Result::ok)
                .map(|line| format_rules(line, &name_str))
                .filter(|line| !line.is_empty())
                .collect::<Vec<String>>() // 每个文件产生一个 Vec
        })
        .collect() // 汇总所有 Vec<String> 成一个 Vec
}

fn process_final_rules(rulesets: Vec<RuleSets>) -> Vec<String> {
    let mut final_rules: Vec<String> = Vec::new();
    rulesets.into_iter().for_each(|ruleset| {
        let name_str = ruleset.name;
        let rule_str = ruleset.rule;
        if rule_str.contains("[]") {
            let rule = rule_str.replacen("[]", "", 1);
            let mut s = String::with_capacity(rule.len() + name_str.len() + 2);
            if rule.contains(NO_RESOLVE) {
                if let Some(pos) = rule.find(NO_RESOLVE) {
                    s.push_str(&rule[..pos]);
                    s.push_str(",");
                    s.push_str(&name_str);
                    s.push_str(&rule[pos..]);
                    final_rules.push(s);
                }
            } else if vec!["FINAL", "GEOSITE,", NO_RESOLVE]
                .iter()
                .all(|s| !rule.contains(s))
            {
                s.push_str(&rule);
                s.push_str(",");
                s.push_str(&name_str);
                final_rules.push(s);
            } else if rule.contains("FINAL") {
                s.push_str("MATCH,");
                s.push_str(&name_str);
                final_rules.push(s);
            }
        }
    });
    final_rules
}

fn format_rules(item: String, name_str: &String) -> String {
    // 既能处理yaml的规则，也能处理list的规则
    let rule = mathrule::extraction_rules(&item);
    if constants::FILTER_KEY.iter().all(|p| !rule.contains(p)) {
        if rule.starts_with("IP-CIDR") {
            let mut new_rule = String::with_capacity(rule.len() + name_str.len() + 1);
            if let Some(pos) = rule.find(NO_RESOLVE) {
                new_rule.push_str(&rule[..pos]);
                new_rule.push(',');
                new_rule.push_str(name_str);
                new_rule.push_str(&rule[pos..]);
            } else {
                new_rule.push_str(&rule);
                new_rule.push(',');
                new_rule.push_str(name_str);
            }
            if !new_rule.is_empty() {
                return new_rule;
            }
        } else {
            let stripped_rule = rule.strip_suffix(NO_RESOLVE).unwrap_or(&rule);
            if !stripped_rule.is_empty() {
                return format!("{},{}", stripped_rule, name_str);
            }
        }
    }
    String::new()
}
