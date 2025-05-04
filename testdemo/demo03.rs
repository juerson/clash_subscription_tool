use blake3::Hasher;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::HashSet;

#[derive(Debug)]
struct Page {
    items: Vec<YamlValue>,
    names: Vec<String>,
}

fn remove_fields(mut value: YamlValue, fields: &[&str]) -> YamlValue {
    if let YamlValue::Mapping(ref mut map) = value {
        for &field in fields {
            map.remove(&YamlValue::String(field.to_string()));
        }
    }
    value
}

fn sort_json_value(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|&(k, _)| k.clone());

            let sorted_map = entries
                .into_iter()
                .map(|(k, v)| (k.clone(), sort_json_value(v)))
                .collect();

            JsonValue::Object(sorted_map)
        }
        JsonValue::Array(arr) => JsonValue::Array(arr.iter().map(sort_json_value).collect()),
        _ => value.clone(),
    }
}

fn compute_hash(value: &YamlValue) -> blake3::Hash {
    let json_value: JsonValue = serde_json::to_value(value).unwrap();
    let sorted_json = sort_json_value(&json_value);
    let serialized = serde_json::to_vec(&sorted_json).unwrap();

    let mut hasher = Hasher::new();
    hasher.update(&serialized);
    hasher.finalize()
}

fn dedup_and_paginate(
    yamls: Vec<YamlValue>,
    page_size: usize,
    fields_to_remove: &[&str],
) -> Vec<Page> {
    let mut seen = HashSet::new();
    let mut unique_items = Vec::new();

    for item in yamls {
        let value_without_fields = remove_fields(item.clone(), fields_to_remove);
        let hash = compute_hash(&value_without_fields);

        if seen.insert(hash) {
            unique_items.push(item); // 保留原item（带完整字段）
        }
    }

    let mut pages = Vec::new();

    for chunk in unique_items.chunks(page_size) {
        let items = chunk.to_vec();
        let names = chunk
            .iter()
            .filter_map(|v| v.get("name"))
            .filter_map(|name_val| name_val.as_str().map(|s| s.to_string()))
            .collect::<Vec<String>>();

        pages.push(Page { items, names });
    }

    pages
}

fn main() {
    let yamls: Vec<YamlValue> = vec![
        serde_yaml::from_str(r#"{ id: 1, name: "Alice", age: 30, city: "Paris" }"#).unwrap(),
        serde_yaml::from_str(r#"{ name: "Charlie", city: "Paris", age: 30, id: 99 }"#).unwrap(), // 字段顺序不同，内容一样
        serde_yaml::from_str(r#"{ name: "Bob", id: 2, age: 25, city: "London" }"#).unwrap(),
        serde_yaml::from_str(r#"{ name: "zhangsan", id: 3, age: 26, city: "Beijing" }"#).unwrap(),
    ];

    let page_size = 2;
    let fields_to_remove = ["name", "id"];
    let paginated_pages = dedup_and_paginate(yamls, page_size, &fields_to_remove);

    for (i, page) in paginated_pages.iter().enumerate() {
        println!("Page {}:", i + 1);
        println!("Names: {:?}", page.names);
        println!("Items: {:#?}", page.items);
    }
}
