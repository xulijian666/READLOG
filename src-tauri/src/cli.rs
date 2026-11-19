//! readgcislog：给 AI agent 使用的命令行入口。
//! 账号密码和日志路径直接读取日志工具的本机配置，任何输出都不包含密码。

use std::collections::{HashMap, VecDeque};
use std::io::{self, Write};
use std::time::{Duration, Instant};

use async_compression::tokio::bufread::GzipDecoder;
use futures_util::{stream, StreamExt};
use read_log::config::{build_full_url, config_path, load_config, AppConfig, LogEntry};
use read_log::directory::{fetch_directory_entries, test_log_entry_connection};
use read_log::error::{AppError, AppResult};
use read_log::search::{LineSearcher, SearchMatchMode, SearchOptions};
use read_log::skill::skill_document;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;

const MAX_LINE_CHARS: usize = 2000;
const MAX_TAIL_LINES: usize = 5000;
const MAX_CAT_SPAN: usize = 2000;

const USAGE: &str = "readgcislog - 读取 GCIS 日志（账号和日志路径来自日志工具配置）

用法:
  readgcislog entries [关键字...]                 列出已配置的日志项
  readgcislog check [-e 日志项]                    检查配置和连接
  readgcislog tail -e <日志项> [-n 200]            读取当前日志末尾 N 行
  readgcislog grep <关键字> [-e 日志项]... [--file 相对路径]... [--mode phrase|all|any] [-C 3] [--max 50] [--case]
                                                   按文件完成顺序输出，默认搜索全部日志项的当前日志；有读取失败时退出码为 1
  readgcislog ls -e <日志项> [子目录]              列出日志项所在目录（查找归档文件）
  readgcislog cat -e <日志项> <文件相对路径> [--from 1] [--to 200]
                                                   按行号读取文件（.gz 自动解压）
  readgcislog skill [--out <SKILL.md 路径>]        输出或写入 readgcislog skill 说明

日志项选择（-e）：按 id、名称或别名精确匹配；否则按空格分隔的关键字匹配分组、名称、别名、路径（忽略大小写，需全部包含）。";

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(args).await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("错误: {error}");
            1
        }
    };
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

async fn run(args: Vec<String>) -> AppResult<()> {
    let Some((command, rest)) = args.split_first() else {
        println!("{USAGE}");
        return Ok(());
    };
    let rest = rest.to_vec();
    match command.as_str() {
        "entries" => cmd_entries(Parsed::parse(rest, &[], &[])?).await,
        "check" => cmd_check(Parsed::parse(rest, &["-e"], &[])?).await,
        "tail" => cmd_tail(Parsed::parse(rest, &["-e", "-n"], &[])?).await,
        "grep" => cmd_grep(Parsed::parse(rest, &["-e", "--file", "--mode", "-C", "--max"], &["--case"])?).await,
        "ls" => cmd_ls(Parsed::parse(rest, &["-e"], &[])?).await,
        "cat" => cmd_cat(Parsed::parse(rest, &["-e", "--from", "--to"], &[])?).await,
        "skill" => cmd_skill(Parsed::parse(rest, &["--out"], &[])?),
        "help" | "-h" | "--help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(AppError::Message(format!("未知命令 {other}\n\n{USAGE}"))),
    }
}

// ---------- 参数解析 ----------

struct Parsed {
    positional: Vec<String>,
    values: HashMap<String, Vec<String>>,
    switches: Vec<String>,
}

impl Parsed {
    fn parse(args: Vec<String>, value_flags: &[&str], bool_flags: &[&str]) -> AppResult<Self> {
        let mut parsed = Parsed { positional: Vec::new(), values: HashMap::new(), switches: Vec::new() };
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            if value_flags.contains(&arg.as_str()) {
                let value = iter
                    .next()
                    .ok_or_else(|| AppError::Message(format!("参数 {arg} 缺少值")))?;
                parsed.values.entry(arg).or_default().push(value);
            } else if bool_flags.contains(&arg.as_str()) {
                parsed.switches.push(arg);
            } else if arg.starts_with('-') && arg.len() > 1 && arg.parse::<i64>().is_err() {
                return Err(AppError::Message(format!("未知参数 {arg}\n\n{USAGE}")));
            } else {
                parsed.positional.push(arg);
            }
        }
        Ok(parsed)
    }

    fn all(&self, flag: &str) -> Vec<String> {
        self.values.get(flag).cloned().unwrap_or_default()
    }

    fn one(&self, flag: &str) -> Option<String> {
        self.values.get(flag).and_then(|values| values.last().cloned())
    }

    fn number(&self, flag: &str, default: usize) -> AppResult<usize> {
        match self.one(flag) {
            Some(value) => value
                .parse()
                .map_err(|_| AppError::Message(format!("参数 {flag} 需要正整数，收到 {value}"))),
            None => Ok(default),
        }
    }

    fn has(&self, flag: &str) -> bool {
        self.switches.iter().any(|item| item == flag)
    }
}

