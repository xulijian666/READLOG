use read_log::{
    config::{load_config, save_config, AppConfig},
    directory::{test_connection, ConnectionCheckResult},
    download::{download_realtime_logs as do_download_realtime_logs, download_archive_logs as do_download_archive_logs, DownloadSummary},
    error::AppError,
};

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
    do_download_realtime_logs(&servers, &config.credentials, &log_type, &output_path).await
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
    do_download_archive_logs(&servers, &config.credentials, &log_type, &month, &day, start, end, &output_path).await
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
            open_file,
            open_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}