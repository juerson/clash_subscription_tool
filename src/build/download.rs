use blake3;
use reqwest::Client;
use std::{fs, path::Path, sync::Arc};
use tokio::sync::Mutex;

// 多线程分片下载网络资源，所下载文件以字节数组形式返回
pub async fn download_multi_threaded(
    url: &str,
    thread: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // let file_name = PathBuf::from(&url)
    //     .file_name()
    //     .unwrap_or_else(|| std::ffi::OsStr::new("unknown"))
    //     .to_string_lossy()
    //     .into_owned();
    let client = Client::new();

    // 获取文件大小
    let res = client.head(url).send().await?;
    let total_size = res
        .headers()
        .get("content-length")
        .ok_or("Missing content-length")?
        .to_str()?
        .parse::<u64>()?;

    // println!(
    //     "🔗 {} 文件大小共 {} bytes，开始下载中...",
    //     file_name, total_size
    // );

    // 初始化共享缓冲区
    let buffer = Arc::new(Mutex::new(vec![0u8; total_size as usize]));
    let chunk_size = total_size / (thread as u64);

    let mut handles = Vec::default();

    for i in 0..thread {
        let start = (i as u64) * chunk_size;
        let end = if i == thread - 1 {
            total_size - 1
        } else {
            ((i + 1) as u64) * chunk_size - 1
        };
        let url = url.to_string();
        let client = client.clone();
        let buffer = buffer.clone();

        // println!("🔢 线程 {} ⬇️{}-{} bytes", i, start, end);

        let handle = tokio::spawn(async move {
            let resp = client
                .get(&url)
                .header("Range", format!("bytes={}-{}", start, end))
                .send()
                .await?;
            let bytes = resp.bytes().await?;
            let mut buffer = buffer.lock().await;
            buffer[start as usize..=end as usize].copy_from_slice(&bytes);

            // println!("✅ 线程 {} 执行完毕", i);

            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let final_buffer = Arc::try_unwrap(buffer)
        .expect("Arc unwrap failed")
        .into_inner();

    Ok(final_buffer)
}

// 保存网络文件到本地，如果本地文件存在，则比较hash值，如果一致，则不保存，如果不一致，则保存
pub fn save_net_file(net_content: Vec<u8>, file_path: &str) -> String {
    if !net_content.is_empty() {
        let path = Path::new(file_path);
        if path.exists() {
            let local_content = fs::read(file_path).expect("读取文件失败");
            let local_hash = blake3::hash(&local_content);
            let net_hash = blake3::hash(&net_content);
            if local_hash == net_hash {
                return format!("{} 文件与网络文件一致，无需保存！", file_path);
            } else {
                fs::write(file_path, &net_content).unwrap();
                return format!("{} 文件与网络文件不一致，已保存本地！", file_path);
            }
        } else {
            fs::write(file_path, &net_content).unwrap();
            return format!("{} 文件不存在，已保存本地！", file_path);
        }
    } else {
        return format!("要写入的数据为空！");
    }
}