// ---------- 配置与日志项 ----------

async fn load_ready_config() -> AppResult<AppConfig> {
    let config = load_config().await?;
    if config.log_entries.is_empty() || config.base_url.trim().is_empty() {
        return Err(AppError::Message(
            "日志工具里还没有配置日志项，请先打开日志工具添加日志或导入 Excel".to_string(),
        ));
    }
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("日志工具里还没有填写用户名和密码".to_string()));
    }
    Ok(config)
}

fn entry_haystack(entry: &LogEntry) -> String {
    format!("{} {} {} {} {}", entry.group_name, entry.name, entry.alias, entry.path, entry.log_file).to_lowercase()
}

fn entry_aliases(entry: &LogEntry) -> Vec<String> {
    entry
        .alias
        .split([',', '，', '、', ';', '；'])
        .map(|alias| alias.trim().to_lowercase())
        .filter(|alias| !alias.is_empty())
        .collect()
}

fn select_entries<'a>(config: &'a AppConfig, selector: &str) -> Vec<&'a LogEntry> {
    let selector = selector.trim();
    if selector.is_empty() {
        return config.log_entries.iter().collect();
    }
    let lower = selector.to_lowercase();
    let exact: Vec<&LogEntry> = config
        .log_entries
        .iter()
        .filter(|entry| {
            entry.id == selector || entry.name.to_lowercase() == lower || entry_aliases(entry).contains(&lower)
        })
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    config
        .log_entries
        .iter()
        .filter(|entry| {
            let haystack = entry_haystack(entry);
            tokens.iter().all(|token| haystack.contains(token))
        })
        .collect()
}

/// 根据多个 -e 选择日志项，去重并保持配置顺序。
fn select_many<'a>(config: &'a AppConfig, selectors: &[String]) -> AppResult<Vec<&'a LogEntry>> {
    if selectors.is_empty() {
        return Ok(config.log_entries.iter().collect());
    }
    let mut ids = Vec::new();
    for selector in selectors {
        let matched = select_entries(config, selector);
        if matched.is_empty() {
            return Err(AppError::Message(format!("没有匹配「{selector}」的日志项，可用 entries 命令查看")));
        }
        ids.extend(matched.into_iter().map(|entry| entry.id.clone()));
    }
    Ok(config.log_entries.iter().filter(|entry| ids.contains(&entry.id)).collect())
}

fn select_one<'a>(config: &'a AppConfig, parsed: &Parsed) -> AppResult<&'a LogEntry> {
    let selector = parsed
        .one("-e")
        .ok_or_else(|| AppError::Message("请用 -e 指定一个日志项".to_string()))?;
    let matched = select_entries(config, &selector);
    match matched.len() {
        0 => Err(AppError::Message(format!("没有匹配「{selector}」的日志项，可用 entries 命令查看"))),
        1 => Ok(matched[0]),
        _ => {
            let names: Vec<String> = matched.iter().map(|entry| format!("  {}", entry.name)).collect();
            Err(AppError::Message(format!(
                "「{selector}」匹配到多个日志项，请写得更具体：\n{}",
                names.join("\n")
            )))
        }
    }
}

fn entry_dir_url(config: &AppConfig, entry: &LogEntry) -> AppResult<String> {
    build_full_url(&config.base_url, &entry.path, "")
}

/// 把相对路径解析到日志项目录下，拒绝跳出目录或指向其他地址。
fn resolve_relative(config: &AppConfig, entry: &LogEntry, relative: &str) -> AppResult<String> {
    let relative = relative.trim().replace('\\', "/");
    let escapes = relative.contains("://")
        || relative.starts_with('/')
        || relative.split('/').any(|part| part == "..");
    if escapes {
        return Err(AppError::Message(format!("路径必须是日志项目录下的相对路径：{relative}")));
    }
    let dir = url::Url::parse(&entry_dir_url(config, entry)?)?;
    Ok(dir.join(&relative)?.to_string())
}

