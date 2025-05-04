use fancy_regex::Regex as FancyRegex;

#[derive(Debug, PartialEq, Eq)]
enum CidrType {
    V4,
    V6,
}

impl CidrType {
    fn as_str(&self) -> &'static str {
        match self {
            CidrType::V4 => "IP-CIDR",
            CidrType::V6 => "IP-CIDR6",
        }
    }
}

fn get_cidr_type(s: &str) -> Option<CidrType> {
    let ipv4_cidr = r"^(?x)
        (?:
            (25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.
            (25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.
            (25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.
            (25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)
        )
        /
        (3[0-2]|[12]?\d)
        $";

    let ipv6_cidr = r"^(?x)
        (
            (
                ([0-9A-Fa-f]{1,4}:){7}[0-9A-Fa-f]{1,4}|
                ([0-9A-Fa-f]{1,4}:){1,7}:|
                :(:[0-9A-Fa-f]{1,4}){1,7}|
                ([0-9A-Fa-f]{1,4}:){1,6}:[0-9A-Fa-f]{1,4}|
                ([0-9A-Fa-f]{1,4}:){1,5}(:[0-9A-Fa-f]{1,4}){1,2}|
                ([0-9A-Fa-f]{1,4}:){1,4}(:[0-9A-Fa-f]{1,4}){1,3}|
                ([0-9A-Fa-f]{1,4}:){1,3}(:[0-9A-Fa-f]{1,4}){1,4}|
                ([0-9A-Fa-f]{1,4}:){1,2}(:[0-9A-Fa-f]{1,4}){1,5}|
                [0-9A-Fa-f]{1,4}:((:[0-9A-Fa-f]{1,4}){1,6})|
                :((:[0-9A-Fa-f]{1,4}){1,7}|:)
            )
        )
        /
        (12[0-8]|1[01][0-9]|[1-9]?[0-9])
        $";

    let re_ipv4 = regex::Regex::new(ipv4_cidr).unwrap();
    let re_ipv6 = regex::Regex::new(ipv6_cidr).unwrap();

    if re_ipv4.is_match(s) {
        Some(CidrType::V4)
    } else if re_ipv6.is_match(s) {
        Some(CidrType::V6)
    } else {
        None
    }
}

fn main() {
    let re = FancyRegex::new(r#"^\s*- (?:(['\"])((?:[^'\"]|\\'|\\")*)\1|([^\s'\"]+))$"#).unwrap();
    let re_domain =
        FancyRegex::new(r#"^(?:(?!-)[A-Za-z0-9-]{1,63}(?<!-)\.)+[A-Za-z]{2,6}$"#).unwrap();

    let tests = [
        "IP-CIDR,149.154.160.0/20,no-resolve",
        "# ",
        "  - 'www.miwifi.com'",
        "- \"content inside\"",
        "  - DOMAIN,xz.pphimalayanrt.com",
        "   - '+.xz.com'",
        "+.255.255.255.255.in-addr.arpa",
        "2001:b28:f23f::/48",
        "192.168.1.0/24",
        "# 你好",
        "payload:",
        "baidu.com",
        "  - '223.0.0.0/12'",
        "  - '1.10.0.0/21'",
        "DOMAIN-REGEX,^.*\\.*\\.shouji\\.sogou\\.com$,"
    ];

    for line in tests.iter() {
        println!("{}", extraction_rules(&re, &re_domain, line));
    }
}

fn extraction_rules(re: &FancyRegex, re_domain: &FancyRegex, line: &str) -> String {
    let filter = vec![
        "USER-AGENT",
        "URL-REGEX",
        "IP-ASN",
        "#",
        "payload:",
        "DOMAIN-REGEX",
    ];
    let rules_key: Vec<&str> = vec![
        "DOMAIN",
        "DOMAIN-SUFFIX",
        "DOMAIN-KEYWORD",
        // "DOMAIN-REGEX",
        // "GEOSITE",
        "GEOIP",
        "SRC-GEOIP",
        // "IP-ASN",
        "SRC-IP-ASN",
        "IP-CIDR",
        "IP-CIDR6",
        "SRC-IP-CIDR",
        "IP-SUFFIX",
        "SRC-IP-SUFFIX",
        "SRC-PORT",
        "DST-PORT",
        "IN-PORT",
        "DSCP",
        "PROCESS-NAME",
        "PROCESS-PATH",
        "PROCESS-NAME-REGEX",
        "PROCESS-PATH-REGEX",
        "NETWORK",
        "UID",
        "IN-TYPE",
        "IN-USER",
        "IN-NAME",
        "SUB-RULE",
        "AND",
        "OR",
        "NOT",
        "RULE-SET",
        "MATCH",
    ];
    
    let match_content: Option<&str> = match re.captures(line) {
        Ok(Some(captures)) => {
            if captures.get(2).is_some() {
                // 存在引号
                Some(captures.get(2).map_or("", |m| m.as_str()))
            } else if captures.get(3).is_some() {
                // 没有引号
                Some(captures.get(3).map_or("", |m| m.as_str()))
            } else {
                None // 理论上不会发生，因为正则表达式已经确保至少有一个捕获组
            }
        }
        _ => {
            // 匹配失败或其他错误，暂时过滤掉filter不要的内容，后续再次处理
            match filter.iter().all(|s| !line.contains(s)) {
                true => Some(line),
                false => None,
            }
        }
    };
    let rule: &str = match_content.unwrap_or_default();
    if rules_key.iter().any(|s| rule.contains(s)) {
        rule.to_string()
    } else if rule.starts_with("+") {
        format!("DOMAIN-SUFFIX,{}", rule.trim_start_matches("+."))
    } else if !rule.is_empty() && re_domain.is_match(rule).unwrap_or_default() {
        format!("DOMAIN,{}", rule).to_string()
    } else if get_cidr_type(rule).is_some() {
        let ip_cidr: &str = get_cidr_type(rule).map(|ct| ct.as_str()).unwrap_or("");
        format!("{},{},no-resolve", ip_cidr, rule)
    } else {
        "".to_string()
    }
}
