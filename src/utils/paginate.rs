use blake3::Hasher;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher as OtherHasher},
};

/// 分页结构体，带names和items
#[derive(Debug)]
pub struct Page<T> {
    pub names: Vec<String>,
    pub items: Vec<T>,
}

/// 移除指定字段
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

/// Base62编码，将哈希值编码为 Base62，比十六进制更紧凑更短。
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
    extract_name: impl Fn(&T) -> Option<String>,
    set_name: impl Fn(&mut T, String),
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
            if let Some(name) = extract_name(&item.clone()) {
                let count = name_counts.entry(name.clone()).or_insert(0);
                if *count > 0 {
                    // 使用哈希值作为后缀
                    let mut hasher = DefaultHasher::new();
                    name.hash(&mut hasher);
                    let hash = hasher.finish();
                    let base62 = base62_encode(hash);
                    let short_id = &base62[..6.min(base62.len())]; // 截取6位
                    let new_name = format!("{}-{}", name, short_id);
                    set_name(item, new_name.clone());
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
