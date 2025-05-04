use glob::glob;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// 重命名输出文件名
pub fn rename_output_filename<P: AsRef<Path>>(
    base_path: P,
    index: usize,
    total: usize,
    prefix: Option<&str>,
    suffix: Option<&str>,
) -> PathBuf {
    let base = base_path.as_ref();
    let file_stem = base.file_stem().and_then(OsStr::to_str).unwrap_or("file");
    let extension = base.extension().and_then(OsStr::to_str);

    let digits = total.to_string().len(); // 计算补零位数
    let num_str = format!("{:0width$}", index + 1, width = digits); // 序号从1开始

    // 构建新文件名主体
    let mut new_name = file_stem.to_string();

    if let Some(prefix) = prefix {
        new_name.push('_');
        new_name.push_str(prefix);
    }

    new_name.push('_');
    new_name.push_str(&num_str);

    if let Some(suffix) = suffix {
        new_name.push('_');
        new_name.push_str(suffix);
    }

    // 拼接扩展名
    let new_file_name = match extension {
        Some(ext) => format!("{}.{}", new_name, ext),
        None => new_name,
    };

    let mut result = base.to_path_buf();
    result.set_file_name(new_file_name);
    result
}

/// 删除所有符合命名规则的旧文件（例如 output_*.yaml）
pub fn delete_old_files_by_pattern<P: AsRef<Path>>(base_path: P) -> std::io::Result<()> {
    let base = base_path.as_ref();
    let file_stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let extension = base.extension().and_then(|s| s.to_str());

    // 构造 glob 模式
    let pattern = match extension {
        Some(ext) => format!("{}_*.{}", file_stem, ext),
        None => format!("{}_*", file_stem),
    };

    for entry in glob(&pattern).expect("无效的通配符模式") {
        if let Ok(path) = entry {
            if path.exists() {
                println!("正在删除历史文件: {:?}", path);
                std::fs::remove_file(path)?;
            }
        }
    }

    Ok(())
}
