use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::Instrument;
use uuid::Uuid;

/// Header name for the request ID
pub static X_REQUEST_ID: &str = "x-request-id";

/// Middleware that assigns a UUID v4 request ID to every incoming request.
///
/// - If the client sends an `X-Request-ID` header, that value is reused (honors
///   upstream proxies); otherwise a new UUID v4 is generated.
/// - The ID is stored in request extensions so downstream handlers can read it.
/// - The `X-Request-ID` response header is always set.
/// - A tracing span with `request_id` is created so all log lines for a request
///   share the same ID.
pub async fn request_id(mut req: Request, next: Next) -> Response {
    // Reuse client-supplied ID or generate a new one
    let id = req
        .headers()
        .get(X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Store in request extensions for handler access
    req.extensions_mut().insert(RequestId(id.clone()));

    // Create a tracing span so all downstream logs share this ID
    let span = tracing::info_span!("request", request_id = %id);

    // Use Instrument to properly propagate the span to the downstream future
    let mut res = next.run(req).instrument(span).await;

    // Set response header
    if let Ok(val) = HeaderValue::from_str(&id) {
        res.headers_mut().insert(X_REQUEST_ID, val);
    }

    res
}

/// Extractor for reading the request ID from handler context.
///
/// # Example
/// ```ignore
/// async fn handler(Extension(rid): Extension<RequestId>) -> String {
///     format!("request id: {}", rid.0)
/// }
/// ```
#[derive(Clone, Debug)]
pub struct RequestId(pub String);
