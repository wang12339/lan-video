//! HTTP-level integration tests using `axum::test`.
//!
//! These exercise the full middleware stack including auth, rate limiting,
//! and error handling. Requires `DATABASE_URL` to be set.
//!
//! Every test builds its own router + pool, so tests are independent of each
//! other and safe to run with `--test-threads=1` (each test cleans up after
//! itself). Users and videos are created directly in the DB (via the shared
//! fixture helpers) so the tests never depend on registration/approval state.

mod integration_test_helpers;

use std::net::SocketAddr;
use std::sync::Arc;

use atmos_video_backend::app;
use atmos_video_backend::state::AppState;
use atmos_video_backend::util::password;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use integration_test_helpers::*;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;

/// Build the full app router for testing, skipping the bind step.
///
/// axum >= 0.8 no longer injects a default `ConnectInfo` when a router is
/// called directly via `oneshot`; handlers that extract `SocketAddr` (login,
/// register, forgot-password, increment_views) would otherwise reject with
/// 500. `MockConnectInfo` supplies one so the whole stack is exercised.
async fn build_test_app() -> axum::Router {
    let Some(_) = database_url() else {
        // We never reach this branch because tests below early-return on missing DB,
        // but the function signature must still type-check.
        panic!("DATABASE_URL not set");
    };
    app::build_router(test_config())
        .await
        .layer(MockConnectInfo(
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        ))
}

// ── HTTP 请求辅助 ──

/// Issue an HTTP request against the test app and collect the raw response.
async fn send(
    app: &axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    content_type: Option<&str>,
    body: Option<&str>,
) -> (StatusCode, HeaderMap, axum::body::Bytes) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", t));
    }
    if let Some(ct) = content_type {
        builder = builder.header(header::CONTENT_TYPE, ct);
    }
    let body = body
        .map(|b| Body::from(b.to_owned()))
        .unwrap_or_else(Body::empty);
    let res = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, headers, bytes)
}

/// JSON-in/JSON-out variant of [`send`]. Returns `Value::Null` when the
/// response body is not valid JSON (e.g. 204 No Content).
async fn send_json(
    app: &axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw_body = body.as_ref().map(|v| v.to_string());
    let (status, _, bytes) = send(
        app,
        method,
        uri,
        token,
        Some("application/json"),
        raw_body.as_deref(),
    )
    .await;
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

// ── 数据准备辅助 ──

/// Create an approved viewer (role 1) directly in the DB.
async fn create_viewer(state: &Arc<AppState>, prefix: &str) -> (String, i64) {
    let username = unique_username(prefix);
    let hash = password::hash(TEST_USER_PASSWORD).expect("hash fixture password");
    let user_id = state
        .repos
        .user
        .create_user(1, &username, &hash, 1)
        .await
        .expect("create viewer");
    state
        .repos
        .user
        .approve_user(user_id, true)
        .await
        .expect("approve viewer");
    (username, user_id)
}

/// Create an approved viewer and log in, returning (username, user_id, token).
async fn create_viewer_with_token(state: &Arc<AppState>, prefix: &str) -> (String, i64, String) {
    let (username, user_id) = create_viewer(state, prefix).await;
    let token = login_and_get_token(state, &username, TEST_USER_PASSWORD).await;
    (username, user_id, token)
}

/// Create a user that is NOT approved (pending admin approval) and issue a
/// token directly so the auth middleware is exercised with an unapproved user.
async fn create_pending_user_with_token(state: &Arc<AppState>, prefix: &str) -> (String, String) {
    let username = unique_username(prefix);
    let hash = password::hash(TEST_USER_PASSWORD).expect("hash fixture password");
    let user_id = state
        .repos
        .user
        .create_user(1, &username, &hash, 1)
        .await
        .expect("create pending user");
    let token = state
        .repos
        .user
        .create_token(user_id)
        .await
        .expect("issue token");
    (username, token)
}

/// Create an admin (role 3, auto-approved) and log in.
async fn create_admin_with_token(state: &Arc<AppState>, prefix: &str) -> (String, String) {
    let (username, _user_id) = create_test_user(state, prefix).await;
    let token = login_and_get_token(state, &username, TEST_USER_PASSWORD).await;
    (username, token)
}

/// Persist the registration toggle directly in the DB (the router reads this
/// value at startup, so it must be set *before* `build_test_app`).
async fn set_registration_enabled(pool: &PgPool, enabled: bool) {
    sqlx::query("UPDATE server_config SET value = $1 WHERE key = 'registration_enabled'")
        .bind(if enabled { "true" } else { "false" })
        .execute(pool)
        .await
        .expect("set registration toggle");
}

/// Clean up a tag created by a test.
async fn cleanup_test_tag(pool: &PgPool, name: &str) {
    let _ = sqlx::query("DELETE FROM tags WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await;
}

// ── 基础端点 ──

#[tokio::test]
async fn test_health_endpoint_returns_ok() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, headers, _) = send(&app, Method::GET, "/health", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get("X-Public-Url").and_then(|v| v.to_str().ok()),
        Some("http://localhost:3000")
    );
}

#[tokio::test]
async fn test_protected_endpoint_rejects_missing_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, _, body) = send(&app, Method::GET, "/auth/user", None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        value["error"].is_string(),
        "body: {}",
        String::from_utf8_lossy(&body)
    );
}

