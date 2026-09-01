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
    use std::sync::atomic::AtomicBool;

    use axum::{body::Body, extract::ConnectInfo, middleware, routing::get, Router};
    use tower::ServiceExt;

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

    /// AppState wired to a dead DB port (1) — only `ip_rate_limiter` is
    /// exercised here, no connection is ever established.
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
        let state = test_state();
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
        let state = test_state();
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
        let state = test_state();
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
