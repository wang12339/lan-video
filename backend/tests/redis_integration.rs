#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Redis-backed integration tests: rate limiter persistence and media-auth cache.
//!
//! Gated on `REDIS_URL`; the media-auth tests additionally need a real database
//! (`DATABASE_URL`) because a real user/token is required to exercise the
//! cache-write path. When the variables are absent the tests skip silently,
//! matching the `DATABASE_URL` gating used by the other integration suites.
//!
//! Run:
//! ```text
//! redis-server --port 6399 --save '' --appendonly no --daemonize yes \
//!     --pidfile /tmp/atmos_redis_test.pid
//! REDIS_URL=redis://127.0.0.1:6399 DATABASE_URL=postgres://user@localhost:5432/atmos_redis \
//!     cargo test --test redis_integration
//! redis-cli -p 6399 shutdown nosave
//! ```

mod integration_test_helpers;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use atmos_video_backend::middleware::auth::{
    invalidate_media_auth_token, invalidate_media_auth_user, media_auth,
};
use atmos_video_backend::middleware::rate_limit::RateLimiter;
use atmos_video_backend::state::AppState;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::routing::get;
use axum::Router;
use integration_test_helpers::{
    cleanup_test_user, create_test_user_with_credentials, database_url, login_and_get_token,
    test_app_state_with_config, test_config, unique_username, TEST_USER_PASSWORD,
};
use redis::aio::ConnectionManager;
use tower::ServiceExt;

/// Returns `REDIS_URL` from the environment, or skips the test if not set.
fn redis_url() -> Option<String> {
    static WARNED: std::sync::Once = std::sync::Once::new();
    match std::env::var("REDIS_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            WARNED.call_once(|| {
                eprintln!(
                    "NOTE: REDIS_URL is not set — Redis integration tests are SKIPPED. `cargo test` green does NOT mean the Redis suite passed."
                );
            });
            None
        }
    }
}

/// Media-auth tests need both a Redis and a real DB (user + auth token).
fn redis_and_db() -> Option<(String, String)> {
    let url = redis_url()?;
    let db = database_url()?;
    Some((url, db))
}

async fn redis_manager(url: &str) -> ConnectionManager {
    let client = redis::Client::open(url).expect("REDIS_URL must be a valid Redis URL");
    ConnectionManager::new(client)
        .await
        .expect("test Redis must be reachable (redis-server --port 6399)")
}

/// Process-unique key so parallel tests / binaries cannot collide in the
/// shared test Redis.
fn unique_key(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("test:{prefix}:{}:{n}", std::process::id())
}

/// A syntactically valid (64 alphanumeric chars) auth token.
fn unique_valid_token(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x<64}", format!("{prefix}{}{n}", std::process::id()))
}

async fn redis_exists(conn: &mut ConnectionManager, key: &str) -> bool {
    let n: i64 = redis::cmd("EXISTS")
        .arg(key)
        .query_async(conn)
        .await
        .expect("EXISTS");
    n == 1
}

async fn redis_ttl(conn: &mut ConnectionManager, key: &str) -> i64 {
    redis::cmd("TTL")
        .arg(key)
        .query_async(conn)
        .await
        .expect("TTL")
}

async fn redis_get_str(conn: &mut ConnectionManager, key: &str) -> Option<String> {
    redis::cmd("GET")
        .arg(key)
        .query_async(conn)
        .await
        .expect("GET")
}

async fn redis_sismember(conn: &mut ConnectionManager, key: &str, member: &str) -> bool {
    let n: i64 = redis::cmd("SISMEMBER")
        .arg(key)
        .arg(member)
        .query_async(conn)
        .await
        .expect("SISMEMBER");
    n == 1
}

// ── Rate limiter Redis semantics ──

