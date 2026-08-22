use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use axum::{
    extract::ConnectInfo,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;

const MAX_BYTES_PER_SEC_PER_IP: u64 = 500 * 1024 * 1024;
const WINDOW_SECS: u64 = 1;
const CLEANUP_INTERVAL: usize = 10_000;

/// Charge for a request with no Range header: a full-file GET, so assume at
/// least this much will be transferred. The previous flat 256 KiB charge let
/// a single connection stream an arbitrarily large file for one token.
const NO_RANGE_CHARGE_BYTES: u64 = 512 * 1024;
/// Charge for an open-ended range (`bytes=N-`): the response size is
/// unbounded and unknowable up front, so charge the upper clamp.
const OPEN_ENDED_RANGE_CHARGE_BYTES: u64 = 8 * 1024 * 1024;
/// Floor for a finite range charge — prevents floods of tiny 1-byte range
/// requests from evading the cap entirely.
const MIN_RANGE_CHARGE_BYTES: u64 = 256 * 1024;
/// Ceiling for a single finite range charge.
const MAX_RANGE_CHARGE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone)]
struct Bucket {
    bytes: u64,
    reset_at: Instant,
}

pub struct IpBandwidth {
    buckets: DashMap<String, Bucket>,
    ops_since_cleanup: AtomicUsize,
}

impl Default for IpBandwidth {
    fn default() -> Self {
        Self::new()
    }
}

impl IpBandwidth {
    pub fn new() -> Self {
        Self {
            buckets: DashMap::new(),
            ops_since_cleanup: AtomicUsize::new(0),
        }
    }

    pub fn check(&self, ip: &str, bytes: u64) -> bool {
        let ops = self.ops_since_cleanup.fetch_add(1, Ordering::Relaxed);
        if ops >= CLEANUP_INTERVAL {
            self.ops_since_cleanup.store(0, Ordering::Relaxed);
            self.cleanup();
        }

        let now = Instant::now();
        let mut bucket = self.buckets.entry(ip.to_string()).or_insert(Bucket {
            bytes: 0,
            reset_at: now + std::time::Duration::from_secs(WINDOW_SECS),
        });
        if now >= bucket.reset_at {
            bucket.bytes = 0;
            bucket.reset_at = now + std::time::Duration::from_secs(WINDOW_SECS);
        }
        let charged = bucket.bytes.saturating_add(bytes);
        if charged > MAX_BYTES_PER_SEC_PER_IP {
            return false;
        }
        bucket.bytes = charged;
        true
    }

    fn cleanup(&self) {
        let now = Instant::now();
        self.buckets.retain(|_, b| now < b.reset_at);
    }
}

static IP_BANDWIDTH: std::sync::OnceLock<IpBandwidth> = std::sync::OnceLock::new();

fn bandwidth() -> &'static IpBandwidth {
    IP_BANDWIDTH.get_or_init(IpBandwidth::new)
}

pub async fn bandwidth_throttle(req: Request, next: Next) -> Response {
    let approx_bytes = req
        .headers()
        .get("range")
        .and_then(|v| v.to_str().ok())
        .and_then(approx_range_bytes)
        .unwrap_or(NO_RANGE_CHARGE_BYTES);

    let client_ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_default();

    if !client_ip.is_empty() && !bandwidth().check(&client_ip, approx_bytes) {
        return (StatusCode::TOO_MANY_REQUESTS, "bandwidth limit exceeded").into_response();
    }

    let mut resp = next.run(req).await;

    resp.headers_mut()
        .entry("x-bandwidth-throttled")
        .or_insert(axum::http::HeaderValue::from_static("0"));

    resp
}

