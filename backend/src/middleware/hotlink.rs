use std::sync::Arc;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

const MAX_HEADER_URI_LEN: usize = 1024;

pub async fn hotlink_guard(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return next.run(req).await;
    };

    let allowed_host = extract_host_from_url(&state.config.public_url);
    // Origin is compared as a full RFC 6454 origin (scheme://host[:port]),
    // not just the host: `https://host:8443` is a *different* origin than
    // `https://host`. Referer keeps its host-only comparison.
    let allowed_origin = origin_value(&state.config.public_url);
    let (Some(allowed_host), Some(allowed_origin)) = (allowed_host, allowed_origin) else {
        tracing::error!("hotlink_guard: invalid PUBLIC_URL configuration");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "server configuration error",
        )
            .into_response();
    };

    match referer_host(req.headers(), "referer") {
        RefererCheck::Host(host) if host != allowed_host => {
            tracing::warn!(referer = %host, "hotlink blocked via Referer");
            return (StatusCode::FORBIDDEN, "hotlinking not allowed").into_response();
        }
        // A present-but-unparseable Referer is treated as a mismatch instead
        // of being ignored: lenient parsing would let crafted values such as
        // `Referer: evil.example/` (no scheme) or `https:/evil.example`
        // slip past the check entirely.
        RefererCheck::Malformed => {
            tracing::warn!("hotlink blocked: malformed Referer");
            return (StatusCode::FORBIDDEN, "hotlinking not allowed").into_response();
        }
        RefererCheck::Absent | RefererCheck::Host(_) => {}
    }
    match origin_header(req.headers()) {
        // Same host on a different port is a *different* origin (RFC 6454) —
        // a page served on :8443 must not hotlink media meant for the
        // configured public origin.
        OriginCheck::Origin(origin) if origin != allowed_origin => {
            tracing::warn!(origin = %origin, "hotlink blocked via Origin");
            return (StatusCode::FORBIDDEN, "hotlinking not allowed").into_response();
        }
        OriginCheck::Malformed => {
            tracing::warn!("hotlink blocked: malformed Origin");
            return (StatusCode::FORBIDDEN, "hotlinking not allowed").into_response();
        }
        OriginCheck::Absent | OriginCheck::Origin(_) => {}
    }
    next.run(req).await
}

enum RefererCheck {
    /// Header not present — a direct download (curl, apps, video players),
    /// not a browser-initiated hotlink. Allowed.
    Absent,
    /// Header present but not a parseable absolute URI — block.
    Malformed,
    /// Extracted, lowercased authority host.
    Host(String),
}

#[inline]
fn referer_host(headers: &HeaderMap, name: &str) -> RefererCheck {
    let Some(value) = headers.get(name) else {
        return RefererCheck::Absent;
    };
    let Ok(value) = value.to_str() else {
        // Non-UTF8 header values are crafted input, not a browser.
        return RefererCheck::Malformed;
    };
    match authority_host(value) {
        Some(host) => RefererCheck::Host(host),
        None => RefererCheck::Malformed,
    }
}

#[inline]
fn authority_host(uri: &str) -> Option<String> {
    parse_authority(uri).map(|a| a.1)
}

/// Parse an absolute `http(s)` URI into `(scheme, host, port)`, lowercasing
/// the scheme and host and stripping any userinfo. The port is captured
/// verbatim (possibly empty or non-numeric) — callers decide whether that is
/// acceptable: [`authority_host`] ignores it, [`origin_value`] rejects it.
fn parse_authority(uri: &str) -> Option<(String, String, Option<String>)> {
    if uri.len() > MAX_HEADER_URI_LEN {
        return None;
    }
    let (scheme, rest) = uri.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.bytes().any(|b| b < 0x20) {
        return None;
    }
    let host_port = match authority.rsplit_once('@') {
        Some((_, host_port)) => host_port,
        None => authority,
    };
    if host_port.contains('\\') {
        return None;
    }
    let (host, port) = if let Some(inner) = host_port.strip_prefix('[') {
        match inner.split_once(']') {
            Some((addr, "")) => (addr, None),
            Some((addr, rest)) => (addr, rest.strip_prefix(':')),
            None => return None,
        }
    } else {
        match host_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_port, None),
        }
    };
    if host.is_empty() {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    let port = port.map(|p| p.to_string());
    Some((scheme, host, port))
}

