//! Auth Gateway SSO 路由层：薄封装，逻辑全部在 services::gateway_service

use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;

use crate::services::gateway_service;
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/auth/gateway/status",
    tag = "gateway",
    summary = "Auth Gateway SSO availability",
    description = "Returns whether Auth Gateway SSO login is enabled (all GATEWAY_* env vars configured).",
    responses((status = 200, description = "Gateway SSO availability", body = serde_json::Value))
)]
/// GET /auth/gateway/status
pub async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    gateway_service::status(State(state)).await
}

#[utoipa::path(
    get,
    path = "/auth/gateway/start",
    tag = "gateway",
    summary = "Start Auth Gateway SSO login",
    description = "Creates a state+PKCE pair and 303-redirects the browser to the Auth Gateway authorization page. Returns 404 when SSO is not configured.",
    responses(
        (status = 303, description = "Redirect to the gateway authorization page"),
        (status = 404, description = "SSO not configured")
    )
)]
/// GET /auth/gateway/start
///
/// 公开接口，按 IP 限流并在 PENDING 容量满时拒绝（见 gateway_service::start）。
pub async fn start(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> axum::response::Response {
    if !gateway_service::enabled(&state) {
        return crate::util::response::error_response(
            axum::http::StatusCode::NOT_FOUND,
            "Not Found",
        )
        .into_response();
    }
    let ip = crate::util::net::client_ip(&req);
    let key = format!("gw:start:{ip}");
    if state
        .ip_rate_limiter
        .check_with(&key, 30, 60, 300)
        .await
        .is_err()
    {
        return crate::util::response::error_response(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "请求过于频繁，请稍后重试",
        )
        .into_response();
    }
    gateway_service::start(State(state)).await
}

#[utoipa::path(
    get,
    path = "/auth/gateway/callback",
    tag = "gateway",
    summary = "Auth Gateway OAuth callback",
    description = "Receives the authorization code from the gateway, exchanges it for tokens, fetches userinfo, links/provisions the local account, mints an Atmos token, and 303-redirects to the webapp with a one-time exchange code (?gw_code=...). On failure redirects with ?gw_error=....",
    params(
        ("code" = Option<String>, Query, description = "Authorization code"),
        ("state" = Option<String>, Query, description = "OAuth state"),
        ("error" = Option<String>, Query, description = "Gateway error code")
    ),
    responses((status = 303, description = "Redirect to the webapp landing page"))
)]
/// GET /auth/gateway/callback
pub async fn callback(
    State(state): State<Arc<AppState>>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> axum::response::Response {
    gateway_service::callback(State(state), axum::extract::RawQuery(raw)).await
}

#[utoipa::path(
    post,
    path = "/auth/gateway/exchange",
    tag = "gateway",
    summary = "Exchange one-time gw_code for the Atmos token",
    description = "Consumes the one-time exchange code produced by the callback (30s TTL) and returns the Atmos auth token. The code is burned on first use.",
    responses(
        (status = 200, description = "Atmos auth token", body = serde_json::Value),
        (status = 400, description = "Invalid or expired code", body = serde_json::Value)
    )
)]
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
