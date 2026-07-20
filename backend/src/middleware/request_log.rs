use axum::{extract::Request, http::HeaderMap, middleware::Next, response::Response};
use std::sync::Arc;
use std::time::Instant;

use crate::middleware::auth::AuthUser;
use crate::state::AppState;

/// Middleware that logs every request at INFO level with method, path, status, and duration.
/// Slow requests (> 1s) are logged at WARN level.
pub async fn request_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let headers = req.headers().clone();

    // Try to get AppState and user from request extensions
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let auth_user = req.extensions().get::<AuthUser>().cloned();

    let start = Instant::now();
    let res = next.run(req).await;

    let duration = start.elapsed();
    let duration_ms = duration.as_millis();
    let status = res.status().as_u16();

    // Record metrics if state is available
    if let Some(ref state) = state {
        state.metrics.record_request(duration);

        // Update active connections (simplified - would need proper tracking)
        // state.metrics.set_active_connections(active_connections as f64);
    }

    // Extract request ID from response headers if present
    let request_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");

    // Skip logging for static resources and frontend routes
    if path.starts_with("/webapp/") || path.starts_with("/media/") || path == "/health" {
        return res;
    }

    // Skip frontend routes (these are handled by the SPA, not backend)
    let is_api_path = path.starts_with("/auth/")
        || path.starts_with("/videos")
        || path.starts_with("/playback/")
        || path.starts_with("/admin/")
        || path.starts_with("/server/")
        || path == "/health";

    if !is_api_path && status == 404 {
        return res;
    }

    // Get username: first try AuthUser, then lookup by token
    let user_str = if let Some(ref user) = auth_user {
        user.username.clone()
    } else if let Some(ref state) = state {
        lookup_username_from_token(&headers, state).await
    } else {
        "匿名".to_string()
    };

    if duration_ms > 1000 {
        tracing::warn!(
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            user = %user_str,
            request_id = %request_id,
            "请求响应缓慢"
        );
    } else if status >= 500 {
        tracing::error!(
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            user = %user_str,
            request_id = %request_id,
            "服务器内部错误"
        );
    } else if status == 401 || status == 403 {
        tracing::warn!(
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            user = %user_str,
            request_id = %request_id,
            "访问被拒绝"
        );
    } else {
        tracing::info!(
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            user = %user_str,
            request_id = %request_id,
            "请求成功"
        );
    }

    res
}

/// Lookup username from token in database
async fn lookup_username_from_token(headers: &HeaderMap, state: &AppState) -> String {
    let token = extract_token(headers);
    match token {
        Some(t) => match state.user_repo.find_user_by_token(&t).await {
            Ok(Some(user)) => user.username,
            _ => "未知用户".to_string(),
        },
        None => "匿名".to_string(),
    }
}

/// Extract token from Authorization header or cookie
fn extract_token(headers: &HeaderMap) -> Option<String> {
    // Try Authorization header first
    if let Some(auth) = headers.get("Authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Some(token.to_string());
        }
    }

    // Try cookie
    if let Some(cookie) = headers.get("Cookie").and_then(|v| v.to_str().ok()) {
        for pair in cookie.split(';') {
            let mut parts = pair.splitn(2, '=');
            if let Some(key) = parts.next() {
                if key.trim() == "token" {
                    if let Some(token) = parts.next() {
                        return Some(token.trim().to_string());
                    }
                }
            }
        }
    }

    None
}
