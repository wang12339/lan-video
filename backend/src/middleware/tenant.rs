use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct TenantContext {
    pub tenant_id: i64,
    pub slug: String,
}

pub async fn resolve_tenant(mut req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "server config error").into_response();
    };

    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    let tenant = state.repos.tenant.resolve_from_host(&host).await;

    let Some(tenant) = tenant else {
        tracing::warn!(host = %host, "no tenant found for host");
        return (StatusCode::NOT_FOUND, "unknown site").into_response();
    };

    tracing::debug!(host = %host, tenant_id = tenant.tenant_id, slug = %tenant.slug, "tenant resolved");
    req.extensions_mut().insert(tenant);
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicBool;

    use axum::{body::Body, middleware, routing::get, Router};
    use tower::ServiceExt;

    use crate::config::AppConfig;
    use crate::metrics::Metrics;
    use crate::middleware::rate_limit::RateLimiter;
    use crate::repositories::comment_repo::CommentRepository;
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
    use crate::services::playback_service::PlaybackService;
    use crate::services::recommendation_service::RecommendationService;
    use crate::services::search_service::SearchService;
    use crate::services::share_service::ShareService;
    use crate::services::tag_service::TagService;
    use crate::services::task_queue::TaskQueue;
    use crate::services::transcoder::Transcoder;
    use crate::services::video_service::VideoService;
    use crate::state::{AppState, PlaybackSessionTracker, RepoLayer, ServiceLayer};
    use dashmap::DashMap;
    use moka::sync::Cache;
    use sqlx::postgres::PgPoolOptions;

    /// AppState whose tenant repo points at a dead port (1): host resolution
    /// always fails fast (connection refused) and the middleware determinis-
    /// tically returns 404 — no live database is required. This exercises the
    /// full pipeline: Host-header extraction, lowercasing, `normalize_host`
    /// (port / IPv6-bracket / trailing-dot stripping), the 255-byte guard and
    /// the subdomain/slug resolution paths.
    fn test_state() -> Arc<AppState> {
        let config = AppConfig {
            database_url: String::new(),
            server_port: 0,
            public_url: "https://video.example.com".to_string(),
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
            share: ShareRepository::new(pool.clone()),
            tag: TagRepository::new(pool.clone()),
            tenant: TenantRepository::new(pool.clone()),
        };
        let playback_service = PlaybackService::new(repos.playback.clone());
        let services = ServiceLayer {
            video: VideoService::new(repos.video.clone(), config.clone()),
            media: MediaService::new(repos.video.clone(), config.clone()),
            playback: playback_service.clone(),
            auth: AuthService::new(
                repos.user.clone(),
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
        };
        let transcoder = Transcoder::new(&std::env::temp_dir());
        Arc::new(AppState {
            repos,
            services,
            config,
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
            task_queue: TaskQueue::new(transcoder, pool),
        })
    }

    fn tenant_app() -> Router {
        Router::new()
            .route("/{*any}", get(|| async { (StatusCode::OK, "ok") }))
            .layer(middleware::from_fn(resolve_tenant))
    }

    fn host_req(host: Option<&axum::http::HeaderValue>, state: &Arc<AppState>) -> Request {
        let mut builder = Request::builder().uri("/");
        if let Some(host) = host {
            builder = builder.header("host", host);
        }
        let mut req = builder.body(Body::empty()).unwrap();
        req.extensions_mut().insert(state.clone());
        req
    }

    #[tokio::test]
    async fn missing_state_returns_500() {
        let app = Router::new()
            .route("/{*any}", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(resolve_tenant));
        let res = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn absent_host_header_yields_404() {
        let state = test_state();
        let app = tenant_app();
        // No Host header at all
        let res = app.clone().oneshot(host_req(None, &state)).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        // Empty Host header
        let res = app
            .oneshot(host_req(Some(&"".parse().unwrap()), &state))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn host_forms_all_resolve_to_404_without_known_tenants() {
        let state = test_state();
        let app = tenant_app();
        // Each Host form must reach `normalize_host` and the resolver without
        // panicking and end in a clean 404 (resolution needs a live DB).
        let hosts = [
            "localhost",
            "localhost:8082",
            "127.0.0.1",
            "127.0.0.1:8082",
            "video.example.com",
            "video.example.com:8082",
            "VIDEO.EXAMPLE.COM",
            "Video.Example.Com.",
            "sub.video.example.com",
            "evil.example",
            "attacker.com:443",
            "example.com@evil.example", // forged userinfo-in-host
            "[::1]",
            "[::1]:8082",
            "[2001:db8::1]",
            "[2001:db8::1]:8082",
        ];
        for host in hosts {
            let res = app
                .clone()
                .oneshot(host_req(Some(&host.parse().unwrap()), &state))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::NOT_FOUND, "host: {host}");
        }
    }

    #[tokio::test]
    async fn overlong_host_yields_404() {
        let state = test_state();
        let app = tenant_app();
        // 300 bytes exceeds MAX_HOST_LEN (255) — rejected before any lookup
        let host = format!("{}.com", "a".repeat(300));
        let res = app
            .oneshot(host_req(Some(&host.parse().unwrap()), &state))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn non_utf8_host_header_yields_404() {
        let state = test_state();
        let app = tenant_app();
        // Non-UTF8 host bytes fail to_str() → treated as absent host
        let host = axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap();
        let res = app.oneshot(host_req(Some(&host), &state)).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
