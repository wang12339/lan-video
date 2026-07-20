use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::state::AppState;

/// SECURITY (H-06): rate-limit the public share-token endpoint to neutralise
/// the size-based side channel that would otherwise let an attacker enumerate
/// valid tokens. 30 req/min per IP is generous for legitimate use (one user
/// opening the shared link, plus range-request retries).
const SHARE_RL_MAX: u32 = 30;
const SHARE_RL_WINDOW_SECS: u64 = 60;
const SHARE_RL_BLOCK_SECS: u64 = 0;

pub async fn share_rate_limit(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "server config error").into_response();
    };

    let ip = client_ip(&req);
    let key = format!("share:{}", ip);
    if state
        .ip_rate_limiter
        .check_with(
            &key,
            SHARE_RL_MAX,
            SHARE_RL_WINDOW_SECS,
            SHARE_RL_BLOCK_SECS,
        )
        .await
        .is_err()
    {
        tracing::warn!(ip = %ip, "share endpoint rate-limited");
        return (StatusCode::TOO_MANY_REQUESTS, "请求过于频繁，请稍后再试").into_response();
    }

    next.run(req).await
}

fn client_ip(req: &Request) -> String {
    if let Some(cf) = req
        .headers()
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        return cf.to_string();
    }
    if let Some(addr) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return addr.0.ip().to_string();
    }
    "unknown".into()
}

#[allow(dead_code)]
fn _force_header_use(_h: &HeaderMap) {}
