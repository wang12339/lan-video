use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::state::AppState;
use crate::util::net::client_ip;

const SHARE_RL_MAX: u32 = 30;
const SHARE_RL_WINDOW_SECS: u64 = 60;
const SHARE_RL_BLOCK_SECS: u64 = 0;

pub async fn share_rate_limit(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "server config error").into_response();
    };

    // Only the share endpoint is rate-limited here. This middleware is
    // attached to the whole public router, and throttling the other public
    // routes would (among other things) trip load-balancer health probes
    // after 30 requests per minute.
    if !req.uri().path().starts_with("/share/") {
        return next.run(req).await;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::SocketAddr;

    use axum::{body::Body, extract::ConnectInfo, middleware, routing::get, Router};
    use tower::ServiceExt;

    use crate::test_support::test_state;

    /// AppState wired to a dead DB port (1) — only `ip_rate_limiter` is
    /// exercised here, no connection is ever established.
    fn state() -> Arc<AppState> {
        test_state("https://video.example.com")
    }

    fn share_app() -> Router {
        Router::new()
            .route("/{*any}", get(|| async { (StatusCode::OK, "ok") }))
            .layer(middleware::from_fn(share_rate_limit))
    }

    fn get_req(uri: &str, ip: &str, state: &Arc<AppState>) -> Request {
        let mut req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let addr: SocketAddr = format!("{ip}:1234").parse().unwrap();
        req.extensions_mut().insert(ConnectInfo(addr));
        req.extensions_mut().insert(state.clone());
        req
    }

    #[tokio::test]
    async fn share_endpoint_is_rate_limited_after_threshold() {
        let state = state();
        let app = share_app();
        // SHARE_RL_MAX - 1 requests succeed
        for i in 0..SHARE_RL_MAX - 1 {
            let res = app
                .clone()
                .oneshot(get_req(&format!("/share/{i}"), "203.0.113.10", &state))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "request {i}");
        }
        // The request that reaches the limit is itself rejected
        let res = app
            .clone()
            .oneshot(get_req("/share/limit", "203.0.113.10", &state))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        // block_secs = 0: the very next request starts fresh
        let res = app
            .oneshot(get_req("/share/again", "203.0.113.10", &state))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn share_limits_are_per_ip() {
        let state = state();
        let app = share_app();
        // SHARE_RL_MAX - 1 requests succeed (the next one hits the limit)
        for _ in 0..SHARE_RL_MAX - 1 {
            let res = app
                .clone()
                .oneshot(get_req("/share/x", "203.0.113.20", &state))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
        }
        assert_eq!(
            app.clone()
                .oneshot(get_req("/share/x", "203.0.113.20", &state))
                .await
                .unwrap()
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        // A different client IP has its own budget
        assert_eq!(
            app.oneshot(get_req("/share/x", "203.0.113.21", &state))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn non_share_paths_are_not_rate_limited() {
        let state = state();
        let app = share_app();
        // Well past the threshold on a non-share path: everything passes.
        for i in 0..SHARE_RL_MAX + 10 {
            let res = app
                .clone()
                .oneshot(get_req(&format!("/videos/{i}"), "203.0.113.30", &state))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "request {i}");
        }
        // The exact string "/share" (no trailing slash) is also outside the
        // middleware's scope — only paths starting with "/share/" are gated.
        assert_eq!(
            app.oneshot(get_req("/share", "203.0.113.30", &state))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn missing_state_returns_500() {
        let app = Router::new()
            .route("/{*any}", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(share_rate_limit));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/share/x")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
