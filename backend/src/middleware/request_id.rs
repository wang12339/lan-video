use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::Instrument;
use uuid::Uuid;

/// Header name for the request ID
pub static X_REQUEST_ID: &str = "x-request-id";

/// Maximum accepted length for a client-supplied request ID.
const MAX_CLIENT_ID_LEN: usize = 64;

/// Middleware that assigns a UUID v4 request ID to every incoming request.
///
/// - If the client sends a *well-formed* `X-Request-ID` header, that value is
///   reused (honors upstream proxies); anything malformed is ignored and a
///   fresh UUID v4 is generated instead. Arbitrary client strings are never
///   echoed verbatim: they would otherwise be injected into log lines
///   (control characters / ANSI escapes) and reflected in response headers.
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
        .and_then(valid_client_request_id)
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

/// Accept only IDs that are safe to echo into logs and response headers:
/// short ASCII alphanumeric strings with `-`/`_`. Everything else (control
/// characters, whitespace, punctuation, over-long values) is rejected so the
/// client gets a freshly generated UUID instead.
fn valid_client_request_id(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > MAX_CLIENT_ID_LEN {
        return None;
    }
    let safe = s
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    safe.then(|| s.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_client_ids_accepted() {
        assert_eq!(
            valid_client_request_id("abc-123_XYZ").as_deref(),
            Some("abc-123_XYZ")
        );
        let max_len = "a".repeat(64);
        assert_eq!(
            valid_client_request_id(&max_len).as_deref(),
            Some(max_len.as_str())
        );
    }

    #[test]
    fn malformed_client_ids_rejected() {
        assert!(valid_client_request_id("").is_none());
        assert!(valid_client_request_id("has space").is_none());
        assert!(valid_client_request_id("tab\there").is_none());
        assert!(valid_client_request_id("new\nline").is_none());
        assert!(valid_client_request_id("esc\x1b[31m").is_none());
        assert!(valid_client_request_id("slash/name").is_none());
        assert!(valid_client_request_id(&"x".repeat(65)).is_none());
        assert!(valid_client_request_id("unicode-✓").is_none());
    }

    #[test]
    fn more_client_id_boundaries() {
        // 64 chars is the boundary: 65 is rejected
        assert!(valid_client_request_id(&"a".repeat(64)).is_some());
        assert!(valid_client_request_id(&"a".repeat(65)).is_none());
        // Every allowed character class
        assert_eq!(
            valid_client_request_id("a0-9_zZ").as_deref(),
            Some("a0-9_zZ")
        );
        // Purely digits and purely dashes/underscores are fine
        assert!(valid_client_request_id("12345").is_some());
        assert!(valid_client_request_id("-----").is_some());
        assert!(valid_client_request_id("___").is_some());
        // A single trailing/leading dash is fine — the guard is only about
        // log-safety, not cosmetic concerns
        assert!(valid_client_request_id("-abc-").is_some());
        // Punctuation and brackets are rejected
        assert!(valid_client_request_id("a.b").is_none());
        assert!(valid_client_request_id("a{b}").is_none());
        assert!(valid_client_request_id("a,b").is_none());
        assert!(valid_client_request_id("a'b").is_none());
    }

    // ---- Middleware-level tests (full request_id path) ----

    use axum::{
        body::{to_bytes, Body},
        extract::Extension,
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    fn app_echoing_handler() -> Router {
        // The handler echoes the RequestId stored in extensions, proving the
        // middleware actually propagated it.
        Router::new()
            .route(
                "/",
                get(|Extension(rid): Extension<RequestId>| async move { rid.0 }),
            )
            .layer(middleware::from_fn(request_id))
    }

    fn plain_app() -> Router {
        Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_id))
    }

    async fn body_string(res: Response) -> String {
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn middleware_reuses_valid_client_id() {
        let res = plain_app()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(X_REQUEST_ID, "req-123_ABC")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.headers().get(X_REQUEST_ID).unwrap(), "req-123_ABC");
    }

    #[tokio::test]
    async fn middleware_rejects_malformed_client_ids_with_fresh_uuid() {
        let app = plain_app();
        // Only values that can actually exist in an HTTP header are reachable
        // here; control chars / non-ASCII are rejected by the HTTP parser
        // before the middleware runs and are covered by the pure unit tests.
        for bad in ["bad id", "tab\there", "/slash", "a.b", "a'b", "comma,id"] {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/")
                        .header(X_REQUEST_ID, bad)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let echoed = res.headers().get(X_REQUEST_ID).unwrap().to_str().unwrap();
            // Never the client's value, always a fresh UUID v4
            assert_ne!(echoed, bad);
            assert!(Uuid::parse_str(echoed).is_ok(), "not a UUID: {echoed}");
        }
    }

    #[tokio::test]
    async fn middleware_generates_uuid_when_header_missing() {
        let res = plain_app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let echoed = res.headers().get(X_REQUEST_ID).unwrap().to_str().unwrap();
        assert!(Uuid::parse_str(echoed).is_ok());
    }

    #[tokio::test]
    async fn middleware_overlong_client_id_replaced() {
        let evil = "x".repeat(65);
        let res = plain_app()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(X_REQUEST_ID, &evil)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let echoed = res.headers().get(X_REQUEST_ID).unwrap().to_str().unwrap();
        assert_ne!(echoed, evil);
        assert!(Uuid::parse_str(echoed).is_ok());
    }

    #[tokio::test]
    async fn middleware_stores_request_id_in_extensions() {
        // The handler reads RequestId from extensions: body must equal the
        // response header, proving propagation works for both paths.
        let res = app_echoing_handler()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(X_REQUEST_ID, "client-42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let header = res.headers().get(X_REQUEST_ID).unwrap().to_str().unwrap();
        assert_eq!(header, "client-42");
        assert_eq!(body_string(res).await, "client-42");

        // Same for the generated-UUID path
        let res = app_echoing_handler()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let header = res
            .headers()
            .get(X_REQUEST_ID)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(Uuid::parse_str(&header).is_ok());
        assert_eq!(body_string(res).await, header);
    }
}
