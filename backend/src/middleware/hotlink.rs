use std::sync::Arc;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

pub async fn hotlink_guard(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let allowed = state
        .as_ref()
        .map(|s| s.config.public_url.clone())
        .unwrap_or_default();

    if let Some(referer_host) = header_host(req.headers(), "referer") {
        if !host_is_allowed(&referer_host, &allowed) {
            tracing::warn!(referer = %referer_host, "hotlink blocked via Referer");
            return (StatusCode::FORBIDDEN, "hotlinking not allowed").into_response();
        }
    }
    if let Some(origin_host) = header_host(req.headers(), "origin") {
        if !host_is_allowed(&origin_host, &allowed) {
            tracing::warn!(origin = %origin_host, "hotlink blocked via Origin");
            return (StatusCode::FORBIDDEN, "hotlinking not allowed").into_response();
        }
    }
    next.run(req).await
}

fn header_host(headers: &HeaderMap, name: &str) -> Option<String> {
    let v = headers.get(name)?.to_str().ok()?;
    let after = v.split_once("://")?.1;
    let host = after
        .split('/')
        .next()?
        .split(':')
        .next()?
        .to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn host_is_allowed(host: &str, allowed: &str) -> bool {
    if allowed.is_empty() {
        tracing::warn!(
            "PUBLIC_URL not configured — hotlink protection disabled. \
             Set PUBLIC_URL env var to enable hotlink protection."
        );
        return true;
    }
    if let Some(cfg_host) = extract_host_from_url(allowed) {
        return host == cfg_host.to_ascii_lowercase();
    }
    false
}

fn extract_host_from_url(url: &str) -> Option<String> {
    let after = url.split_once("://")?.1;
    let host = after
        .split('/')
        .next()?
        .split(':')
        .next()?
        .to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}
