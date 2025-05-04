use blake3::Hasher;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use serde_yaml::{Deserializer, Value as YamlValue};
use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::{DefaultHasher, Hash, Hasher as OtherHasher},
    path::Path,
};

/// 分页结构体，带items和names
#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub names: Vec<String>,
}

/// 移除指定字段（通用版）
fn remove_fields_from_json(mut value: JsonValue, fields: &[&str]) -> JsonValue {
    if let JsonValue::Object(ref mut map) = value {
        for &field in fields {
            map.remove(field);
        }
    }
    value
}

/// 排序JsonValue的对象字段
fn sort_json_value(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|&(k, _)| k.clone());

            let sorted_map: JsonMap<String, JsonValue> = entries
                .into_iter()
                .map(|(k, v)| (k.clone(), sort_json_value(v)))
                .collect();

            JsonValue::Object(sorted_map)
        }
        JsonValue::Array(arr) => JsonValue::Array(arr.iter().map(sort_json_value).collect()),
        _ => value.clone(),
    }
}

/// 通用版哈希计算（支持任何T: Serialize）
fn compute_hash<T: Serialize>(item: &T, fields_to_remove: &[&str]) -> blake3::Hash {
    let json_value = serde_json::to_value(item).unwrap();
    let cleaned = remove_fields_from_json(json_value, fields_to_remove);
    let sorted = sort_json_value(&cleaned);

    let serialized = serde_json::to_vec(&sorted).unwrap();

    let mut hasher = Hasher::new();
    hasher.update(&serialized);
    hasher.finalize()
}

// Base62编码，将哈希值编码为 Base62，比十六进制更紧凑
fn base62_encode(mut n: u64) -> String {
    const CHARSET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut s = String::new();
    while n > 0 {
        s.push(CHARSET[(n % 62) as usize] as char);
        n /= 62;
    }
    s.chars().rev().collect()
}

/// 通用分页去重 + 提取标题 + 使用哈希后缀重命名重复 name
pub fn dedup_and_paginate<T: Serialize + Clone>(
    items: Vec<T>,
    page_size: usize,
    fields_to_remove: &[&str],
    extract_title: impl Fn(&T) -> Option<String>,
    set_title: impl Fn(&mut T, String),
) -> Vec<Page<T>> {
    let mut seen = HashSet::new();
    let mut unique_items = Vec::new();

    // 去重
    for item in items {
        let hash = compute_hash(&item, fields_to_remove);
        if seen.insert(hash) {
            unique_items.push(item);
        }
    }

    let mut name_counts: HashMap<String, usize> = HashMap::new();
    let mut pages = Vec::new();

    // 分页和处理重复名称
    for chunk in unique_items.chunks(page_size) {
        let mut items = chunk.to_vec();
        let mut names = Vec::new();

        for item in items.iter_mut() {
            if let Some(name) = extract_title(&item.clone()) {
                let count = name_counts.entry(name.clone()).or_insert(0);
                if *count > 0 {
                    // 使用哈希值作为后缀
                    let mut hasher = DefaultHasher::new();
                    name.hash(&mut hasher);
                    let hash = hasher.finish();
                    let base62 = base62_encode(hash);
                    let short_id = &base62[..6.min(base62.len())]; // 截取6位
                    let new_name = format!("{}-{}", name, short_id);
                    // let new_name = format!("{}-{:x}", name, hash); // 哈希值转换为十六进制
                    set_title(item, new_name.clone());
                    names.push(new_name);
                } else {
                    names.push(name.clone());
                }
                *count += 1;
            }
        }

        pages.push(Page { items, names });
    }

    pages
}

#[derive(Debug, Serialize, Clone)]
struct User {
    id: u32,
    name: String,
    age: u32,
    city: String,
}

fn main() {
    // 测试YamlValue结构体
    let _yamls: Vec<YamlValue> = vec![
        serde_yaml::from_str(r#"{ id: 1, name: "Alice", age: 30, city: "Paris", "hobby": ["basketball", "football"] }"#).unwrap(),
        serde_yaml::from_str(r#"{ name: "Charlie", city: "Paris", age: 30, id: 99, "hobby": ["basketball", "football"] }"#).unwrap(), // 字段顺序不同，内容一样
        serde_yaml::from_str(r#"{ name: "Bob", id: 2, age: 25, city: "London", "hobby": ["basketball", "football"] }"#).unwrap(),
        serde_yaml::from_str(r#"{ name: "zhangsan", id: 3, age: 26, city: "Beijing", "hobby": ["football"] }"#).unwrap(),
        serde_yaml::from_str(r#"{ name: "zhangsan", id: 4, age: 26, city: "shanghai", "hobby": ["football", "Ping Pong"] }"#).unwrap(),
    ];

    let merge_proxies = extract_and_merge_field_from_files("clash.yaml", "proxies");
    println!("merge_proxies len: {}", merge_proxies.len());
    println!();

    let page_size = 2;
    let fields_to_remove = ["id", "name"];

    let paginated_pages = dedup_and_paginate(
        merge_proxies,
        page_size,
        &fields_to_remove,
        |item: &YamlValue| {
            item.get("name") // 获取名为 "name" 的字段
                .and_then(|v| v.as_str()) // 如果字段存在且是字符串，就取出来
                .map(|s| s.to_string()) // 转成 String
        },
        |item: &mut YamlValue, new_name| {
            if let YamlValue::Mapping(map) = item {
                map.insert(
                    YamlValue::String("name".to_string()),
                    YamlValue::String(new_name),
                );
            }
        },
    );

    for (i, page) in paginated_pages.iter().enumerate() {
        println!("Page {}:", i + 1);
        println!("names: {:?}", page.names);
        println!("Items: {:#?}", page.items);
    }
    // ———————————————————————————————————————————————————————————————————————————————
    // 测试User结构体
    let users = vec![
        User {
            id: 1,
            name: "Alice".into(),
            age: 30,
            city: "Paris".into(),
        },
        User {
            id: 2,
            name: "Charlie".into(),
            age: 30,
            city: "Paris".into(),
        }, // 跟Alice内容一样
        User {
            id: 3,
            name: "Bob".into(),
            age: 25,
            city: "London".into(),
        },
    ];

    let pages = dedup_and_paginate(
        users,
        2,
        &["name", "id"],                                  // 去重时忽略 name, id
        |user| Some(user.name.clone()),                   // 提取标题字段（这里提取name）
        |item: &mut User, new_name| item.name = new_name, // 设置名字
    );

    for (i, page) in pages.iter().enumerate() {
        println!("Page {}:", i + 1);
        println!("names: {:?}", page.names);
        for item in &page.items {
            println!("{:?}", item);
        }
    }
}

/// 提取并合并多个 YAML 文件中某个字段的数组值（例如 hobby 字段）
pub fn extract_and_merge_field_from_files(paths_str: &str, field_name: &str) -> Vec<YamlValue> {
    let mut result = Vec::new();

    for path in paths_str.split(',').map(str::trim) {
        let content =
            fs::read_to_string(Path::new(path)).expect(&format!("Failed to read file: {}", path));

        // 多文档支持
        let docs: Vec<YamlValue> = Deserializer::from_str(&content)
            .map(|doc| YamlValue::deserialize(doc).expect("Invalid YAML"))
            .collect();

        for doc in docs {
            if let Some(field_value) = doc.get(field_name) {
                match field_value {
                    YamlValue::Sequence(seq) => {
                        // 追加数组里的每一项
                        result.extend(seq.clone());
                    }
                    other => {
                        // 不是数组，也保留（比如直接是一个对象）
                        result.push(other.clone());
                    }
                }
            }
        }
    }

    result
}
