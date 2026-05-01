use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

use crate::config::{validate_log_url, AuthConfig, ServerConfig};
use crate::error::{AppError, AppResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub url: String,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionCheckResult {
    pub ok: bool,
    pub server_id: String,
    pub server_name: String,
    pub status_code: u16,
    pub message: String,
    pub file_count: usize,
    pub file_size: Option<u64>,
}

pub fn parse_directory_listing(html: &str, base_url: &str) -> AppResult<Vec<DirEntry>> {
    let base = Url::parse(base_url)?;
    let selector = Selector::parse("a[href]").map_err(|error| AppError::Message(error.to_string()))?;
    let document = Html::parse_document(html);
    let mut entries = Vec::new();

    for element in document.select(&selector) {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        if href == "../" || href == ".." {
            continue;
        }

        let url = base.join(href)?;
        let text = element.text().collect::<String>().trim().to_string();
        let name = if text.is_empty() { href.trim_end_matches('/').to_string() } else { text };
        entries.push(DirEntry {
            name,
            url: url.to_string(),
            is_dir: href.ends_with('/'),
        });
    }

    Ok(entries)
}

pub async fn fetch_directory_entries(server: &ServerConfig, credentials: &AuthConfig) -> AppResult<Vec<DirEntry>> {
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let html = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?
        .get(&server.base_url)
        .basic_auth(&credentials.username, Some(password))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_directory_listing(&html, &server.base_url)
}

pub async fn test_connection(server: &ServerConfig, credentials: &AuthConfig, log_type: &str) -> AppResult<ConnectionCheckResult> {
    validate_log_url(&server.base_url)?;
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let test_url = build_test_url(&server.base_url, log_type)?;
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?
        .get(test_url.clone())
        .basic_auth(&credentials.username, Some(password))
        .send()
        .await?;
    let status = response.status();
    let status_code = status.as_u16();

    if !status.is_success() {
        return Ok(ConnectionCheckResult {
            ok: false,
            server_id: server.id.clone(),
            server_name: server.name.clone(),
            status_code,
            message: format!("连接失败，HTTP {status_code}"),
            file_count: 0,
            file_size: None,
        });
    }
    let file_size = response.content_length();

    Ok(ConnectionCheckResult {
        ok: true,
        server_id: server.id.clone(),
        server_name: server.name.clone(),
        status_code,
        message: match file_size {
            Some(size) => format!("连接成功，文件大小 {}", format_bytes(size)),
            None => format!("连接成功，HTTP {status_code}，未返回文件大小"),
        },
        file_count: 1,
        file_size,
    })
}

fn build_test_url(base_url: &str, log_type: &str) -> AppResult<Url> {
    let base = Url::parse(base_url)?;
    let test_url = if base.path().ends_with('/') {
        base.join(&format!("{log_type}.log"))?
    } else {
        base
    };
    Ok(test_url)
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}
