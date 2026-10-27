use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

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
    String::new()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            log_entries: Vec::new(),
            credentials: AuthConfig::default(),
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
    // This user-owned location is stable across upgrades and outside the install directory.
    let mut base = dirs::config_dir().ok_or_else(|| AppError::NotFound("config directory".to_string()))?;
    base.push("LogViewer");
    base.push("config.json");
    Ok(base)
}

pub async fn load_config() -> AppResult<AppConfig> {
    load_config_from_path(&config_path()?).await
}

async fn load_config_from_path(path: &Path) -> AppResult<AppConfig> {
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
    config.servers.clear();
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

pub async fn save_config(config: AppConfig) -> AppResult<AppConfig> {
    save_config_to_path(config, &config_path()?).await
}

async fn save_config_to_path(mut config: AppConfig, path: &Path) -> AppResult<AppConfig> {
    config.base_url = config.base_url.trim().to_string();
    if !config.base_url.is_empty() && !config.base_url.ends_with('/') {
        config.base_url.push('/');
    }
    if !config.base_url.is_empty() {
        validate_log_url(&config.base_url)?;
    }

    for entry in &mut config.log_entries {
        if !entry.path.ends_with('/') {
            entry.path.push('/');
        }
    }

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

    #[tokio::test]
    async fn first_launch_has_no_bundled_servers_and_does_not_create_config() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        let config = super::load_config_from_path(&path).await.unwrap();
        assert!(config.base_url.is_empty());
        assert!(config.log_entries.is_empty());
        assert!(config.credentials.username.is_empty());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn existing_user_config_survives_repeated_launches_without_rewriting() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("user-data/config.json");
        let mut config = super::AppConfig::default();
        config.base_url = "https://logs.example.test".into();
        config.credentials.username = "user".into();
        config.credentials.password = "test-password".into();
        config.settings.download_path = "D:/my-logs".into();
        config.log_entries.push(super::LogEntry {
            id: "custom-id".into(), name: "custom-service".into(),
            path: "/custom/node/".into(), log_file: "server.log.gz".into(),
            visible: true, enabled: false, display_order: 7,
            group_id: "custom-group".into(), group_name: "my-group".into(),
        });
        super::save_config_to_path(config, &path).await.unwrap();
        let bytes = tokio::fs::read(&path).await.unwrap();
        for _ in 0..2 {
            let loaded = super::load_config_from_path(&path).await.unwrap();
            assert_eq!(loaded.base_url, "https://logs.example.test/");
            assert_eq!(loaded.credentials.password, "test-password");
            assert_eq!(loaded.settings.download_path, "D:/my-logs");
            let entry = &loaded.log_entries[0];
            assert_eq!(entry.id, "custom-id");
            assert_eq!(entry.path, "/custom/node/");
            assert_eq!(entry.log_file, "server.log.gz");
            assert_eq!(entry.group_id, "custom-group");
            assert!(!entry.enabled);
            assert_eq!(tokio::fs::read(&path).await.unwrap(), bytes);
        }
    }

    #[tokio::test]
    async fn configuration_can_be_saved_before_connection_is_set() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        super::save_config_to_path(super::AppConfig::default(), &path).await.unwrap();
        assert!(super::load_config_from_path(&path).await.unwrap().base_url.is_empty());
    }

    #[tokio::test]
    async fn invalid_existing_config_is_reported_without_replacing_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        tokio::fs::write(&path, "invalid json").await.unwrap();
        assert!(super::load_config_from_path(&path).await.is_err());
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "invalid json");
    }

    #[tokio::test]
    async fn legacy_server_migration_keeps_url_and_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        let text = r#"{"servers":[{"id":"old","name":"old-service","baseUrl":"https://old.example.test/logs/","username":"old-user","password":"old-password","enabled":true,"displayOrder":0}],"settings":{"maxConcurrentServers":3,"defaultBatchSize":500,"defaultLevel":"ALL"}}"#;
        tokio::fs::write(&path, text).await.unwrap();
        let loaded = super::load_config_from_path(&path).await.unwrap();
        assert_eq!(loaded.base_url, "https://old.example.test/");
        assert_eq!(loaded.log_entries[0].path, "/logs/");
        assert_eq!(loaded.credentials.username, "old-user");
        assert_eq!(loaded.credentials.password, "old-password");
        assert!(loaded.servers.is_empty());
    }

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
