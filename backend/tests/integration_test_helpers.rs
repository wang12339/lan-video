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
use atmos_video_backend::services::transcoder::Transcoder;
use atmos_video_backend::services::video_service::VideoService;
use atmos_video_backend::state::{
    AppState, RecommendationCache, RepoLayer, ServiceLayer, VideoListCache,
};
use atmos_video_backend::util::password;
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
        smtp_host: String::new(),
        smtp_port: 587,
        smtp_username: String::new(),
        smtp_password: String::new(),
        smtp_from: String::new(),
        redis_url: String::new(),
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
        atmos_video_backend::repositories::comment_repo::CommentRepository::new(pool.clone());
    let share_repo =
        atmos_video_backend::repositories::share_repo::ShareRepository::new(pool.clone());
    let tag_repo = TagRepository::new(pool.clone());
    let tenant_repo =
        atmos_video_backend::repositories::tenant_repo::TenantRepository::new(pool.clone());
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

    let transcoder = Transcoder::new(&config.media_root);
    let task_queue = TaskQueue::new(transcoder.clone(), pool.clone());

    Arc::new(AppState {
        repos: RepoLayer {
            registration: registration_repo,
            user: user_repo.clone(),
            video: video_repo,
            playback: playback_repo,
            playlist: playlist_repo,
            comment: comment_repo,
            share: share_repo,
            tag: tag_repo,
            tenant: tenant_repo,
        },
        services: ServiceLayer {
            video: video_service,
            media: media_service,
            playback: playback_service.clone(),
            auth: AuthService::new(
                user_repo,
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
        .create_comment(video_id, user_id, content, parent_id, false)
        .await
        .expect("create test comment")
}

/// Format an `Authorization: Bearer <token>` header value for HTTP-level tests.
pub fn auth_header_value(token: &str) -> String {
    format!("Bearer {}", token)
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

        let video_id = create_test_video(&state, "fixture").await;
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
            .list_comments(video_id, 0, 10)
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
