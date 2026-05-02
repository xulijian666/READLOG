use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::crypto::{protect_password, reveal_password};
use crate::error::{AppError, AppResult};

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
    #[serde(default = "default_log_type")]
    pub log_type: String,
    #[serde(default)]
    pub download_path: String,
}

fn default_log_type() -> String {
    "app".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub servers: Vec<ServerConfig>,
    #[serde(default)]
    pub credentials: AuthConfig,
    pub settings: Settings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            servers: vec![
                ServerConfig {
                    id: Uuid::new_v4().to_string(),
                    name: "SIT-124".to_string(),
                    base_url: "http://10.142.149.25:61000/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.124/".to_string(),
                    username: "cgisteam".to_string(),
                    password: String::new(),
                    enabled: true,
                    display_order: 0,
                },
                ServerConfig {
                    id: Uuid::new_v4().to_string(),
                    name: "SIT-186".to_string(),
                    base_url: "http://10.142.149.25:61000/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.186/".to_string(),
                    username: "cgisteam".to_string(),
                    password: String::new(),
                    enabled: true,
                    display_order: 1,
                },
                ServerConfig {
                    id: Uuid::new_v4().to_string(),
                    name: "SIT-50".to_string(),
                    base_url: "http://10.142.149.25:61000/fileviewer/gcis/SIT/log/coregroup/core_log/INTERFACE/10.142.149.50/".to_string(),
                    username: "cgisteam".to_string(),
                    password: String::new(),
                    enabled: false,
                    display_order: 2,
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
                log_type: "app".to_string(),
                download_path: String::new(),
            },
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
    let mut config: AppConfig = serde_json::from_str(&text)?;
    if config.credentials.username.is_empty() {
        if let Some(server) = config.servers.iter().find(|server| !server.username.is_empty()) {
            config.credentials.username = server.username.clone();
        }
    }
    if config.credentials.password.is_empty() {
        if let Some(server) = config.servers.iter().find(|server| !server.password.is_empty()) {
            config.credentials.password = server.password.clone();
        }
    }
    if !config.credentials.password.is_empty() {
        config.credentials.password = reveal_password(&config.credentials.password)?;
    }
    Ok(config)
}

pub async fn save_config(mut config: AppConfig) -> AppResult<AppConfig> {
    for server in &mut config.servers {
        if !server.base_url.ends_with('/') {
            server.base_url.push('/');
        }
        validate_log_url(&server.base_url)?;
    }

    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let public_config = config.clone();
    if !config.credentials.password.is_empty() && !config.credentials.password.starts_with("encrypted:") {
        config.credentials.password = protect_password(&config.credentials.password)?;
    }

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

#[cfg(test)]
mod tests {
    use super::validate_log_url;

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
}
