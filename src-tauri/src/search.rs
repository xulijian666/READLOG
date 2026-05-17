use std::collections::VecDeque;
use std::sync::Arc;

use flate2::read::GzDecoder;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Read;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::config::{build_full_url, load_config, AuthConfig, LogEntry};
use crate::error::{AppError, AppResult};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SearchMatchMode {
    Phrase,
    All,
    Any,
}

fn default_match_mode() -> SearchMatchMode {
    SearchMatchMode::Phrase
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchOptions {
    pub keyword: String,
    #[serde(default = "default_match_mode")]
    pub match_mode: SearchMatchMode,
    pub case_sensitive: bool,
    pub before_lines: usize,
    pub after_lines: usize,
    pub detail_context_lines: usize,
    pub max_results: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub line_number: usize,
    pub matched_line: String,
    pub preview_before_lines: Vec<String>,
    pub preview_after_lines: Vec<String>,
    pub detail_before_lines: Vec<String>,
    pub detail_after_lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSearchRequest {
    pub query_id: String,
    pub log_entry_ids: Vec<String>,
    pub keyword: String,
    #[serde(default = "default_match_mode")]
    pub match_mode: SearchMatchMode,
    pub case_sensitive: bool,
    pub before_lines: usize,
    pub after_lines: usize,
    pub detail_context_lines: usize,
    pub max_results: usize,
    pub batch_size: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSearchHit {
    pub id: String,
    pub log_entry_id: String,
    pub server_name: String,
    pub file_name: String,
    pub line_number: usize,
    pub matched_line: String,
    pub preview_before_lines: Vec<String>,
    pub preview_after_lines: Vec<String>,
    pub detail_before_lines: Vec<String>,
    pub detail_after_lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSearchProgressEvent {
    pub query_id: String,
    pub status: String,
    pub scanned_bytes: usize,
    pub scanned_lines: usize,
    pub matched_count: usize,
    pub current_server: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSearchResultEvent {
    pub query_id: String,
    pub batch_index: usize,
    pub results: Vec<LogSearchHit>,
    pub is_last_batch: bool,
}

#[derive(Default)]
pub struct SearchRegistry {
    tokens: Mutex<std::collections::HashMap<String, CancellationToken>>,
}

struct PendingHit {
    line_number: usize,
    matched_line: String,
    before: Vec<String>,
    after: Vec<String>,
}

struct LineSearcher {
    options: SearchOptions,
    before: VecDeque<String>,
    pending: Vec<PendingHit>,
    detail_after_limit: usize,
    detail_before_limit: usize,
    next_line_number: usize,
    matched_count: usize,
}

impl LineSearcher {
    fn new(options: SearchOptions) -> Self {
        let detail_before_limit = options.before_lines.max(options.detail_context_lines);
        let detail_after_limit = options.after_lines.max(options.detail_context_lines);
        Self {
            options,
            before: VecDeque::with_capacity(detail_before_limit),
            pending: Vec::new(),
            detail_after_limit,
            detail_before_limit,
            next_line_number: 1,
            matched_count: 0,
        }
    }

    fn process_line(&mut self, line: &str) -> Vec<SearchHit> {
        let mut completed = Vec::new();

        let mut still_pending = Vec::new();
        for mut pending in self.pending.drain(..) {
            if pending.after.len() < self.detail_after_limit {
                pending.after.push(line.to_string());
            }
            if pending.after.len() >= self.detail_after_limit {
                completed.push(build_hit(&self.options, pending));
            } else {
                still_pending.push(pending);
            }
        }
        self.pending = still_pending;

        if self.matched_count < self.options.max_results && self.matches(line) {
            let before = self.before.iter().cloned().collect();
            let pending = PendingHit {
                line_number: self.next_line_number,
                matched_line: line.to_string(),
                before,
                after: Vec::new(),
            };
            self.matched_count += 1;
            if self.detail_after_limit == 0 {
                completed.push(build_hit(&self.options, pending));
            } else {
                self.pending.push(pending);
            }
        }

        if self.detail_before_limit > 0 {
            if self.before.len() >= self.detail_before_limit {
                self.before.pop_front();
            }
            self.before.push_back(line.to_string());
        }
        self.next_line_number += 1;

        completed
    }

    fn finish(&mut self) -> Vec<SearchHit> {
        self.pending
            .drain(..)
            .map(|pending| build_hit(&self.options, pending))
            .collect()
    }

    fn can_stop_after_max_results(&self) -> bool {
        self.matched_count >= self.options.max_results && self.pending.is_empty()
    }

    fn matches(&self, line: &str) -> bool {
        let keyword = self.options.keyword.trim();
        if keyword.is_empty() {
            return false;
        }
        let line = normalize_for_match(line, self.options.case_sensitive);
        if self.options.case_sensitive {
            match_keywords(&line, keyword, self.options.match_mode)
        } else {
            match_keywords(&line, &keyword.to_lowercase(), self.options.match_mode)
        }
    }
}

fn normalize_for_match(value: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        value.to_string()
    } else {
        value.to_lowercase()
    }
}

fn match_keywords(line: &str, keyword: &str, mode: SearchMatchMode) -> bool {
    match mode {
        SearchMatchMode::Phrase => line.contains(keyword),
        SearchMatchMode::All => {
            let parts = split_keywords(keyword);
            !parts.is_empty() && parts.iter().all(|part| line.contains(part))
        }
        SearchMatchMode::Any => split_keywords(keyword).iter().any(|part| line.contains(part)),
    }
}

fn split_keywords(keyword: &str) -> Vec<&str> {
    keyword.split_whitespace().filter(|part| !part.is_empty()).collect()
}

fn build_hit(options: &SearchOptions, pending: PendingHit) -> SearchHit {
    SearchHit {
        line_number: pending.line_number,
        matched_line: pending.matched_line,
        preview_before_lines: take_tail(&pending.before, options.before_lines),
        preview_after_lines: pending.after.iter().take(options.after_lines).cloned().collect(),
        detail_before_lines: take_tail(&pending.before, options.detail_context_lines),
        detail_after_lines: pending.after.iter().take(options.detail_context_lines).cloned().collect(),
    }
}

fn take_tail(lines: &[String], count: usize) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    let start = lines.len().saturating_sub(count);
    lines[start..].to_vec()
}

pub fn search_lines_for_test(lines: &[&str], options: &SearchOptions) -> Vec<SearchHit> {
    let mut searcher = LineSearcher::new(options.clone());
    let mut hits = Vec::new();
    for line in lines {
        hits.extend(searcher.process_line(line));
    }
    hits.extend(searcher.finish());
    hits
}

enum SearchWorkerMessage {
    Batch {
        results: Vec<LogSearchHit>,
    },
    Progress {
        log_entry_id: String,
        server_name: String,
        scanned_bytes: usize,
        scanned_lines: usize,
        matched_count: usize,
    },
}

#[tauri::command]
pub async fn search_log_files(
    app: AppHandle,
    request: LogSearchRequest,
    registry: State<'_, SearchRegistry>,
) -> Result<String, AppError> {
    if request.keyword.trim().is_empty() {
        return Err(AppError::Message("请输入关键词".to_string()));
    }

    let token = CancellationToken::new();
    registry
        .tokens
        .lock()
        .await
        .insert(request.query_id.clone(), token.clone());

    let query_id = request.query_id.clone();
    let spawned_query_id = query_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_log_search(app.clone(), request, token).await {
            let _ = app.emit(
                "search-progress",
                LogSearchProgressEvent {
                    query_id: spawned_query_id,
                    status: format!("error: {error}"),
                    scanned_bytes: 0,
                    scanned_lines: 0,
                    matched_count: 0,
                    current_server: String::new(),
                },
            );
        }
    });

    Ok(query_id)
}

#[tauri::command]
pub async fn cancel_log_search(query_id: String, registry: State<'_, SearchRegistry>) -> Result<(), AppError> {
    if let Some(token) = registry.tokens.lock().await.remove(&query_id) {
        token.cancel();
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSearchRequest {
    pub query_id: String,
    pub file_urls: Vec<String>,
    pub keyword: String,
    #[serde(default = "default_match_mode")]
    pub match_mode: SearchMatchMode,
    pub case_sensitive: bool,
    pub before_lines: usize,
    pub after_lines: usize,
    pub detail_context_lines: usize,
    pub max_results: usize,
    pub batch_size: usize,
}

#[tauri::command]
pub async fn search_archive_files(
    app: AppHandle,
    request: ArchiveSearchRequest,
    registry: State<'_, SearchRegistry>,
) -> Result<String, AppError> {
    if request.keyword.trim().is_empty() {
        return Err(AppError::Message("请输入关键词".to_string()));
    }
    if request.file_urls.is_empty() {
        return Err(AppError::Message("请至少选择一个归档文件".to_string()));
    }

    let token = CancellationToken::new();
    registry
        .tokens
        .lock()
        .await
        .insert(request.query_id.clone(), token.clone());

    let query_id = request.query_id.clone();
    let spawned_query_id = query_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_archive_search(app.clone(), request, token).await {
            let _ = app.emit(
                "search-progress",
                LogSearchProgressEvent {
                    query_id: spawned_query_id,
                    status: format!("error: {error}"),
                    scanned_bytes: 0,
                    scanned_lines: 0,
                    matched_count: 0,
                    current_server: String::new(),
                },
            );
        }
    });

    Ok(query_id)
}

async fn run_archive_search(app: AppHandle, request: ArchiveSearchRequest, token: CancellationToken) -> AppResult<()> {
    let config = load_config().await?;
    let password = crate::crypto::reveal_password(&config.credentials.password)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let options = SearchOptions {
        keyword: request.keyword.clone(),
        match_mode: request.match_mode,
        case_sensitive: request.case_sensitive,
        before_lines: request.before_lines.min(50),
        after_lines: request.after_lines.min(50),
        detail_context_lines: request.detail_context_lines.min(500),
        max_results: request.max_results.clamp(1, 5000),
    };
    let batch_size = request.batch_size.clamp(1, 200);

    let mut batch_index = 0usize;
    let mut emitted_count = 0usize;
    let mut total_scanned_bytes = 0usize;
    let mut total_scanned_lines = 0usize;

    for file_url in &request.file_urls {
        if token.is_cancelled() {
            break;
        }

        let file_name = file_url.rsplit('/').next().unwrap_or(file_url).to_string();

        let _ = app.emit(
            "search-progress",
            LogSearchProgressEvent {
                query_id: request.query_id.clone(),
                status: "running".to_string(),
                scanned_bytes: total_scanned_bytes,
                scanned_lines: total_scanned_lines,
                matched_count: emitted_count,
                current_server: file_name.clone(),
            },
        );

        let response = match client
            .get(file_url.as_str())
            .basic_auth(&config.credentials.username, Some(&password))
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
            let gz_bytes = match response.bytes().await {
                Ok(b) => b,
                Err(_) => continue,
            };
            total_scanned_bytes += gz_bytes.len();
            let mut decoder = GzDecoder::new(&gz_bytes[..]);
            let mut decompressed = String::new();
            if decoder.read_to_string(&mut decompressed).is_err() {
                continue;
            }
            decompressed
        } else {
            match response.text().await {
                Ok(t) => {
                    total_scanned_bytes += t.len();
                    t
                }
                Err(_) => continue,
            }
        };

        let mut searcher = LineSearcher::new(options.clone());
        let mut file_batch = Vec::new();

        for line in content.lines() {
            if token.is_cancelled() {
                break;
            }
            total_scanned_lines += 1;

            for hit in searcher.process_line(line) {
                file_batch.push(LogSearchHit {
                    id: format!("{}:{}", file_name, hit.line_number),
                    log_entry_id: file_name.clone(),
                    server_name: file_name.clone(),
                    file_name: file_name.clone(),
                    line_number: hit.line_number,
                    matched_line: hit.matched_line,
                    preview_before_lines: hit.preview_before_lines,
                    preview_after_lines: hit.preview_after_lines,
                    detail_before_lines: hit.detail_before_lines,
                    detail_after_lines: hit.detail_after_lines,
                });
            }

            if file_batch.len() >= batch_size {
                let remaining = options.max_results.saturating_sub(emitted_count);
                if remaining == 0 {
                    token.cancel();
                    break;
                }
                let results = if file_batch.len() > remaining {
                    file_batch.truncate(remaining);
                    token.cancel();
                    std::mem::take(&mut file_batch)
                } else {
                    std::mem::take(&mut file_batch)
                };
                emitted_count += results.len();
                let _ = app.emit(
                    "search-result",
                    LogSearchResultEvent {
                        query_id: request.query_id.clone(),
                        batch_index,
                        results,
                        is_last_batch: false,
                    },
                );
                batch_index += 1;
            }

            if emitted_count >= options.max_results {
                break;
            }
        }

        // Flush remaining hits from this file
        for hit in searcher.finish() {
            file_batch.push(LogSearchHit {
                id: format!("{}:{}", file_name, hit.line_number),
                log_entry_id: file_name.clone(),
                server_name: file_name.clone(),
                file_name: file_name.clone(),
                line_number: hit.line_number,
                matched_line: hit.matched_line,
                preview_before_lines: hit.preview_before_lines,
                preview_after_lines: hit.preview_after_lines,
                detail_before_lines: hit.detail_before_lines,
                detail_after_lines: hit.detail_after_lines,
            });
        }
        if !file_batch.is_empty() {
            let remaining = options.max_results.saturating_sub(emitted_count);
            if remaining > 0 {
                if file_batch.len() > remaining {
                    file_batch.truncate(remaining);
                    token.cancel();
                }
                emitted_count += file_batch.len();
                let _ = app.emit(
                    "search-result",
                    LogSearchResultEvent {
                        query_id: request.query_id.clone(),
                        batch_index,
                        results: std::mem::take(&mut file_batch),
                        is_last_batch: false,
                    },
                );
                batch_index += 1;
            }
        }

        let _ = app.emit(
            "search-progress",
            LogSearchProgressEvent {
                query_id: request.query_id.clone(),
                status: "running".to_string(),
                scanned_bytes: total_scanned_bytes,
                scanned_lines: total_scanned_lines,
                matched_count: emitted_count,
                current_server: file_name,
            },
        );
    }

    let _ = app.emit(
        "search-result",
        LogSearchResultEvent {
            query_id: request.query_id.clone(),
            batch_index,
            results: Vec::new(),
            is_last_batch: true,
        },
    );

    let _ = app.emit(
        "search-progress",
        LogSearchProgressEvent {
            query_id: request.query_id,
            status: if token.is_cancelled() { "cancelled" } else { "completed" }.to_string(),
            scanned_bytes: total_scanned_bytes,
            scanned_lines: total_scanned_lines,
            matched_count: emitted_count,
            current_server: String::new(),
        },
    );
    Ok(())
}