fn file_name_of(url: &str) -> String {
    url.trim_end_matches('/').rsplit('/').next().unwrap_or(url).to_string()
}

// ---------- HTTP 按行读取 ----------

fn http_client() -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder().timeout(Duration::from_secs(300)).build()?)
}

struct ReadInfo {
    /// 读取是否从文件开头开始（Range 请求可能只读到末尾一段）。
    from_start: bool,
    /// 是否读到了文件末尾。
    reached_end: bool,
}

#[derive(Default)]
struct ReadStats {
    received_bytes: u64,
    scanned_lines: usize,
}

/// 逐行读取远程文件，回调返回 false 时停止。`.gz` 文件自动解压。
/// `tail_bytes` 只对普通文件生效，用 Range 请求只取末尾若干字节。
async fn for_each_line<F>(
    client: &reqwest::Client,
    config: &AppConfig,
    url: &str,
    tail_bytes: Option<u64>,
    stats: &mut ReadStats,
    on_line: F,
) -> AppResult<ReadInfo>
where
    F: FnMut(&str) -> bool,
{
    let is_gz = url.ends_with(".gz");
    let mut request = client
        .get(url)
        .basic_auth(&config.credentials.username, Some(&config.credentials.password));
    if let (Some(bytes), false) = (tail_bytes, is_gz) {
        request = request.header(reqwest::header::RANGE, format!("bytes=-{bytes}"));
    }
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::Message(format!("HTTP {} {}", status.as_u16(), url)));
    }

    let partial_start = status == reqwest::StatusCode::PARTIAL_CONTENT
        && response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("bytes "))
            .and_then(|value| value.split('-').next())
            .and_then(|start| start.trim().parse::<u64>().ok())
            .is_some_and(|start| start > 0);

    let received_bytes = &mut stats.received_bytes;
    let reader = StreamReader::new(response.bytes_stream().map(|chunk| {
        let chunk = chunk.map_err(io::Error::other)?;
        *received_bytes += chunk.len() as u64;
        Ok::<_, io::Error>(chunk)
    }));
    let reached_end = if is_gz {
        let mut decoder = GzipDecoder::new(reader);
        decoder.multiple_members(true);
        scan_lines(BufReader::new(decoder), false, &mut stats.scanned_lines, on_line).await?
    } else {
        scan_lines(reader, partial_start, &mut stats.scanned_lines, on_line).await?
    };
    Ok(ReadInfo { from_start: !partial_start, reached_end })
}

async fn scan_lines(
    mut reader: impl AsyncBufRead + Unpin,
    skip_first: bool,
    scanned_lines: &mut usize,
    mut on_line: impl FnMut(&str) -> bool,
) -> io::Result<bool> {
    let mut line = Vec::new();
    if skip_first {
        reader.read_until(b'\n', &mut line).await?;
        line.clear();
    }
    while reader.read_until(b'\n', &mut line).await? != 0 {
        *scanned_lines += 1;
        let text = String::from_utf8_lossy(&line);
        if !on_line(text.trim_end_matches(['\n', '\r'])) {
            return Ok(false);
        }
        line.clear();
    }
    Ok(true)
}

fn clip(line: &str) -> String {
    let count = line.chars().count();
    if count <= MAX_LINE_CHARS {
        return line.to_string();
    }
    let head: String = line.chars().take(MAX_LINE_CHARS).collect();
    format!("{head} …（本行过长已截断，共 {count} 字符）")
}

// ---------- 命令 ----------

async fn cmd_entries(parsed: Parsed) -> AppResult<()> {
    let config = load_ready_config().await?;
    let selector = parsed.positional.join(" ");
    let entries = select_entries(&config, &selector);
    println!("共 {} 个日志项（分组 | 名称 | 别名 | 当前日志文件 | 目录 URL）", entries.len());
    for entry in entries {
        println!(
            "{} | {} | {} | {} | {}",
            if entry.group_name.is_empty() { "-" } else { &entry.group_name },
            entry.name,
            if entry.alias.is_empty() { "-".to_string() } else { entry.alias.replace(',', "、") },
            entry.log_file,
            entry_dir_url(&config, entry)?
        );
    }
    Ok(())
}

