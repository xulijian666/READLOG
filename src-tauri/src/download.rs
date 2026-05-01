use flate2::read::GzDecoder;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::config::{AuthConfig, ServerConfig};
use crate::error::{AppError, AppResult};

const PAGE_SIZE_BYTES: u64 = 15 * 1024 * 1024; // 15MB

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSummary {
    pub server_count: usize,
    pub bytes_written: u64,
    pub output_path: String,
}

pub async fn download_realtime_logs(
    servers: &[ServerConfig],
    credentials: &AuthConfig,
    log_type: &str,
    output_path: &str,
) -> AppResult<DownloadSummary> {
    let output_path = resolve_output_path(output_path, log_type, "realtime")?;
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut output = tokio::fs::File::create(&output_path).await?;
    let mut bytes_written = 0_u64;

    for server in servers {
        let url = build_realtime_url(&server.base_url, log_type)?;
        let header = format!("\n===== {} | {} =====\n", server.name, url);
        output.write_all(header.as_bytes()).await?;
        bytes_written += header.len() as u64;

        let response = client
            .get(url.clone())
            .basic_auth(&credentials.username, Some(password.clone()))
            .send()
            .await?
            .error_for_status()?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            output.write_all(&chunk).await?;
            bytes_written += chunk.len() as u64;
        }
        output.write_all(b"\n").await?;
        bytes_written += 1;
    }

    Ok(DownloadSummary {
        server_count: servers.len(),
        bytes_written,
        output_path: output_path.to_string_lossy().to_string(),
    })
}

pub async fn download_archive_logs(
    servers: &[ServerConfig],
    credentials: &AuthConfig,
    log_type: &str,
    month: &str,
    day: &str,
    hour_start: u32,
    hour_end: u32,
    output_path: &str,
) -> AppResult<DownloadSummary> {
    let base_output_path = resolve_output_path(output_path, log_type, "archive")?;
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    if let Some(parent) = base_output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut all_content = Vec::new();
    let mut total_bytes = 0_u64;
    let mut server_count = 0_usize;

    for server in servers {
        let server_content = download_server_archive(
            server,
            &credentials.username,
            &password,
            log_type,
            month,
            day,
            hour_start,
            hour_end,
            &client,
        ).await?;
        if server_content.is_empty() {
            continue;
        }
        server_count += 1;
        let header = format!("\n===== {} | {} | {} {} =====\n", server.name, server.base_url, month, day);
        all_content.extend_from_slice(header.as_bytes());
        total_bytes += header.len() as u64;
        all_content.extend_from_slice(&server_content);
        total_bytes += server_content.len() as u64;
    }

    if all_content.is_empty() {
        return Ok(DownloadSummary {
            server_count: 0,
            bytes_written: 0,
            output_path: String::new(),
        });
    }

    // 分页写入
    let page_count = (total_bytes / PAGE_SIZE_BYTES + 1) as usize;
    let output_paths = if page_count == 1 {
        tokio::fs::write(&base_output_path, &all_content).await?;
        vec![base_output_path.to_string_lossy().to_string()]
    } else {
        let mut paths = Vec::new();
        let content_slice = &all_content;
        for page in 0..page_count {
            let start = (page as u64 * PAGE_SIZE_BYTES) as usize;
            let end = std::cmp::min(start + PAGE_SIZE_BYTES as usize, content_slice.len());
            if start >= content_slice.len() {
                break;
            }
            let page_path = base_output_path.with_extension(format!("part{}.log", page + 1));
            tokio::fs::write(&page_path, &content_slice[start..end]).await?;
            paths.push(page_path.to_string_lossy().to_string());
        }
        paths
    };

    Ok(DownloadSummary {
        server_count,
        bytes_written: total_bytes,
        output_path: output_paths.join(", "),
    })
}

