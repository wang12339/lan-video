use std::net::SocketAddr;
use std::time::Duration;

use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderName, HeaderValue, Method},
    middleware::Next,
    response::IntoResponse,
};
use tower_http::cors::CorsLayer;

use crate::util::cloudflare_ips::is_cloudflare_peer;

/// Security headers middleware — adds standard security headers to every response
pub async fn security_headers(req: Request, next: Next) -> impl IntoResponse {
    let is_https = is_https_request(&req);
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
    // RFC 6797 §7.2: HSTS MUST NOT be sent over plain HTTP. `is_https_request`
    // only honours the X-Forwarded-Proto hint from a trusted edge proxy, so a
    // plaintext client forging the header still gets no HSTS.
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

    // Content Security Policy：HTTPS 下额外启用 upgrade-insecure-requests /
    // block-all-mixed-content；纯 HTTP 只发基础策略，避免子资源被强制
    // 升级到不存在的 HTTPS 导致 ERR_SSL_PROTOCOL_ERROR。
    let csp = if is_https {
        CSP_POLICY
    } else {
        CSP_POLICY_HTTP
    };
    headers.insert(
        axum::http::header::HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(csp),
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

    // Cross-Origin-Embedder-Policy: enable cross-origin isolation for SharedArrayBuffer
    headers.insert(
        axum::http::header::HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("require-corp"),
    );

    res
}

#[inline]
fn is_https_request(req: &Request) -> bool {
    if !is_trusted_peer(req) {
        return false;
    }
    req.headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

#[inline]
fn is_trusted_peer(req: &Request) -> bool {
    if crate::util::net::trusted_proxy_is_enabled() {
        return true;
    }
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| is_cloudflare_peer(ci.0.ip()))
        .unwrap_or(false)
}

static CSP_POLICY: &str = "default-src 'self'; \
    base-uri 'self'; \
    form-action 'self'; \
    frame-ancestors 'none'; \
    frame-src 'none'; \
    object-src 'none'; \
    img-src 'self' data:; \
    media-src 'self' blob:; \
    style-src 'self'; \
    font-src 'self' data:; \
    script-src 'self' https://static.cloudflareinsights.com; \
    connect-src 'self' https://static.cloudflareinsights.com; \
    upgrade-insecure-requests; \
    block-all-mixed-content";

/// 与 `CSP_POLICY` 相同，但去掉 HTTPS 专属指令（`upgrade-insecure-requests` /
/// `block-all-mixed-content`）。`upgrade-insecure-requests` 在纯 HTTP 页面下
/// 会把所有子资源强制升级为 HTTPS，而服务器只提供 HTTP，导致
/// `ERR_SSL_PROTOCOL_ERROR`（局域网 IP 直连时尤为明显）。HTTP 场景只发送基础策略。
static CSP_POLICY_HTTP: &str = "default-src 'self'; \
    base-uri 'self'; \
    form-action 'self'; \
    frame-ancestors 'none'; \
    frame-src 'none'; \
    object-src 'none'; \
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
                .map_err(|e| {
                    tracing::warn!("invalid CORS_ORIGIN value '{}': {}; using default", s, e)
                })
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
            HeaderName::from_static("x-csrf-token"),
            HeaderName::from_static("x-upload-hash"),
            HeaderName::from_static("x-upload-name"),
            HeaderName::from_static("x-upload-size"),
            HeaderName::from_static("x-upload-category"),
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
        assert!(CSP_POLICY.contains("object-src 'none'"));
        assert!(CSP_POLICY.contains("frame-src 'none'"));
    }

    #[test]
    fn test_csp_policy_no_unsafe_directives() {
        // unsafe-inline and unsafe-eval weaken XSS protections
        assert!(
            !CSP_POLICY.contains("'unsafe-inline'"),
            "CSP must not allow unsafe-inline"
        );
        assert!(
            !CSP_POLICY.contains("'unsafe-eval'"),
            "CSP must not allow unsafe-eval"
        );
    }

    #[test]
    fn test_csp_policy_has_mixed_content_protections() {
        assert!(
            CSP_POLICY.contains("upgrade-insecure-requests"),
            "CSP should upgrade insecure requests"
        );
        assert!(
            CSP_POLICY.contains("block-all-mixed-content"),
            "CSP should block mixed content (legacy browsers)"
        );
    }

    #[test]
    fn test_csp_policy_restricts_critical_directives() {
        assert!(CSP_POLICY.contains("default-src 'self'"));
        assert!(CSP_POLICY.contains("base-uri 'self'"));
        assert!(CSP_POLICY.contains("form-action 'self'"));
        assert!(CSP_POLICY.contains("frame-ancestors 'none'"));
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

    #[test]
    fn is_https_request_detects_proxy_hints() {
        // A Cloudflare-range peer is trusted to set X-Forwarded-Proto.
        let cf_peer: SocketAddr = "104.16.42.1:443".parse().unwrap();
        let req_with = |v: Option<&str>| {
            let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
            req.extensions_mut().insert(ConnectInfo(cf_peer));
            if let Some(v) = v {
                req.headers_mut()
                    .insert("x-forwarded-proto", v.parse().unwrap());
            }
            req
        };
        assert!(!is_https_request(&req_with(None)));
        assert!(is_https_request(&req_with(Some("https"))));
        assert!(is_https_request(&req_with(Some("HTTPS"))));
        assert!(!is_https_request(&req_with(Some("http"))));
        assert!(!is_https_request(&req_with(Some("ftp"))));
        // Non-UTF8 values cannot claim HTTPS
        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(ConnectInfo(cf_peer));
        req.headers_mut().insert(
            "x-forwarded-proto",
            axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        assert!(!is_https_request(&req));
    }

    #[test]
    fn is_https_request_ignores_hint_from_untrusted_peer() {
        // A non-Cloudflare peer connecting straight to the origin cannot
        // forge the proto hint — even `https` is treated as plain HTTP.
        let peer: SocketAddr = "203.0.113.7:443".parse().unwrap();
        let req_with = |v: Option<&str>| {
            let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
            req.extensions_mut().insert(ConnectInfo(peer));
            if let Some(v) = v {
                req.headers_mut()
                    .insert("x-forwarded-proto", v.parse().unwrap());
            }
            req
        };
        assert!(!is_https_request(&req_with(Some("https"))));
        assert!(!is_https_request(&req_with(None)));
    }

    #[test]
    fn is_https_request_falls_back_to_http_without_peer_info() {
        // No ConnectInfo extension (e.g. tests, unusual service setups):
        // treat as untrusted, never issue HSTS on a possibly-plaintext path.
        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.headers_mut()
            .insert("x-forwarded-proto", "https".parse().unwrap());
        assert!(!is_https_request(&req));
    }

    // ---- Middleware-level tests (full security_headers path) ----

    use axum::{
        body::{to_bytes, Body},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    fn header_app() -> Router {
        Router::new()
            .route("/", get(|| async { "hi" }))
            .layer(middleware::from_fn(security_headers))
    }

    /// Run a request through the middleware with a Cloudflare-range peer by
    /// default (a trusted proxy that may set X-Forwarded-Proto). Pass
    /// `untrusted_peer` to simulate a client connecting straight to the
    /// origin, in which case the proto hint must be ignored.
    async fn run_with_proto(
        proto: Option<&str>,
        untrusted_peer: bool,
    ) -> axum::http::Response<axum::body::Body> {
        let peer: SocketAddr = if untrusted_peer {
            "203.0.113.7:443".parse().unwrap()
        } else {
            "104.16.42.1:443".parse().unwrap()
        };
        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(ConnectInfo(peer));
        if let Some(proto) = proto {
            req.headers_mut()
                .insert("x-forwarded-proto", proto.parse().unwrap());
        }
        header_app().oneshot(req).await.unwrap()
    }

    #[tokio::test]
    async fn all_security_headers_set_on_https_requests() {
        let res = run_with_proto(Some("https"), false).await;
        assert_eq!(
            res.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(res.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(res.headers().get("referrer-policy").unwrap(), "no-referrer");
        assert_eq!(
            res.headers().get("strict-transport-security").unwrap(),
            "max-age=31536000; includeSubDomains; preload"
        );
        assert_eq!(
            res.headers().get("permissions-policy").unwrap(),
            "geolocation=(), microphone=(), camera=()"
        );
        assert_eq!(
            res.headers().get("content-security-policy").unwrap(),
            CSP_POLICY
        );
        assert_eq!(
            res.headers().get("cross-origin-opener-policy").unwrap(),
            "same-origin"
        );
        assert_eq!(
            res.headers().get("cross-origin-resource-policy").unwrap(),
            "same-origin"
        );
        assert_eq!(
            res.headers().get("cross-origin-embedder-policy").unwrap(),
            "require-corp"
        );
        // Response body is untouched by the middleware
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"hi");
    }

    #[tokio::test]
    async fn all_security_headers_set_on_http_requests() {
        // Even without HTTPS, most security headers should still be present
        // (only HSTS is omitted for plain HTTP per RFC 6797).
        let res = run_with_proto(None, false).await;
        assert_eq!(
            res.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(res.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(res.headers().get("referrer-policy").unwrap(), "no-referrer");
        assert_eq!(
            res.headers().get("permissions-policy").unwrap(),
            "geolocation=(), microphone=(), camera=()"
        );
        // HTTP 下必须使用基础策略：upgrade-insecure-requests 会把子资源
        // 强制升级为 HTTPS，纯 HTTP 部署会 ERR_SSL_PROTOCOL_ERROR。
        assert_eq!(
            res.headers().get("content-security-policy").unwrap(),
            CSP_POLICY_HTTP
        );
        assert!(
            !res.headers()
                .get("content-security-policy")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("upgrade-insecure-requests"),
            "HTTP 下不得发送 upgrade-insecure-requests"
        );
        assert_eq!(
            res.headers().get("cross-origin-opener-policy").unwrap(),
            "same-origin"
        );
        assert_eq!(
            res.headers().get("cross-origin-resource-policy").unwrap(),
            "same-origin"
        );
        assert_eq!(
            res.headers().get("cross-origin-embedder-policy").unwrap(),
            "require-corp"
        );
        // HSTS must NOT be present on plain HTTP
        assert!(res.headers().get("strict-transport-security").is_none());
    }

    #[tokio::test]
    async fn hsts_not_sent_over_plain_http() {
        // No proxy hint at all
        let res = run_with_proto(None, false).await;
        assert!(res.headers().get("strict-transport-security").is_none());
        assert_eq!(res.headers().get("x-frame-options").unwrap(), "DENY");
        // Explicit plain-http hint
        let res = run_with_proto(Some("http"), false).await;
        assert!(res.headers().get("strict-transport-security").is_none());
        // Uppercase hint is still honoured
        let res = run_with_proto(Some("HTTPS"), false).await;
        assert!(res.headers().get("strict-transport-security").is_some());
    }

    #[tokio::test]
    async fn hsts_not_sent_on_non_utf8_proto_hint() {
        let peer: SocketAddr = "104.16.42.1:443".parse().unwrap();
        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(ConnectInfo(peer));
        req.headers_mut().insert(
            "x-forwarded-proto",
            axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        let res = header_app().oneshot(req).await.unwrap();
        assert!(res.headers().get("strict-transport-security").is_none());
    }

    #[tokio::test]
    async fn hsts_never_sent_on_forged_hint_from_untrusted_peer() {
        // L-01: a client connecting straight to the origin must not be able
        // to forge `X-Forwarded-Proto: https` to receive HSTS over plaintext.
        let res = run_with_proto(Some("https"), true).await;
        assert!(res.headers().get("strict-transport-security").is_none());
        assert_eq!(
            res.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
    }

    // ---- CORS layer behavioural tests ----

    use axum::http::{Method, StatusCode};

    fn preflight(origin: &str) -> Request {
        Request::builder()
            .method(Method::OPTIONS)
            .uri("/")
            .header("origin", origin)
            .header("access-control-request-method", "POST")
            .header(
                "access-control-request-headers",
                "content-type,authorization",
            )
            .body(Body::empty())
            .unwrap()
    }

    /// CorsLayer is a Layer, not a Service — wrap it in a Router to exercise
    /// the full CORS pipeline (the same composition used in production).
    fn cors_app(cors: CorsLayer) -> Router {
        Router::new()
            .route("/{*any}", get(|| async { "ok" }))
            .layer(cors)
    }

    #[tokio::test]
    async fn cors_preflight_allows_configured_origin() {
        let app = cors_app(create_cors_layer("https://a.example, https://b.example"));
        let res = app
            .clone()
            .oneshot(preflight("https://a.example"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("access-control-allow-origin").unwrap(),
            "https://a.example"
        );
        assert_eq!(
            res.headers()
                .get("access-control-allow-credentials")
                .unwrap(),
            "true"
        );
        assert_eq!(res.headers().get("access-control-max-age").unwrap(), "3600");
        let methods = res
            .headers()
            .get("access-control-allow-methods")
            .unwrap()
            .to_str()
            .unwrap();
        for m in ["GET", "POST", "PUT", "DELETE", "OPTIONS"] {
            assert!(methods.contains(m), "missing method {m} in {methods}");
        }
        // The other configured origin works too
        let res = app.oneshot(preflight("https://b.example")).await.unwrap();
        assert_eq!(
            res.headers().get("access-control-allow-origin").unwrap(),
            "https://b.example"
        );
    }

    #[tokio::test]
    async fn cors_preflight_denies_unconfigured_origin() {
        let app = cors_app(create_cors_layer("https://a.example"));
        let res = app
            .oneshot(preflight("https://evil.example"))
            .await
            .unwrap();
        assert!(res.headers().get("access-control-allow-origin").is_none());
    }

    #[tokio::test]
    async fn cors_without_origins_sends_no_allow_origin() {
        let app = cors_app(create_cors_layer(""));
        let res = app.oneshot(preflight("https://a.example")).await.unwrap();
        assert!(res.headers().get("access-control-allow-origin").is_none());
        // No credentials header either — allow_credentials is only enabled
        // together with an explicit allow-list.
        assert!(res
            .headers()
            .get("access-control-allow-credentials")
            .is_none());
    }

    #[tokio::test]
    async fn cors_trims_whitespace_and_filters_invalid_entries() {
        // Whitespace around entries is trimmed
        let app = cors_app(create_cors_layer(" https://a.example , "));
        let res = app.oneshot(preflight("https://a.example")).await.unwrap();
        assert_eq!(
            res.headers().get("access-control-allow-origin").unwrap(),
            "https://a.example"
        );
        // A value containing a control character fails header parsing and is
        // dropped, leaving the valid entry functional
        let app = cors_app(create_cors_layer("https://a.example,\u{1}"));
        let res = app.oneshot(preflight("https://a.example")).await.unwrap();
        assert_eq!(
            res.headers().get("access-control-allow-origin").unwrap(),
            "https://a.example"
        );
    }

    #[tokio::test]
    async fn cors_simple_request_gets_origin_header() {
        let app = cors_app(create_cors_layer("https://a.example"));
        let req = Request::builder()
            .uri("/")
            .header("origin", "https://a.example")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(
            res.headers().get("access-control-allow-origin").unwrap(),
            "https://a.example"
        );
        assert_eq!(
            res.headers()
                .get("access-control-allow-credentials")
                .unwrap(),
            "true"
        );
    }
}
