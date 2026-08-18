use axum::{extract::Request, middleware::Next, response::Response};
use std::sync::Arc;
use std::time::Instant;

use crate::middleware::request_id::RequestId;
use crate::state::AppState;

/// Middleware that logs every request at INFO level with method, path, status, and duration.
/// Slow requests (> 1s) are logged at WARN level.
/// Media range requests and static assets are skipped entirely.
pub async fn request_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let is_range = req.headers().contains_key("range");

    // Resolve the request ID from the `request_id` middleware (an outer
    // layer, so it has already run by the time we see the request). Reading
    // it from the *response* headers would always fail here: `request_id`
    // attaches the header only after `next.run()` — i.e. after us — returns.
    let ext_request_id = req.extensions().get::<RequestId>().map(|r| r.0.clone());

    let state = req.extensions().get::<Arc<AppState>>().cloned();

    // Track active connections
    if let Some(ref state) = state {
        state.metrics.active_connections.inc();
    }

    let start = Instant::now();
    let res = next.run(req).await;

    let duration = start.elapsed();
    let duration_ms = duration.as_millis();
    let status = res.status().as_u16();

    // Record metrics if state is available (zero-cost)
    if let Some(ref state) = state {
        state.metrics.record_request(duration);
        state.metrics.active_connections.dec();
    }

    // Skip logging for media range requests (the vast majority of traffic at scale),
    // static assets, and health checks
    if path.starts_with("/media/") || path.starts_with("/webapp/") || path == "/health" {
        return res;
    }

    // Skip frontend routes that return 404 (SPA fallback noise)
    let is_api_path = path.starts_with("/auth/")
        || path.starts_with("/videos")
        || path.starts_with("/playback/")
        || path.starts_with("/admin/")
        || path.starts_with("/server/");
    if !is_api_path && status == 404 {
        return res;
    }

    // Fall back to the response header if the extension is missing
    // (e.g. middleware ordering changed).
    let request_id = ext_request_id
        .as_deref()
        .or_else(|| {
            res.headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
        })
        .unwrap_or("-");

    // The path may embed a share token (`/share/{token}`) which grants
    // access to a private video — redact it from logs. Control characters
    // are also stripped so a crafted path cannot forge log lines.
    let log_path = redact_log_path(&path);

    let range_hint = if is_range { " range" } else { "" };

    if duration_ms > 1000 {
        tracing::warn!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            "请求响应缓慢"
        );
    } else if status >= 500 {
        tracing::error!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            "服务器内部错误"
        );
    } else if status == 401 || status == 403 {
        tracing::warn!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            "访问被拒绝"
        );
    } else {
        tracing::info!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            "请求成功{}",
            range_hint,
        );
    }

    res
}

/// Strip control characters and redact share tokens (`/share/{token}` →
/// `/share/{token}` literal) before a path is written to the logs.
fn redact_log_path(path: &str) -> String {
    let cleaned: String = path
        .chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect();
    match cleaned.strip_prefix("/share/") {
        // Exactly one path segment after /share/ is the share token.
        Some(rest) if !rest.is_empty() && !rest.contains('/') => "/share/{token}".to_string(),
        _ => cleaned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_token_redacted() {
        assert_eq!(redact_log_path("/share/AbCdEf1234567890"), "/share/{token}");
        assert_eq!(redact_log_path("/share/"), "/share/");
        // Other routes are untouched
        assert_eq!(redact_log_path("/videos/42"), "/videos/42");
        assert_eq!(
            redact_log_path("/share/videos/extra"),
            "/share/videos/extra"
        );
    }

    #[test]
    fn control_characters_stripped() {
        assert_eq!(redact_log_path("/auth/login\x1b[31m"), "/auth/login?[31m");
        assert_eq!(redact_log_path("/videos/42\nx"), "/videos/42?x");
    }
}