/// Two independent limiter instances share the Redis counter: quota consumed
/// through A is visible to B, and the block applied by A blocks B too.
#[tokio::test]
async fn redis_rate_limiter_instances_share_counter_and_block() {
    let Some(url) = redis_url() else {
        return;
    };
    let mgr = redis_manager(&url).await;
    let a = RateLimiter::with_redis(mgr.clone());
    let b = RateLimiter::with_redis(mgr.clone());
    let key = unique_key("rl_share");
    let counter_key = format!("rl:c:{key}");
    let block_key = format!("rl:b:{key}");
    let mut conn = mgr.clone();

    // max_attempts = 3: exactly two calls may succeed across BOTH instances.
    assert!(a.check_with(&key, 3, 60, 600).await.is_ok(), "A #1");
    assert!(b.check_with(&key, 3, 60, 600).await.is_ok(), "B #2");
    assert!(
        a.check_with(&key, 3, 60, 600).await.is_err(),
        "A #3 must block"
    );
    assert!(
        b.check_with(&key, 3, 60, 600).await.is_err(),
        "block must be shared with B"
    );

    assert!(
        redis_exists(&mut conn, &block_key).await,
        "block key written"
    );
    assert!(
        !redis_exists(&mut conn, &counter_key).await,
        "counter is cleared when the block is applied"
    );
    let ttl = redis_ttl(&mut conn, &block_key).await;
    assert!(
        (1..=600).contains(&ttl),
        "block key TTL must be within the 600s window, got {ttl}"
    );

    // reset() clears both Redis keys and restores a fresh budget on B.
    a.reset(&key).await;
    assert!(!redis_exists(&mut conn, &block_key).await);
    assert!(!redis_exists(&mut conn, &counter_key).await);
    assert!(
        b.check_with(&key, 3, 60, 600).await.is_ok(),
        "fresh budget after reset"
    );
    a.reset(&key).await;
}

/// Block expiry and `block_secs = 0` behave like the in-memory backend.
#[tokio::test]
async fn redis_rate_limiter_block_expiry_and_zero_block() {
    let Some(url) = redis_url() else {
        return;
    };
    let mgr = redis_manager(&url).await;
    let limiter = RateLimiter::with_redis(mgr.clone());
    let mut conn = mgr.clone();

    let key = unique_key("rl_expire");
    let block_key = format!("rl:b:{key}");
    assert!(limiter.check_with(&key, 2, 30, 1).await.is_ok());
    assert!(limiter.check_with(&key, 2, 30, 1).await.is_err());
    assert!(redis_exists(&mut conn, &block_key).await);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        !redis_exists(&mut conn, &block_key).await,
        "1s block key must expire"
    );
    assert!(
        limiter.check_with(&key, 2, 30, 1).await.is_ok(),
        "after the block expires a fresh budget is granted"
    );

    // block_secs = 0 => no block key, immediate recovery (same as memory path).
    let key0 = unique_key("rl_noblock");
    let block_key0 = format!("rl:b:{key0}");
    assert!(limiter.check_with(&key0, 2, 30, 0).await.is_ok());
    assert!(limiter.check_with(&key0, 2, 30, 0).await.is_err());
    assert!(!redis_exists(&mut conn, &block_key0).await);
    assert!(
        limiter.check_with(&key0, 2, 30, 0).await.is_ok(),
        "zero-length block recovers immediately"
    );

    limiter.reset(&key).await;
    limiter.reset(&key0).await;
}

/// The Lua script is atomic: concurrent calls spread over two instances may
/// never allow more than max_attempts - 1 successes.
#[tokio::test]
async fn redis_rate_limiter_concurrent_calls_are_atomic() {
    let Some(url) = redis_url() else {
        return;
    };
    let mgr = redis_manager(&url).await;
    let a = RateLimiter::with_redis(mgr.clone());
    let b = RateLimiter::with_redis(mgr.clone());
    let key = unique_key("rl_conc");

    let mut tasks = Vec::new();
    for i in 0..8 {
        let limiter = if i % 2 == 0 { a.clone() } else { b.clone() };
        let key = key.clone();
        tasks.push(tokio::spawn(async move {
            limiter.check_with(&key, 3, 60, 600).await.is_ok()
        }));
    }
    let mut allowed = 0;
    for task in tasks {
        if task.await.expect("join") {
            allowed += 1;
        }
    }
    assert_eq!(
        allowed, 2,
        "exactly max_attempts-1 concurrent calls may succeed, got {allowed}"
    );
    a.reset(&key).await;
}

// ── services::redis::init_redis ──

/// `init_redis` connects and reuses the shared manager; `get_redis` exposes it.
#[tokio::test]
async fn redis_init_connects_and_reuses_manager() {
    let Some(url) = redis_url() else {
        return;
    };
    let cm = atmos_video_backend::services::redis::init_redis(&url)
        .await
        .expect("init_redis must connect to the test Redis");
    let again = atmos_video_backend::services::redis::init_redis(&url)
        .await
        .expect("second init_redis must return the shared manager");

    let mut conn = (*cm).clone();
    let pong: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .expect("PING via init_redis manager");
    assert_eq!(pong, "PONG");

    let mut conn2 = (*again).clone();
    let pong2: String = redis::cmd("PING")
        .query_async(&mut conn2)
        .await
        .expect("PING via reused manager");
    assert_eq!(pong2, "PONG");
    assert!(
        atmos_video_backend::services::redis::get_redis()
            .await
            .is_some(),
        "get_redis must return the shared manager"
    );
}

