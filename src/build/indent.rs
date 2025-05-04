use crate::build::patterns;
use yaml_rust::{YamlEmitter, YamlLoader};

// 调整yaml缩进，并去掉多余的引号。如果传入的ymal字符串比较大，会耗费大量时间，特别是几万行以上的。(慎用)
pub fn fix_yaml_indent(yaml_str: &str) -> String {
    if let Ok(docs) = YamlLoader::load_from_str(yaml_str) {
        if let Some(doc) = docs.first() {
            let mut out = String::with_capacity(yaml_str.len()); // 减少多次扩容
            if YamlEmitter::new(&mut out).dump(doc).is_ok() {
                let stripped = out.strip_prefix("---\n").unwrap_or(&out);

                // 使用 Cow 避免不必要复制
                let result = patterns::RE_INDENT_QUOTES.replace_all(stripped, "$1$3");
                return result.into_owned(); // 只有在需要的时候才分配
            }
        }
    }
    "Error: Invalid YAML input".to_string()
}
