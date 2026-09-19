use axum::{
    extract::{FromRequest, Request, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::state::AppState;
use crate::util::net::client_ip;
use crate::util::response::{ErrorResponse, SafeJson};

/// 前端 ErrorBoundary 上报的客户端错误。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientErrorReport {
    pub message: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(default)]
    pub component_stack: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub ua: Option<String>,
    #[serde(default)]
    pub ts: Option<String>,
}

/// 清洗并截断用户可控字段：剔除控制字符（防日志行伪造/终端控制序列注入）。
fn sanitize(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

/// POST /client-errors — 接收前端崩溃上报（无需登录，按 IP 限速）。
///
/// 用于 `VITE_ERROR_REPORT_URL`（默认 `/client-errors`）：ErrorBoundary 捕获
/// 渲染错误后通过 `navigator.sendBeacon` 上报，写入服务端 WARN 日志，
/// 管理端"日志"页可直接查看。超限静默返回 204，避免客户端重试风暴。
#[utoipa::path(
    post,
    path = "/client-errors",
    tag = "user",
    description = "接收前端 ErrorBoundary 上报的客户端错误并写入服务端日志；按 IP 限速，超限静默丢弃",
    request_body = ClientErrorReport,
    responses(
        (status = 204, description = "Report recorded"),
        (status = 400, description = "Bad request")
    )
)]
pub async fn report_client_error(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let ip = client_ip(&req);
    // 每 IP 30 次/60s，超限封 300s；超限静默 204
    if state
        .ip_rate_limiter
        .check_with(&format!("client_error:{ip}"), 30, 60, 300)
        .await
        .is_err()
    {
        return Ok(StatusCode::NO_CONTENT);
    }

    let SafeJson(report) = SafeJson::<ClientErrorReport>::from_request(req, &state).await?;

    let message = sanitize(&report.message, 500);
    if message.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }

    tracing::warn!(
        category = %sanitize(report.category.as_deref().unwrap_or("unknown"), 50),
        url = %sanitize(report.url.as_deref().unwrap_or(""), 300),
        ua = %sanitize(report.ua.as_deref().unwrap_or(""), 200),
        stack = %sanitize(report.stack.as_deref().unwrap_or(""), 2000),
        component_stack = %sanitize(report.component_stack.as_deref().unwrap_or(""), 1000),
        reported_at = %sanitize(report.ts.as_deref().unwrap_or(""), 40),
        client_ip = %ip,
        "客户端错误: {}",
        message
    );
    Ok(StatusCode::NO_CONTENT)
}