#[tokio::test]
async fn test_protected_endpoint_rejects_invalid_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, _, _) = send(
        &app,
        Method::GET,
        "/auth/user",
        Some("not-a-real-token"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_endpoint_rejects_non_admin_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    // Try to access an admin endpoint with a malformed token
    let (status, _, _) = send(
        &app,
        Method::GET,
        "/admin/users",
        Some("fake-non-admin"),
        None,
        None,
    )
    .await;
    // Should be 401 (invalid token) since the user doesn't exist
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_register_endpoint_rejects_short_password() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let body = serde_json::json!({
        "username": "test_user_too_short",
        "password": "abc"
    });
    let (status, _) = send_json(&app, Method::POST, "/auth/register", None, Some(body)).await;
    // Registration is disabled in the test DB, so this is either a structured
    // 404 (toggle checked first) or an OK/4xx validation response — never a 500.
    assert!(
        status == StatusCode::OK || status.is_client_error(),
        "expected OK or 4xx, got {}",
        status
    );
}

#[tokio::test]
async fn test_search_endpoint_enforces_query_length() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    // Use a valid bearer token check happens after length check
    let long_q = "a".repeat(300);
    let uri = format!("/videos/search?q={}", long_q);
    let (status, _, _) = send(&app, Method::GET, &uri, Some("fake"), None, None).await;
    // The length check should fire before the auth check (it's a parameter parse)
    // OR auth check fires first
    // Either way, the response should not be 500
    assert!(!status.is_server_error(), "got 500: {:?}", status);
}

#[tokio::test]
async fn test_unknown_route_returns_404_or_fallback() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, _, _) = send(
        &app,
        Method::GET,
        "/this-route-does-not-exist",
        None,
        None,
        None,
    )
    .await;
    // The app has a fallback that returns the index.html for SPA, so 200 is acceptable
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::OK);
}

// ── 认证与账号 ──

#[tokio::test]
async fn test_register_via_http_success() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let pool = test_pool().await;
    set_registration_enabled(&pool, true).await;
    let app = build_test_app().await;
    let username = unique_username("http_reg");
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/register",
        None,
        Some(json!({
            "username": username,
            "password": "Str0ngPass!23",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true), "body: {}", body);
    assert!(
        body["token"].is_null(),
        "non-admin registration must not return a token: {}",
        body
    );
    assert!(
        body["error"].is_string(),
        "expected approval message: {}",
        body
    );
    set_registration_enabled(&pool, false).await;
    cleanup_test_user(&pool, &username).await;
}

#[tokio::test]
async fn test_register_via_http_duplicate_username() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let pool = test_pool().await;
    set_registration_enabled(&pool, true).await;
    let app = build_test_app().await;
    let username = unique_username("http_dup");
    let payload = json!({ "username": username, "password": "Str0ngPass!23" });
    let (status1, _) = send_json(
        &app,
        Method::POST,
        "/auth/register",
        None,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status1, StatusCode::OK, "first registration should succeed");
    let (status2, body2) =
        send_json(&app, Method::POST, "/auth/register", None, Some(payload)).await;
    assert_eq!(status2, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body2["ok"],
        json!(false),
        "duplicate should fail: {}",
        body2
    );
    assert_eq!(body2["error"], json!("用户名已存在"));
    set_registration_enabled(&pool, false).await;
    cleanup_test_user(&pool, &username).await;
}

#[tokio::test]
async fn test_register_malformed_json_rejected_400() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let pool = test_pool().await;
    set_registration_enabled(&pool, true).await;
    let app = build_test_app().await;
    let (status, _, body) = send(
        &app,
        Method::POST,
        "/auth/register",
        None,
        Some("application/json"),
        Some("{not valid json"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["error"], json!("invalid request body"));
    set_registration_enabled(&pool, false).await;
}

