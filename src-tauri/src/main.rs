use read_log::{
    config::{load_config, save_config, AppConfig},
    directory::{test_connection, ConnectionCheckResult},
    download::{download_realtime_logs as do_download_realtime_logs, download_archive_logs as do_download_archive_logs, download_tail_logs as do_download_tail_logs, DownloadSummary},
    error::AppError,
};
use serde::Serialize;

#[derive(Serialize)]
struct AgentStatus {
    installed: bool,
}

#[tauri::command]
async fn load_app_config() -> Result<AppConfig, AppError> {
    load_config().await
}

#[tauri::command]
async fn save_app_config(config: AppConfig) -> Result<AppConfig, AppError> {
    save_config(config).await
}

#[tauri::command]
async fn test_all_connections(server_ids: Vec<String>) -> Result<Vec<ConnectionCheckResult>, AppError> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let log_type = config.settings.log_type.clone();
    let mut results = Vec::new();
    for server in config
        .servers
        .iter()
        .filter(|server| server_ids.is_empty() || server_ids.contains(&server.id))
    {
        results.push(test_connection(server, &config.credentials, &log_type).await?);
    }
    Ok(results)
}

#[tauri::command]
async fn download_realtime_logs(
    server_ids: Vec<String>,
    log_type: String,
    output_path: String,
) -> Result<DownloadSummary, AppError> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let servers: Vec<_> = config
        .servers
        .iter()
        .filter(|server| server.enabled && server_ids.contains(&server.id))
        .cloned()
        .collect();
    if servers.is_empty() {
        return Err(AppError::Message("请至少勾选一条服务器".to_string()));
    }
    let effective_path = if output_path.is_empty() { &config.settings.download_path } else { &output_path };
    do_download_realtime_logs(&servers, &config.credentials, &log_type, effective_path).await
}

#[tauri::command]
async fn download_archive_logs(
    server_ids: Vec<String>,
    log_type: String,
    month: String,
    day: String,
    hour_start: String,
    hour_end: String,
    output_path: String,
) -> Result<DownloadSummary, AppError> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let servers: Vec<_> = config
        .servers
        .iter()
        .filter(|server| server.enabled && server_ids.contains(&server.id))
        .cloned()
        .collect();
    if servers.is_empty() {
        return Err(AppError::Message("请至少勾选一条服务器".to_string()));
    }
    let start: u32 = hour_start.parse().map_err(|_| AppError::Message("起始小时格式错误".to_string()))?;
    let end: u32 = hour_end.parse().map_err(|_| AppError::Message("结束小时格式错误".to_string()))?;
    let effective_path = if output_path.is_empty() { &config.settings.download_path } else { &output_path };
    do_download_archive_logs(&servers, &config.credentials, &log_type, &month, &day, start, end, effective_path).await
}

#[tauri::command]
async fn download_tail_logs(
    server_ids: Vec<String>,
    log_type: String,
    line_count: String,
    output_path: String,
) -> Result<DownloadSummary, AppError> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let servers: Vec<_> = config
        .servers
        .iter()
        .filter(|server| server.enabled && server_ids.contains(&server.id))
        .cloned()
        .collect();
    if servers.is_empty() {
        return Err(AppError::Message("请至少勾选一条服务器".to_string()));
    }
    let count: usize = line_count.parse().map_err(|_| AppError::Message("行数格式错误".to_string()))?;
    if count == 0 {
        return Err(AppError::Message("行数必须大于 0".to_string()));
    }
    let effective_path = if output_path.is_empty() { &config.settings.download_path } else { &output_path };
    do_download_tail_logs(&servers, &config.credentials, &log_type, count, effective_path).await
}

#[tauri::command]
fn check_agent_status() -> Result<AgentStatus, AppError> {
    let installed = std::process::Command::new("where")
        .arg("claude")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    Ok(AgentStatus { installed })
}

#[tauri::command]
fn copy_agent_prompt(file_path: String) -> Result<(), AppError> {
    let full_prompt = format!(
        "以下是需要分析的日志文件路径：{}\n\n\
         请围绕我的问题分析该日志，只提取与问题相关的内容，忽略无关信息。不要主动读取其他日志文件，但我在问题中明确要求查看的文件（如源码、配置等）除外。结论优先，细节随后。\n\n\
         我的问题：",
        file_path
    );
    #[cfg(target_os = "windows")]
    {
        let temp_file = std::env::temp_dir().join("readlog_clip.txt");
        std::fs::write(&temp_file, full_prompt.as_bytes())?;
        std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-ExecutionPolicy", "Bypass",
                "-Command",
                &format!("Set-Clipboard -Value (Get-Content -Raw -Encoding UTF8 '{}')", temp_file.display()),
            ])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(full_prompt.as_bytes())?;
        child.wait()?;
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(full_prompt.as_bytes())?;
        child.wait()?;
    }
    Ok(())
}

#[tauri::command]
fn open_file(path: String) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&path).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&path).spawn()?;
    }
    Ok(())
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let folder = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path);
        std::process::Command::new("xdg-open").arg(&folder).spawn()?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            load_app_config,
            save_app_config,
            test_all_connections,
            download_realtime_logs,
            download_archive_logs,
            download_tail_logs,
            check_agent_status,
            copy_agent_prompt,
            open_file,
            open_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}