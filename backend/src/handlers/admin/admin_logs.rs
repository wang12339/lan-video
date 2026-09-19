use axum::{extract::Query, extract::State, http::StatusCode, Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::admin::LogQuery;
use crate::services::log_parser;
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

#[utoipa::path(
    get,
    path = "/admin/logs",
    tag = "admin",
    summary = "Read recent log entries",
    description = "从最新日志文件尾部读取日志条目，支持 level/search 过滤与分页",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("level" = Option<String>, Query, description = "按日志级别过滤（INFO/WARN/ERROR…）"),
        ("search" = Option<String>, Query, description = "按消息/路径/用户等关键字过滤"),
        ("limit" = Option<usize>, Query, description = "返回条数（默认 200，最大 1000）"),
        ("offset" = Option<usize>, Query, description = "跳过条数（从最新往旧）")
    ),
    responses(
        (status = 200, description = "Log entries", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LogQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let limit = params.limit.unwrap_or(200).min(1000);
    let offset = params.offset.unwrap_or(0);
    let level = params.level;
    let search = params.search;

    let log_dir = state.config.log_dir.clone();
    let result = tokio::task::spawn_blocking(move || {
        if !log_dir.exists() {
            return (String::new(), Vec::new());
        }
        let files = log_parser::discover_log_files(&log_dir);
        let Some(log_file) = files.first() else {
            return (String::new(), Vec::new());
        };
        let file_name = log_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let entries = log_parser::parse_log_file(log_file, limit + offset);
        let entries = log_parser::filter_entries(entries, level.as_deref(), search.as_deref());
        (file_name, entries)
    })
    .await;

    match result {
        Ok((file_name, all_entries)) => {
            let total = all_entries.len();
            let entries: Vec<_> = all_entries.into_iter().skip(offset).take(limit).collect();
            Ok(Json(serde_json::json!({
                "entries": entries,
                "total": total,
                "file": file_name,
            })))
        }
        Err(_e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "日志读取任务失败",
        )),
    }
}

/// DELETE /admin/logs — 清空当前日志文件
#[utoipa::path(
    delete,
    path = "/admin/logs",
    tag = "admin",
    description = "清空当前日志文件内容",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    responses(
        (status = 200, description = "Logs cleared", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn clear_logs(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let log_dir = state.config.log_dir.clone();
    let actor = auth_user.username.clone();

    // Find the current log file (reuse shared discovery logic)
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        if !log_dir.exists() {
            return Ok(());
        }
        if let Some(path) = log_parser::discover_log_files(&log_dir).first() {
            std::fs::write(path, "").map_err(|_| "无法清空日志文件".to_string())?;
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
