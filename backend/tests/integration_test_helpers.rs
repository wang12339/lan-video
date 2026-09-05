#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Shared helpers may appear unused in some test binaries
#![allow(dead_code)]
//! Shared helpers for integration tests that run against a real PostgreSQL database.
//!
//! All tests in this module are gated behind the `DATABASE_URL` environment variable.
//! If not set, tests are silently skipped.

use std::sync::Arc;
use std::time::Duration;

use atmos_video_backend::config::AppConfig;
use atmos_video_backend::metrics::Metrics;
use atmos_video_backend::middleware::rate_limit::RateLimiter;
use atmos_video_backend::models::auth::AuthRequest;
use atmos_video_backend::repositories::comment_repo::CommentRow;
use atmos_video_backend::repositories::playback_repo::PlaybackRepository;
use atmos_video_backend::repositories::playlist_repo::PlaylistRepository;
use atmos_video_backend::repositories::registration_repo::RegistrationRepository;
use atmos_video_backend::repositories::tag_repo::TagRepository;
use atmos_video_backend::repositories::user_repo::UserRepository;
use atmos_video_backend::repositories::video_repo::VideoRepository;
use atmos_video_backend::services::admin_service::AdminService;
use atmos_video_backend::services::auth_service::AuthService;
use atmos_video_backend::services::comment_service::CommentService;
use atmos_video_backend::services::email_service::EmailService;
use atmos_video_backend::services::media_service::MediaService;
use atmos_video_backend::services::playback_service::PlaybackService;
use atmos_video_backend::services::recommendation_service::RecommendationService;
use atmos_video_backend::services::search_service::SearchService;
use atmos_video_backend::services::share_service::ShareService;
use atmos_video_backend::services::tag_service::TagService;
use atmos_video_backend::services::task_queue::TaskQueue;
use atmos_video_backend::services::tenant_service::TenantService;
use atmos_video_backend::services::transcoder::Transcoder;
use atmos_video_backend::services::video_service::VideoService;
use atmos_video_backend::state::{
    AppState, RecommendationCache, RepoLayer, ServiceLayer, VideoListCache,
};
use atmos_video_backend::util::hashid;
use atmos_video_backend::util::password;
use sqlx::PgPool;

/// Returns the database URL from the environment, or skips the test if not set.
pub fn database_url() -> Option<String> {
    static WARNED: std::sync::Once = std::sync::Once::new();
    match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            WARNED.call_once(|| {
                eprintln!(
                    "NOTE: DATABASE_URL is not set — DB integration tests are SKIPPED. `cargo test` green does NOT mean the integration suite passed."
                );
            });
            None
        }
    }
}

/// Create a PgPool connected to the test database.
/// Panics if connection fails (tests should not run without a working DB).
pub async fn test_pool() -> PgPool {
    static SCHEMA_READY: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    let url = database_url().expect("DATABASE_URL not set — skipping integration test");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&url)
        .await
        .expect("Failed to connect to test database");
    // Fixture helpers run raw SQL against the pool, so the schema must exist
    // even when a binary's first tests never build a router. `build_router`
    // applies migrations (idempotent); do it once per test process.
    SCHEMA_READY
        .get_or_init(|| async {
            let _ = atmos_video_backend::app::build_router(test_config()).await;
        })
        .await;
    pool
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
        smtp_host: String::new(),
        smtp_port: 587,
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
    }
}

/// Create a full AppState backed by a real database pool.
pub async fn test_app_state() -> Arc<AppState> {
    test_app_state_with_config(test_config()).await
}

