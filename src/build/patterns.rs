use fancy_regex::Regex as FancyRegex;
use once_cell::sync::Lazy;
use regex::Regex;

// 匹配坐标样子的数字: "300,,50"或者"180"（数字分别代表：interval、tolerance）
pub static RE_INI_COORDS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\d+)(?:,,(\d+))?$").unwrap());

// 匹配 (||||) 这种 正则表达式的字符串，粗略判断是否为正则表达式
pub static RE_INI_GROUP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\(([^|()]+(\|[^|()]+)*)\)").unwrap());

// 匹配yaml中，每一行以若干空格开头，紧跟着"- "的行
pub static RE_DASH_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^( *)- ").unwrap());

// ————————————————————————————————————————————————————————————————————————————————————————————————————
// 下面是使用FancyRegex处理复杂的正则表达式（regex::Regex无法处理），处理速度稍慢一点
// ————————————————————————————————————————————————————————————————————————————————————————————————————

// 去掉缩进yaml字符串中数组内的 双引号，除非引号后面跟着*
pub static RE_INDENT_QUOTES: Lazy<FancyRegex> = Lazy::new(|| {
    FancyRegex::new(r#"(?m)^(\s*-\s)(['"])((?![^'"\n]*\*[^'"\n]*)[^'"\n]+)\2$"#).unwrap()
});

// 提取可能是yaml格式规则文件的规则内容，也能提取list规则文件的规则内容
pub static RE_YAML_RULES: Lazy<FancyRegex> = Lazy::new(|| {
    FancyRegex::new(r#"^\s*- (?:(['\"])((?:[^'\"]|\\'|\\")*)\1|([^\s'\"]+))$"#).unwrap()
});

// 匹配YAML的域名（包括子域名）
pub static RE_YAML_DOMAIN: Lazy<FancyRegex> = Lazy::new(|| {
    FancyRegex::new(r#"^(?:(?!-)[A-Za-z0-9-]{1,63}(?<!-)\.)+[A-Za-z]{2,6}$"#).unwrap()
});
