use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::crypto::{protect_password, reveal_password};
use crate::error::{AppError, AppResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default = "default_log_file")]
    pub log_file: String,
    pub visible: bool,
    pub enabled: bool,
    pub display_order: usize,
    #[serde(default)]
    pub group_id: String,
    #[serde(default)]
    pub group_name: String,
}

fn default_log_file() -> String {
    "app.log".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default, skip_serializing)]
    pub username: String,
    #[serde(default, skip_serializing)]
    pub password: String,
    pub enabled: bool,
    pub display_order: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub max_concurrent_servers: usize,
    pub default_batch_size: usize,
    pub default_level: String,
    #[serde(default)]
    pub download_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub log_entries: Vec<LogEntry>,
    #[serde(default)]
    pub credentials: AuthConfig,
    pub settings: Settings,
    // Legacy field for migration
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
}

fn default_base_url() -> String {
    "http://10.142.149.25:61000/".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            log_entries: vec![
                LogEntry {
                    id: Uuid::new_v4().to_string(),
                    name: "SIT-124".to_string(),
                    path: "/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.124/".to_string(),
                    log_file: "app.log".to_string(),
                    visible: true,
                    enabled: true,
                    display_order: 0,
                    group_id: String::new(),
                    group_name: String::new(),
                },
                LogEntry {
                    id: Uuid::new_v4().to_string(),
                    name: "SIT-186".to_string(),
                    path: "/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.186/".to_string(),
                    log_file: "app.log".to_string(),
                    visible: true,
                    enabled: true,
                    display_order: 1,
                    group_id: String::new(),
                    group_name: String::new(),
                },
                LogEntry {
                    id: Uuid::new_v4().to_string(),
                    name: "SIT-50".to_string(),
                    path: "/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.50/".to_string(),
                    log_file: "app.log".to_string(),
                    visible: true,
                    enabled: false,
                    display_order: 2,
                    group_id: String::new(),
                    group_name: String::new(),
                },
            ],
            credentials: AuthConfig {
                username: "cgisteam".to_string(),
                password: String::new(),
            },
            settings: Settings {
                max_concurrent_servers: 3,
                default_batch_size: 500,
                default_level: "ALL".to_string(),
                download_path: String::new(),
            },
            servers: Vec::new(),
        }
    }
}

pub fn config_path() -> AppResult<PathBuf> {
    let mut base = dirs::config_dir().ok_or_else(|| AppError::NotFound("config directory".to_string()))?;
    base.push("LogViewer");
    base.push("config.json");
    Ok(base)
}

pub async fn load_config() -> AppResult<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let text = fs::read_to_string(path).await?;
    let mut config: AppConfig = parse_config_text(&text)?;

    // Migrate legacy servers to log_entries
    if config.log_entries.is_empty() && !config.servers.is_empty() {
        // Extract base_url from first server URL
        if let Some(first) = config.servers.first() {
            if config.base_url.is_empty() {
                config.base_url = extract_base_url(&first.base_url);
            }
        }
        for (i, server) in config.servers.iter().enumerate() {
            let path = extract_path_from_url(&server.base_url, &config.base_url);
            config.log_entries.push(LogEntry {
                id: server.id.clone(),
                name: server.name.clone(),
                path,
                log_file: "app.log".to_string(),
                visible: true,
                enabled: server.enabled,
                display_order: i,
                group_id: String::new(),
                group_name: String::new(),
            });
        }
        config.servers.clear();
    }

    // Migrate credentials from legacy per-server
    if config.credentials.username.is_empty() {
        if let Some(server) = config.servers.iter().find(|s| !s.username.is_empty()) {
            config.credentials.username = server.username.clone();
        }
    }
    if config.credentials.password.is_empty() {
        if let Some(server) = config.servers.iter().find(|s| !s.password.is_empty()) {
            config.credentials.password = server.password.clone();
        }
    }
    if !config.credentials.password.is_empty() {
        config.credentials.password = reveal_password(&config.credentials.password)?;
    }
    Ok(config)
}

fn parse_config_text(text: &str) -> AppResult<AppConfig> {
    let text = text.trim_start_matches('\u{feff}');
    Ok(serde_json::from_str(text)?)
}

