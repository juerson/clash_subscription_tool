use crate::build::patterns;

use fancy_regex::Regex as FancyRegex;
use indexmap::IndexSet;
use ini::Ini;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RuleSet {
    pub rule_name: String,       // 规则集名称
    pub net_rule_path: String,   // 网络规则路径(url)
    pub local_rule_path: String, // 本地规则路径(相对路径)
    pub final_rule: String,      // 最后兜底的规则
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SelectGroup {
    pub name: String,

    #[serde(rename = "type")]
    pub select_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<u32>,

    pub proxies: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxies_regexp: Option<String>, // 这个是正则表达式，用于过滤节点到 proxies 中
}

#[derive(Serialize, Deserialize, Debug)]
struct ProxyGroup {
    #[serde(rename = "proxy-groups")]
    group: Vec<SelectGroup>,
}

pub fn read_ini(config: Ini) -> (Vec<String>, Vec<RuleSet>, Vec<SelectGroup>) {
    // 规则集名称
    let mut ruleset_names: IndexSet<String> = IndexSet::new();
    // 规则集
    let mut ruleset: Vec<RuleSet> = Vec::new();
    // 自定义代理组
    let mut custom_proxy_group: Vec<SelectGroup> = Vec::new();

    for (_sec, prop) in &config {
        for (key, value) in prop.iter() {
            if key == "ruleset" {
                let parts = value.splitn(2, ',').collect::<Vec<_>>();
                if parts.len() == 2 {
                    let ruleset_name = parts[0].to_string();
                    let mut ruleset_value = parts[1].to_string();
                    let remove_list = vec!["clash-classic:", "clash-ipcidr:", "clash-domain:"];
                    for target in &remove_list {
                        ruleset_value = ruleset_value.replace(target, "").trim().to_string();
                    }
                    if ["https://", "http://"]
                        .iter()
                        .any(|p| ruleset_value.starts_with(p))
                    {
                        // 网络规则的地址，后续需要下载处理
                        ruleset.push(RuleSet {
                            rule_name: ruleset_name.clone(),
                            net_rule_path: ruleset_value,
                            ..Default::default()
                        });
                    } else if !ruleset_value.contains("[]") {
                        // 本地规则的路径，后续需要读取处理
                        ruleset.push(RuleSet {
                            rule_name: ruleset_name.clone(),
                            local_rule_path: ruleset_value,
                            ..Default::default()
                        });
                    } else if ruleset_value.contains("[]") {
                        // 写在最后的规则，不需要处理
                        ruleset.push(RuleSet {
                            rule_name: ruleset_name.clone(),
                            final_rule: ruleset_value,
                            ..Default::default()
                        });
                    }
                    ruleset_names.insert(ruleset_name);
                }
            }
            if key == "custom_proxy_group" {
                let parts: Vec<&str> = value.split('`').collect();
                let (interval, tolerance) = parts
                    .iter()
                    .find_map(|s| patterns::RE_INI_COORDS.captures(s))
                    .map(|caps| {
                        (
                            caps[1].parse().ok(),
                            caps.get(2).and_then(|m| m.as_str().parse().ok()),
                        )
                    })
                    .unwrap_or((None, None));
                let name = parts[0].to_string();
                let select_type = parts[1].to_string();
                let url = parts
                    .iter()
                    .find(|ele| {
                        ["https://", "http://", "benchmark-url="]
                            .iter()
                            .any(|p| ele.starts_with(p))
                    })
                    .map(|s| s.replacen("benchmark-url=", "", 1).to_string());
                let group_regular = parts
                    .iter()
                    .find(|ele| patterns::RE_INI_GROUP.is_match(ele))
                    .map(|s| s.to_string());
                let any_regular = parts
                    .iter()
                    .find(|ele| ele.contains(".*"))
                    .map(|s| s.to_string());
                let square_brackets_rules: Vec<String> = parts
                    .iter()
                    .filter(|s| s.contains("[]"))
                    .map(|s| s.replacen("[]", "", 1))
                    .collect();
                custom_proxy_group.push(SelectGroup {
                    name,
                    select_type,
                    url,
                    interval,
                    tolerance,
                    proxies: square_brackets_rules,
                    proxies_regexp: group_regular.or(any_regular),
                    ..Default::default()
                });
            }
        }
    }
    // 转换为 Vec
    let ruleset_names_vec: Vec<String> = ruleset_names.into_iter().collect();

    (ruleset_names_vec, ruleset, custom_proxy_group)
}

pub fn modify_proxy_groups(
    pending_proxy_group: Vec<SelectGroup>,
    proxy_names: Vec<String>,
    ruleset_names: Vec<String>,
) -> String {
    let mut custom_proxy_group = pending_proxy_group.clone();
    let mut remove_proxy_group_proxies_names: Vec<String> = Vec::new();
    for proxy_group in &mut custom_proxy_group {
        let pattern_option = proxy_group.proxies_regexp.clone().unwrap_or_default();

        if !pattern_option.is_empty() {
            let re = FancyRegex::new(&pattern_option).unwrap();
            let filter_node_names: Vec<String> = proxy_names
                .iter()
                .filter(|name| re.is_match(name).unwrap_or(false))
                .map(|name| name.to_string())
                .collect();
            proxy_group.proxies.extend(filter_node_names);
        }
        // 确保有规则对应的分组，proxies不为空，如果实际为空，则移除该分组
        if proxy_group.proxies.is_empty() {
            if ruleset_names.contains(&proxy_group.name) {
                // 防止有规则的分组，没有对应的proxies
                proxy_group.proxies.extend(ruleset_names.clone());
            } else {
                // 没有规则的分组，可以移除，但是在其它分组的proxies内中有这个分组名称
                remove_proxy_group_proxies_names.push(proxy_group.name.clone());
            }
        }

        //  proxies_regexp 字段赋值为 None ，方便后面去掉这个字段
        proxy_group.proxies_regexp = None;
    }

    // 移除proxies为空的代理分组
    custom_proxy_group.retain(|selectgroup| !selectgroup.proxies.is_empty());
    // 移除proxies内无效的分组名称
    custom_proxy_group.iter_mut().for_each(|selectgroup| {
        selectgroup
            .proxies
            .retain(|pn| !remove_proxy_group_proxies_names.contains(pn));
    });

    // 使用结构体，方便序列化后，字段的顺序保持一致
    let proxy_group_struct = ProxyGroup {
        group: custom_proxy_group,
    };

    let proxy_group_string = serde_yaml::to_string(&proxy_group_struct).unwrap();

    proxy_group_string
}
