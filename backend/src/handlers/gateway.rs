//! Auth Gateway SSO 路由层：薄封装，逻辑全部在 services::gateway_service

use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;

use crate::services::gateway_service;
use crate::state::AppState;

/// GET /auth/gateway/status
pub async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    gateway_service::status(State(state)).await
}

/// GET /auth/gateway/start
pub async fn start(State(state): State<Arc<AppState>>) -> axum::response::Response {
    gateway_service::start(State(state)).await
}

/// GET /auth/gateway/callback
pub async fn callback(
    State(state): State<Arc<AppState>>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> axum::response::Response {
    gateway_service::callback(State(state), axum::extract::RawQuery(raw)).await
}

/// POST /auth/gateway/exchange
pub async fn exchange(
    State(state): State<Arc<AppState>>,
    body: axum::Json<serde_json::Value>,
) -> axum::response::Response {
    // 与密码登录同路径：成功时除 JSON token 外还 Set-Cookie(HttpOnly)，
    // 否则 <img>/<video> 的 cookie 认证缺失，媒体文件会全部 401。
    let resp = gateway_service::exchange(State(state.clone()), body).await;
    let (parts, body) = resp.into_parts();
    let bytes = match axum::body::to_bytes(body, 64 * 1024).await {
        Ok(b) => b,
        Err(_) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    // 成功体形如 {"ok":true,"token":"..."}：直接从 JSON 里取 token 来设置 cookie。
    let token = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|v| {
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            let token = v
                .get("token")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            (ok && token.is_some()).then_some(token).flatten()
        });
    match token {
        Some(token) => {
            let cookie_str = crate::middleware::auth::set_token_cookie(
                &token,
                crate::services::auth_service::COOKIE_MAX_AGE,
                state.config.cookie_secure,
            );
            let csrf_str = crate::middleware::auth::set_csrf_cookie(
                crate::services::auth_service::COOKIE_MAX_AGE,
                state.config.cookie_secure,
            );
            let mut http_resp = (parts.status, bytes).into_response();
            if let Ok(val) = axum::http::HeaderValue::from_str(&cookie_str) {
                http_resp
                    .headers_mut()
                    .insert(axum::http::header::SET_COOKIE, val);
            }
            if let Ok(val) = axum::http::HeaderValue::from_str(&csrf_str) {
                http_resp
                    .headers_mut()
                    .append(axum::http::header::SET_COOKIE, val);
            }
            http_resp
        }
        // 失败分支原样透传（结构不同，无从取 token）
        None => (parts.status, bytes).into_response(),
    }
}