async fn cmd_check(parsed: Parsed) -> AppResult<()> {
    let path = config_path()?;
    println!("配置文件: {}", path.display());
    let config = load_ready_config().await?;
    println!("日志服务器: {}", config.base_url);
    let entries = select_many(&config, &parsed.all("-e"))?;
    println!("检查 {} 个日志项的连接：", entries.len());
    let concurrency = config.settings.max_concurrent_servers.max(4);
    let results: Vec<String> = stream::iter(entries)
        .map(|entry| {
            let config = &config;
            async move {
                match test_log_entry_connection(&config.base_url, &config.credentials, entry).await {
                    Ok(result) if result.ok => format!("  OK   {} - {}", entry.name, result.message),
                    Ok(result) => format!("  FAIL {} - {}", entry.name, result.message),
                    Err(error) => format!("  FAIL {} - {}", entry.name, error),
                }
            }
        })
        .buffered(concurrency)
        .collect()
        .await;
    for line in results {
        println!("{line}");
    }
    Ok(())
}

async fn cmd_tail(parsed: Parsed) -> AppResult<()> {
    let config = load_ready_config().await?;
    let entries = select_many(&config, &parsed.all("-e"))?;
    if parsed.all("-e").is_empty() {
        return Err(AppError::Message("请用 -e 指定日志项".to_string()));
    }
    let count = parsed.number("-n", 200)?.clamp(1, MAX_TAIL_LINES);
    let client = http_client()?;
    let mut failures = 0;
    for entry in entries {
        let url = build_full_url(&config.base_url, &entry.path, &entry.log_file)?;
        println!("===== {} | {} | 末尾 {} 行 =====", entry.name, url, count);
        match tail_lines(&client, &config, &url, count).await {
            Ok((lines, includes_start)) => {
                for line in &lines {
                    println!("{}", clip(line));
                }
                let coverage = if includes_start { "包含文件开头" } else { "不包含文件开头，仅尾部样本" };
                println!("（实际返回 {} 行，{coverage}）", lines.len());
            }
            Err(error) => {
                failures += 1;
                println!("!! 读取失败: {error}");
            }
        }
    }
    if failures > 0 {
        return Err(AppError::Message(format!("{failures} 个文件未能完成读取")));
    }
    Ok(())
}

async fn tail_lines(client: &reqwest::Client, config: &AppConfig, url: &str, count: usize) -> AppResult<(VecDeque<String>, bool)> {
    let mut range = (count as u64 * 400).max(64 * 1024);
    loop {
        let use_range = range <= 64 * 1024 * 1024;
        let mut lines = VecDeque::with_capacity(count + 1);
        let mut stats = ReadStats::default();
        let info = for_each_line(client, config, url, use_range.then_some(range), &mut stats, |line| {
            if lines.len() == count {
                lines.pop_front();
            }
            lines.push_back(line.to_string());
            true
        })
        .await?;
        if info.from_start || lines.len() >= count || !use_range {
            return Ok((lines, info.from_start && stats.scanned_lines <= count));
        }
        range *= 8;
    }
}

async fn cmd_grep(parsed: Parsed) -> AppResult<()> {
    let keyword = parsed.positional.join(" ");
    if keyword.trim().is_empty() {
        return Err(AppError::Message("请提供搜索关键字，例如: grep traceId123".to_string()));
    }
    let match_mode = match parsed.one("--mode").as_deref() {
        None | Some("phrase") => SearchMatchMode::Phrase,
        Some("all") => SearchMatchMode::All,
        Some("any") => SearchMatchMode::Any,
        Some(other) => return Err(AppError::Message(format!("--mode 只支持 phrase/all/any，收到 {other}"))),
    };
    let context = parsed.number("-C", 3)?.min(50);
    let max_results = parsed.number("--max", 50)?.clamp(1, 1000);
    let options = SearchOptions {
        keyword: keyword.clone(),
        match_mode,
        case_sensitive: parsed.has("--case"),
        before_lines: context,
        after_lines: context,
        detail_context_lines: 0,
        max_results,
    };

    let config = load_ready_config().await?;
    let entries = select_many(&config, &parsed.all("-e"))?;
    let files = parsed.all("--file");
    let mut targets = Vec::new();
    for entry in &entries {
        if files.is_empty() {
            targets.push((*entry, build_full_url(&config.base_url, &entry.path, &entry.log_file)?));
        } else {
            for file in &files {
                targets.push((*entry, resolve_relative(&config, entry, file)?));
            }
        }
    }

    let mut output = io::stdout();
    writeln!(output, "搜索「{keyword}」，共 {} 个文件，每个文件最多 {max_results} 条", targets.len())?;
    output.flush()?;
    let client = http_client()?;
    let concurrency = config.settings.max_concurrent_servers.max(4);
    write_grep_results(&client, &config, targets, &options, concurrency, &mut output).await
}

