use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::config::{load_config, AuthConfig, ServerConfig};
use crate::error::{AppError, AppResult};
use crate::filter::QueryFilter;
use crate::parser::{LogEvent, ServerContext};
use crate::stream_reader::LogStreamReader;

#[derive(Default)]
pub struct QueryRegistry {
    tokens: Mutex<HashMap<String, CancellationToken>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRequest {
    pub query_id: String,
    pub server_ids: Vec<String>,
    pub file_path: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub keyword: String,
    pub level: String,
    pub batch_size: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryProgressEvent {
    pub query_id: String,
    pub status: String,
    pub scanned_bytes: usize,
    pub scanned_events: usize,
    pub matched_events: usize,
    pub servers_completed: Vec<String>,
    pub servers_pending: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultEvent {
    pub query_id: String,
    pub batch_index: usize,
    pub events: Vec<LogEvent>,
    pub is_last_batch: bool,
}

#[derive(Clone, Debug)]
struct ServerQueryStats {
    scanned_bytes: usize,
    scanned_events: usize,
    matched_events: usize,
}

#[derive(Debug)]
enum ServerMessage {
    Batch {
        events: Vec<LogEvent>,
    },
    Progress {
        server_id: String,
        scanned_bytes: usize,
        scanned_events: usize,
        matched_events: usize,
    },
    Done {
        server_id: String,
    },
}

#[tauri::command]
pub async fn execute_query(
    app: AppHandle,
    request: QueryRequest,
    registry: State<'_, QueryRegistry>,
) -> Result<String, AppError> {
    let token = CancellationToken::new();
    registry
        .tokens
        .lock()
        .await
        .insert(request.query_id.clone(), token.clone());

    let query_id = request.query_id.clone();
    let spawned_query_id = query_id.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_query(app.clone(), request, token).await;
        if let Err(error) = result {
            let _ = app.emit(
                "query-progress",
                QueryProgressEvent {
                    query_id: spawned_query_id,
                    status: format!("error: {error}"),
                    scanned_bytes: 0,
                    scanned_events: 0,
                    matched_events: 0,
                    servers_completed: Vec::new(),
                    servers_pending: Vec::new(),
                },
            );
        }
    });

    Ok(query_id)
}

#[tauri::command]
pub async fn cancel_query(query_id: String, registry: State<'_, QueryRegistry>) -> Result<(), AppError> {
    if let Some(token) = registry.tokens.lock().await.remove(&query_id) {
        token.cancel();
    }
    Ok(())
}

async fn run_query(app: AppHandle, request: QueryRequest, token: CancellationToken) -> AppResult<()> {
    let config = load_config().await?;
    if config.credentials.username.trim().is_empty() {
        return Err(AppError::Message("请先填写统一用户名".to_string()));
    }
    let selected: Vec<ServerConfig> = config
        .servers
        .into_iter()
        .filter(|server| server.enabled && request.server_ids.contains(&server.id))
        .collect();
    if selected.is_empty() {
        return Err(AppError::Message("请至少启用一台服务器".to_string()));
    }
    let pending: HashSet<String> = selected.iter().map(|server| server.id.clone()).collect();
    let semaphore = Arc::new(Semaphore::new(config.settings.max_concurrent_servers.max(1)));
    let filter = Arc::new(QueryFilter {
        start_time: parse_optional_datetime(request.start_time.as_deref())?,
        end_time: parse_optional_datetime(request.end_time.as_deref())?,
        keyword: request.keyword.clone(),
        level: request.level.clone(),
    });
    let batch_size = request.batch_size.max(1);

    let (tx, mut rx) = mpsc::unbounded_channel();
    for server in selected {
        let permit = semaphore.clone().acquire_owned().await.expect("semaphore closed");
        let filter = filter.clone();
        let file_path = request.file_path.clone();
        let token = token.child_token();
        let tx = tx.clone();
        let credentials = config.credentials.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let result = query_server(
                server,
                credentials,
                &file_path,
                (*filter).clone(),
                batch_size,
                token,
                tx.clone(),
            )
            .await;
            if let Err(error) = result {
                let _ = tx.send(Err(error));
            }
        });
    }
    drop(tx);

    let mut completed = Vec::new();
    let mut server_stats: HashMap<String, ServerQueryStats> = HashMap::new();
    let mut batch_index = 0;
    emit_progress(&app, &request.query_id, "running", &server_stats, &completed, &pending)?;

    while let Some(message) = rx.recv().await {
        if token.is_cancelled() {
            break;
        }
        match message? {
            ServerMessage::Batch { events } => {
                app.emit(
                    "query-result",
                    QueryResultEvent {
                        query_id: request.query_id.clone(),
                        batch_index,
                        events,
                        is_last_batch: false,
                    },
                )
                .map_err(|error| AppError::Message(error.to_string()))?;
                batch_index += 1;
            }
            ServerMessage::Progress {
                server_id,
                scanned_bytes,
                scanned_events,
                matched_events,
            } => {
                server_stats.insert(
                    server_id,
                    ServerQueryStats {
                        scanned_bytes,
                        scanned_events,
                        matched_events,
                    },
                );
                emit_progress(
                    &app,
                    &request.query_id,
                    "running",
                    &server_stats,
                    &completed,
                    &pending,
                )?;
            }
            ServerMessage::Done { server_id } => {
                if !completed.contains(&server_id) {
                    completed.push(server_id);
                }
                emit_progress(
                    &app,
                    &request.query_id,
                    "running",
                    &server_stats,
                    &completed,
                    &pending,
                )?;
            }
        }
    }

    app.emit(
        "query-result",
        QueryResultEvent {
            query_id: request.query_id.clone(),
            batch_index,
            events: Vec::new(),
            is_last_batch: true,
        },
    )
    .map_err(|error| AppError::Message(error.to_string()))?;

    app.emit(
        "query-progress",
        QueryProgressEvent {
            query_id: request.query_id,
            status: if token.is_cancelled() { "cancelled" } else { "completed" }.to_string(),
            scanned_bytes: server_stats.values().map(|stats| stats.scanned_bytes).sum(),
            scanned_events: server_stats.values().map(|stats| stats.scanned_events).sum(),
            matched_events: server_stats.values().map(|stats| stats.matched_events).sum(),
            servers_completed: completed,
            servers_pending: Vec::new(),
        },
    )
    .map_err(|error| AppError::Message(error.to_string()))?;
    Ok(())
}

