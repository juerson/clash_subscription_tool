use crate::utils::read;
use chardetng::EncodingDetector;
use serde::Deserialize;
use serde_yaml::{Deserializer, Value as YamlValue};
use std::{fs, path::Path};

/// 提取并合并多个 YAML 文件中某个字段的数组值（例如 name 字段）
pub fn extract_and_merge_proxies(paths_str: &str, field_name: &str) -> Vec<YamlValue> {
    let mut result = Vec::new();

    for path in paths_str.split(',').map(str::trim) {
        let msg = format!("Failed to read file: {}", path);
        let raw_bytes = fs::read(Path::new(path)).expect(&msg);

        // 1、自动识别编码（包括 UTF-8、GBK、ISO-8859-1、Big5 等）
        let mut detector = EncodingDetector::new();
        detector.feed(&raw_bytes, true);
        let encoding = detector.guess(None, true);

        // 2、解码为 UTF-8
        let (cow, _, _) = encoding.decode(&raw_bytes);
        let mut content = cow.to_string();

        // 3、移除 UTF-8 BOM（如果是 UTF-8 并带 BOM）
        const BOM: &str = "\u{FEFF}";
        if content.starts_with(BOM) {
            content = content[BOM.len()..].to_string();
        }

        // 4、解析 YAML
        let docs: Vec<YamlValue> = Deserializer::from_str(&content)
            .map(|doc| YamlValue::deserialize(doc).expect("Invalid YAML"))
            .collect();

        for doc in docs {
            if let Some(field_value) = doc.get(field_name) {
                match field_value {
                    YamlValue::Sequence(seq) => result.extend(seq.clone()),
                    other => result.push(other.clone()),
                }
            }
        }
    }

    result
}

#[allow(dead_code)]
fn get_proxies_names_and_values(file_path: &str) -> (Vec<String>, Vec<YamlValue>) {
    let mut names: Vec<String> = Vec::new();
    let mut proxies_value: Vec<YamlValue> = Vec::new();
    if let Some(yamlvalue) = read::read_yaml(file_path).get("proxies") {
        if let YamlValue::Sequence(seq) = yamlvalue {
            proxies_value = seq.clone();
            for item in &proxies_value {
                let name: String = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                names.push(name);
            }
        }
    }
    (names, proxies_value)
}
