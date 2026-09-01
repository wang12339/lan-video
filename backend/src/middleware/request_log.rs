use axum::{extract::Request, middleware::Next, response::Response};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

use crate::middleware::request_id::RequestId;
use crate::state::AppState;

pub async fn request_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let is_range = req.headers().contains_key("range");

    let ext_request_id = req.extensions().get::<RequestId>().cloned();

    let state = req.extensions().get::<Arc<AppState>>().cloned();

    if let Some(ref state) = state {
        state.metrics.active_connections.inc();
    }

    let path_ref = req.uri().path().to_string();
    // 仅健康检查跳过;静态资源(/webapp /media)与 API 均记录,便于访问审计。
    let always_skip = path_ref == "/health";

    let client_ip = crate::util::net::client_ip(&req);

    let start = Instant::now();
    let res = next.run(req).await;
    let elapsed = start.elapsed();

    if let Some(ref state) = state {
        state.metrics.record_request(elapsed);
        state.metrics.active_connections.dec();
    }

    if always_skip {
        return res;
    }

    let status = res.status().as_u16();

    let is_api_path = path_ref.starts_with("/auth/")
        || path_ref.starts_with("/videos")
        || path_ref.starts_with("/playback/")
        || path_ref.starts_with("/admin/")
        || path_ref.starts_with("/server/");
    if !is_api_path && status == 404 {
        return res;
    }

    let duration_ms = elapsed.as_millis();

    let request_id = ext_request_id
        .as_ref()
        .map(|r| r.0.as_str())
        .or_else(|| {
            res.headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
        })
        .unwrap_or("-");

    let log_path = redact_log_path(&path_ref);

    let range_hint = if is_range { " range" } else { "" };

    // 分级策略（降低日志噪音、保留可审计性）：
    //   - 慢请求(>1s)/5xx/401/403 → warn/error（不受 method 限制）
    //   - 写操作(POST/PUT/DELETE/PATCH)→ info（可审计）
    //   - 静态资源(/webapp /media)GET → info（访问审计可见 IP）
    //   - 其余读操作(GET/HEAD)→ debug（RUST_LOG=debug 时可见）
    let is_read = matches!(method.as_str(), "GET" | "HEAD");
    let is_static = path_ref.starts_with("/webapp/") || path_ref.starts_with("/media/");

    if duration_ms > 1000 {
        tracing::warn!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            client_ip = %client_ip,
            "请求响应缓慢"
        );
    } else if status >= 500 {
        tracing::error!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            client_ip = %client_ip,
            "服务器内部错误"
        );
    } else if status == 401 || status == 403 {
        tracing::warn!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            client_ip = %client_ip,
            "访问被拒绝"
        );
    } else if is_read && !is_static {
        tracing::debug!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            client_ip = %client_ip,
            "读请求完成"
        );
    } else {
        tracing::info!(
            method = %method,
            path = %log_path,
            status = status,
            duration_ms = duration_ms,
            request_id = %request_id,
            client_ip = %client_ip,
            "{}完成{}",
            if is_read { "静态资源请求" } else { "写请求" },
            range_hint,
        );
    }

    res
}

fn redact_log_path(path: &str) -> Cow<'_, str> {
    let needs_cleaning = path.bytes().any(|b| b < 0x20 || b == 0x7f);
    let cleaned = if needs_cleaning {
        Cow::Owned(
            path.chars()
                .map(|c| if c.is_control() { '?' } else { c })
                .collect(),
        )
    } else {
        Cow::Borrowed(path)
    };
    match cleaned.strip_prefix("/share/") {
        Some(rest) if !rest.is_empty() && !rest.contains('/') => {
            Cow::Owned("/share/{token}".to_string())
        }
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