async fn query_server(
    server: ServerConfig,
    credentials: AuthConfig,
    file_path: &str,
    filter: QueryFilter,
    batch_size: usize,
    token: CancellationToken,
    tx: mpsc::UnboundedSender<Result<ServerMessage, AppError>>,
) -> AppResult<()> {
    let server_id = server.id.clone();
    let url = Url::parse(&server.base_url)?.join(file_path)?;
    let password = crate::crypto::reveal_password(&credentials.password)?;
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?
        .get(url)
        .basic_auth(&credentials.username, Some(password))
        .send()
        .await?
        .error_for_status()?;

    let context = ServerContext {
        id: server.id,
        name: server.name,
        display_order: server.display_order,
    };
    let mut reader = LogStreamReader::with_current_year(context, filter, batch_size);
    let mut stream = response.bytes_stream();
    let mut scanned_bytes = 0;
    let mut scanned_events = 0;
    let mut matched_events = 0;

    while let Some(chunk) = stream.next().await {
        if token.is_cancelled() {
            break;
        }
        let chunk = chunk?;
        scanned_bytes += chunk.len();
        for batch in reader.process_chunk(&chunk) {
            matched_events += batch.len();
            let _ = tx.send(Ok(ServerMessage::Batch { events: batch }));
        }
        if let Some(batch) = reader.drain_pending_results() {
            matched_events += batch.len();
            let _ = tx.send(Ok(ServerMessage::Batch { events: batch }));
        }
        scanned_events += reader.take_scanned_events_delta();
        let _ = tx.send(Ok(ServerMessage::Progress {
            server_id: server_id.clone(),
            scanned_bytes,
            scanned_events,
            matched_events,
        }));
        if reader.is_pruned() {
            break;
        }
    }
    for batch in reader.finish() {
        matched_events += batch.len();
        let _ = tx.send(Ok(ServerMessage::Batch { events: batch }));
    }
    scanned_events += reader.take_scanned_events_delta();
    let _ = tx.send(Ok(ServerMessage::Progress {
        server_id: server_id.clone(),
        scanned_bytes,
        scanned_events,
        matched_events,
    }));
    let _ = tx.send(Ok(ServerMessage::Done { server_id }));
    Ok(())
}

fn emit_progress(
    app: &AppHandle,
    query_id: &str,
    status: &str,
    server_stats: &HashMap<String, ServerQueryStats>,
    completed: &[String],
    pending: &HashSet<String>,
) -> AppResult<()> {
    let servers_pending = pending
        .iter()
        .filter(|server_id| !completed.contains(server_id))
        .cloned()
        .collect();
    app.emit(
        "query-progress",
        QueryProgressEvent {
            query_id: query_id.to_string(),
            status: status.to_string(),
            scanned_bytes: server_stats.values().map(|stats| stats.scanned_bytes).sum(),
            scanned_events: server_stats.values().map(|stats| stats.scanned_events).sum(),
            matched_events: server_stats.values().map(|stats| stats.matched_events).sum(),
            servers_completed: completed.to_vec(),
            servers_pending,
        },
    )
    .map_err(|error| AppError::Message(error.to_string()))
}

fn parse_optional_datetime(value: Option<&str>) -> AppResult<Option<DateTime<Utc>>> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(value)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|error| AppError::Message(format!("invalid datetime '{value}': {error}")))
}
