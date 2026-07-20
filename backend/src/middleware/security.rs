use std::time::Duration;

use axum::{
    extract::Request,
    http::{HeaderValue, Method},
    middleware::Next,
    response::IntoResponse,
};
use tower_http::cors::CorsLayer;

/// Security headers middleware — adds standard security headers to every response
pub async fn security_headers(req: Request, next: Next) -> impl IntoResponse {
    let is_https = is_https_request(req.headers());
    let mut res = next.run(req).await;
    let headers = res.headers_mut();

    // Prevent MIME type sniffing
    headers.insert(
        axum::http::header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    // Prevent clickjacking
    headers.insert(
        axum::http::header::HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );

    // Control referrer information — no-referrer prevents share token leakage
    headers.insert(
        axum::http::header::HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );

    // Force HTTPS (1 year, with preload).
    // RFC 6797 §7.2: HSTS MUST NOT be sent over plain HTTP — gate on the
    // X-Forwarded-Proto hint from the edge proxy.
    if is_https {
        headers.insert(
            axum::http::header::HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
        );
    }

    // Restrict browser features
    headers.insert(
        axum::http::header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );

    // Content Security Policy
    headers.insert(
        axum::http::header::HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(CSP_POLICY),
    );

    // Cross-Origin-Opener-Policy: isolate browsing context
    headers.insert(
        axum::http::header::HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );

    // Cross-Origin-Resource-Policy: only same-origin can embed resources
    headers.insert(
        axum::http::header::HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );

    res
}

/// Return true if the request was made over HTTPS (or is fronted by a proxy
/// that says it was).
fn is_https_request(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

/// CSP policy string — restrict resources to same-origin where possible.
/// Includes `frame-ancestors 'none'` to redundantly block framing (defence in
/// depth alongside X-Frame-Options: DENY).
const CSP_POLICY: &str = "default-src 'self'; \
    base-uri 'self'; \
    form-action 'self'; \
    frame-ancestors 'none'; \
    img-src 'self' data:; \
    media-src 'self' blob:; \
    style-src 'self'; \
    font-src 'self' data:; \
    script-src 'self' https://static.cloudflareinsights.com; \
    connect-src 'self' https://static.cloudflareinsights.com";

/// Create CORS layer with configured origins (comma-separated).
///
/// SECURITY:
/// - `allow_credentials(true)` is only meaningful together with an explicit
///   allow-listed origin, so we only enable it when at least one origin is
///   configured. This prevents the browser-rejected but still confusing
///   `Allow-Credentials: true` on responses with no `Allow-Origin`.
/// - `Access-Control-Max-Age` caps preflight caching to 1 hour.
pub fn create_cors_layer(origins: &str) -> CorsLayer {
    let origins: Vec<HeaderValue> = origins
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            s.parse::<HeaderValue>()
                .map_err(|e| tracing::warn!("invalid CORS_ORIGIN value '{}': {}; using default", s, e))
                .ok()
        })
        .collect();

    let mut layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            "x-csrf-token".parse().unwrap(),
            "x-upload-hash".parse().unwrap(),
            "x-upload-name".parse().unwrap(),
            "x-upload-size".parse().unwrap(),
            "x-upload-category".parse().unwrap(),
        ])
        .max_age(Duration::from_secs(3600));

    if !origins.is_empty() {
        layer = layer.allow_origin(origins).allow_credentials(true);
    }

    layer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csp_policy_contains_self() {
        assert!(CSP_POLICY.contains("'self'"));
        assert!(CSP_POLICY.contains("frame-ancestors"));
    }

    #[test]
    fn test_cors_layer_with_origin() {
        let cors = create_cors_layer("https://example.com");
        drop(cors);
    }

    #[test]
    fn test_cors_layer_empty_origins() {
        let cors = create_cors_layer("");
        drop(cors);
    }
}
