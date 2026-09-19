//! 单元测试共享辅助（`#[cfg(test)]`，由 lib.rs 挂载）。
//!
//! 中间件测试（hotlink / share_rate_limit）需要构造一个完整的 `AppState`，
//! 但只用到 `config` / `ip_rate_limiter` 等少数字段。为避免每个测试模块
//! 各自复制约 100 行 AppState/RepoLayer/ServiceLayer 构建代码，这里集中
//! 提供 [`test_state`]。集成测试（tests/ 目录）使用
//! `tests/integration_test_helpers.rs`，与本模块无关。

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use moka::sync::Cache;
use sqlx::postgres::PgPoolOptions;

use crate::config::AppConfig;
use crate::metrics::Metrics;
use crate::middleware::rate_limit::RateLimiter;
use crate::repositories::comment_repo::CommentRepository;
use crate::repositories::danmaku_repo::DanmakuRepository;
use crate::repositories::playback_repo::PlaybackRepository;
use crate::repositories::playlist_repo::PlaylistRepository;
use crate::repositories::registration_repo::RegistrationRepository;
use crate::repositories::share_repo::ShareRepository;
use crate::repositories::tag_repo::TagRepository;
use crate::repositories::user_repo::UserRepository;
use crate::repositories::video_repo::VideoRepository;
use crate::services::admin_service::AdminService;
use crate::services::auth_service::AuthService;
use crate::services::comment_service::CommentService;
use crate::services::email_service::EmailService;
use crate::services::media_service::MediaService;
use crate::services::playback_service::PlaybackService;
use crate::services::playlist_service::PlaylistService;
use crate::services::recommendation_service::RecommendationService;
use crate::services::search_service::SearchService;
use crate::services::share_service::ShareService;
use crate::services::tag_service::TagService;
use crate::services::task_queue::TaskQueue;
use crate::services::transcoder::Transcoder;
use crate::services::video_service::VideoService;
use crate::state::{AppState, PlaybackSessionTracker, RepoLayer, ServiceLayer};

/// 构造一个指向死端口（1）的 `AppState`：懒连接池不会真正建立连接，
/// 供只读取 config / 内存限流器的中间件测试使用。
///
/// `public_url` 由测试控制（hotlink_guard 的允许来源检查依赖它）。
pub fn test_state(public_url: &str) -> Arc<AppState> {
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
        gateway_url: String::new(),
        gateway_internal_url: String::new(),
        gateway_client_id: String::new(),
        gateway_client_secret: String::new(),
        gateway_redirect_uri: String::new(),
        metrics_token: String::new(),
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
        chat: crate::repositories::chat_repo::ChatRepository::new(pool.clone()),
        danmaku: DanmakuRepository::new(pool.clone()),
        share: ShareRepository::new(pool.clone()),
        tag: TagRepository::new(pool.clone()),
    };
    let playback_service = PlaybackService::new(repos.playback.clone());
    let playlist_service = PlaylistService::new(repos.playlist.clone());
    let services = ServiceLayer {
        video: VideoService::new(repos.video.clone(), repos.playback.clone(), config.clone()),
        media: MediaService::new(repos.video.clone(), config.clone()),
        playback: playback_service.clone(),
        playlist: playlist_service,
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
        chat_hub: Arc::new(crate::state::ChatHub::new()),
        metrics: Metrics::new(),
        redis: None,
        transcoder: transcoder.clone(),
        task_queue: TaskQueue::new(transcoder, pool, config.media_root.clone()),
    })
}
