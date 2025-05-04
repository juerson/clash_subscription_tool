mod build;
mod utils;

use build::{indent, ini as MyIni, rules};
use clap::{CommandFactory, Parser};
use ini::Ini;
use serde::{Deserialize, Serialize};
use serde_yaml::{self, Value as YamlValue};
use std::{
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};
use utils::{filename, paginate, proxy, read};

/// 功能：该工具用于clash订阅文件的代理组和规则重新构建，支持合并多个clash订阅文件再次重新构建。
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// ini配置文件
    #[arg(short = 'c', default_value = "config/ACL4SSR.ini")]
    ini_file_path: String,

    /// clash配置的头信息
    #[arg(short = 'b', default_value = "mihomo/base.yaml")]
    header_file_path: String,

    /// 输入含有proxies节点的clash配置文件，支持多个配置文件(用英文逗号隔开)
    #[arg(short = 'f', default_value = "clash.yaml")]
    proxies_file_path: String,

    /// 生成的clash文件输出路径
    #[arg(short = 'o', default_value = "output.yaml")]
    output_file_path: String,

    /// 网上下载的规则，保存的文件夹路径
    #[arg(short = 's', default_value = "rules/download/")]
    save_rules_dir: String,

    /// 数据分页，每个配置最大节点数
    #[arg(short = 'n', value_name = "page_size", default_value_t = 50)]
    page_size: usize,

    /// 设置同一URL分片下载的份数(缩短下载时间)，有概率致使只有两条规则
    #[arg(short = 'k', value_name = "down_chunk_size", default_value_t = 50)]
    down_chunk_size: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct Proxies {
    proxies: Vec<YamlValue>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() {
    let cli = Args::try_parse().unwrap_or_else(|_err| {
        Args::command().print_help().unwrap();
        println!();
        std::process::exit(1);
    });

    let ini_file_path = cli.ini_file_path;
    let base_yaml_path = cli.header_file_path;
    let node_file_path = cli.proxies_file_path;
    let output_yaml_path = cli.output_file_path;
    let save_rules_dir = cli.save_rules_dir;
    let page_size = cli.page_size;
    let down_chunk_size = cli.down_chunk_size;

    // 删除上次运行输出的历史文件
    filename::delete_old_files_by_pattern(&output_yaml_path).unwrap();

    // 读取 base.yaml 文件
    let base_config: YamlValue = read::read_yaml(&base_yaml_path);
    let base_yaml_str = serde_yaml::to_string(&base_config).unwrap();
    let base_yaml_indent = indent::fix_yaml_indent(&base_yaml_str);

    // 提取和合并多个proxies的值
    let merge_proxies = proxy::extract_and_merge_proxies(&node_file_path, "proxies");
    if merge_proxies.is_empty() {
        return;
    }

    // 对merge_proxies节点进行分页
    let paginated_pages = paginate::dedup_and_paginate(
        merge_proxies,
        page_size,
        &["name", "skip-cert-verify"], // 暂时移除的key-value，移除它们再计算hash，判断是否跟其它的节点重复
        |item: &YamlValue| {
            item.get("name") // 获取名为"name"的字段，提到外面
                .and_then(|v| v.as_str()) // 如果字段存在且是字符串，就取出来
                .map(|s| s.to_string())
        },
        |item: &mut YamlValue, new_name| {
            if let YamlValue::Mapping(map) = item {
                map.insert(
                    YamlValue::String("name".to_string()), // 如果发现name字段跟其它节点的name重复，就改为其它name名称
                    YamlValue::String(new_name),
                );
            }
        },
    );

    // 读取ini配置文件的信息
    let ini_config: Ini = Ini::load_from_file(&ini_file_path).unwrap();
    let (ruleset_names, ruleset, pending_proxy_group) = MyIni::read_ini(ini_config);

    // 记录当前时间
    let start_time = Instant::now();

    let (all_rules, rules_count) =
        rules::build_rules(ruleset, save_rules_dir, down_chunk_size).await;

    // 构建分页的yaml文件
    for (i, page) in paginated_pages.iter().enumerate() {
        let proxies = Proxies {
            proxies: page.items.clone(),
        };
        let yaml_string = serde_yaml::to_string(&proxies).unwrap();
        let proxies_indent = indent::fix_yaml_indent(&yaml_string);

        // 修改代理组
        let proxy_group_string = MyIni::modify_proxy_groups(
            pending_proxy_group.clone(),
            page.names.clone(),
            ruleset_names.clone(),
        );
        let proxy_group_indent = indent::fix_yaml_indent(&proxy_group_string);

        let clash_yaml = format!(
            "{}\n{}\n{}\n{}",
            base_yaml_indent,
            proxies_indent.clone(),
            proxy_group_indent,
            all_rules
        );
        println!("{}", clash_yaml);

        // 构建输出文件名
        let output_path = filename::rename_output_filename(
            &output_yaml_path,
            i,
            paginated_pages.len(),
            Some("snap"), // 自定义数字的前缀
            None,         // 自定义数字的后缀
        );
        // 创建并写入 yaml 文件
        let file = File::create(&output_path).unwrap();
        let mut writer = BufWriter::new(file);

        writer.write_all(base_yaml_indent.as_bytes()).unwrap();
        writer.write_all("\n".as_bytes()).unwrap();
        writer.write_all(proxies_indent.as_bytes()).unwrap();
        writer.write_all("\n".as_bytes()).unwrap();
        writer.write_all(proxy_group_indent.as_bytes()).unwrap();
        writer.write_all("\n".as_bytes()).unwrap();
        writer.write_all(all_rules.as_bytes()).unwrap();

        println!(
            "构建的配置耗时: {:?}，规则共：{} 条！",
            start_time.elapsed(),
            rules_count
        );
    }
}