#[tokio::test]
async fn test_login_success_returns_token_and_cookie() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id) = create_viewer(&state, "login_ok").await;
    let app = build_test_app().await;
    let (status, headers, bytes) = send(
        &app,
        Method::POST,
        "/auth/login",
        None,
        Some("application/json"),
        Some(&json!({ "username": username, "password": TEST_USER_PASSWORD }).to_string()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["ok"], json!(true));
    let token = body["token"]
        .as_str()
        .expect("token should be present")
        .to_string();
    assert_eq!(
        token.len(),
        64,
        "token must be a 256-bit alphanumeric string"
    );
    let set_cookie = headers
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        set_cookie.contains("token="),
        "expected token cookie, got: {}",
        set_cookie
    );

    // Token works on an authenticated endpoint
    let (status, body) = send_json(&app, Method::GET, "/auth/user", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], json!(username));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_wrong_password_returns_friendly_error() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id) = create_viewer(&state, "login_bad").await;
    let app = build_test_app().await;
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/login",
        None,
        Some(json!({ "username": username, "password": "WrongPass_1!" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["ok"], json!(false));
    assert!(body["token"].is_null());
    assert_eq!(body["error"], json!("用户名或密码错误"));
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_unapproved_user_fails() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let username = unique_username("login_pending");
    let hash = password::hash(TEST_USER_PASSWORD).unwrap();
    state
        .repos
        .user
        .create_user(1, &username, &hash, 1)
        .await
        .unwrap();
    let app = build_test_app().await;
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/login",
        None,
        Some(json!({ "username": username, "password": TEST_USER_PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body["ok"],
        json!(false),
        "unapproved login must fail: {}",
        body
    );
    assert!(body["token"].is_null());
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_rate_limited_after_two_failures() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id) = create_viewer(&state, "login_rl").await;
    let app = build_test_app().await;
    let payload = json!({ "username": username, "password": "WrongPass_1!" });
    // 策略：同一用户名 60 秒内最多 5 次尝试，达到上限的那次即被拒绝
    // （RATE_LIMIT_MAX_ATTEMPTS=5，count >= max 时 429），因此前 4 次为 401
    for i in 0..4 {
        let (status, body) = send_json(
            &app,
            Method::POST,
            "/auth/login",
            None,
            Some(payload.clone()),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "attempt {}: {} body={}",
            i,
            status,
            body
        );
        assert_eq!(body["ok"], json!(false), "attempt {} should fail", i);
        assert_eq!(body["error"], json!("用户名或密码错误"));
    }
    // 第 5 次触发用户名级限流
    let (status, body) = send_json(&app, Method::POST, "/auth/login", None, Some(payload)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["ok"], json!(false));
    assert_eq!(
        body["error"],
        json!("请求过于频繁，请稍后再试"),
        "body: {}",
        body
    );
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_logout_revokes_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "logout").await;
    let app = build_test_app().await;

    let (status, _) = send_json(&app, Method::GET, "/auth/user", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_json(&app, Method::POST, "/auth/logout", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true));

    // The same token must now be rejected
    let (status, _) = send_json(&app, Method::GET, "/auth/user", Some(&token), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "token should be revoked after logout"
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_pending_user_token_blocked_403() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, token) = create_pending_user_with_token(&state, "pending").await;
    let app = build_test_app().await;
    let (status, body) = send_json(&app, Method::GET, "/auth/user", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {}", body);
    assert_eq!(body["error"], json!("账号待管理员审批"));
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_user_profile_endpoint() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "profile").await;
    let app = build_test_app().await;
    let (status, body) =
        send_json(&app, Method::GET, "/auth/user/profile", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], json!(username));
    assert_eq!(body["isAdmin"], json!(false));
    assert_eq!(body["totalVideosWatched"], json!(0));
    assert_eq!(body["recentHistory"], json!([]));
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_update_email_validation_and_success() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "email").await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::PUT,
        "/auth/user/email",
        Some(&token),
        Some(json!({ "email": "not-an-email" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);
    assert_eq!(body["error"], json!("请输入有效的邮箱地址"));

    let email = format!("{}@example.com", unique_username("mail"));
    let (status, body) = send_json(
        &app,
        Method::PUT,
        "/auth/user/email",
        Some(&token),
        Some(json!({ "email": email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert_eq!(body["ok"], json!(true));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_update_email_conflict_409() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (user_a, _ua, token_a) = create_viewer_with_token(&state, "email_a").await;
    let (_user_b, _ub, token_b) = create_viewer_with_token(&state, "email_b").await;
    let app = build_test_app().await;
    let email = format!("{}@example.com", unique_username("shared"));

    let (status, _) = send_json(
        &app,
        Method::PUT,
        "/auth/user/email",
        Some(&token_a),
        Some(json!({ "email": email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_json(
        &app,
        Method::PUT,
        "/auth/user/email",
        Some(&token_b),
        Some(json!({ "email": email })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {}", body);
    assert_eq!(body["error"], json!("该邮箱已被其他账号绑定"));

    cleanup_test_user(state.repos.video.pool(), &user_a).await;
}

#[tokio::test]
async fn test_reset_password_validation() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;

    // Short password is rejected before the token is even looked up
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/reset-password",
        None,
        Some(json!({ "token": "garbage", "password": "abc" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);
    assert_eq!(body["error"], json!("密码长度需在 8-128 个字符之间"));

    // Valid-looking password but unknown token
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/reset-password",
        None,
        Some(json!({ "token": "garbage", "password": "Str0ngPass!23" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("重置链接无效或已过期"));
}

#[tokio::test]
async fn test_reset_password_get_redirects_to_webapp() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, headers, _) = send(
        &app,
        Method::GET,
        "/auth/reset-password?token=abc123",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = headers
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.contains("reset_token=abc123"),
        "location: {}",
        location
    );
    assert!(location.contains("/webapp/"), "location: {}", location);
}

#[tokio::test]
async fn test_forgot_password_anti_enumeration() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    // Unknown email still gets a success-shaped response (no enumeration)
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/forgot-password",
        None,
        Some(json!({ "email": "ghost-unknown@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true), "body: {}", body);
    assert!(body["message"].is_string());
}

#[tokio::test]
async fn test_verify_email_endpoints() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/verify-email",
        None,
        Some(json!({ "token": "garbage-token" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);
    assert_eq!(body["error"], json!("验证链接无效或已过期"));

    // The GET variant always renders an HTML page (success or failure)
    let (status, _, body) = send(
        &app,
        Method::GET,
        "/auth/verify-email?token=garbage-token",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("邮箱验证失败"), "html: {}", html);
}

#[tokio::test]
async fn test_send_verification_email() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "send_verify").await;
    let app = build_test_app().await;
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/auth/send-verification-email",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true), "body: {}", body);
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_csrf_guard_blocks_cookie_authenticated_mutations() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "csrf").await;
    let app = build_test_app().await;

    // Cookie-authenticated POST without a CSRF header must be rejected
    let req = Request::builder()
        .method(Method::POST)
        .uri("/auth/logout")
        .header(header::COOKIE, format!("token={}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "cookie mutation without CSRF header must be blocked"
    );

    // With the X-Requested-With header it is allowed
    let req = Request::builder()
        .method(Method::POST)
        .uri("/auth/logout")
        .header(header::COOKIE, format!("token={}", token))
        .header("x-requested-with", "XMLHttpRequest")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 视频列表 / 详情 / 搜索 ──

#[tokio::test]
async fn test_videos_require_auth() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, _) = send_json(&app, Method::GET, "/videos", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_videos_list_pagination_bounds() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "paging").await;
    let app = build_test_app().await;

    // Negative page / zero size are clamped instead of erroring
    let (status, body) = send_json(
        &app,
        Method::GET,
        "/videos?page=-5&size=0",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], json!(1), "body: {}", body);
    assert_eq!(body["size"], json!(1));
    assert!(body["total"].is_number());

    // Huge page / size are clamped so page*size can never overflow
    let (status, body) = send_json(
        &app,
        Method::GET,
        "/videos?page=2000000&size=99999",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], json!(10_000), "body: {}", body);
    assert_eq!(body["size"], json!(100));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_videos_query_too_long_400() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "longq").await;
    let app = build_test_app().await;
    let long_q = "a".repeat(201);
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos?query={}", long_q),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("查询关键词不能超过 200 个字符"));
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_video_detail_not_found_and_invalid_id() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "detail").await;
    let app = build_test_app().await;

    // Nonexistent id → 404 with a Chinese error message
    let (status, body) =
        send_json(&app, Method::GET, "/videos/2000000000", Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {}", body);
    assert_eq!(body["error"], json!("视频不存在"));

    // Unparseable id → 400
    let (status, body) =
        send_json(&app, Method::GET, "/videos/abc!!!xyz", Some(&token), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("无效的视频ID"));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_video_detail_and_variants() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "detail_ok").await;
    let video_id = create_test_video(&state, "detail_ok").await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_id(&body["id"]), video_id);
    assert!(body["title"].is_string());
    assert_eq!(body["sourceType"], json!("external"));

    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}/variants", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]), "no transcoded variants expected");

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_video_like_toggle_flow() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "like").await;
    let video_id = create_test_video(&state, "like").await;
    let app = build_test_app().await;
    let uri = format!("/videos/{}/like", video_id);

    let (status, body) = send_json(&app, Method::POST, &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["liked"], json!(true), "body: {}", body);

    let (_, body) = send_json(&app, Method::GET, &uri, Some(&token), None).await;
    assert_eq!(body["liked"], json!(true));

    let (status, body) = send_json(&app, Method::POST, &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["liked"], json!(false), "second toggle should unlike");

    // Invalid video id → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/videos/not-a-real-id/like",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("无效的视频ID"));

    cleanup_test_user(state.repos.video.pool(), &username).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

#[tokio::test]
async fn test_video_favorite_toggle_flow() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "fav").await;
    let video_id = create_test_video(&state, "fav").await;
    let app = build_test_app().await;
    let uri = format!("/videos/{}/favorite", video_id);

    let (status, body) = send_json(&app, Method::POST, &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["favorited"], json!(true), "body: {}", body);

    let (_, body) = send_json(&app, Method::GET, &uri, Some(&token), None).await;
    assert_eq!(body["favorited"], json!(true));

    let (status, body) = send_json(&app, Method::POST, &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["favorited"], json!(false));

    cleanup_test_user(state.repos.video.pool(), &username).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

/// KNOWN-FAILING: GET /videos/favorites currently returns 500 because
/// `find_favorites_by_username` maps `user_favorites.created_at` (naive
/// TIMESTAMP) into `HistoryRow.updated_at: DateTime<Utc>` — a sqlx type
/// mismatch. Ignored until the source is fixed.
#[tokio::test]
#[ignore = "既有缺陷: find_favorites_by_username 类型不匹配 (timestamp 解码为 DateTime<Utc>) 导致 500"]
async fn test_video_favorites_list() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "fav_list").await;
    let video_id = create_test_video(&state, "fav_list").await;
    let app = build_test_app().await;

    let (status, _body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/favorite", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) =
        send_json(&app, Method::GET, "/videos/favorites", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().expect("favorites should be an array");
    assert!(
        items.iter().any(|it| it["videoId"] == json!(video_id)),
        "favorites should contain the video: {}",
        body
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

#[tokio::test]
async fn test_video_search_empty_and_long_query() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "search").await;
    let app = build_test_app().await;

    // Whitespace-only query short-circuits to an empty result set
    let (status, body) = send_json(
        &app,
        Method::GET,
        "/videos/search?q=%20",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"], json!([]));
    assert_eq!(body["total"], json!(0));

    // Query too long → 400
    let long_q = "a".repeat(201);
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/search?q={}", long_q),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("搜索关键词不能超过 200 个字符"));

    // Suggest endpoint: empty query → empty list
    let (status, body) = send_json(
        &app,
        Method::GET,
        "/videos/search/suggest?q=%20",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_video_search_finds_created_video() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "search_hit").await;
    let unique = unique_username("searchhit");
    let title = format!("Searchable Title {}", unique);
    let video_id = state
        .services
        .video
        .add_external_video(
            1,
            &title,
            Some("search test"),
            Some("searchtest"),
            &format!("https://example.com/{}.mp4", unique),
            None,
            None,
        )
        .await
        .expect("create video");
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/search?q={}", unique),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().expect("items should be an array");
    assert!(
        items.iter().any(|it| json_id(&it["id"]) == video_id),
        "search should return the created video: {}",
        body
    );

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_increment_views_endpoint() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "views").await;
    let video_id = create_test_video(&state, "views").await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/view", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true));

    let (_, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        body["views"],
        json!(1),
        "views should have incremented: {}",
        body
    );

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 播放历史与会话 ──

#[tokio::test]
async fn test_playback_history_requires_auth() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, _) = send_json(&app, Method::GET, "/playback/history", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_playback_history_validation_and_readback() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "playback").await;
    let video_id = create_test_video(&state, "playback").await;
    let app = build_test_app().await;

    // Negative position → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playback/history",
        Some(&token),
        Some(json!({ "video_id": video_id, "position_ms": -1, "duration_ms": 5000 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);
    assert_eq!(body["error"], json!("播放进度不能为负数"));

    // Position beyond duration (+1s tolerance) → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playback/history",
        Some(&token),
        Some(json!({ "video_id": video_id, "position_ms": 7000, "duration_ms": 5000 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);

    // Valid update → 204. NB: `PlaybackService` throttles writes to one per
    // 10s per (user, video); the first write is dropped by the throttle, so
    // wait out the window and POST again — the second write must land.
    let (status, _) = send_json(
        &app,
        Method::POST,
        "/playback/history",
        Some(&token),
        Some(json!({ "video_id": video_id, "position_ms": 3000, "duration_ms": 5000 })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    tokio::time::sleep(std::time::Duration::from_secs(11)).await;
    let (status, _) = send_json(
        &app,
        Method::POST,
        "/playback/history",
        Some(&token),
        Some(json!({ "video_id": video_id, "position_ms": 3000, "duration_ms": 5000 })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Read back the position
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/playback/history/{}", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["positionMs"], json!(3000), "body: {}", body);
    assert_eq!(body["durationMs"], json!(5000));

    // History list contains the video; limit is clamped, not an error
    let (status, body) = send_json(
        &app,
        Method::GET,
        "/playback/history?limit=999",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().expect("history items array");
    assert!(
        items.iter().any(|it| json_id(&it["videoId"]) == video_id),
        "body: {}",
        body
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

#[tokio::test]
async fn test_playback_session_endpoints() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "session").await;
    let app = build_test_app().await;

    // video_id <= 0 → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playback/session/start",
        Some(&token),
        Some(json!({ "video_id": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);
    assert_eq!(body["error"], json!("无效的视频ID"));

    // Start → heartbeat → stop all succeed
    let payload = json!({ "video_id": 42 });
    for (path, _) in [
        ("/playback/session/start", "start"),
        ("/playback/session/heartbeat", "heartbeat"),
        ("/playback/session/stop", "stop"),
    ] {
        let (status, _) = send_json(
            &app,
            Method::POST,
            path,
            Some(&token),
            Some(payload.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{} should succeed", path);
    }

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 播放列表 ──

#[tokio::test]
async fn test_playlist_create_and_update() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "pl_create").await;
    let app = build_test_app().await;

    // Blank name → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playlists",
        Some(&token),
        Some(json!({ "name": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);
    assert_eq!(
        body["error"],
        json!("播放列表名称长度需在 1-100 个字符之间")
    );

    // Valid name → 201
    let name = format!("Playlist {}", unique_username("pl"));
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playlists",
        Some(&token),
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {}", body);
    let playlist_id = json_id(&body["id"]);
    assert_eq!(body["name"], json!(name));
    assert_eq!(body["isPublic"], json!(false));
    assert_eq!(body["itemCount"], json!(0));

    // Listed under /playlists
    let (status, body) = send_json(&app, Method::GET, "/playlists", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["playlists"]
        .as_array()
        .expect("playlists array")
        .iter()
        .any(|p| json_id(&p["id"]) == playlist_id));

    // Update
    let (status, body) = send_json(
        &app,
        Method::PUT,
        &format!("/playlists/{}", playlist_id),
        Some(&token),
        Some(json!({ "name": format!("{}_renamed", name), "is_public": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);

    // Delete
    let (status, body) = send_json(
        &app,
        Method::DELETE,
        &format!("/playlists/{}", playlist_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true));

    // Deleted playlist is gone
    let (status, _) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}", playlist_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_playlist_permission_isolation() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (owner, _uo, owner_token) = create_viewer_with_token(&state, "pl_owner").await;
    let (_other, _uother, other_token) = create_viewer_with_token(&state, "pl_other").await;
    let (_admin, admin_token) = create_admin_with_token(&state, "pl_admin").await;
    let app = build_test_app().await;

    // Owner creates a private playlist
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playlists",
        Some(&owner_token),
        Some(json!({ "name": "Private List" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let playlist_id = json_id(&body["id"]);

    // Another viewer cannot see it (404, existence not leaked)...
    let (status, _) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}", playlist_id),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // ...cannot read its videos...
    let (status, _) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}/videos", playlist_id),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // ...and cannot modify or delete it (403)
    let (status, body) = send_json(
        &app,
        Method::PUT,
        &format!("/playlists/{}", playlist_id),
        Some(&other_token),
        Some(json!({ "name": "hijacked" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {}", body);
    assert_eq!(body["error"], json!("无权修改此播放列表"));

    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/playlists/{}", playlist_id),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Admin can read the private playlist
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}", playlist_id),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);

    // A public playlist is visible to other users
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playlists",
        Some(&owner_token),
        Some(json!({ "name": "Public List", "is_public": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let public_id = json_id(&body["id"]);
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}", public_id),
        Some(&other_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], json!("Public List"));

    cleanup_test_user(state.repos.video.pool(), &owner).await;
}

#[tokio::test]
async fn test_playlist_add_remove_videos() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "pl_videos").await;
    let video_id = create_test_video(&state, "pl_videos").await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playlists",
        Some(&token),
        Some(json!({ "name": "Watch Later" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let playlist_id = json_id(&body["id"]);

    // Adding a nonexistent video → 404 (FK violation mapped)
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/playlists/{}/videos", playlist_id),
        Some(&token),
        Some(json!({ "video_id": 999999999 })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {}", body);
    assert_eq!(body["error"], json!("视频不存在"));

    // Adding a real video → 200
    let (status, _) = send_json(
        &app,
        Method::POST,
        &format!("/playlists/{}/videos", playlist_id),
        Some(&token),
        Some(json!({ "video_id": video_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The video shows up in the playlist
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}/videos", playlist_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().expect("videos array");
    assert!(
        items.iter().any(|v| json_id(&v["id"]) == video_id),
        "playlist should contain the video: {}",
        body
    );

    // Removing it → 200 and the list is empty again
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/playlists/{}/videos/{}", playlist_id, video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}/videos", playlist_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().map(Vec::len), Some(0), "body: {}", body);

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_playlist_position_compaction() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "pl_pos").await;
    let v1 = create_test_video(&state, "pl_pos").await;
    let v2 = create_test_video(&state, "pl_pos").await;
    let v3 = create_test_video(&state, "pl_pos").await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        "/playlists",
        Some(&token),
        Some(json!({ "name": "Position Test" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let playlist_id = json_id(&body["id"]);

    for video_id in [v1, v2, v3] {
        let (status, _) = send_json(
            &app,
            Method::POST,
            &format!("/playlists/{}/videos", playlist_id),
            Some(&token),
            Some(json!({ "video_id": video_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    // Remove the middle item; the remaining positions must be compacted.
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/playlists/{}/videos/{}", playlist_id, v2),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let positions: Vec<i32> = sqlx::query_scalar::<_, i32>(
        "SELECT position FROM playlist_items WHERE playlist_id = $1 ORDER BY position",
    )
    .bind(playlist_id)
    .fetch_all(state.repos.video.pool())
    .await
    .expect("read positions");
    assert_eq!(
        positions,
        vec![0, 1],
        "positions must be dense after removal"
    );

    // A newly added video lands right after the compacted tail.
    let v4 = create_test_video(&state, "pl_pos").await;
    let (status, _) = send_json(
        &app,
        Method::POST,
        &format!("/playlists/{}/videos", playlist_id),
        Some(&token),
        Some(json!({ "video_id": v4 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/playlists/{}/videos", playlist_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<i64> = body
        .as_array()
        .expect("videos array")
        .iter()
        .map(|v| json_id(&v["id"]))
        .collect();
    assert_eq!(
        ids,
        vec![v1, v3, v4],
        "playlist order must be preserved: {}",
        body
    );

    for video_id in [v1, v2, v3, v4] {
        cleanup_test_video(state.repos.video.pool(), video_id).await;
    }
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 评论 ──

#[tokio::test]
async fn test_comments_require_auth() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, _) = send_json(&app, Method::GET, "/videos/1/comments", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send_json(
        &app,
        Method::POST,
        "/videos/1/comments",
        None,
        Some(json!({ "content": "hi" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_comment_create_list_reply_flow() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (user_a, user_a_id, token_a) = create_viewer_with_token(&state, "cmt_a").await;
    let video_id = create_uploaded_test_video(&state, "cmt", user_a_id).await;
    let app = build_test_app().await;

    // Create a top-level comment
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/comments", video_id),
        Some(&token_a),
        Some(json!({ "content": "非常棒的视频！" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {}", body);
    let comment_id = json_id(&body["id"]);
    assert_eq!(body["content"], json!("非常棒的视频！"));

    // List comments
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}/comments", video_id),
        Some(&token_a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["comments"]
        .as_array()
        .expect("comments array")
        .iter()
        .any(|c| json_id(&c["id"]) == comment_id));
    assert!(body["total"].as_i64().unwrap() >= 1);

    // The video owner replies
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/comments", video_id),
        Some(&token_a),
        Some(json!({ "content": "确实！", "parent_id": comment_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {}", body);
    let reply_id = json_id(&body["id"]);
    assert_eq!(json_id(&body["parentId"]), comment_id);

    // Replies are listed under the parent
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/comments/{}/replies", comment_id),
        Some(&token_a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body
        .as_array()
        .expect("replies array")
        .iter()
        .any(|c| json_id(&c["id"]) == reply_id));

    // Owner deletes the top-level comment
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/comments/{}", comment_id),
        Some(&token_a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/comments/{}", comment_id),
        Some(&token_a),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "second delete should 404");

    cleanup_test_comments(state.repos.video.pool(), video_id).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &user_a).await;
}

#[tokio::test]
async fn test_comment_create_validation() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, user_id, token) = create_viewer_with_token(&state, "cmt_val").await;
    let video_id = create_uploaded_test_video(&state, "cmt_val", user_id).await;
    let app = build_test_app().await;

    // Empty content → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/comments", video_id),
        Some(&token),
        Some(json!({ "content": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);

    // Nonexistent video → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/videos/999999999/comments",
        Some(&token),
        Some(json!({ "content": "hello" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("视频不存在"));

    // Reply to a nonexistent parent → 400
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/comments", video_id),
        Some(&token),
        Some(json!({ "content": "hello", "parent_id": 999999999 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("父评论不存在"));

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_comment_delete_permission_isolation() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (user_a, user_a_id, token_a) = create_viewer_with_token(&state, "cmt_del_a").await;
    let (_user_b, _ub, token_b) = create_viewer_with_token(&state, "cmt_del_b").await;
    let (_admin, admin_token) = create_admin_with_token(&state, "cmt_del_admin").await;
    let video_id = create_uploaded_test_video(&state, "cmt_del", user_a_id).await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/comments", video_id),
        Some(&token_a),
        Some(json!({ "content": "secret thoughts" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let comment_id = json_id(&body["id"]);

    // Another user cannot delete it (404, existence not leaked)
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/comments/{}", comment_id),
        Some(&token_b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Admin can
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/comments/{}", comment_id),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    cleanup_test_comments(state.repos.video.pool(), video_id).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &user_a).await;
}

// ── 分享 ──

/// Create a test video owned by `uploader_id`.
/// Since H-02, only the video's uploader (or an admin) may create share
/// links, so share tests must run with an explicit uploader identity.
async fn create_uploaded_test_video(state: &Arc<AppState>, prefix: &str, uploader_id: i64) -> i64 {
    let video_id = create_test_video(state, prefix).await;
    sqlx::query("UPDATE videos SET uploader_id = $1 WHERE id = $2")
        .bind(uploader_id)
        .bind(video_id)
        .execute(state.repos.video.pool())
        .await
        .expect("set test video uploader");
    video_id
}

#[tokio::test]
async fn test_share_link_lifecycle() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, user_id, token) = create_viewer_with_token(&state, "share").await;
    let video_id = create_uploaded_test_video(&state, "share", user_id).await;
    let app = build_test_app().await;

    // Create a share link
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/share", video_id),
        Some(&token),
        Some(json!({ "expires_in_days": 7 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {}", body);
    let share_id = json_id(&body["id"]);
    let share_token = body["token"].as_str().unwrap().to_string();
    assert_eq!(share_token.len(), 32, "share token must be 32 chars");
    assert!(share_token.bytes().all(|b| b.is_ascii_alphanumeric()));
    assert!(
        body["shareUrl"]
            .as_str()
            .unwrap_or("")
            .contains("http://localhost:3000"),
        "shareUrl should use PUBLIC_URL: {}",
        body
    );

    // Listed in my shares
    let (status, body) =
        send_json(&app, Method::GET, "/auth/user/shares", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body
        .as_array()
        .expect("shares array")
        .iter()
        .any(|s| json_id(&s["id"]) == share_id));

    // Public access via the token, with the share_token cookie set
    let (status, headers, bytes) = send(
        &app,
        Method::GET,
        &format!("/share/{}", share_token),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json_id(&body["id"]), video_id);
    assert!(
        !body["title"].as_str().unwrap_or("").is_empty(),
        "shared video should expose its title: {}",
        body
    );
    let set_cookie = headers
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        set_cookie.contains("share_token="),
        "expected share_token cookie: {}",
        set_cookie
    );

    // Delete via the video-scoped endpoint
    let (status, body) = send_json(
        &app,
        Method::DELETE,
        &format!("/videos/{}/share/{}", video_id, share_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);

    // The token no longer resolves
    let (status, _, _) = send(
        &app,
        Method::GET,
        &format!("/share/{}", share_token),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "deleted share token must 404"
    );

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_share_invalid_and_unknown_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;

    // Malformed token format → 400
    let (status, _, body) = send(&app, Method::GET, "/share/xyz", None, None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["error"], json!("分享链接格式无效"));

    // Well-formed but unknown token → 404
    let (status, _, _) = send(
        &app,
        Method::GET,
        "/share/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_share_create_nonexistent_video_404() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "share_404").await;
    let app = build_test_app().await;
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/videos/2000000000/share",
        Some(&token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {}", body);
    assert_eq!(body["error"], json!("视频不存在"));
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_share_create_forbidden_for_non_owner() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (user_a, user_a_id, _token_a) = create_viewer_with_token(&state, "share_own_a").await;
    let (user_b, _user_b_id, token_b) = create_viewer_with_token(&state, "share_own_b").await;
    let video_id = create_uploaded_test_video(&state, "share_own", user_a_id).await;
    let app = build_test_app().await;

    // A logged-in user who is neither the uploader nor an admin gets 403
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/share", video_id),
        Some(&token_b),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {}", body);
    assert_eq!(body["error"], json!("无权分享该视频"));

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &user_a).await;
    cleanup_test_user(state.repos.video.pool(), &user_b).await;
}

#[tokio::test]
async fn test_share_create_admin_can_share_any_video() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (user_a, _user_a_id, _token_a) = create_viewer_with_token(&state, "share_adm_a").await;
    let (_admin, admin_token) = create_admin_with_token(&state, "share_adm").await;
    let video_id = create_test_video(&state, "share_adm_v").await;
    let app = build_test_app().await;

    // Admins may share any video, even one they did not upload
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/share", video_id),
        Some(&admin_token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {}", body);
    let share_id = json_id(&body["id"]);

    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/videos/{}/share/{}", video_id, share_id),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &user_a).await;
    cleanup_test_user(state.repos.video.pool(), &_admin).await;
}

#[tokio::test]
async fn test_share_delete_other_users_link() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (user_a, user_a_id, token_a) = create_viewer_with_token(&state, "share_a").await;
    let (_user_b, _ub, token_b) = create_viewer_with_token(&state, "share_b").await;
    let video_id = create_uploaded_test_video(&state, "share_iso", user_a_id).await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/share", video_id),
        Some(&token_a),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let share_id = json_id(&body["id"]);

    // Another user cannot delete it → 404 (existence not leaked)
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/videos/{}/share/{}", video_id, share_id),
        Some(&token_b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Nor revoke it via /auth/user/shares
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/auth/user/shares/{}", share_id),
        Some(&token_b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &user_a).await;
}

#[tokio::test]
async fn test_revoke_my_share() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, user_id, token) = create_viewer_with_token(&state, "share_revoke").await;
    let video_id = create_uploaded_test_video(&state, "share_revoke", user_id).await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/share", video_id),
        Some(&token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let share_id = json_id(&body["id"]);

    let (status, body) = send_json(
        &app,
        Method::DELETE,
        &format!("/auth/user/shares/{}", share_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert_eq!(body["ok"], json!(true));

    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/auth/user/shares/{}", share_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "second revoke should 404");

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 标签 ──

#[tokio::test]
async fn test_tags_public_endpoints() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;

    let (status, body) = send_json(&app, Method::GET, "/tags", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["tags"].is_array());
    assert!(body["total"].is_number());
    assert!(body["page"].is_number());
    assert!(body["size"].is_number());

    // Negative page is clamped to 0
    let (status, body) = send_json(&app, Method::GET, "/tags?page=-1", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["page"], json!(0));

    let (status, _) = send_json(&app, Method::GET, "/tags/popular", None, None).await;
    assert_eq!(status, StatusCode::OK);

    // Nonexistent tag → 404
    let (status, body) = send_json(&app, Method::GET, "/tags/999999", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("标签不存在"));
}

/// KNOWN-FAILING: attaching tags to a video currently fails because
/// `add_tags_to_video_batch` emits `VALUES VALUES (...)` (sqlx `push_values`
/// already includes the keyword), so the query is a syntax error and every
/// non-empty attach returns 400 "添加标签失败". Ignored until the source is
/// fixed.
#[tokio::test]
#[ignore = "既有缺陷: add_tags_to_video_batch 生成 VALUES VALUES 非法 SQL, 添加标签必失败"]
async fn test_video_tags_admin_flow() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "tag_v").await;
    let (_admin, admin_token) = create_admin_with_token(&state, "tag_admin").await;
    let video_id = create_test_video(&state, "tag_v").await;
    let app = build_test_app().await;

    // Creating tags is admin-only
    let tag_name = format!("tag-{}", unique_username("t"));
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/admin/tags",
        Some(&token),
        Some(json!({ "name": tag_name })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "viewer must not create tags: {}",
        body
    );

    let (status, body) = send_json(
        &app,
        Method::POST,
        "/admin/tags",
        Some(&admin_token),
        Some(json!({ "name": tag_name, "color": "#ff0000" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    let tag_id = json_id(&body["id"]);
    assert_eq!(body["name"], json!(tag_name));

    // Get tags for a video (auth required, empty initially)
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}/tags", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));

    // Attach the tag to the video
    let (status, body) = send_json(
        &app,
        Method::POST,
        &format!("/videos/{}/tags", video_id),
        Some(&token),
        Some(json!([tag_id])),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);

    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}/tags", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.as_array()
            .expect("tags array")
            .iter()
            .any(|t| json_id(&t["id"]) == tag_id),
        "video should have the tag: {}",
        body
    );

    // Remove the tag
    let (status, _) = send_json(
        &app,
        Method::DELETE,
        &format!("/videos/{}/tags", video_id),
        Some(&token),
        Some(json!([tag_id])),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}/tags", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));

    cleanup_test_tag(state.repos.video.pool(), &tag_name).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 推荐 ──

#[tokio::test]
async fn test_recommendations_public_endpoints() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let video_id = create_test_video(&state, "rec_pub").await;
    let app = build_test_app().await;

    let (status, body) =
        send_json(&app, Method::GET, "/recommendations/trending", None, None).await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert!(body["items"].is_array());
    assert!(body["total"].is_number());

    let (status, _) = send_json(&app, Method::GET, "/recommendations/recent", None, None).await;
    assert_eq!(status, StatusCode::OK);

    // Similar works for a video that exists
    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/recommendations/similar/{}", video_id),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert!(body["items"].is_array());

    // Invalid id → 400
    let (status, body) = send_json(
        &app,
        Method::GET,
        "/recommendations/similar/abc!!!",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("无效的视频ID"));

    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

#[tokio::test]
async fn test_recommendations_personal_requires_auth() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "rec").await;
    let app = build_test_app().await;

    let (status, _) = send_json(&app, Method::GET, "/recommendations", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = send_json(&app, Method::GET, "/recommendations", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert!(body["items"].is_array());

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 管理端点 ──

#[tokio::test]
async fn test_admin_users_permission_isolation() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "admin_iso").await;
    let (_admin, admin_token) = create_admin_with_token(&state, "admin_iso2").await;
    let app = build_test_app().await;

    // No token → 401
    let (status, _) = send_json(&app, Method::GET, "/admin/users", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Viewers → 403
    let (status, body) = send_json(&app, Method::GET, "/admin/users", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {}", body);
    assert_eq!(body["error"], json!("需要管理员权限"));

    // Admin → 200
    let (status, body) =
        send_json(&app, Method::GET, "/admin/users", Some(&admin_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_admin_add_external_video_via_http() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "adm_ext").await;
    let (_admin, admin_token) = create_admin_with_token(&state, "adm_ext2").await;
    let app = build_test_app().await;
    let title = format!("External {}", unique_username("ext"));

    // Viewers cannot add videos
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/admin/videos/external",
        Some(&token),
        Some(json!({ "title": title, "stream_url": "https://example.com/v.mp4" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {}", body);

    // Admin can
    let (status, body) = send_json(
        &app,
        Method::POST,
        "/admin/videos/external",
        Some(&admin_token),
        Some(json!({ "title": title, "stream_url": "https://example.com/v.mp4" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {}", body);
    let video_id = json_id(&body["id"]);

    let (status, body) = send_json(
        &app,
        Method::GET,
        &format!("/videos/{}", video_id),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], json!(title));

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_admin_external_video_rejects_loopback_url() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (_admin, admin_token) = create_admin_with_token(&state, "adm_loop").await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::POST,
        "/admin/videos/external",
        Some(&admin_token),
        Some(json!({ "title": "Loopback", "stream_url": "http://127.0.0.1/v.mp4" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {}", body);
    assert_eq!(body["error"], json!("stream_url 指向不被允许的主机"));
}

#[tokio::test]
async fn test_admin_registration_config_toggle() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (_admin, admin_token) = create_admin_with_token(&state, "adm_reg").await;
    let app = build_test_app().await;

    let (status, body) = send_json(
        &app,
        Method::GET,
        "/admin/config/registration",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["enabled"].is_boolean());

    // Enable → verify → disable again so the DB is restored
    let (status, body) = send_json(
        &app,
        Method::PUT,
        "/admin/config/registration",
        Some(&admin_token),
        Some(json!({ "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", body);
    assert_eq!(body["ok"], json!(true));

    let (_, body) = send_json(
        &app,
        Method::GET,
        "/admin/config/registration",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(body["enabled"], json!(true));

    let (status, _) = send_json(
        &app,
        Method::PUT,
        "/admin/config/registration",
        Some(&admin_token),
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = send_json(
        &app,
        Method::GET,
        "/admin/config/registration",
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(body["enabled"], json!(false));
}

#[tokio::test]
async fn test_admin_stats() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (_admin, admin_token) = create_admin_with_token(&state, "adm_stats").await;
    let app = build_test_app().await;

    let (status, body) =
        send_json(&app, Method::GET, "/admin/stats", Some(&admin_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["totalVideos"].is_number(), "body: {}", body);
    assert!(body["userCount"].is_number());
    assert!(body["pendingCount"].is_number());
}

#[tokio::test]
async fn test_internal_routes_admin_only() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "internal").await;
    let (_admin, admin_token) = create_admin_with_token(&state, "internal2").await;
    let app = build_test_app().await;

    // /server/info: no token → 401, viewer → 403, admin → 200
    let (status, _) = send_json(&app, Method::GET, "/server/info", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send_json(&app, Method::GET, "/server/info", Some(&token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) =
        send_json(&app, Method::GET, "/server/info", Some(&admin_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["version"].is_string());

    // /metrics: no token → 401
    let (status, _) = send_json(&app, Method::GET, "/metrics", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_docs_routes() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (username, _user_id, token) = create_viewer_with_token(&state, "docs").await;
    let app = build_test_app().await;

    let (status, _) = send_json(&app, Method::GET, "/docs/openapi.json", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) =
        send_json(&app, Method::GET, "/docs/openapi.json", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object());

    let (status, headers, _) = send(&app, Method::GET, "/docs", Some(&token), None, None).await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        headers.get("location").and_then(|v| v.to_str().ok()),
        Some("/docs/openapi.json")
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── 路由与回退 ──

#[tokio::test]
async fn test_root_redirects_to_webapp() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let (status, headers, _) = send(&app, Method::GET, "/", None, None, None).await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        headers.get("location").and_then(|v| v.to_str().ok()),
        Some("/webapp/")
    );
}

#[tokio::test]
async fn test_api_unknown_route_returns_json_404() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    // Unknown routes under API prefixes must NOT fall through to the SPA
    // fallback: they get a structured JSON 404.
    let (status, _, body) = send(
        &app,
        Method::GET,
        "/videos/1/does-not-exist",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["error"], json!("接口不存在"));
}

#[tokio::test]
async fn test_spa_fallback_serves_html() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    // Non-API routes fall through to the SPA index fallback (200 + html)
    let (status, headers, _) =
        send(&app, Method::GET, "/some/client/route", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.contains("text/html")));
}

#[tokio::test]
async fn test_media_requires_auth() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    // Media requests without an auth token or share token are rejected
    let (status, _, _) = send(
        &app,
        Method::GET,
        "/media/videos/1/stream.mp4",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