/// Estimate the bytes a Range request will transfer, so each request is
/// charged a fair amount against the per-IP bandwidth budget.
///
/// - `bytes=a-b` (finite): the requested span, clamped into
///   [`MIN_RANGE_CHARGE_BYTES`..`MAX_RANGE_CHARGE_BYTES`].
/// - `bytes=N-` (open-ended): the response is unbounded, so charge
///   [`OPEN_ENDED_RANGE_CHARGE_BYTES`] rather than a token-sized amount.
/// - Anything unparseable (`bytes=-500` suffixes, garbage): `None`, and the
///   caller falls back to [`NO_RANGE_CHARGE_BYTES`].
fn approx_range_bytes(s: &str) -> Option<u64> {
    let bytes = s.strip_prefix("bytes=")?;
    let first = bytes.split(',').next()?;
    let (start_str, end_str) = first.trim().split_once('-')?;
    let start: u64 = start_str.parse().ok()?;
    if end_str.trim().is_empty() {
        // Open-ended range: bounded by the (unknown) file size.
        return Some(OPEN_ENDED_RANGE_CHARGE_BYTES);
    }
    let end: u64 = end_str.parse().ok()?;
    let size = end.saturating_sub(start);
    Some(size.clamp(MIN_RANGE_CHARGE_BYTES, MAX_RANGE_CHARGE_BYTES))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_ranges_are_clamped() {
        assert_eq!(
            approx_range_bytes("bytes=0-0"),
            Some(MIN_RANGE_CHARGE_BYTES)
        );
        assert_eq!(approx_range_bytes("bytes=0-1048575"), Some(1024 * 1024 - 1));
        assert_eq!(
            approx_range_bytes("bytes=0-999999999"),
            Some(MAX_RANGE_CHARGE_BYTES)
        );
        // Inverted range still charges at least the floor
        assert_eq!(
            approx_range_bytes("bytes=100-50"),
            Some(MIN_RANGE_CHARGE_BYTES)
        );
    }

    #[test]
    fn open_ended_ranges_charge_the_conservative_amount() {
        assert_eq!(
            approx_range_bytes("bytes=0-"),
            Some(OPEN_ENDED_RANGE_CHARGE_BYTES)
        );
        assert_eq!(
            approx_range_bytes("bytes=1048576-"),
            Some(OPEN_ENDED_RANGE_CHARGE_BYTES)
        );
    }

    #[test]
    fn malformed_ranges_return_none() {
        assert!(approx_range_bytes("garbage").is_none());
        assert!(approx_range_bytes("bytes=").is_none());
        assert!(approx_range_bytes("bytes=-500").is_none());
        assert!(approx_range_bytes("bytes=abc-").is_none());
        assert!(approx_range_bytes("bytes=1-2-3").is_none());
    }

    #[test]
    fn range_edge_cases() {
        // Exactly 1 MiB sits between the floor and ceiling — no clamping
        assert_eq!(approx_range_bytes("bytes=0-1048576"), Some(1024 * 1024));
        // Only the first range of a multi-range request is considered
        assert_eq!(
            approx_range_bytes("bytes=0-99,200-299"),
            Some(MIN_RANGE_CHARGE_BYTES)
        );
        // Whitespace around the spec is tolerated
        assert_eq!(
            approx_range_bytes("bytes= 0-99 "),
            Some(MIN_RANGE_CHARGE_BYTES)
        );
        // Overflowing the u64 parse is treated as unparseable
        assert!(approx_range_bytes("bytes=18446744073709551616-0").is_none());
        // Huge-but-representable values still clamp (saturating_sub avoids
        // underflow; an inverted huge range costs the floor)
        assert_eq!(
            approx_range_bytes("bytes=18446744073709551614-18446744073709551615"),
            Some(MIN_RANGE_CHARGE_BYTES)
        );
        // The prefix is case-sensitive
        assert!(approx_range_bytes("Bytes=0-100").is_none());
        // Space before '=' breaks the prefix match
        assert!(approx_range_bytes("bytes =0-100").is_none());
        // Empty first range member
        assert!(approx_range_bytes("bytes=,0-100").is_none());
    }

    // ---- Middleware-level tests (full bandwidth_throttle path) ----
    //
    // NOTE: the middleware uses a process-global bucket (`bandwidth()`), so
    // every test below uses a distinct TEST-NET-3 IP to keep buckets isolated.

    use axum::{body::Body, extract::ConnectInfo, middleware, routing::get, Router};
    use tower::ServiceExt;

    fn bw_app() -> Router {
        Router::new()
            .route("/{*any}", get(|| async { (StatusCode::OK, "ok") }))
            .layer(middleware::from_fn(bandwidth_throttle))
    }

    fn bw_req(uri: &str, ip: Option<&str>, range: Option<&str>) -> Request {
        let mut builder = Request::builder().uri(uri);
        if let Some(range) = range {
            builder = builder.header("range", range);
        }
        let mut req = builder.body(Body::empty()).unwrap();
        if let Some(ip) = ip {
            let addr: SocketAddr = format!("{ip}:1234").parse().unwrap();
            req.extensions_mut().insert(ConnectInfo(addr));
        }
        req
    }

    #[tokio::test]
    async fn request_within_budget_passes_and_marks_response() {
        let app = bw_app();
        let res = app
            .oneshot(bw_req(
                "/media/v.mp4",
                Some("198.51.100.2"),
                Some("bytes=0-1023"),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers().get("x-bandwidth-throttled").unwrap(), "0");
    }

    #[tokio::test]
    async fn request_without_peer_ip_skips_check() {
        // No ConnectInfo extension → client_ip is empty → the check is
        // skipped, but the response header is still set.
        let app = bw_app();
        let res = app
            .oneshot(bw_req("/media/v.mp4", None, Some("bytes=0-999999999")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.headers().contains_key("x-bandwidth-throttled"));
    }

    #[tokio::test]
    async fn budget_is_charged_per_finite_range() {
        let app = bw_app();
        // Each request charges 4 MiB (the ceiling); 200 MiB budget lasts for
        // 50 requests, the 51st is rejected.
        for i in 0..50 {
            let res = app
                .clone()
                .oneshot(bw_req(
                    &format!("/media/{i}"),
                    Some("198.51.100.3"),
                    Some("bytes=0-999999999"),
                ))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "request {i}");
        }
        let res = app
            .oneshot(bw_req(
                "/media/51",
                Some("198.51.100.3"),
                Some("bytes=0-999999999"),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        // The 429 path never reaches the response-header insertion
        assert!(!res.headers().contains_key("x-bandwidth-throttled"));
    }

    #[tokio::test]
    async fn budget_is_charged_when_range_header_missing() {
        let app = bw_app();
        // No Range header → NO_RANGE_CHARGE_BYTES (2 MiB) per request.
        // 200 MiB / 2 MiB = 100 requests fit; the 101st is rejected.
        for i in 0..100 {
            let res = app
                .clone()
                .oneshot(bw_req(&format!("/media/{i}"), Some("198.51.100.4"), None))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "request {i}");
        }
        let res = app
            .oneshot(bw_req("/media/101", Some("198.51.100.4"), None))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
