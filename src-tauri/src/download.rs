use flate2::read::GzDecoder;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::config::{build_full_url, extract_log_type, AuthConfig, LogEntry};
use crate::directory::{self, DirEntry};
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
    base_url: &str,
    entries: &[&LogEntry],
    credentials: &AuthConfig,
    output_path: &str,
) -> AppResult<DownloadSummary> {
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut total_bytes = 0_u64;
    let mut first_output = String::new();

    for entry in entries {
        let url = build_full_url(base_url, &entry.path, &entry.log_file)?;
        let entry_output_path = resolve_output_path(output_path, &entry.name, "realtime")?;
        if let Some(parent) = entry_output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let response = client
            .get(&url)
            .basic_auth(&credentials.username, Some(password.clone()))
            .send()
            .await?
            .error_for_status()?;

        if entry.log_file.ends_with(".gz") {
            // Download and decompress .gz
            let gz_bytes = response.bytes().await?;
            let mut decoder = GzDecoder::new(&gz_bytes[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            tokio::fs::write(&entry_output_path, &decompressed).await?;
            total_bytes += decompressed.len() as u64;
        } else {
            let mut output = tokio::fs::File::create(&entry_output_path).await?;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                output.write_all(&chunk).await?;
                total_bytes += chunk.len() as u64;
            }
        }

        if first_output.is_empty() {
            first_output = entry_output_path.to_string_lossy().to_string();
        }
    }

    Ok(DownloadSummary {
        server_count: entries.len(),
        bytes_written: total_bytes,
        output_path: first_output,
    })
}

pub async fn download_archive_logs(
    base_url: &str,
    entries: &[&LogEntry],
    credentials: &AuthConfig,
    month: &str,
    day: &str,
    hour_start: u32,
    hour_end: u32,
    output_path: &str,
) -> AppResult<DownloadSummary> {
    let log_type = entries.first().map_or("app".to_string(), |e| extract_log_type(&e.log_file));
    let base_output_path = if output_path.trim().is_empty() {
        let mut path = dirs::download_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| AppError::NotFound("Downloads directory".to_string()))?;
        path.push(format!("{log_type}_archive_{month}-{day}_{:02}h-{:02}h.log", hour_start, hour_end));
        path
    } else {
        PathBuf::from(output_path)
    };
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

    for entry in entries {
        let full_path = format!("{}{}", entry.path.trim_end_matches('/'), "/");
        let entry_base_url = build_full_url(base_url, &full_path, "")?;
        let server_content = download_entry_archive(
            &entry.name,
            &entry_base_url,
            &entry.log_file,
            &credentials.username,
            &password,
            &log_type,
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
        let header = format!("\n===== {} | {} | {} {} =====\n", entry.name, entry_base_url, month, day);
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

async fn download_entry_archive(
    _entry_name: &str,
    entry_base_url: &str,
    _log_file: &str,
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
    let base = Url::parse(entry_base_url)?;
    let base_path = base.path().trim_end_matches('/');

    for hour in hour_start..=hour_end {
        let year_month: Vec<&str> = month.split('-').collect();
        let year = year_month.first().map_or("2026", |v| v);
        let month_num = year_month.get(1).map_or("04", |v| v);
        let archive_name = format!("{log_type}-{year}-{month_num}-{day}_{hour:02}.log.gz");
        let archive_path = format!("{base_path}/{month}/{day}/{archive_name}");
        let archive_url = base.join(&archive_path)?;

        let response = match client
            .get(archive_url.clone())
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

        let time_marker = format!("\n--- {}:{} | {} ---\n", day, hour, archive_url);
        all_content.extend_from_slice(time_marker.as_bytes());
        all_content.extend_from_slice(&decompressed);
    }

    Ok(all_content)
}

pub async fn download_tail_logs(
    base_url: &str,
    entries: &[&LogEntry],
    credentials: &AuthConfig,
    line_count: usize,
    output_path: &str,
) -> AppResult<DownloadSummary> {
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut total_bytes = 0_u64;
    let mut first_output = String::new();

    for entry in entries {
        let url = build_full_url(base_url, &entry.path, &entry.log_file)?;
        let entry_output_path = resolve_output_path(output_path, &entry.name, "tail")?;
        if let Some(parent) = entry_output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let response = client
            .get(&url)
            .basic_auth(&credentials.username, Some(password.clone()))
            .send()
            .await?
            .error_for_status()?;

        let body = if entry.log_file.ends_with(".gz") {
            let gz_bytes = response.bytes().await?;
            let mut decoder = GzDecoder::new(&gz_bytes[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            String::from_utf8_lossy(&decompressed).to_string()
        } else {
            response.text().await?
        };

        let lines: Vec<&str> = body.lines().collect();
        let tail_start = lines.len().saturating_sub(line_count);
        let tail_content = lines[tail_start..].join("\n");
        if !tail_content.is_empty() {
            tokio::fs::write(&entry_output_path, &tail_content).await?;
            total_bytes += tail_content.len() as u64;
        }

        if first_output.is_empty() {
            first_output = entry_output_path.to_string_lossy().to_string();
        }
    }

    Ok(DownloadSummary {
        server_count: entries.len(),
        bytes_written: total_bytes,
        output_path: first_output,
    })
}

fn resolve_output_path(output_path: &str, entry_name: &str, mode: &str) -> AppResult<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let safe_name = entry_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let filename = format!("{safe_name}_{mode}_{stamp}.log");

    if !output_path.trim().is_empty() {
        let p = PathBuf::from(output_path);
        if p.is_dir() {
            let mut path = p;
            path.push(&filename);
            return Ok(path);
        }
        return Ok(p);
    }

    let mut path = dirs::download_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| AppError::NotFound("Downloads directory".to_string()))?;
    path.push(&filename);
    Ok(path)
}

pub async fn download_raw_file(
    base_url: &str,
    credentials: &AuthConfig,
    file_path: &str,
    output_path: &str,
) -> AppResult<()> {
    let url = Url::parse(base_url)?.join(file_path)?;
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

pub async fn list_archive_files(
    base_url: &str,
    entry: &LogEntry,
    credentials: &AuthConfig,
) -> AppResult<Vec<DirEntry>> {
    let full_path = format!("{}{}", entry.path.trim_end_matches('/'), "/");
    let dir_url = build_full_url(base_url, &full_path, "")?;
    let all_entries = directory::fetch_directory_entries(&dir_url, credentials).await?;
    let files: Vec<DirEntry> = all_entries
        .into_iter()
        .filter(|e| {
            !e.is_dir && (e.name.ends_with(".log.gz") || e.name.ends_with(".log"))
        })
        .collect();
    Ok(files)
}

pub async fn download_selected_archive_files(
    credentials: &AuthConfig,
    file_urls: Vec<String>,
    output_path: &str,
) -> AppResult<DownloadSummary> {
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let base_output_path = if output_path.trim().is_empty() {
        let mut path = dirs::download_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| AppError::NotFound("Downloads directory".to_string()))?;
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        path.push(format!("archive_selected_{stamp}.log"));
        path
    } else {
        PathBuf::from(output_path)
    };
    if let Some(parent) = base_output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut all_content = Vec::new();
    let mut total_bytes = 0_u64;
    let mut server_count = 0_usize;

    for file_url in &file_urls {
        let response = match client
            .get(file_url.as_str())
            .basic_auth(&credentials.username, Some(&password))
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !response.status().is_success() {
            continue;
        }

        let content = if file_url.ends_with(".gz") {
            let gz_bytes = response.bytes().await?;
            let mut decoder = GzDecoder::new(&gz_bytes[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        } else {
            response.bytes().await?.to_vec()
        };

        if content.is_empty() {
            continue;
        }
        server_count += 1;
        let header = format!("\n===== {} =====\n", file_url);
        all_content.extend_from_slice(header.as_bytes());
        total_bytes += header.len() as u64;
        all_content.extend_from_slice(&content);
        total_bytes += content.len() as u64;
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
        for page in 0..page_count {
            let start = (page as u64 * PAGE_SIZE_BYTES) as usize;
            let end = std::cmp::min(start + PAGE_SIZE_BYTES as usize, all_content.len());
            if start >= all_content.len() {
                break;
            }
            let page_path = base_output_path.with_extension(format!("part{}.log", page + 1));
            tokio::fs::write(&page_path, &all_content[start..end]).await?;
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