async fn run_log_search(app: AppHandle, request: LogSearchRequest, token: CancellationToken) -> AppResult<()> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let selected: Vec<LogEntry> = config
        .log_entries
        .into_iter()
        .filter(|entry| entry.enabled && request.log_entry_ids.contains(&entry.id))
        .collect();
    if selected.is_empty() {
        return Err(AppError::Message("请至少勾选一条日志".to_string()));
    }

    let options = SearchOptions {
        keyword: request.keyword.clone(),
        match_mode: request.match_mode,
        case_sensitive: request.case_sensitive,
        before_lines: request.before_lines.min(50),
        after_lines: request.after_lines.min(50),
        detail_context_lines: request.detail_context_lines.min(500),
        max_results: request.max_results.clamp(1, 5000),
    };
    let batch_size = request.batch_size.clamp(1, 200);
    let semaphore = Arc::new(Semaphore::new(config.settings.max_concurrent_servers.max(1)));
    let (tx, mut rx) = mpsc::unbounded_channel();

    for entry in selected {
        let permit = semaphore.clone().acquire_owned().await.expect("semaphore closed");
        let token = token.child_token();
        let tx = tx.clone();
        let base_url = config.base_url.clone();
        let credentials = config.credentials.clone();
        let options = options.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let result = search_entry(&base_url, entry, credentials, options, batch_size, token, tx.clone()).await;
            if let Err(error) = result {
                let _ = tx.send(Err(error));
            }
        });
    }
    drop(tx);

    let mut batch_index = 0;
    let mut emitted_count = 0usize;
    let mut worker_stats: std::collections::HashMap<String, (usize, usize, usize)> = std::collections::HashMap::new();
    while let Some(message) = rx.recv().await {
        if token.is_cancelled() {
            break;
        }
        match message? {
            SearchWorkerMessage::Batch { mut results } => {
                let remaining = options.max_results.saturating_sub(emitted_count);
                if remaining == 0 {
                    token.cancel();
                    continue;
                }
                if results.len() > remaining {
                    results.truncate(remaining);
                    token.cancel();
                }
                emitted_count += results.len();
                app.emit(
                    "search-result",
                    LogSearchResultEvent {
                        query_id: request.query_id.clone(),
                        batch_index,
                        results,
                        is_last_batch: false,
                    },
                )
                .map_err(|error| AppError::Message(error.to_string()))?;
                batch_index += 1;
            }
            SearchWorkerMessage::Progress {
                log_entry_id,
                server_name,
                scanned_bytes: worker_bytes,
                scanned_lines: worker_lines,
                matched_count: worker_matches,
            } => {
                worker_stats.insert(log_entry_id, (worker_bytes, worker_lines, worker_matches));
                let scanned_bytes = worker_stats.values().map(|stats| stats.0).sum();
                let scanned_lines = worker_stats.values().map(|stats| stats.1).sum();
                app.emit(
                    "search-progress",
                    LogSearchProgressEvent {
                        query_id: request.query_id.clone(),
                        status: "running".to_string(),
                        scanned_bytes,
                        scanned_lines,
                        matched_count: emitted_count,
                        current_server: server_name,
                    },
                )
                .map_err(|error| AppError::Message(error.to_string()))?;
            }
        }
    }

    app.emit(
        "search-result",
        LogSearchResultEvent {
            query_id: request.query_id.clone(),
            batch_index,
            results: Vec::new(),
            is_last_batch: true,
        },
    )
    .map_err(|error| AppError::Message(error.to_string()))?;

    app.emit(
        "search-progress",
        LogSearchProgressEvent {
            query_id: request.query_id,
            status: if token.is_cancelled() { "cancelled" } else { "completed" }.to_string(),
            scanned_bytes: worker_stats.values().map(|stats| stats.0).sum(),
            scanned_lines: worker_stats.values().map(|stats| stats.1).sum(),
            matched_count: emitted_count,
            current_server: String::new(),
        },
    )
    .map_err(|error| AppError::Message(error.to_string()))?;
    Ok(())
}

