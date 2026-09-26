#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `bandwidth_throttle` must bucket by the *client* address, not by the TCP peer.
//!
//! Regression coverage for a bug where the middleware read `ConnectInfo` directly
//! instead of going through `util::net::client_ip`. Behind a reverse proxy every
//! request shares one peer address, so the "per-IP" 500 MB/s budget silently
//! collapsed into a single global bucket: one heavy viewer could throttle every
//! other user, and `TRUSTED_PROXY` had no effect on this middleware.
//!
//! This lives in its own integration-test binary on purpose: the trusted-proxy
//! configuration is process-global (`OnceLock`) and can only be set once, so it
//! needs a process where nothing else has already claimed it.
//!
//! The bucket map (`IP_BANDWIDTH`) is also process-global, so every test here
//! uses its own TEST-NET-3 addresses to stay isolated.

use std::net::SocketAddr;

use atmos_video_backend::middleware::upload_bandwidth::bandwidth_throttle;
use atmos_video_backend::util::net::{configure_trusted_proxy, configure_trusted_proxy_peers};
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

/// Per-IP budget is 500 MB/s and a clamped `Range` request is charged the 4 MiB
/// ceiling, so 125 requests exhaust a bucket and the 126th is rejected.
const REQUESTS_TO_EXHAUST: usize = 125;

/// Largest per-request charge, so the budget runs out after
/// `REQUESTS_TO_EXHAUST` requests.
const BIG_RANGE: &str = "bytes=0-999999999";

/// Two proxy addresses, both allowlisted as trusted so `client_ip` honours the
/// forwarded headers for either.
const PROXY_A: &str = "127.0.0.1";
const PROXY_B: &str = "127.0.0.2";

fn app() -> Router {
    Router::new()
        .route("/{*any}", get(|| async { (StatusCode::OK, "ok") }))
        .layer(middleware::from_fn(bandwidth_throttle))
}

fn req(peer: &str, xff: Option<&str>) -> Request {
    let mut builder = Request::builder()
        .uri("/media/v.mp4")
        .header("range", BIG_RANGE);
    if let Some(xff) = xff {
        builder = builder.header("x-forwarded-for", xff);
    }
    let mut req = builder.body(Body::empty()).unwrap();
    let addr: SocketAddr = format!("{peer}:1234").parse().unwrap();
    req.extensions_mut().insert(ConnectInfo(addr));
    req
}

/// Enable the trusted-proxy machinery once for this process.
fn enable_trusted_proxy() {
    configure_trusted_proxy(true);
    configure_trusted_proxy_peers(Some(vec![
        PROXY_A.parse().unwrap(),
        PROXY_B.parse().unwrap(),
    ]));
}

/// Send `n` requests, returning how many were rejected.
async fn drain(app: &Router, peer: &str, xff: Option<&str>, n: usize) -> usize {
    let mut rejected = 0;
    for _ in 0..n {
        let res = app
            .clone()
            .oneshot(req(peer, xff))
            .await
            .expect("send throttled request");
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            rejected += 1;
        }
    }
    rejected
}

/// The core regression: two different clients tunnelled through the *same* proxy
/// peer must get independent budgets. The old implementation keyed on the peer
/// address, so client B inherited client A's exhausted budget and was rejected.
#[tokio::test]
async fn distinct_clients_behind_one_proxy_get_separate_budgets() {
    enable_trusted_proxy();
    let app = app();

    let client_a = "198.51.100.10";
    let client_b = "198.51.100.11";

    // Client A burns its whole budget through proxy A.
    let rejected_a = drain(&app, PROXY_A, Some(client_a), REQUESTS_TO_EXHAUST + 1).await;
    assert!(
        rejected_a > 0,
        "client A should have exhausted its own budget"
    );

    // Client B is a different person and must not be affected, even though it
    // reaches the backend through the very same proxy socket.
    let res = app
        .clone()
        .oneshot(req(PROXY_A, Some(client_b)))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "client B must not inherit client A's exhausted budget \
         (all requests shared one peer-keyed bucket)"
    );
}

/// The mirror image: the same client routed through two different proxy
/// addresses is still one client and must share a single budget. This is what
/// makes the limit per-user rather than per-connection.
#[tokio::test]
async fn one_client_across_two_proxies_shares_one_budget() {
    enable_trusted_proxy();
    let app = app();

    let client = "198.51.100.20";

    // Alternate proxies while exhausting the client's budget.
    let mut rejected = 0;
    for i in 0..=REQUESTS_TO_EXHAUST {
        let peer = if i % 2 == 0 { PROXY_A } else { PROXY_B };
        let res = app.clone().oneshot(req(peer, Some(client))).await.unwrap();
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            rejected += 1;
        }
    }
    assert!(
        rejected > 0,
        "one client must share a single budget across proxy addresses"
    );
}

/// `cf-connecting-ip` takes precedence over `x-forwarded-for` in `client_ip`,
/// and the throttle must follow the same resolution order as every other
/// middleware.
#[tokio::test]
async fn cf_connecting_ip_drives_the_bucket() {
    enable_trusted_proxy();
    let app = app();

    let cf_client = "198.51.100.30";
    let xff_client = "198.51.100.31";

    // Exhaust the CF-declared client's budget while the XFF header names
    // somebody else.
    let mut rejected = 0;
    for _ in 0..=REQUESTS_TO_EXHAUST {
        let mut r = req(PROXY_A, Some(xff_client));
        r.headers_mut()
            .insert("cf-connecting-ip", cf_client.parse().unwrap());
        let res = app.clone().oneshot(r).await.unwrap();
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            rejected += 1;
        }
    }
    assert!(rejected > 0, "CF client budget should be exhausted");

    // A request that only carries X-IP must be judged on its own address.
    let res = app
        .clone()
        .oneshot(req(PROXY_A, Some(xff_client)))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "the XFF-declared client must be budgeted separately from the CF client"
    );
}

/// Requests from a peer that is *not* allowlisted must ignore the forwarded
/// headers, otherwise anyone could spoof `x-forwarded-for` to escape the limit.
#[tokio::test]
async fn untrusted_peer_cannot_spoof_its_way_out_of_the_limit() {
    enable_trusted_proxy();
    let app = app();

    let attacker_peer = "127.0.0.9"; // not in TRUSTED_PROXY_PEERS
    let victim = "198.51.100.40";

    // Exhaust the bucket keyed on the real peer address.
    let rejected = drain(&app, attacker_peer, None, REQUESTS_TO_EXHAUST + 1).await;
    assert!(rejected > 0, "attacker peer budget should be exhausted");

    // Rotating the forged header must not mint a fresh budget: the header is
    // ignored, so the peer-keyed bucket is still full.
    let res = app
        .clone()
        .oneshot(req(attacker_peer, Some(victim)))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "an untrusted peer's x-forwarded-for must be ignored, not honoured"
    );
}