async fn write_grep_results(
    client: &reqwest::Client,
    config: &AppConfig,
    targets: Vec<(&LogEntry, String)>,
    options: &SearchOptions,
    concurrency: usize,
    output: &mut impl Write,
) -> AppResult<()> {
    let mut results = stream::iter(targets)
        .map(|(entry, url)| {
            let options = options.clone();
            async move { grep_file(client, config, entry, &url, options).await }
        })
        .buffer_unordered(concurrency);

    let mut total_hits = 0;
    let mut complete = 0;
    let mut limited = 0;
    let mut failed = 0;
    while let Some(report) = results.next().await {
        total_hits += report.hits;
        match report.status {
            GrepStatus::Complete => complete += 1,
            GrepStatus::Limited => limited += 1,
            GrepStatus::Failed => failed += 1,
        }
        output.write_all(report.text.as_bytes())?;
        output.flush()?;
    }
    writeln!(output, "合计命中 {total_hits} 条；完整扫描 {complete} 个，达到上限提前停止 {limited} 个，读取失败 {failed} 个")?;
    output.flush()?;
    if failed > 0 {
        return Err(AppError::Message(format!("{failed} 个文件未能完成读取，命中汇总可能不完整")));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum GrepStatus {
    Complete,
    Limited,
    Failed,
}

struct GrepReport {
    hits: usize,
    status: GrepStatus,
    text: String,
}

async fn grep_file(
    client: &reqwest::Client,
    config: &AppConfig,
    entry: &LogEntry,
    url: &str,
    options: SearchOptions,
) -> GrepReport {
    let started = Instant::now();
    let mut searcher = LineSearcher::new(options);
    let mut hits = Vec::new();
    let mut stats = ReadStats::default();
    let result = for_each_line(client, config, url, None, &mut stats, |line| {
        hits.extend(searcher.process_line(line));
        !searcher.can_stop_after_max_results()
    })
    .await;
    let elapsed = started.elapsed().as_millis();
    hits.extend(searcher.finish());

    let metrics = format!("扫描 {} 行，收到 {} 字节，耗时 {elapsed} ms", stats.scanned_lines, stats.received_bytes);
    let mut out = String::new();
    if hits.is_empty() && result.is_ok() {
        return GrepReport {
            hits: 0,
            status: GrepStatus::Complete,
            text: format!("----- {} | {} | 无命中（{metrics}）\n", entry.name, file_name_of(url)),
        };
    }
    out.push_str(&format!("===== {} | {} =====\n", entry.name, url));
    for hit in &hits {
        let first = hit.line_number - hit.preview_before_lines.len();
        for (index, line) in hit.preview_before_lines.iter().enumerate() {
            out.push_str(&format!("{}- {}\n", first + index, clip(line)));
        }
        out.push_str(&format!("{}: {}\n", hit.line_number, clip(&hit.matched_line)));
        for (index, line) in hit.preview_after_lines.iter().enumerate() {
            out.push_str(&format!("{}- {}\n", hit.line_number + 1 + index, clip(line)));
        }
        out.push_str("--\n");
    }
    let status = match result {
        Ok(info) if !info.reached_end => {
            out.push_str(&format!("命中 {} 条，达到上限提前停止（{metrics}）\n", hits.len()));
            GrepStatus::Limited
        }
        Ok(_) => {
            out.push_str(&format!("命中 {} 条，完整扫描（{metrics}）\n", hits.len()));
            GrepStatus::Complete
        }
        Err(error) => {
            out.push_str(&format!("!! 读取中断: {error}（{metrics}）\n"));
            GrepStatus::Failed
        }
    };
    GrepReport { hits: hits.len(), status, text: out }
}

async fn cmd_ls(parsed: Parsed) -> AppResult<()> {
    let config = load_ready_config().await?;
    let entry = select_one(&config, &parsed)?;
    let url = match parsed.positional.first() {
        Some(sub) => {
            let sub = if sub.ends_with('/') || sub.ends_with('\\') { sub.clone() } else { format!("{sub}/") };
            resolve_relative(&config, entry, &sub)?
        }
        None => entry_dir_url(&config, entry)?,
    };
    let items = fetch_directory_entries(&url, &config.credentials).await?;
    println!("目录 {url}（共 {} 项，d=目录 f=文件）", items.len());
    for item in items {
        println!("{} {}", if item.is_dir { "d" } else { "f" }, item.name);
    }
    Ok(())
}

async fn cmd_cat(parsed: Parsed) -> AppResult<()> {
    let config = load_ready_config().await?;
    let entry = select_one(&config, &parsed)?;
    let relative = parsed
        .positional
        .first()
        .ok_or_else(|| AppError::Message("请提供文件相对路径，例如: cat -e gateway app.log".to_string()))?;
    let url = resolve_relative(&config, entry, relative)?;
    let from = parsed.number("--from", 1)?.max(1);
    let to = parsed.number("--to", from + 199)?.max(from);
    let to = to.min(from + MAX_CAT_SPAN - 1);

    println!("===== {} | {} | 第 {from}-{to} 行 =====", entry.name, url);
    let client = http_client()?;
    let mut number = 0usize;
    let info = for_each_line(&client, &config, &url, None, &mut ReadStats::default(), |line| {
        number += 1;
        if number >= from {
            println!("{number}: {}", clip(line));
        }
        number < to
    })
    .await?;
    if info.reached_end {
        println!("（文件结束，共 {number} 行）");
    }
    Ok(())
}

fn cmd_skill(parsed: Parsed) -> AppResult<()> {
    let exe = std::env::current_exe()?;
    let content = skill_document(&exe);
    match parsed.one("--out") {
        Some(out) => {
            let path = std::path::PathBuf::from(out);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content.as_bytes())?;
            println!("已写入 {}", path.display());
        }
        None => print!("{content}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use tokio::{io::AsyncWriteExt, net::TcpListener, sync::oneshot, time::timeout};

    async fn serve_once(
        path: &str,
        headers: &str,
        first_chunk: Vec<u8>,
        rest: Option<(Vec<u8>, oneshot::Receiver<()>)>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}{path}", listener.local_addr().unwrap());
        let headers = headers.as_bytes().to_vec();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut request = BufReader::new(socket);
            let mut line = String::new();
            loop {
                line.clear();
                if request.read_line(&mut line).await.unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            let mut socket = request.into_inner();
            socket.write_all(&headers).await.unwrap();
            socket.write_all(&first_chunk).await.unwrap();
            if let Some((remaining, release)) = rest {
                if release.await.is_ok() {
                    let _ = socket.write_all(&remaining).await;
                }
            }
        });
        url
    }

    fn gzip(text: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(text).unwrap();
        encoder.finish().unwrap()
    }

    fn grep_options(max_results: usize) -> SearchOptions {
        SearchOptions {
            keyword: "ERROR".into(),
            match_mode: SearchMatchMode::Phrase,
            case_sensitive: true,
            before_lines: 1,
            after_lines: 1,
            detail_context_lines: 0,
            max_results,
        }
    }

    #[tokio::test]
    async fn gzip_can_stop_before_the_response_finishes() {
        let config = sample();
        let first = gzip(b"before\nERROR hit\nafter\n");
        let received = first.len();
        let remaining = gzip(b"ERROR later\n");
        let (_release, hold) = oneshot::channel();
        let headers = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", received + remaining.len());
        let url = serve_once("/archive.log.gz", &headers, first, Some((remaining, hold))).await;

        let report = timeout(Duration::from_secs(5), grep_file(&http_client().unwrap(), &config, &config.log_entries[0], &url, grep_options(1)))
            .await.unwrap();
        assert_eq!(report.status, GrepStatus::Limited);
        assert!(report.text.contains("1- before\n2: ERROR hit\n3- after"));
        assert!(report.text.contains(&format!("扫描 3 行，收到 {received} 字节")));
    }

    #[tokio::test]
    async fn fast_file_is_flushed_while_an_earlier_file_is_pending_and_failures_stay_visible() {
        struct ReleaseOnFlush {
            bytes: Vec<u8>,
            release: Option<oneshot::Sender<()>>,
        }

        impl Write for ReleaseOnFlush {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                if let Some(release) = self.release.take() {
                    release.send(()).unwrap();
                }
                Ok(())
            }
        }

        let config = sample();
        let (release, hold) = oneshot::channel();
        let slow = serve_once("/slow.log", "HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n", b"ERROR slow\n".to_vec(), Some((Vec::new(), hold))).await;
        let fast = serve_once("/fast.log", "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n", b"ERROR fast".to_vec(), None).await;
        let targets = vec![(&config.log_entries[0], slow), (&config.log_entries[0], fast)];
        let mut output = ReleaseOnFlush { bytes: Vec::new(), release: Some(release) };

        let result = timeout(Duration::from_secs(5), write_grep_results(&http_client().unwrap(), &config, targets, &grep_options(50), 2, &mut output))
            .await.unwrap();
        assert!(result.is_err());
        let text = String::from_utf8(output.bytes).unwrap();
        assert!(text.find("1: ERROR fast").unwrap() < text.find("1: ERROR slow").unwrap());
        assert!(text.contains("读取中断"));
        assert!(text.contains("完整扫描 1 个，达到上限提前停止 0 个，读取失败 1 个"));
    }

    #[tokio::test]
    async fn tail_distinguishes_range_and_ring_buffer_samples_from_the_whole_file() {
        let config = sample();
        let client = http_client().unwrap();
        let partial = serve_once("/app.log", "HTTP/1.1 206 Partial Content\r\nContent-Length: 9\r\nContent-Range: bytes 10-18/19\r\n\r\n", b"x\nlast\n\r\n".to_vec(), None).await;
        let (lines, includes_start) = tail_lines(&client, &config, &partial, 2).await.unwrap();
        assert_eq!(lines, ["last", ""]);
        assert!(!includes_start);

        for count in [1, 2] {
            let full = serve_once("/app.log", "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n", b"first\nlast\n".to_vec(), None).await;
            let (lines, includes_start) = tail_lines(&client, &config, &full, count).await.unwrap();
            assert_eq!(lines.len(), count);
            assert_eq!(includes_start, count == 2);
        }
    }

    fn entry(id: &str, group: &str, name: &str, alias: &str) -> LogEntry {
        LogEntry {
            id: id.into(),
            name: name.into(),
            path: format!("/logs/{group}/{name}/"),
            log_file: "app.log".into(),
            visible: true,
            enabled: true,
            display_order: 0,
            group_id: group.into(),
            group_name: group.into(),
            alias: alias.into(),
        }
    }

    fn sample() -> AppConfig {
        let mut config = AppConfig::default();
        config.base_url = "http://logs.test/".into();
        config.log_entries = vec![
            entry("1", "SIT-C1", "cgis-web-gateway-10.0.0.1", "SIT网关,gw"),
            entry("2", "UAT1-C1", "cgis-web-gateway-10.0.0.2", "UAT网关"),
            entry("3", "SIT-C1", "cgis-web-claim-10.0.0.3", ""),
        ];
        config
    }

    fn ids(entries: Vec<&LogEntry>) -> Vec<String> {
        entries.into_iter().map(|entry| entry.id.clone()).collect()
    }

    #[test]
    fn alias_selects_exactly_one_entry() {
        let config = sample();
        assert_eq!(ids(select_entries(&config, "SIT网关")), vec!["1"]);
        assert_eq!(ids(select_entries(&config, "GW")), vec!["1"]);
        assert_eq!(ids(select_entries(&config, "UAT网关")), vec!["2"]);
    }

    #[test]
    fn keywords_match_group_name_alias_and_path() {
        let config = sample();
        assert_eq!(ids(select_entries(&config, "gateway")), vec!["1", "2"]);
        assert_eq!(ids(select_entries(&config, "sit gateway")), vec!["1"]);
        assert_eq!(ids(select_entries(&config, "网关")), vec!["1", "2"]);
        assert!(select_entries(&config, "nothing-here").is_empty());
    }

    #[test]
    fn relative_paths_cannot_escape_entry_directory() {
        let config = sample();
        let entry = &config.log_entries[0];
        assert!(resolve_relative(&config, entry, "../other/app.log").is_err());
        assert!(resolve_relative(&config, entry, "/etc/passwd").is_err());
        assert!(resolve_relative(&config, entry, "http://evil.test/x").is_err());
        assert_eq!(
            resolve_relative(&config, entry, "app.2026-05-06-18.log.gz").unwrap(),
            "http://logs.test/logs/SIT-C1/cgis-web-gateway-10.0.0.1/app.2026-05-06-18.log.gz"
        );
    }
}