async fn search_entry(
    base_url: &str,
    entry: LogEntry,
    credentials: AuthConfig,
    options: SearchOptions,
    batch_size: usize,
    token: CancellationToken,
    tx: mpsc::UnboundedSender<Result<SearchWorkerMessage, AppError>>,
) -> AppResult<()> {
    let url = build_full_url(base_url, &entry.path, &entry.log_file)?;
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?
        .get(&url)
        .basic_auth(&credentials.username, Some(password))
        .send()
        .await?
        .error_for_status()?;

    let mut searcher = LineSearcher::new(options);
    let mut batch = Vec::new();
    let mut scanned_bytes = 0usize;
    let mut scanned_lines = 0usize;
    let mut sent_matches = 0usize;

    if entry.log_file.ends_with(".gz") {
        let gz_bytes = response.bytes().await?;
        scanned_bytes += gz_bytes.len();
        let mut decoder = GzDecoder::new(&gz_bytes[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed)?;
        for line in decompressed.lines() {
            if token.is_cancelled() {
                break;
            }
            scanned_lines += 1;
            push_hits_for_line(&entry, line, &mut searcher, &mut batch, batch_size, &tx)?;
            sent_matches += flush_batch_if_needed(&mut batch, batch_size, &tx)?;
            if searcher.can_stop_after_max_results() {
                break;
            }
        }
    } else {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            if token.is_cancelled() {
                break;
            }
            let chunk = chunk?;
            scanned_bytes += chunk.len();
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(newline_index) = buffer.find('\n') {
                let mut line = buffer[..newline_index].to_string();
                if line.ends_with('\r') {
                    line.pop();
                }
                buffer.drain(..=newline_index);
                scanned_lines += 1;
                push_hits_for_line(&entry, &line, &mut searcher, &mut batch, batch_size, &tx)?;
                sent_matches += flush_batch_if_needed(&mut batch, batch_size, &tx)?;
                if searcher.can_stop_after_max_results() {
                    break;
                }
            }
            let _ = tx.send(Ok(SearchWorkerMessage::Progress {
                log_entry_id: entry.id.clone(),
                server_name: entry.name.clone(),
                scanned_bytes,
                scanned_lines,
                matched_count: sent_matches + batch.len(),
            }));
            if searcher.can_stop_after_max_results() {
                break;
            }
        }
        if !buffer.is_empty() && !token.is_cancelled() {
            scanned_lines += 1;
            push_hits_for_line(&entry, buffer.trim_end_matches('\r'), &mut searcher, &mut batch, batch_size, &tx)?;
        }
    }

    for hit in searcher.finish() {
        batch.push(to_log_search_hit(&entry, hit));
    }
    if !batch.is_empty() {
        let len = batch.len();
        let _ = tx.send(Ok(SearchWorkerMessage::Batch {
            results: std::mem::take(&mut batch),
        }));
        sent_matches += len;
    }
    let _ = tx.send(Ok(SearchWorkerMessage::Progress {
        log_entry_id: entry.id,
        server_name: entry.name,
        scanned_bytes,
        scanned_lines,
        matched_count: sent_matches,
    }));
    Ok(())
}