/// Create a full AppState backed by a real database pool with a custom config.
pub async fn test_app_state_with_config(config: AppConfig) -> Arc<AppState> {
    let pool = test_pool().await;

    let user_repo = UserRepository::new(pool.clone());
    let video_repo = VideoRepository::new(pool.clone());
    let playback_repo = PlaybackRepository::new(pool.clone());
    let playlist_repo = PlaylistRepository::new(pool.clone());
    let comment_repo =
        atmos_video_backend::repositories::comment_repo::CommentRepository::new(pool.clone());
    let share_repo =
        atmos_video_backend::repositories::share_repo::ShareRepository::new(pool.clone());
    let tag_repo = TagRepository::new(pool.clone());
    let tenant_repo = atmos_video_backend::repositories::tenant_repo::TenantRepository::new(
        pool.clone(),
        config.public_url.clone(),
    );
    let plan_repo = atmos_video_backend::repositories::plan_repo::PlanRepository::new(pool.clone());
    let danmaku_repo =
        atmos_video_backend::repositories::danmaku_repo::DanmakuRepository::new(pool.clone());
    let registration_repo = RegistrationRepository::new(pool.clone());
    let video_service = VideoService::new(video_repo.clone(), config.clone());
    let media_service = MediaService::new(video_repo.clone(), config.clone());
    let playback_service = PlaybackService::new(playback_repo.clone());
    let tag_service = TagService::new(tag_repo.clone(), video_repo.clone());
    let search_service = SearchService::new(video_repo.clone());
    let recommendation_service = RecommendationService::new(video_repo.clone());
    let comment_service = CommentService::new(comment_repo.clone(), video_repo.clone());
    let share_service = ShareService::new(share_repo.clone());
    let admin_service = AdminService::new(user_repo.clone());
    let email_service = EmailService::new(config.clone());
    let playlist_service = atmos_video_backend::services::playlist_service::PlaylistService::new(
        playlist_repo.clone(),
    );
    let plan_service =
        atmos_video_backend::services::plan_service::PlanService::new(plan_repo.clone());

    let video_cache = VideoListCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(64)
        .build();

    let recommendation_cache = RecommendationCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(64)
        .build();

    let video_detail_cache = atmos_video_backend::state::VideoDetailCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(64)
        .build();

    let transcoder = Transcoder::new(&config.media_root, config.transcode_settings());
    let task_queue = TaskQueue::new(transcoder.clone(), pool.clone(), config.media_root.clone());

    Arc::new(AppState {
        repos: RepoLayer {
            registration: registration_repo,
            user: user_repo.clone(),
            video: video_repo,
            playback: playback_repo,
            playlist: playlist_repo,
            comment: comment_repo,
            danmaku: danmaku_repo,
            share: share_repo,
            tag: tag_repo,
            tenant: tenant_repo.clone(),
            plan: plan_repo,
        },
        services: ServiceLayer {
            video: video_service,
            media: media_service,
            playback: playback_service.clone(),
            playlist: playlist_service,
            auth: AuthService::new(
                user_repo,
                tenant_repo.clone(),
                playback_service,
                RateLimiter::new(),
                RateLimiter::new(),
                config.clone(),
            ),
            email: email_service,
            tag: tag_service,
            search: search_service,
            recommendation: recommendation_service,
            comment: comment_service,
            share: share_service,
            admin: admin_service,
            tenant: TenantService::new(tenant_repo),
            plan: plan_service,
        },
        config,
        rate_limiter: RateLimiter::new(),
        ip_rate_limiter: RateLimiter::new(),
        video_cache,
        recommendation_cache,
        video_detail_cache,
        playback_sessions: std::sync::Arc::new(
            atmos_video_backend::state::PlaybackSessionTracker::new(),
        ),
        upload_locks: std::sync::Arc::new(dashmap::DashMap::new()),
        metrics: Metrics::new(),
        redis: None,
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

/// Clean up comments belonging to a test video.
/// (Video deletion cascades comments, but deleting explicitly first keeps the
/// cleanup order deterministic.)
#[allow(dead_code)]
pub async fn cleanup_test_comments(pool: &PgPool, video_id: i64) {
    let _ = sqlx::query("DELETE FROM comments WHERE video_id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
}

/// Build an AuthService wired to the same state as the test app.
/// Extracted here so test binaries don't each re-implement it.
pub fn auth_service(state: &AppState) -> AuthService {
    AuthService::new(
        state.repos.user.clone(),
        state.repos.tenant.clone(),
        state.services.playback.clone(),
        state.rate_limiter.clone(),
        state.ip_rate_limiter.clone(),
        state.config.clone(),
    )
}

/// Password used for users created by the fixture helpers below.
/// Deliberately strong enough to pass registration policy if a test
/// registers through the service instead of inserting directly.
pub const TEST_USER_PASSWORD: &str = "TestPass_123!";

/// Create an approved test user directly in the DB (skipping the
/// register → admin-approval flow). Returns `(username, user_id)`.
pub async fn create_test_user(state: &Arc<AppState>, prefix: &str) -> (String, i64) {
    let username = unique_username(prefix);
    let hash = password::hash(TEST_USER_PASSWORD).expect("hash fixture password");
    let user_id = state
        .repos
        .user
        .create_user(1, &username, &hash, 3)
        .await
        .expect("create test user");
    (username, user_id)
}

/// Create an approved test user and log in to obtain an auth token.
/// Returns `(username, password, user_id, token)`.
pub async fn create_test_user_with_credentials(
    state: &Arc<AppState>,
    prefix: &str,
) -> (String, String, i64, String) {
    let (username, user_id) = create_test_user(state, prefix).await;
    let token = login_and_get_token(state, &username, TEST_USER_PASSWORD).await;
    (username, TEST_USER_PASSWORD.to_string(), user_id, token)
}

/// Log in with the given credentials and return the auth token.
/// Panics if login fails or no token is issued.
pub async fn login_and_get_token(state: &Arc<AppState>, username: &str, password: &str) -> String {
    let svc = auth_service(state);
    let resp = svc
        .login(
            &AuthRequest {
                username: username.to_string(),
                password: password.to_string(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login should not error");
    assert!(resp.ok, "login failed: {:?}", resp.error);
    resp.token.expect("token should be present")
}

/// Insert an external test video and return its id.
pub async fn create_test_video(state: &Arc<AppState>, prefix: &str) -> i64 {
    let title = unique_username(prefix);
    state
        .services
        .video
        .add_external_video(
            1,
            &title,
            Some("test fixture video"),
            Some("fixture"),
            &format!("https://example.com/{}.mp4", title),
            None,
            None,
        )
        .await
        .expect("create test video")
}

/// Insert a test video owned by `uploader_id`. Comments and share links are
/// restricted to the video's uploader (or an admin), so any test that wants a
/// third party to interact with a video must run with an explicit uploader.
pub async fn create_test_video_owned_by(
    state: &Arc<AppState>,
    prefix: &str,
    uploader_id: i64,
) -> i64 {
    let video_id = create_test_video(state, prefix).await;
    sqlx::query("UPDATE videos SET uploader_id = $1 WHERE id = $2")
        .bind(uploader_id)
        .bind(video_id)
        .execute(state.repos.video.pool())
        .await
        .expect("set test video uploader");
    video_id
}

/// Create a comment on `video_id` as `user_id` and return the row.
/// `parent_id` may thread it as a reply to an existing comment.
pub async fn create_test_comment(
    state: &Arc<AppState>,
    video_id: i64,
    user_id: i64,
    content: &str,
    parent_id: Option<i64>,
) -> CommentRow {
    state
        .services
        .comment
        .create_comment(1, video_id, user_id, content, parent_id, false)
        .await
        .expect("create test comment")
}

/// Format an `Authorization: Bearer <token>` header value for HTTP-level tests.
pub fn auth_header_value(token: &str) -> String {
    format!("Bearer {}", token)
}

/// Parse a JSON id that the API may serialize as either a raw number or a
/// hashid string (most resource ids are hashid-obfuscated).
pub fn json_id(v: &serde_json::Value) -> i64 {
    v.as_i64()
        .or_else(|| v.as_str().and_then(hashid::decode_id))
        .expect("JSON id must be numeric or a decodable hashid")
}

// ── Self-tests for the helpers above ──
// Non-DB unit tests run in every binary that includes this module; DB-gated
// tests early-return unless DATABASE_URL is set.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_unique_username_is_unique() {
        let a = unique_username("self");
        let b = unique_username("self");
        assert_ne!(a, b, "sequential calls must produce distinct usernames");
    }

    #[test]
    fn test_unique_username_keeps_prefix() {
        let name = unique_username("my_prefix");
        assert!(
            name.starts_with("my_prefix_"),
            "username should retain its prefix: {name}"
        );
    }

    #[test]
    fn test_unique_username_fits_db_column() {
        let name = unique_username(&"x".repeat(200));
        assert!(
            name.len() <= 255,
            "username must fit VARCHAR(255), got {} chars",
            name.len()
        );
    }

    #[test]
    fn test_auth_header_value_format() {
        assert_eq!(auth_header_value("tok123"), "Bearer tok123");
        assert_eq!(auth_header_value(""), "Bearer ");
    }

    #[test]
    fn test_test_config_sane_defaults() {
        let cfg = test_config();
        assert_eq!(cfg.server_port, 0, "random port for tests");
        assert!(
            !cfg.cookie_secure,
            "cookie_secure must be off for plain-HTTP tests"
        );
        assert!(
            cfg.registration_enabled.load(Ordering::SeqCst),
            "registration must be enabled for fixture users"
        );
        assert_eq!(cfg.public_url, "http://localhost:3000");
        assert_eq!(
            cfg.media_root,
            std::path::PathBuf::from("/tmp/atmos-test-media")
        );
    }

    #[test]
    fn test_database_url_matches_environment() {
        assert_eq!(database_url(), std::env::var("DATABASE_URL").ok());
    }

    #[test]
    fn test_fixture_password_is_hashable_and_verifiable() {
        let hash = password::hash(TEST_USER_PASSWORD).unwrap();
        assert!(password::verify(TEST_USER_PASSWORD, &hash).unwrap());
        assert!(!password::verify("wrong", &hash).unwrap());
    }

    // ── DB-gated self-tests: exercise the fixture helpers end to end ──

    #[tokio::test]
    async fn test_helper_roundtrip_user_video_comment() {
        let Some(_) = database_url() else {
            eprintln!("DATABASE_URL not set, skipping");
            return;
        };

        let state = test_app_state().await;

        let (username, _password, user_id, token) =
            create_test_user_with_credentials(&state, "fixture").await;
        assert!(user_id > 0);
        assert!(!token.is_empty(), "logged-in user must get a token");

        // Wrong password must not yield a token (2 auth attempts total for
        // this username — the 3rd within 60s would be rate-limited)
        let svc = auth_service(&state);
        let bad = svc
            .login(
                &AuthRequest {
                    username: username.clone(),
                    password: "WrongPassword_1".into(),
                },
                "127.0.0.1",
                1,
            )
            .await
            .expect("login should not error");
        assert!(!bad.ok && bad.token.is_none());

        let video_id = create_test_video_owned_by(&state, "fixture", user_id).await;
        assert!(video_id > 0);

        let comment = create_test_comment(&state, video_id, user_id, "fixture comment", None).await;
        assert_eq!(comment.video_id, video_id);
        assert_eq!(comment.user_id, user_id);
        assert_eq!(comment.content, "fixture comment");
        assert_eq!(comment.username, username);

        // A reply to the comment works too and is threaded to the parent
        let reply =
            create_test_comment(&state, video_id, user_id, "fixture reply", Some(comment.id)).await;
        assert!(reply.id > 0);
        assert_eq!(reply.parent_id, Some(comment.id));

        let (comments, total) = state
            .services
            .comment
            .list_comments(1, video_id, 0, 10)
            .await
            .expect("list comments");
        assert!(total >= 1);
        assert!(comments.iter().any(|c| c.id == comment.id));

        let pool = state.repos.video.pool();
        cleanup_test_comments(pool, video_id).await;
        cleanup_test_video(pool, video_id).await;
        cleanup_test_user(pool, &username).await;
    }
}
