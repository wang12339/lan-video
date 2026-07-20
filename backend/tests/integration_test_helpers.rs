//! Shared helpers for integration tests that run against a real PostgreSQL database.
//!
//! All tests in this module are gated behind the `DATABASE_URL` environment variable.
//! If not set, tests are silently skipped.

use std::sync::Arc;
use std::time::Duration;

use lan_video_backend::config::AppConfig;
use lan_video_backend::metrics::Metrics;
use lan_video_backend::middleware::rate_limit::RateLimiter;
use lan_video_backend::repositories::playback_repo::PlaybackRepository;
use lan_video_backend::repositories::playlist_repo::PlaylistRepository;
use lan_video_backend::repositories::registration_repo::RegistrationRepository;
use lan_video_backend::repositories::tag_repo::TagRepository;
use lan_video_backend::repositories::user_repo::UserRepository;
use lan_video_backend::repositories::video_repo::VideoRepository;
use lan_video_backend::services::admin_service::AdminService;
use lan_video_backend::services::auth_service::AuthService;
use lan_video_backend::services::comment_service::CommentService;
use lan_video_backend::services::share_service::ShareService;
use lan_video_backend::services::media_service::MediaService;
use lan_video_backend::services::playback_service::PlaybackService;
use lan_video_backend::services::recommendation_service::RecommendationService;
use lan_video_backend::services::search_service::SearchService;
use lan_video_backend::services::tag_service::TagService;
use lan_video_backend::services::task_queue::TaskQueue;
use lan_video_backend::services::transcoder::Transcoder;
use lan_video_backend::services::video_service::VideoService;
use lan_video_backend::state::{AppState, VideoListCache};
use sqlx::PgPool;

/// Returns the database URL from the environment, or skips the test if not set.
pub fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

/// Create a PgPool connected to the test database.
/// Panics if connection fails (tests should not run without a working DB).
pub async fn test_pool() -> PgPool {
    let url = database_url().expect("DATABASE_URL not set — skipping integration test");
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&url)
        .await
        .expect("Failed to connect to test database")
}

/// Build a minimal AppConfig suitable for integration tests.
pub fn test_config() -> AppConfig {
    AppConfig {
        database_url: database_url().unwrap_or_default(),
        server_port: 0, // random port for tests
        public_url: "http://localhost:3000".into(),
        media_root: std::path::PathBuf::from("/tmp/atmos-test-media"),
        webapp_root: std::path::PathBuf::from("/tmp/atmos-test-webapp"),
        log_dir: std::path::PathBuf::from("/tmp/atmos-test-logs"),
        data_dir: std::path::PathBuf::from("/tmp/atmos-test-data"),
        registration_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        cors_origin: "http://localhost:3000".into(),
        cookie_secure: false,
    }
}

/// Create a full AppState backed by a real database pool.
pub async fn test_app_state() -> Arc<AppState> {
    let pool = test_pool().await;
    let config = test_config();

    let user_repo = UserRepository::new(pool.clone());
    let video_repo = VideoRepository::new(pool.clone());
    let playback_repo = PlaybackRepository::new(pool.clone());
    let playlist_repo = PlaylistRepository::new(pool.clone());
    let comment_repo =
        lan_video_backend::repositories::comment_repo::CommentRepository::new(pool.clone());
    let share_repo =
        lan_video_backend::repositories::share_repo::ShareRepository::new(pool.clone());
    let tag_repo = TagRepository::new(pool.clone());
    let registration_repo = RegistrationRepository::new(pool.clone());
    let video_service = VideoService::new(video_repo.clone(), config.clone());
    let media_service = MediaService::new(video_repo.clone(), config.clone());
    let playback_service = PlaybackService::new(playback_repo.clone());
    let tag_service = TagService::new(tag_repo.clone());
    let search_service = SearchService::new(video_repo.clone());
    let recommendation_service = RecommendationService::new(video_repo.clone());
    let comment_service = CommentService::new(comment_repo.clone());
    let share_service = ShareService::new(share_repo.clone());
    let admin_service = AdminService::new(user_repo.clone());

    let video_cache = VideoListCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(64)
        .build();

    let transcoder = Transcoder::new(video_repo.clone(), config.clone());
    let task_queue = TaskQueue::new(transcoder.clone(), pool.clone());

    Arc::new(AppState {
        registration_repo,
        user_repo: user_repo.clone(),
        video_repo,
        playback_repo,
        playlist_repo,
        comment_repo,
        share_repo,
        tag_repo,
        video_service,
        media_service,
        playback_service: playback_service.clone(),
        auth_service: AuthService::new(
            user_repo,
            playback_service,
            RateLimiter::new(),
            RateLimiter::new(),
            config.clone(),
        ),
        tag_service,
        search_service,
        recommendation_service,
        comment_service,
        share_service,
        admin_service,
        config,
        rate_limiter: RateLimiter::new(),
        ip_rate_limiter: RateLimiter::new(),
        video_cache,
        playback_sessions: std::sync::Arc::new(
            lan_video_backend::state::PlaybackSessionTracker::new(),
        ),
        upload_locks: std::sync::Arc::new(dashmap::DashMap::new()),
        metrics: Metrics::new(),
        transcoder,
        task_queue,
    })
}

/// Generate a unique username for test isolation.
pub fn unique_username(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}_{}", prefix, std::process::id(), n)
}

/// Clean up test data created during a test.
/// Removes users (cascades to auth_tokens), likes, favorites, playback_history.
/// Videos are NOT deleted to avoid filesystem side effects.
#[allow(dead_code)]
pub async fn cleanup_test_user(pool: &PgPool, username: &str) {
    // Delete in dependency order
    let _ = sqlx::query("DELETE FROM user_likes WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM user_favorites WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM playback_history WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "DELETE FROM auth_tokens WHERE user_id IN (SELECT id FROM users WHERE username = $1)",
    )
    .bind(username)
    .execute(pool)
    .await;
    let _ = sqlx::query("DELETE FROM users WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
}

/// Clean up a test video by ID.
#[allow(dead_code)]
pub async fn cleanup_test_video(pool: &PgPool, video_id: i64) {
    let _ = sqlx::query("DELETE FROM playback_history WHERE video_id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM user_likes WHERE video_id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM user_favorites WHERE video_id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM videos WHERE id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
}
