use axum::{extract::Query, extract::State, http::StatusCode, Extension, Json};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

/// Log entry parsed from JSON log file
#[derive(serde::Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

#[derive(Deserialize)]
pub struct LogQuery {
    pub level: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// GET /admin/logs — 读取日志文件
pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    Extension(_auth_user): Extension<AuthUser>,
    Query(params): Query<LogQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let log_dir = state.config.log_dir.clone();
    let limit = params.limit.unwrap_or(200).min(1000);
    let offset = params.offset.unwrap_or(0);
    let level_filter = params.level.clone().unwrap_or_default();
    let search_filter = params.search.clone().unwrap_or_default();

    // Use spawn_blocking for synchronous file I/O
    let result = tokio::task::spawn_blocking(move || -> Result<(String, Vec<LogEntry>), String> {
        if !log_dir.exists() {
            return Ok((String::new(), Vec::new()));
        }

        // Find the most recent log file (files containing .log in name)
        let mut log_files: Vec<_> = std::fs::read_dir(&log_dir)
            .map_err(|_| "无法读取日志目录".to_string())?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                name.contains(".log") && !name.starts_with('.')
            })
            .collect();

        log_files.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

        let log_file = match log_files.first() {
            Some(f) => f.path(),
            None => return Ok((String::new(), Vec::new())),
        };

        let file_name = log_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Read from the end of the file to avoid loading large files entirely into memory.
        // We read backwards in chunks, collecting up to (limit + offset) matching lines.
        let file = std::fs::File::open(&log_file).map_err(|_| "无法打开日志文件".to_string())?;
        let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);

        let mut reader = BufReader::new(file);
        let seek_pos = if file_size > 1_000_000 {
            SeekFrom::End(-1_000_000)
        } else {
            SeekFrom::Start(0)
        };
        reader
            .seek(seek_pos)
            .map_err(|_| "无法读取日志文件".to_string())?;

        // Read all lines from the seek point, then process in reverse
        let mut all_lines: Vec<String> = Vec::new();
        for line_result in reader.lines() {
            let line = line_result.map_err(|_| "读取日志文件失败".to_string())?;
            all_lines.push(line);
        }

        // Process lines in reverse (most recent first), apply filters, collect up to limit+offset
        let mut entries: Vec<LogEntry> = Vec::new();
        for line in all_lines.iter().rev() {
            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as JSON
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                let level = parsed
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("INFO")
                    .to_uppercase();

                // Level filter
                if !level_filter.is_empty() && level != level_filter.to_uppercase() {
                    continue;
                }

                // tracing-subscriber JSON format: fields are nested in "fields" object
                let fields = parsed.get("fields").and_then(|v| v.as_object());

                // Extract message from fields.message or top-level message
                let message = fields
                    .and_then(|f| f.get("message"))
                    .or_else(|| parsed.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Filter out static resource requests (JS, CSS, images, etc.)
                let path = fields
                    .and_then(|f| f.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if path.starts_with("/webapp/") || path.starts_with("/media/") || path == "/health"
                {
                    continue;
                }

                // Search filter
                if !search_filter.is_empty()
                    && !message
                        .to_lowercase()
                        .contains(&search_filter.to_lowercase())
                {
                    let method = fields
                        .and_then(|f| f.get("method"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let path = fields
                        .and_then(|f| f.get("path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let user = fields
                        .and_then(|f| f.get("user"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !method
                        .to_lowercase()
                        .contains(&search_filter.to_lowercase())
                        && !path.to_lowercase().contains(&search_filter.to_lowercase())
                        && !user.to_lowercase().contains(&search_filter.to_lowercase())
                    {
                        continue;
                    }
                }

                let timestamp = parsed
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .or_else(|| parsed.get("time").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();

                let request_id = parsed
                    .get("span")
                    .and_then(|s| s.get("request_id"))
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        fields
                            .and_then(|f| f.get("request_id"))
                            .and_then(|v| v.as_str())
                    })
                    .map(String::from);

                let entry = LogEntry {
                    timestamp,
                    level,
                    message,
                    method: fields
                        .and_then(|f| f.get("method"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    path: fields
                        .and_then(|f| f.get("path"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    status: fields
                        .and_then(|f| f.get("status"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u16),
                    duration_ms: fields
                        .and_then(|f| f.get("duration_ms"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u128),
                    request_id,
                    user: fields
                        .and_then(|f| f.get("user"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    video_id: fields
                        .and_then(|f| f.get("video_id"))
                        .and_then(|v| v.as_i64()),
                    error: fields
                        .and_then(|f| f.get("error"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    action: fields
                        .and_then(|f| f.get("action"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    target: fields
                        .and_then(|f| f.get("target"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    page: fields
                        .and_then(|f| f.get("page"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                };

                entries.push(entry);

                if entries.len() >= limit + offset {
                    break;
                }
            }
        }

        Ok((file_name, entries))
    })
    .await;

    match result {
        Ok(Ok((file_name, all_entries))) => {
            let total = all_entries.len();
            let entries: Vec<_> = all_entries.into_iter().skip(offset).take(limit).collect();

            Ok(Json(serde_json::json!({
                "entries": entries,
                "total": total,
                "file": file_name,
            })))
        }
        Ok(Err(e)) => {
            tracing::error!("list_logs failed: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "日志读取失败",
            ))
        }
        Err(_e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "日志读取任务失败",
        )),
    }
}

/// DELETE /admin/logs — 清空当前日志文件
pub async fn clear_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let log_dir = state.config.log_dir.clone();
    let actor = auth_user.username.clone();

    // Find the current log file (same logic as get_logs)
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        if !log_dir.exists() {
            return Ok(());
        }
        let mut log_files: Vec<_> = std::fs::read_dir(&log_dir)
            .map_err(|_| "无法读取日志目录".to_string())?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                name.contains(".log") && !name.starts_with('.')
            })
            .collect();

        log_files.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

        if let Some(f) = log_files.first() {
            let path = f.path();
            // Truncate the file
            std::fs::write(&path, "").map_err(|_| "无法清空日志文件".to_string())?;
        }

        Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => {
            tracing::info!(actor = %actor, "admin cleared logs");
            Ok(Json(serde_json::json!({"ok": true})))
        }
        Ok(Err(e)) => {
            tracing::error!("clear_logs failed: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "日志清空失败",
            ))
        }
        Err(_) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "日志清空任务失败",
        )),
    }
}
