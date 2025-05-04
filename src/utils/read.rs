use serde_yaml::Value as YamlValue;

pub fn read_yaml(file_path: &str) -> YamlValue {
    let content = std::fs::read_to_string(file_path).unwrap();
    let yaml: YamlValue = serde_yaml::from_str(&content).unwrap();
    yaml
}
