use blake3::Hasher;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use serde_yaml::Value as YamlValue;
use std::collections::HashSet;

#[derive(Debug, Serialize, Clone)]
struct User {
    id: u32,
    name: String,
    age: u32,
    city: String,
}

/// 分页结构体
#[derive(Debug, Serialize, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
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

/// 通用分页去重
pub fn dedup_and_paginate<T: Serialize + Clone>(
    items: Vec<T>,
    page_size: usize,
    fields_to_remove: &[&str],
) -> Vec<Page<T>> {
    let mut seen = HashSet::new();
    let mut unique_items = Vec::new();

    for item in items {
        let hash = compute_hash(&item, fields_to_remove);
        if seen.insert(hash) {
            unique_items.push(item);
        }
    }

    let mut pages = Vec::new();

    for chunk in unique_items.chunks(page_size) {
        pages.push(Page {
            items: chunk.to_vec(),
        });
    }

    pages
}

fn main() {
    // 测试YamlValue结构体
    let yamls: Vec<YamlValue> = vec![
        serde_yaml::from_str(r#"{ id: 1, name: "Alice", age: 30, city: "Paris", "hobby": ["basketball", "football"] }"#).unwrap(),
        serde_yaml::from_str(r#"{ name: "Charlie", city: "Paris", age: 30, id: 99, "hobby": ["basketball", "football"] }"#).unwrap(), // 字段顺序不同，内容一样
        serde_yaml::from_str(r#"{ name: "Bob", id: 2, age: 25, city: "London", "hobby": ["basketball", "football"] }"#).unwrap(),
        serde_yaml::from_str(r#"{ name: "zhangsan", id: 3, age: 26, city: "Beijing", "hobby": ["football"] }"#).unwrap(),
    ];

    let page_size = 2;
    let fields_to_remove = ["id"];
    let paginated_pages = dedup_and_paginate(yamls, page_size, &fields_to_remove);

    for (i, page) in paginated_pages.iter().enumerate() {
        println!("Page {}:", i + 1);
        println!("Items: {:#?}", page.items);
    }

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
        }, // 内容跟Alice一样，顺序无关
        User {
            id: 3,
            name: "Bob".into(),
            age: 25,
            city: "London".into(),
        },
    ];
    let page_size = 2;
    let fields_to_remove = ["name", "id"];
    let paginated_pages = dedup_and_paginate(users, page_size, &fields_to_remove);
    for (i, page) in paginated_pages.iter().enumerate() {
        println!("Page {}:", i + 1);
        println!("Items: {:#?}", page.items);
    }
}