/// A blank URL disables Redis without touching the shared cell.
#[tokio::test]
async fn redis_init_returns_none_for_blank_url() {
    assert!(
        atmos_video_backend::services::redis::init_redis("   ")
            .await
            .is_none(),
        "blank REDIS_URL must disable Redis"
    );
}

// ── Media auth cache (media.rs) ──

/// AppState as the production app wires it, but with `redis` pointing at the
/// test server. Repos/services come from the shared DB test helpers.
async fn state_with_redis(url: &str) -> Arc<AppState> {
    let mut config = test_config();
    config.redis_url = url.to_string();
    let base = test_app_state_with_config(config).await;
    let mut inner = (*base).clone();
    inner.redis = Some(redis_manager(url).await);
    Arc::new(inner)
}

async fn media_probe() -> &'static str {
    "ok"
}

/// Router with `media_auth` around a trivial handler; the state is injected
/// into request extensions the same way `app::build_router` does.
fn media_probe_router(state: Arc<AppState>, path: &str) -> Router {
    let inject = {
        let state = state.clone();
        from_fn(move |mut req: Request, next: Next| {
            let state = state.clone();
            async move {
                req.extensions_mut().insert(state);
                next.run(req).await
            }
        })
    };
    Router::new()
        .route(path, get(media_probe))
        .layer(from_fn(media_auth))
        .layer(inject)
}

async fn get_with_bearer(app: &Router, path: &str, token: &str) -> StatusCode {
    let req = axum::http::Request::builder()
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("build media request");
    app.clone()
        .oneshot(req)
        .await
        .expect("send media request")
        .status()
}

