use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::OnceLock;
use tokio::sync::Semaphore;

const PERMIT_CHUNK_BYTES: usize = 8 * 1024 * 1024;
const PERMITS: usize = 32;

static IN_FLIGHT: OnceLock<Semaphore> = OnceLock::new();

fn semaphore() -> &'static Semaphore {
    IN_FLIGHT.get_or_init(|| Semaphore::new(PERMITS))
}

pub async fn bandwidth_throttle(req: Request, next: Next) -> Response {
    let approx = req
        .headers()
        .get("range")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_range_length)
        .unwrap_or(PERMIT_CHUNK_BYTES * PERMITS);
    let permits = approx.div_ceil(PERMIT_CHUNK_BYTES).clamp(1, 32);
    let permit = match semaphore().acquire_many(permits as u32).await {
        Ok(p) => p,
        Err(_) => {
            return (axum::http::StatusCode::SERVICE_UNAVAILABLE, "server busy").into_response()
        }
    };
    let resp = next.run(req).await;
    drop(permit);
    let mut resp = resp;
    resp.headers_mut()
        .entry("x-bandwidth-throttled")
        .or_insert(HeaderValue::from_static("1"));
    resp
}

fn parse_range_length(s: &str) -> Option<usize> {
    let bytes = s.strip_prefix("bytes=")?;
    let first = bytes.split(',').next()?;
    let (start_str, end_str) = first.trim().split_once('-')?;
    let start: i64 = start_str.parse().ok()?;
    let end: i64 = if end_str.is_empty() {
        (PERMIT_CHUNK_BYTES * PERMITS) as i64
    } else {
        end_str.parse().ok()?
    };
    Some((end - start).max(0) as usize)
}