async fn download_server_archive(
    server: &ServerConfig,
    username: &str,
    password: &str,
    log_type: &str,
    month: &str,
    day: &str,
    hour_start: u32,
    hour_end: u32,
    client: &reqwest::Client,
) -> AppResult<Vec<u8>> {
    let mut all_content = Vec::new();

    for hour in hour_start..=hour_end {
        let url = build_archive_url(&server.base_url, log_type, month, day, hour)?;
        let response = match client
            .get(url.clone())
            .basic_auth(username, Some(password))
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };

        if !response.status().is_success() {
            continue;
        }

        let gz_bytes = response.bytes().await?;
        let mut decoder = GzDecoder::new(&gz_bytes[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        // 添加时间标记
        let time_marker = format!("\n--- {}:{} | {} ---\n", day, hour, url);
        all_content.extend_from_slice(time_marker.as_bytes());
        all_content.extend_from_slice(&decompressed);
    }

    Ok(all_content)
}

fn build_realtime_url(base_url: &str, log_type: &str) -> AppResult<Url> {
    let base = Url::parse(base_url)?;
    let realtime_url = if base.path().ends_with('/') {
        base.join(&format!("{log_type}.log"))?
    } else if base.path().ends_with(".log") {
        base
    } else {
        base.join(&format!("{log_type}.log"))?
    };
    Ok(realtime_url)
}

fn build_archive_url(base_url: &str, log_type: &str, month: &str, day: &str, hour: u32) -> AppResult<Url> {
    let base = Url::parse(base_url)?;
    // 确保基础路径以 / 结尾
    let base_path = if base.path().ends_with('/') {
        base.path()
    } else if base.path().ends_with(".log") {
        base.path().rsplit_once('/').map(|(p, _)| p).unwrap_or("")
    } else {
        base.path()
    };
    // 归档路径: {month}/{day}/{log_type}-{YYYY-MM-DD}_{HH}.log.gz
    let year_month: Vec<&str> = month.split('-').collect();
    let year = year_month.get(0).map_or("2026", |v| *v);
    let month_num = year_month.get(1).map_or("04", |v| *v);
    let archive_name = format!("{log_type}-{year}-{month_num}-{day}_{hour:02}.log.gz");
    let archive_path = format!("{base_path}/{month}/{day}/{archive_name}");
    let archive_url = base.join(&archive_path)?;
    Ok(archive_url)
}

fn resolve_output_path(output_path: &str, log_type: &str, mode: &str) -> AppResult<PathBuf> {
    if !output_path.trim().is_empty() {
        return Ok(PathBuf::from(output_path));
    }

    let mut path = dirs::download_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| AppError::NotFound("Downloads directory".to_string()))?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    path.push(format!("{log_type}_{mode}_{stamp}.log"));
    Ok(path)
}

pub async fn download_raw_file(
    server: &ServerConfig,
    credentials: &AuthConfig,
    file_path: &str,
    output_path: &str,
) -> AppResult<()> {
    let url = Url::parse(&server.base_url)?.join(file_path)?;
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let response = reqwest::Client::new()
        .get(url)
        .basic_auth(&credentials.username, Some(password))
        .send()
        .await?
        .error_for_status()?;

    let mut file = tokio::fs::File::create(output_path).await?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    Ok(())
}

pub async fn export_filtered_results(events: Vec<crate::parser::LogEvent>, output_path: &str) -> AppResult<()> {
    let mut text = String::new();
    for event in events {
        for line in event.raw_text.lines() {
            text.push_str(&format!("[{}] {}\n", event.server_name, line));
        }
    }
    tokio::fs::write(output_path, text).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_archive_url, build_realtime_url};

    #[test]
    fn builds_realtime_url_from_directory() {
        let url = build_realtime_url("http://host/path/", "app").unwrap();
        assert_eq!(url.as_str(), "http://host/path/app.log");
    }

    #[test]
    fn builds_realtime_url_from_log_file() {
        let url = build_realtime_url("http://host/path/app.log", "sql").unwrap();
        assert_eq!(url.as_str(), "http://host/path/app.log");
    }

    #[test]
    fn builds_archive_url() {
        let url = build_archive_url("http://host/path/", "app", "2026-04", "30", 17).unwrap();
        assert_eq!(url.as_str(), "http://host/path/2026-04/30/app-2026-04-30_17.log.gz");
    }
}