#[inline]
fn origin_value(uri: &str) -> Option<String> {
    let (scheme, host, port) = parse_authority(uri)?;
    let port = match port {
        Some(p) if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) => return None,
        Some(p) => p,
        None => return Some(format!("{scheme}://{host}")),
    };
    let default = match scheme.as_str() {
        "http" => "80",
        "https" => "443",
        _ => unreachable!("parse_authority only accepts http(s)"),
    };
    if port == default {
        Some(format!("{scheme}://{host}"))
    } else {
        Some(format!("{scheme}://{host}:{port}"))
    }
}

enum OriginCheck {
    /// Header not present — a direct download (curl, apps, video players),
    /// not a browser-initiated hotlink. Allowed.
    Absent,
    /// Header present but not a valid RFC 6454 origin — block.
    Malformed,
    /// The full serialized origin (`scheme://host[:port]`, lowercased).
    Origin(String),
}

fn origin_header(headers: &HeaderMap) -> OriginCheck {
    let Some(value) = headers.get("origin") else {
        return OriginCheck::Absent;
    };
    let Ok(value) = value.to_str() else {
        // Non-UTF8 header values are crafted input, not a browser.
        return OriginCheck::Malformed;
    };
    match origin_value(value) {
        Some(origin) => OriginCheck::Origin(origin),
        None => OriginCheck::Malformed,
    }
}

