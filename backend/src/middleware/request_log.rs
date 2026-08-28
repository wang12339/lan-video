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
    let always_skip = path_ref.starts_with("/media/")
        || path_ref.starts_with("/webapp/")
        || path_ref == "/health";

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
