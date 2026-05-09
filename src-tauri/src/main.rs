use read_log::{
    config::{load_config, save_config, AppConfig},
    directory::{test_log_entry_connection, ConnectionCheckResult},
    download::{download_realtime_logs as do_download_realtime_logs, download_archive_logs as do_download_archive_logs, download_tail_logs as do_download_tail_logs, DownloadSummary},
    error::AppError,
    search::SearchRegistry,
};
use rust_xlsxwriter::{Workbook, Format};
use serde::{Deserialize, Serialize};

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
async fn test_all_connections(log_entry_ids: Vec<String>) -> Result<Vec<ConnectionCheckResult>, AppError> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let mut results = Vec::new();
    for entry in config
        .log_entries
        .iter()
        .filter(|entry| log_entry_ids.is_empty() || log_entry_ids.contains(&entry.id))
    {
        results.push(test_log_entry_connection(&config.base_url, &config.credentials, entry).await?);
    }
    Ok(results)
}

#[tauri::command]
async fn download_realtime_logs(
    log_entry_ids: Vec<String>,
    output_path: String,
) -> Result<DownloadSummary, AppError> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let entries: Vec<_> = config
        .log_entries
        .iter()
        .filter(|entry| entry.enabled && log_entry_ids.contains(&entry.id))
        .collect();
    if entries.is_empty() {
        return Err(AppError::Message("请至少勾选一条日志".to_string()));
    }
    let effective_path = if output_path.is_empty() { &config.settings.download_path } else { &output_path };
    do_download_realtime_logs(&config.base_url, &entries, &config.credentials, effective_path).await
}

#[tauri::command]
async fn download_archive_logs(
    log_entry_ids: Vec<String>,
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
    let entries: Vec<_> = config
        .log_entries
        .iter()
        .filter(|entry| entry.enabled && log_entry_ids.contains(&entry.id))
        .collect();
    if entries.is_empty() {
        return Err(AppError::Message("请至少勾选一条日志".to_string()));
    }
    let start: u32 = hour_start.parse().map_err(|_| AppError::Message("起始小时格式错误".to_string()))?;
    let end: u32 = hour_end.parse().map_err(|_| AppError::Message("结束小时格式错误".to_string()))?;
    let effective_path = if output_path.is_empty() { &config.settings.download_path } else { &output_path };
    do_download_archive_logs(&config.base_url, &entries, &config.credentials, &month, &day, start, end, effective_path).await
}

#[tauri::command]
async fn download_tail_logs(
    log_entry_ids: Vec<String>,
    line_count: String,
    output_path: String,
) -> Result<DownloadSummary, AppError> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let entries: Vec<_> = config
        .log_entries
        .iter()
        .filter(|entry| entry.enabled && log_entry_ids.contains(&entry.id))
        .collect();
    if entries.is_empty() {
        return Err(AppError::Message("请至少勾选一条日志".to_string()));
    }
    let count: usize = line_count.parse().map_err(|_| AppError::Message("行数格式错误".to_string()))?;
    if count == 0 {
        return Err(AppError::Message("行数必须大于 0".to_string()));
    }
    let effective_path = if output_path.is_empty() { &config.settings.download_path } else { &output_path };
    do_download_tail_logs(&config.base_url, &entries, &config.credentials, count, effective_path).await
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

#[derive(Deserialize)]
struct ExportEntry {
    group_name: String,
    name: String,
    url: String,
}

#[tauri::command]
async fn export_xlsx(entries: Vec<ExportEntry>, output_path: String) -> Result<(), AppError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_fmt = Format::new().set_bold();
    worksheet.write_string_with_format(0, 0, "分组名称", &header_fmt).map_err(|e| AppError::Message(e.to_string()))?;
    worksheet.write_string_with_format(0, 1, "日志名称", &header_fmt).map_err(|e| AppError::Message(e.to_string()))?;
    worksheet.write_string_with_format(0, 2, "日志URL", &header_fmt).map_err(|e| AppError::Message(e.to_string()))?;

    worksheet.set_column_width(0, 20).map_err(|e| AppError::Message(e.to_string()))?;
    worksheet.set_column_width(1, 25).map_err(|e| AppError::Message(e.to_string()))?;
    worksheet.set_column_width(2, 80).map_err(|e| AppError::Message(e.to_string()))?;

    for (i, entry) in entries.iter().enumerate() {
        let row = (i + 1) as u32;
        worksheet.write_string(row, 0, &entry.group_name).map_err(|e| AppError::Message(e.to_string()))?;
        worksheet.write_string(row, 1, &entry.name).map_err(|e| AppError::Message(e.to_string()))?;
        worksheet.write_string(row, 2, &entry.url).map_err(|e| AppError::Message(e.to_string()))?;
    }

    workbook.save(&output_path).map_err(|e| AppError::Message(e.to_string()))?;
    Ok(())
}

#[tauri::command]
fn pick_directory() -> Result<Option<String>, AppError> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = '选择下载目录'
$dialog.ShowNewFolderButton = $true
if ($dialog.ShowDialog() -eq 'OK') { $dialog.SelectedPath }
"#;
        let output = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
            .output()
            .map_err(|e| AppError::Message(e.to_string()))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(path))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("zenity")
            .args(["--file-selection", "--directory", "--title=选择下载目录"])
            .output()
            .map_err(|e| AppError::Message(e.to_string()))?;

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(path))
        }
    }
}

fn main() {
    tauri::Builder::default()
        .manage(SearchRegistry::default())
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
            open_folder,
            export_xlsx,
            pick_directory,
            read_log::search::search_log_files,
            read_log::search::cancel_log_search
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