#[inline]
fn extract_host_from_url(url: &str) -> Option<String> {
    authority_host(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(value: &str) -> RefererCheck {
        let mut headers = HeaderMap::new();
        headers.insert("referer", value.parse().unwrap());
        referer_host(&headers, "referer")
    }

    fn host(value: &str) -> Option<String> {
        match check(value) {
            RefererCheck::Host(h) => Some(h),
            _ => None,
        }
    }

    #[test]
    fn parses_valid_absolute_uris() {
        assert_eq!(host("https://example.com/").as_deref(), Some("example.com"));
        assert_eq!(
            host("HTTPS://EXAMPLE.COM/Video.mp4").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            host("https://example.com:8443/path").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            host("https://user:pass@example.com/").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            host("http://[2001:db8::1]:8080/x").as_deref(),
            Some("2001:db8::1")
        );
        assert_eq!(
            host("https://example.com?q=1").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            host("https://example.com#frag").as_deref(),
            Some("example.com")
        );
    }

    #[test]
    fn rejects_malformed_or_absent_values() {
        // Header absent
        let mut headers = HeaderMap::new();
        assert!(matches!(
            referer_host(&headers, "referer"),
            RefererCheck::Absent
        ));
        headers.insert("referer", axum::http::HeaderValue::from_static(""));
        assert!(matches!(
            referer_host(&headers, "referer"),
            RefererCheck::Malformed
        ));
        // No scheme
        assert!(matches!(check("example.com/"), RefererCheck::Malformed));
        // Scheme-relative
        assert!(matches!(check("//example.com/"), RefererCheck::Malformed));
        // Missing slash after scheme
        assert!(matches!(
            check("https:/example.com/"),
            RefererCheck::Malformed
        ));
        // Empty authority
        assert!(matches!(check("https://"), RefererCheck::Malformed));
        assert!(matches!(check("https:///path"), RefererCheck::Malformed));
        // Non-http(s) scheme
        assert!(matches!(
            check("javascript:alert(1)//x"),
            RefererCheck::Malformed
        ));
        // Null origin (sandboxed iframe / data: URI) — never from this app
        assert!(matches!(check("null"), RefererCheck::Malformed));
        // NB: raw control characters are rejected by the HTTP parser before
        // this middleware ever runs, so they are not a reachable input.
    }

    #[test]
    fn encoded_and_trailing_dot_hosts_never_match() {
        // URL-encoded host cannot equal the decoded allowlisted host
        assert_eq!(
            host("https://example%2Ecom/").as_deref(),
            Some("example%2ecom")
        );
        // Trailing dot is a different DNS name
        assert_eq!(
            host("https://example.com./").as_deref(),
            Some("example.com.")
        );
        // Backslash variants: the "host" segment before `@` is treated as
        // userinfo and discarded, so only the attacker's host is compared.
        assert_eq!(
            host("https://example.com\\@evil.example/").as_deref(),
            Some("evil.example")
        );
        // userinfo pointing at an attacker host must not match
        assert_eq!(
            host("https://example.com@evil.example/").as_deref(),
            Some("evil.example")
        );
    }

    #[test]
    fn extract_host_from_public_url() {
        assert_eq!(
            extract_host_from_url("https://video.example.com:8443/").as_deref(),
            Some("video.example.com")
        );
        assert_eq!(
            extract_host_from_url("https://video.example.com").as_deref(),
            Some("video.example.com")
        );
        assert!(extract_host_from_url("no-scheme").is_none());
    }

    #[test]
    fn origin_extraction_keeps_port() {
        assert_eq!(
            origin_value("https://video.example.com"),
            Some("https://video.example.com".to_string())
        );
        // A non-default port is part of the origin (RFC 6454)
        assert_eq!(
            origin_value("https://video.example.com:8443/").as_deref(),
            Some("https://video.example.com:8443")
        );
        // Explicit default ports normalize away — browsers serialize them
        // without a port, so a PUBLIC_URL of `https://host:443` still
        // matches a browser's `https://host`.
        assert_eq!(
            origin_value("https://video.example.com:443/").as_deref(),
            Some("https://video.example.com")
        );
        assert_eq!(
            origin_value("http://video.example.com:80/").as_deref(),
            Some("http://video.example.com")
        );
        // A non-default port on the scheme's default is kept
        assert_eq!(
            origin_value("https://video.example.com:80/").as_deref(),
            Some("https://video.example.com:80")
        );
        // Userinfo is stripped; host + port survive
        assert_eq!(
            origin_value("https://user:pass@video.example.com:8443/").as_deref(),
            Some("https://video.example.com:8443")
        );
        // Invalid inputs → None
        assert!(origin_value("no-scheme").is_none());
        assert!(origin_value("https://example.com:abc/").is_none());
        assert!(origin_value("https://example.com:/").is_none());
        assert!(origin_value("https://user@\\evil.example/").is_none());
        assert!(origin_value("https://").is_none());
    }

    #[test]
    fn more_host_extraction_edge_cases() {
        // Scheme comparison is case-insensitive on both sides
        assert_eq!(host("HTTP://EXAMPLE.COM/x").as_deref(), Some("example.com"));
        // Multiple userinfo segments: only the last @ matters
        assert_eq!(
            host("https://a@b@example.com/").as_deref(),
            Some("example.com")
        );
        // userinfo plus port
        assert_eq!(
            host("https://user:pass@example.com:8443/").as_deref(),
            Some("example.com")
        );
        // Non-numeric port does not corrupt host extraction
        assert_eq!(
            host("https://example.com:abc/x").as_deref(),
            Some("example.com")
        );
        // Percent-encoding confined to the path
        assert_eq!(
            host("https://example.com/a%2Fb").as_deref(),
            Some("example.com")
        );
        // IPv6 literal without a port
        assert_eq!(
            host("http://[2001:db8::1]/x").as_deref(),
            Some("2001:db8::1")
        );
        // IPv6 with uppercase hex and port
        assert_eq!(
            host("http://[2001:DB8::1]:80/").as_deref(),
            Some("2001:db8::1")
        );
    }

    #[test]
    fn more_malformed_values_rejected() {
        // Scheme lookalikes
        assert!(matches!(
            check("ftp://example.com/"),
            RefererCheck::Malformed
        ));
        assert!(matches!(
            check("httpss://example.com/"),
            RefererCheck::Malformed
        ));
        assert!(matches!(check("http:///path"), RefererCheck::Malformed));
        // Authority that is only a port — empty host
        assert!(matches!(check("https://:8080/"), RefererCheck::Malformed));
        // Userinfo with empty host after '@'
        assert!(matches!(check("https://user@"), RefererCheck::Malformed));
        // A backslash in the host itself (`https://user@\evil.example/`) is
        // never valid in a URI authority — rejected outright rather than
        // half-parsed (the `example.com\@evil` trick, where the backslash
        // sits in the discarded userinfo, is covered in
        // `encoded_and_trailing_dot_hosts_never_match`).
        assert!(matches!(
            check("https://user@\\evil.example/"),
            RefererCheck::Malformed
        ));
    }

    #[test]
    fn header_length_boundary() {
        // Exactly MAX_HEADER_URI_LEN is accepted
        let mut ok = String::from("https://example.com/");
        ok.push_str(&"a".repeat(MAX_HEADER_URI_LEN - ok.len()));
        assert_eq!(ok.len(), MAX_HEADER_URI_LEN);
        assert!(host(&ok).is_some());
        // One byte over is rejected
        let mut long = String::from("https://example.com/");
        long.push_str(&"a".repeat(MAX_HEADER_URI_LEN));
        assert!(long.len() > MAX_HEADER_URI_LEN);
        assert!(host(&long).is_none());
    }

    #[test]
    fn referer_header_name_is_case_insensitive() {
        let mut headers = HeaderMap::new();
        headers.insert("REFERER", "https://evil.example/".parse().unwrap());
        assert!(matches!(
            referer_host(&headers, "referer"),
            RefererCheck::Host(h) if h == "evil.example"
        ));
        headers.insert("Origin", "https://evil.example".parse().unwrap());
        assert!(matches!(
            referer_host(&headers, "origin"),
            RefererCheck::Host(h) if h == "evil.example"
        ));
    }

    // ---- Middleware-level tests (full hotlink_guard path) ----

    use axum::{
        body::{to_bytes, Body},
        middleware,
        routing::get,
        Router,
    };
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    use crate::config::AppConfig;
    use crate::metrics::Metrics;
    use crate::middleware::rate_limit::RateLimiter;
    use crate::repositories::comment_repo::CommentRepository;
    use crate::repositories::danmaku_repo::DanmakuRepository;
    use crate::repositories::plan_repo::PlanRepository;
    use crate::repositories::playback_repo::PlaybackRepository;
    use crate::repositories::playlist_repo::PlaylistRepository;
    use crate::repositories::registration_repo::RegistrationRepository;
    use crate::repositories::share_repo::ShareRepository;
    use crate::repositories::tag_repo::TagRepository;
    use crate::repositories::tenant_repo::TenantRepository;
    use crate::repositories::user_repo::UserRepository;
    use crate::repositories::video_repo::VideoRepository;
    use crate::services::admin_service::AdminService;
    use crate::services::auth_service::AuthService;
    use crate::services::comment_service::CommentService;
    use crate::services::email_service::EmailService;
    use crate::services::media_service::MediaService;
    use crate::services::plan_service::PlanService;
    use crate::services::playback_service::PlaybackService;
    use crate::services::playlist_service::PlaylistService;
    use crate::services::recommendation_service::RecommendationService;
    use crate::services::search_service::SearchService;
    use crate::services::share_service::ShareService;
    use crate::services::tag_service::TagService;
    use crate::services::task_queue::TaskQueue;
    use crate::services::tenant_service::TenantService;
    use crate::services::transcoder::Transcoder;
    use crate::services::video_service::VideoService;
    use crate::state::{AppState, PlaybackSessionTracker, RepoLayer, ServiceLayer};
    use dashmap::DashMap;
    use moka::sync::Cache;
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    /// Build an AppState whose tenant repo points at a dead port (1), so the
    /// only field hotlink_guard reads — `config.public_url` — is under test
    /// control and no DB connection is ever established.
    fn test_state(public_url: &str) -> Arc<AppState> {
        let config = AppConfig {
            database_url: String::new(),
            server_port: 0,
            public_url: public_url.to_string(),
            media_root: std::env::temp_dir(),
            webapp_root: std::env::temp_dir(),
            log_dir: std::env::temp_dir(),
            data_dir: std::env::temp_dir(),
            registration_enabled: Arc::new(AtomicBool::new(false)),
            cors_origin: String::new(),
            cookie_secure: false,
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_from: String::new(),
            redis_url: String::new(),
            admin_ip_whitelist: Vec::new(),
            upload_quota_bytes: 0,
            db_max_connections: 100,
            db_min_connections: 2,
            migrations_dir: None,
            sentry_dsn: String::new(),
            sentry_environment: "production".into(),
            app_env: "test".into(),
            allow_first_user_admin: false,
            trusted_proxy: false,
            hashid_salt: String::new(),
            transcode_timeout_secs: 3600,
            ffprobe_timeout_secs: 30,
            transcode_concurrency: 1,
            transcode_max_duration_secs: 7200,
            ffmpeg_path: "ffmpeg".into(),
            ffprobe_path: "ffprobe".into(),
        };
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(500))
            .connect_lazy("postgres://127.0.0.1:1/atmos_video_test")
            .expect("lazy pool");
        let repos = RepoLayer {
            registration: RegistrationRepository::new(pool.clone()),
            user: UserRepository::new(pool.clone()),
            video: VideoRepository::new(pool.clone()),
            playback: PlaybackRepository::new(pool.clone()),
            playlist: PlaylistRepository::new(pool.clone()),
            comment: CommentRepository::new(pool.clone()),
            danmaku: DanmakuRepository::new(pool.clone()),
            share: ShareRepository::new(pool.clone()),
            tag: TagRepository::new(pool.clone()),
            tenant: TenantRepository::new(pool.clone(), config.public_url.clone()),
            plan: PlanRepository::new(pool.clone()),
        };
        let playback_service = PlaybackService::new(repos.playback.clone());
        let playlist_service = PlaylistService::new(repos.playlist.clone());
        let services = ServiceLayer {
            video: VideoService::new(repos.video.clone(), config.clone()),
            media: MediaService::new(repos.video.clone(), config.clone()),
            playback: playback_service.clone(),
            playlist: playlist_service,
            auth: AuthService::new(
                repos.user.clone(),
                repos.tenant.clone(),
                playback_service,
                RateLimiter::new(),
                RateLimiter::new(),
                config.clone(),
            ),
            email: EmailService::new(config.clone()),
            tag: TagService::new(repos.tag.clone(), repos.video.clone()),
            search: SearchService::new(repos.video.clone()),
            recommendation: RecommendationService::new(repos.video.clone()),
            comment: CommentService::new(repos.comment.clone(), repos.video.clone()),
            share: ShareService::new(repos.share.clone()),
            admin: AdminService::new(repos.user.clone()),
            tenant: TenantService::new(repos.tenant.clone()),
            plan: PlanService::new(repos.plan.clone()),
        };
        let transcoder = Transcoder::new(&std::env::temp_dir(), Default::default());
        Arc::new(AppState {
            repos,
            services,
            config: config.clone(),
            rate_limiter: RateLimiter::new(),
            ip_rate_limiter: RateLimiter::new(),
            video_cache: Cache::builder().max_capacity(10_000).build(),
            recommendation_cache: Cache::builder().max_capacity(10_000).build(),
            video_detail_cache: Cache::builder().max_capacity(10_000).build(),
            playback_sessions: Arc::new(PlaybackSessionTracker::new()),
            upload_locks: Arc::new(DashMap::new()),
            metrics: Metrics::new(),
            redis: None,
            transcoder: transcoder.clone(),
            task_queue: TaskQueue::new(transcoder, pool, config.media_root.clone()),
        })
    }

    fn request_with_headers(headers: &[(&str, &str)]) -> Request {
        let mut builder = Request::builder().uri("/x");
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.body(Body::empty()).unwrap()
    }

    async fn run_hotlink(req: Request) -> Response {
        Router::new()
            .route("/{*any}", get(|| async { (StatusCode::OK, "ok") }))
            .layer(middleware::from_fn(hotlink_guard))
            .oneshot(req)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn middleware_allows_matching_referer_and_origin() {
        let state = test_state("https://video.example.com");
        // Same host, with path
        let mut req = request_with_headers(&[("referer", "https://video.example.com/a/b.mp4")]);
        req.extensions_mut().insert(state.clone());
        assert_eq!(run_hotlink(req).await.status(), StatusCode::OK);
        // Referer comparison is host-only: the same host on another port is
        // still the same site for referrer purposes.
        let mut req = request_with_headers(&[("referer", "https://video.example.com:8443/")]);
        req.extensions_mut().insert(state.clone());
        assert_eq!(run_hotlink(req).await.status(), StatusCode::OK);
        // Same host via Origin (cross-origin fetch without Referer)
        let mut req = request_with_headers(&[("origin", "https://video.example.com")]);
        req.extensions_mut().insert(state.clone());
        assert_eq!(run_hotlink(req).await.status(), StatusCode::OK);
        // An explicit default port normalizes away, exactly as a browser
        // serializes the origin (RFC 6454).
        let mut req = request_with_headers(&[("origin", "https://video.example.com:443")]);
        req.extensions_mut().insert(state.clone());
        assert_eq!(run_hotlink(req).await.status(), StatusCode::OK);
        // No Referer and no Origin — direct download, allowed
        let mut req = request_with_headers(&[]);
        req.extensions_mut().insert(state);
        assert_eq!(run_hotlink(req).await.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_origin_compares_scheme_host_and_port() {
        // PUBLIC_URL carries a non-default port: the browser's Origin, which
        // includes the same port (RFC 6454), must match…
        let state = test_state("https://video.example.com:8443");
        let mut req = request_with_headers(&[("origin", "https://video.example.com:8443")]);
        req.extensions_mut().insert(state.clone());
        assert_eq!(run_hotlink(req).await.status(), StatusCode::OK);
        // …while the same host without the port is a different origin and is
        // blocked.
        let mut req = request_with_headers(&[("origin", "https://video.example.com")]);
        req.extensions_mut().insert(state);
        assert_eq!(run_hotlink(req).await.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn middleware_blocks_foreign_referer_and_origin() {
        let state = test_state("https://video.example.com");
        let cases = [
            ("referer", "https://evil.example/"),
            ("referer", "https://EVIL.example/"),
            ("referer", "https://video.example.com./"), // trailing dot
            ("referer", "https://video.example.com@evil.example/"), // userinfo
            ("referer", "https://video.example.com\\@evil.example/"), // backslash
            ("referer", "https://example%2Ecom/"),      // encoded allowlist host
            ("origin", "https://evil.example"),
            // Same host, different port = different origin per RFC 6454
            ("origin", "https://video.example.com:8443"),
            ("origin", "https://video.example.com:80"),
            ("origin", "https://user@\\evil.example"), // backslash host
        ];
        for (name, value) in cases {
            let mut req = request_with_headers(&[(name, value)]);
            req.extensions_mut().insert(state.clone());
            let res = run_hotlink(req).await;
            assert_eq!(res.status(), StatusCode::FORBIDDEN, "case {name}: {value}");
        }
    }

    #[tokio::test]
    async fn middleware_blocks_malformed_referer_values() {
        let state = test_state("https://video.example.com");
        for value in [
            "example.com/",
            "//example.com/",
            "https:/evil.example",
            "https://",
            "null",
            "javascript:alert(1)",
        ] {
            let mut req = request_with_headers(&[("referer", value)]);
            req.extensions_mut().insert(state.clone());
            let res = run_hotlink(req).await;
            assert_eq!(res.status(), StatusCode::FORBIDDEN, "case: {value}");
        }
    }

    #[tokio::test]
    async fn middleware_blocks_non_utf8_referer() {
        let state = test_state("https://video.example.com");
        let mut req = Request::builder()
            .uri("/")
            .header(
                "referer",
                axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
            )
            .body(Body::empty())
            .unwrap();
        req.extensions_mut().insert(state);
        assert_eq!(run_hotlink(req).await.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn middleware_passes_through_without_state() {
        // No AppState extension — the guard must not crash, just pass through.
        let req = request_with_headers(&[("referer", "https://evil.example/")]);
        assert_eq!(run_hotlink(req).await.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_500s_on_invalid_public_url() {
        let state = test_state("no-scheme");
        let mut req = request_with_headers(&[("referer", "https://video.example.com/")]);
        req.extensions_mut().insert(state);
        let res = run_hotlink(req).await;
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"server configuration error");
    }
}
