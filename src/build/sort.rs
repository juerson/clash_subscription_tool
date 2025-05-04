use rayon::prelude::*;
use std::net::IpAddr;

/// 将 IP 地址统一转换为 u128 排序键
fn ip_to_u128(ip_str: &str) -> Option<u128> {
    match ip_str.parse::<IpAddr>() {
        Ok(IpAddr::V4(ipv4)) => Some(u32::from(ipv4) as u128),
        Ok(IpAddr::V6(ipv6)) => Some(u128::from(ipv6)),
        _ => None,
    }
}

/// 排序：支持 DOMAIN/DOMAIN-SUFFIX 等按名称排序，IP-CIDR（IPv4/IPv6）按 IP 数值排序
pub fn sort_rules(lines: Vec<String>) -> Vec<String> {
    let mut tuples: Vec<(String, Option<u128>, String, String)> = lines
        .into_par_iter()
        .map(|line| {
            let mut parts = line.splitn(3, ',');
            let type_part = parts.next().unwrap_or("").to_string();
            let key_part = parts.next().unwrap_or("").to_string();
            let ip_sort_key = if type_part == "IP-CIDR" {
                key_part.split('/').next().and_then(ip_to_u128)
            } else {
                None
            };
            (type_part, ip_sort_key, key_part, line)
        })
        .collect();

    tuples.par_sort_unstable_by(|a, b| match a.0.cmp(&b.0) {
        std::cmp::Ordering::Equal => {
            if a.0 == "IP-CIDR" {
                a.1.cmp(&b.1)
            } else {
                a.2.cmp(&b.2)
            }
        }
        other => other,
    });

    let mut result: Vec<String> = tuples.into_iter().map(|(_, _, _, line)| line).collect();
    result.dedup(); // 去掉连续重复的元素

    result
}