fn push_hits_for_line(
    entry: &LogEntry,
    line: &str,
    searcher: &mut LineSearcher,
    batch: &mut Vec<LogSearchHit>,
    _batch_size: usize,
    _tx: &mpsc::UnboundedSender<Result<SearchWorkerMessage, AppError>>,
) -> AppResult<()> {
    for hit in searcher.process_line(line) {
        batch.push(to_log_search_hit(entry, hit));
    }
    Ok(())
}

fn flush_batch_if_needed(
    batch: &mut Vec<LogSearchHit>,
    batch_size: usize,
    tx: &mpsc::UnboundedSender<Result<SearchWorkerMessage, AppError>>,
) -> AppResult<usize> {
    if batch.len() < batch_size {
        return Ok(0);
    }
    let len = batch.len();
    let _ = tx.send(Ok(SearchWorkerMessage::Batch {
        results: std::mem::take(batch),
    }));
    Ok(len)
}

fn to_log_search_hit(entry: &LogEntry, hit: SearchHit) -> LogSearchHit {
    LogSearchHit {
        id: format!("{}:{}", entry.id, hit.line_number),
        log_entry_id: entry.id.clone(),
        server_name: entry.name.clone(),
        file_name: entry.log_file.clone(),
        line_number: hit.line_number,
        matched_line: hit.matched_line,
        preview_before_lines: hit.preview_before_lines,
        preview_after_lines: hit.preview_after_lines,
        detail_before_lines: hit.detail_before_lines,
        detail_after_lines: hit.detail_after_lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_preview_and_detail_context_around_match() {
        let lines = [
            "line 1",
            "line 2",
            "line 3",
            "line 4",
            "needle is here",
            "line 6",
            "line 7",
            "line 8",
        ];
        let options = SearchOptions {
            keyword: "needle".to_string(),
            match_mode: SearchMatchMode::Phrase,
            case_sensitive: false,
            before_lines: 2,
            after_lines: 2,
            detail_context_lines: 3,
            max_results: 10,
        };

        let hits = search_lines_for_test(&lines, &options);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_number, 5);
        assert_eq!(hits[0].preview_before_lines, vec!["line 3", "line 4"]);
        assert_eq!(hits[0].preview_after_lines, vec!["line 6", "line 7"]);
        assert_eq!(hits[0].detail_before_lines, vec!["line 2", "line 3", "line 4"]);
        assert_eq!(hits[0].detail_after_lines, vec!["line 6", "line 7", "line 8"]);
    }

    #[test]
    fn respects_case_sensitive_matching() {
        let lines = ["Needle", "needle"];
        let options = SearchOptions {
            keyword: "Needle".to_string(),
            match_mode: SearchMatchMode::Phrase,
            case_sensitive: true,
            before_lines: 1,
            after_lines: 1,
            detail_context_lines: 1,
            max_results: 10,
        };

        let hits = search_lines_for_test(&lines, &options);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_number, 1);
    }

    #[test]
    fn stops_at_max_results() {
        let lines = ["needle 1", "needle 2", "needle 3"];
        let options = SearchOptions {
            keyword: "needle".to_string(),
            match_mode: SearchMatchMode::Phrase,
            case_sensitive: false,
            before_lines: 0,
            after_lines: 0,
            detail_context_lines: 0,
            max_results: 2,
        };

        let hits = search_lines_for_test(&lines, &options);

        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn phrase_mode_matches_the_whole_keyword_text() {
        let lines = ["aaa middle bbb", "aaa bbb"];
        let options = SearchOptions {
            keyword: "aaa bbb".to_string(),
            match_mode: SearchMatchMode::Phrase,
            case_sensitive: false,
            before_lines: 0,
            after_lines: 0,
            detail_context_lines: 0,
            max_results: 10,
        };

        let hits = search_lines_for_test(&lines, &options);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_number, 2);
    }

    #[test]
    fn all_mode_requires_every_keyword_on_the_same_line() {
        let lines = ["aaa only", "bbb only", "aaa middle bbb", "ccc aaa"];
        let options = SearchOptions {
            keyword: "aaa bbb".to_string(),
            match_mode: SearchMatchMode::All,
            case_sensitive: false,
            before_lines: 0,
            after_lines: 0,
            detail_context_lines: 0,
            max_results: 10,
        };

        let hits = search_lines_for_test(&lines, &options);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_number, 3);
    }

    #[test]
    fn any_mode_matches_when_one_keyword_is_present() {
        let lines = ["aaa only", "no match", "bbb only", "ccc"];
        let options = SearchOptions {
            keyword: "aaa bbb".to_string(),
            match_mode: SearchMatchMode::Any,
            case_sensitive: false,
            before_lines: 0,
            after_lines: 0,
            detail_context_lines: 0,
            max_results: 10,
        };

        let hits = search_lines_for_test(&lines, &options);

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line_number, 1);
        assert_eq!(hits[1].line_number, 3);
    }
}