/// A real media request populates `media:auth:{token}` + the
/// `media:auth:user:{uid}` set with a ~10s TTL; token invalidation deletes the
/// token key, user invalidation deletes every token key and the set.
#[tokio::test]
async fn media_auth_redis_cache_write_and_invalidation() {
    let Some((url, _db)) = redis_and_db() else {
        return;
    };
    let state = state_with_redis(&url).await;
    let (username, _password, user_id, token) =
        create_test_user_with_credentials(&state, "redis_media").await;

    let path = "/media/avatars/redis_probe.jpg";
    let app = media_probe_router(state.clone(), path);
    assert_eq!(
        get_with_bearer(&app, path, &token).await,
        StatusCode::OK,
        "public avatar path must pass media_auth for a valid token"
    );

    let mut conn = state.redis.as_ref().expect("redis wired").clone();
    let token_key = format!("media:auth:{token}");
    let set_key = format!("media:auth:user:{user_id}");

    assert_eq!(
        redis_get_str(&mut conn, &token_key).await.as_deref(),
        Some(format!("{user_id}|1|{username}").as_str()),
        "cached value must encode user_id|is_admin|username"
    );
    let token_ttl = redis_ttl(&mut conn, &token_key).await;
    assert!(
        (1..=10).contains(&token_ttl),
        "token cache TTL must be <= 10s, got {token_ttl}"
    );
    assert!(
        redis_sismember(&mut conn, &set_key, &token).await,
        "token must be registered in the per-user set"
    );
    let set_ttl = redis_ttl(&mut conn, &set_key).await;
    assert!(
        (1..=10).contains(&set_ttl),
        "user set TTL must be <= 10s, got {set_ttl}"
    );

    // Logout-style invalidation deletes the token key.
    invalidate_media_auth_token(&state, &token).await;
    assert!(
        !redis_exists(&mut conn, &token_key).await,
        "invalidate_media_auth_token must DEL media:auth:{{token}}"
    );

    // User-wide invalidation removes the set (and any remaining token keys).
    invalidate_media_auth_user(&state, user_id).await;
    assert!(
        !redis_exists(&mut conn, &set_key).await,
        "invalidate_media_auth_user must DEL media:auth:user:{{uid}}"
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

/// A cache entry seeded directly in Redis authenticates without any DB row,
/// proving `media_auth_cache_get_redis` parses hits correctly.
#[tokio::test]
async fn media_auth_reads_cached_user_from_redis() {
    let Some((url, _db)) = redis_and_db() else {
        return;
    };
    let state = state_with_redis(&url).await;
    let path = "/media/avatars/redis_probe.jpg";
    let app = media_probe_router(state.clone(), path);

    let token = unique_valid_token("r");
    let mut conn = state.redis.as_ref().expect("redis wired").clone();
    let token_key = format!("media:auth:{token}");
    redis::cmd("SETEX")
        .arg(&token_key)
        .arg(10)
        .arg("424242|0|redis_only_user")
        .query_async::<()>(&mut conn)
        .await
        .expect("seed media auth cache");

    assert_eq!(
        get_with_bearer(&app, path, &token).await,
        StatusCode::OK,
        "a Redis cache hit must authenticate even without a DB user (otherwise this would be 401)"
    );

    let _: i64 = redis::cmd("DEL")
        .arg(&token_key)
        .query_async(&mut conn)
        .await
        .expect("cleanup seeded key");
}

/// `invalidate_media_auth_user` enumerates the per-user set and deletes every
/// token key plus the set itself.
#[tokio::test]
async fn media_auth_user_invalidation_deletes_all_tokens_and_set() {
    let Some((url, _db)) = redis_and_db() else {
        return;
    };
    let state = state_with_redis(&url).await;
    let (username, _password, user_id, token1) =
        create_test_user_with_credentials(&state, "redis_media_user").await;
    let token2 = login_and_get_token(&state, &username, TEST_USER_PASSWORD).await;
    assert_ne!(token1, token2, "two logins must issue distinct tokens");

    let path = "/media/avatars/redis_probe.jpg";
    let app = media_probe_router(state.clone(), path);
    assert_eq!(get_with_bearer(&app, path, &token1).await, StatusCode::OK);
    assert_eq!(get_with_bearer(&app, path, &token2).await, StatusCode::OK);

    let mut conn = state.redis.as_ref().expect("redis wired").clone();
    let key1 = format!("media:auth:{token1}");
    let key2 = format!("media:auth:{token2}");
    let set_key = format!("media:auth:user:{user_id}");
    assert!(redis_exists(&mut conn, &key1).await);
    assert!(redis_exists(&mut conn, &key2).await);
    assert!(redis_sismember(&mut conn, &set_key, &token1).await);
    assert!(redis_sismember(&mut conn, &set_key, &token2).await);

    invalidate_media_auth_user(&state, user_id).await;

    assert!(!redis_exists(&mut conn, &key1).await, "token1 key deleted");
    assert!(!redis_exists(&mut conn, &key2).await, "token2 key deleted");
    assert!(!redis_exists(&mut conn, &set_key).await, "user set deleted");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

/// End-to-end through the production router: `config.redis_url` is wired into
/// `AppState.redis`, so a real login + media request populates the shared cache.
#[tokio::test]
async fn full_router_wires_redis_into_media_auth_cache() {
    let Some((url, _db)) = redis_and_db() else {
        return;
    };

    let mut config = test_config();
    config.redis_url = url.clone();
    let app = atmos_video_backend::app::build_router(config)
        .await
        .layer(MockConnectInfo(
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        ));

    let pool = integration_test_helpers::test_pool().await;
    let username = unique_username("fullredis");
    let hash = atmos_video_backend::util::password::hash(TEST_USER_PASSWORD).expect("hash");
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, approved, role, is_guest) VALUES ($1, $2, true, 3, false) RETURNING id",
    )
    .bind(&username)
    .bind(&hash)
    .fetch_one(&pool)
    .await
    .expect("insert test user");

    // Login through the real router (also exercises the Redis rate limiter).
    let login_body = serde_json::json!({
        "username": &username,
        "password": TEST_USER_PASSWORD,
    });
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(login_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "login must succeed");
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = json["token"]
        .as_str()
        .expect("login must return a token")
        .to_string();

    // The media file itself may 404 in the test environment (ServeDir root is
    // fake); the auth middleware still runs first and populates the cache.
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/media/avatars/redis_probe.jpg")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "valid bearer token must authenticate through the production router"
    );

    let mgr = redis_manager(&url).await;
    let mut conn = mgr.clone();
    let key = format!("media:auth:{token}");
    assert_eq!(
        redis_get_str(&mut conn, &key).await.as_deref(),
        Some(format!("{user_id}|1|{username}").as_str()),
        "production router must write the media auth cache to Redis"
    );

    let _: i64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .expect("cleanup cache key");
    cleanup_test_user(&pool, &username).await;
}