fn extract_base_url(url: &str) -> String {
    // Try to extract http://host:port/ from a full URL
    if let Ok(parsed) = url::Url::parse(url) {
        let scheme = parsed.scheme();
        let host = parsed.host_str().unwrap_or("");
        let port = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
        return format!("{scheme}://{host}{port}/");
    }
    String::new()
}

fn extract_path_from_url(full_url: &str, base_url: &str) -> String {
    if let (Ok(base), Ok(full)) = (url::Url::parse(base_url), url::Url::parse(full_url)) {
        let base_path = base.path().trim_end_matches('/');
        let full_path = full.path();
        if full_path.starts_with(base_path) {
            let relative = &full_path[base_path.len()..];
            if !relative.starts_with('/') {
                return format!("/{relative}");
            }
            return relative.to_string();
        }
        return full_path.to_string();
    }
    String::new()
}

pub async fn save_config(mut config: AppConfig) -> AppResult<AppConfig> {
    if !config.base_url.ends_with('/') {
        config.base_url.push('/');
    }
    validate_log_url(&config.base_url)?;

    for entry in &mut config.log_entries {
        if !entry.path.ends_with('/') {
            entry.path.push('/');
        }
    }

    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let public_config = config.clone();
    if !config.credentials.password.is_empty() && !config.credentials.password.starts_with("encrypted:") {
        config.credentials.password = protect_password(&config.credentials.password)?;
    }

    // Clear legacy servers before saving
    config.servers.clear();

    let text = serde_json::to_string_pretty(&config)?;
    fs::write(path, text).await?;
    Ok(public_config)
}

pub fn validate_log_url(value: &str) -> AppResult<()> {
    let url = url::Url::parse(value).map_err(|error| AppError::Message(format!("URL 不合法: {error}")))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(AppError::Message("URL 必须是 http 或 https".to_string()));
    }
    Ok(())
}

pub fn build_full_url(base_url: &str, path: &str, log_file: &str) -> AppResult<String> {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_matches('/');
    let full = if path.is_empty() {
        format!("{base}/{log_file}")
    } else {
        format!("{base}/{path}/{log_file}")
    };
    Ok(full)
}

/// Extract log type from log_file name for archive URL construction
/// e.g. "app.log" -> "app", "server.log.gz" -> "server"
pub fn extract_log_type(log_file: &str) -> String {
    let s = log_file;
    let s = s.strip_suffix(".gz").unwrap_or(s);
    let s = s.strip_suffix(".log").unwrap_or(s);
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::{extract_log_type, validate_log_url};

    #[test]
    fn accepts_http_url() {
        validate_log_url("http://host/path/").unwrap();
        validate_log_url("http://host/path/app.log").unwrap();
    }

    #[test]
    fn rejects_non_http_url() {
        let error = validate_log_url("ftp://host/path").unwrap_err();
        assert!(error.to_string().contains("http"));
    }

    #[test]
    fn extracts_log_type() {
        assert_eq!(extract_log_type("app.log"), "app");
        assert_eq!(extract_log_type("app.log.gz"), "app");
        assert_eq!(extract_log_type("server.log"), "server");
        assert_eq!(extract_log_type("server.log.gz"), "server");
    }

    #[test]
    fn loads_config_without_legacy_servers_field() {
        let text = r#"{
          "baseUrl": "http://10.142.149.25:61000/",
          "logEntries": [],
          "credentials": {
            "username": "cgisteam",
            "password": ""
          },
          "settings": {
            "maxConcurrentServers": 3,
            "defaultBatchSize": 500,
            "defaultLevel": "ALL",
            "downloadPath": ""
          }
        }"#;

        let config: super::AppConfig = serde_json::from_str(text).unwrap();

        assert!(config.servers.is_empty());
    }

    #[test]
    fn saved_config_keeps_empty_legacy_servers_field() {
        let text = serde_json::to_string(&super::AppConfig::default()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert!(value.get("servers").is_some());
        assert_eq!(value["servers"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn loads_config_with_utf8_bom() {
        let text = "\u{feff}{\"settings\":{\"maxConcurrentServers\":3,\"defaultBatchSize\":500,\"defaultLevel\":\"ALL\",\"downloadPath\":\"\"}}";

        let config = super::parse_config_text(text).unwrap();

        assert_eq!(config.settings.max_concurrent_servers, 3);
    }
}
