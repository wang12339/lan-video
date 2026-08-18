//! Integration tests for the authentication flow.
//!
//! Requires a running PostgreSQL database. Set `DATABASE_URL` to enable.

mod integration_test_helpers;

use std::net::SocketAddr;
use std::time::Duration;

use atmos_video_backend::app;
use atmos_video_backend::models::auth::AuthRequest;
use atmos_video_backend::repositories::registration_repo::RegistrationRepository;
use atmos_video_backend::services::auth_service::AuthService;
use atmos_video_backend::util::password;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use integration_test_helpers::*;
use tower::ServiceExt;

/// Strong password that satisfies the 3-of-4-category rule (>= 8 chars).
const STRONG_PASSWORD: &str = "Str0ng!Pass1";

/// Helper: create an AuthService backed by the test AppState.
fn auth_service(state: &atmos_video_backend::state::AppState) -> AuthService {
    AuthService::new(
        state.repos.user.clone(),
        state.services.playback.clone(),
        state.rate_limiter.clone(),
        state.ip_rate_limiter.clone(),
        state.config.clone(),
    )
}

/// Helper: register a user and assert success. Non-admin users get no token.
async fn register_user(svc: &AuthService, username: &str, password: &str) {
    let reg = svc
        .register(
            &AuthRequest {
                username: username.into(),
                password: password.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register should not error");
    assert!(reg.ok, "registration should succeed for {}", username);
    assert!(
        reg.token.is_none(),
        "non-admin registration should not return a token"
    );
}

/// Helper: approve a freshly registered user (bypassing the admin flow).
async fn approve_user(state: &atmos_video_backend::state::AppState, username: &str) -> i64 {
    let user = state
        .repos
        .user
        .find_by_username(1, username)
        .await
        .expect("find user")
        .expect("user should exist");
    state
        .repos
        .user
        .approve_user(user.id, true)
        .await
        .expect("approve");
    user.id
}

/// Helper: build the full HTTP router with registration enabled in the DB,
/// returning the app plus a pool for direct repo access in assertions.
async fn build_http_app() -> (Router, sqlx::PgPool) {
    let pool = test_pool().await;
    RegistrationRepository::new(pool.clone())
        .set_enabled(true)
        .await
        .expect("enable registration");
    let addr: SocketAddr = "127.0.0.1:54321".parse().expect("valid addr");
    let app = app::build_router(test_config())
        .await
        .layer(MockConnectInfo(addr));
    (app, pool)
}

async fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn send(app: &Router, req: Request<Body>) -> axum::response::Response {
    app.clone().oneshot(req).await.expect("request should run")
}

async fn read_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("valid JSON body")
}

// ── Register → Login → Get User flow ──

#[tokio::test]
async fn test_register_login_get_user() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("auth_flow");
    let password = STRONG_PASSWORD;

    // Register — non-admin users don't get a token (need approval)
    let reg_result = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register should not error");

    assert!(reg_result.ok, "registration should succeed");
    // Non-admin registration returns no token (needs admin approval)
    assert!(
        reg_result.token.is_none(),
        "non-admin registration should not return a token"
    );

    // Get user info directly (simulating admin lookup)
    let user_info = svc
        .user_info(&username, false, 1)
        .await
        .expect("user_info should not error");

    assert_eq!(user_info.username, username);

    // Cleanup
    let pool = state.repos.video.pool();
    cleanup_test_user(pool, &username).await;
}

// ── Registration validation: length & content boundaries ──

#[tokio::test]
async fn test_register_username_length_boundaries() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);

    // 1 char: too short
    let reg = svc
        .register(
            &AuthRequest {
                username: "a".into(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(!reg.ok, "1-char username should be rejected");
    assert_eq!(reg.error.as_deref(), Some("用户名长度需在 2-64 个字符之间"));

    // 65 chars: too long
    let long_name = "a".repeat(65);
    let reg = svc
        .register(
            &AuthRequest {
                username: long_name.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(!reg.ok, "65-char username should be rejected");

    // 2 chars: minimum boundary — should succeed
    let short_name = unique_username("mi");
    let reg = svc
        .register(
            &AuthRequest {
                username: short_name.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok, "2-char username should be accepted");
    cleanup_test_user(state.repos.video.pool(), &short_name).await;

    // 64 chars: maximum boundary — should succeed
    let max_name = {
        let suffix = format!("_{}_{}", std::process::id(), 99999);
        let pad = 64usize.saturating_sub(suffix.len());
        format!("{}{}", "x".repeat(pad), suffix)
    };
    assert_eq!(max_name.len(), 64, "max boundary must be exactly 64 bytes");
    let reg = svc
        .register(
            &AuthRequest {
                username: max_name.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok, "64-char username should be accepted");
    cleanup_test_user(state.repos.video.pool(), &max_name).await;
}

#[tokio::test]
async fn test_register_empty_username_or_password() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);

    let reg = svc
        .register(
            &AuthRequest {
                username: "".into(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(!reg.ok, "empty username should be rejected");
    assert_eq!(reg.error.as_deref(), Some("用户名和密码不能为空"));

    let reg = svc
        .register(
            &AuthRequest {
                username: unique_username("empty_pw"),
                password: "".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(!reg.ok, "empty password should be rejected");
}

#[tokio::test]
async fn test_register_password_length_boundaries() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);

    // 7 chars: too short
    let reg = svc
        .register(
            &AuthRequest {
                username: unique_username("pw7"),
                password: "Abcdef1".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(!reg.ok, "7-char password should be rejected");
    assert_eq!(reg.error.as_deref(), Some("密码长度需在 8-128 个字符之间"));

    // 8 chars with all four categories: minimum boundary — accepted
    let name = unique_username("pw8");
    let reg = svc
        .register(
            &AuthRequest {
                username: name.clone(),
                password: "Abcdef1!".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok, "8-char strong password should be accepted");
    cleanup_test_user(state.repos.video.pool(), &name).await;

    // 129 chars: too long
    let reg = svc
        .register(
            &AuthRequest {
                username: unique_username("pw129"),
                password: "A".repeat(129),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(!reg.ok, "129-char password should be rejected");

    // 128 chars: maximum boundary — accepted (strength passes via categories)
    let name = unique_username("pw128");
    let mut long = "Aa1!".repeat(32); // 128 chars, 4 categories
    assert_eq!(long.chars().count(), 128);
    let reg = svc
        .register(
            &AuthRequest {
                username: name.clone(),
                password: std::mem::take(&mut long),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok, "128-char password should be accepted");
    cleanup_test_user(state.repos.video.pool(), &name).await;
}

#[tokio::test]
async fn test_register_weak_password_rejected() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);

    // Common / trivial passwords that fail the 3-of-4-category rule (<12 chars)
    for (i, weak) in ["password", "12345678", "abcdefgh", "PASSWORD1", "qwertyui"]
        .iter()
        .enumerate()
    {
        let reg = svc
            .register(
                &AuthRequest {
                    username: format!("{}_weak_{}", unique_username("weak"), i),
                    password: weak.to_string(),
                },
                "127.0.0.1",
                1,
            )
            .await
            .expect("register");
        assert!(!reg.ok, "{:?} should be rejected as weak", weak);
        assert_eq!(
            reg.error.as_deref(),
            Some("密码过于简单，请使用更复杂的密码")
        );
    }
}

#[tokio::test]
async fn test_register_username_control_chars_rejected() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);

    for bad in ["abc\n123", "a\rb", "abc\u{0}def"] {
        let reg = svc
            .register(
                &AuthRequest {
                    username: bad.into(),
                    password: STRONG_PASSWORD.into(),
                },
                "127.0.0.1",
                1,
            )
            .await
            .expect("register");
        assert!(!reg.ok, "{:?} should be rejected for control chars", bad);
        assert_eq!(reg.error.as_deref(), Some("用户名包含非法字符"));
    }
}

#[tokio::test]
async fn test_register_username_trimmed() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("trim_user");
    let padded = format!("  {}  ", username);

    let reg = svc
        .register(
            &AuthRequest {
                username: padded.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(
        reg.ok,
        "username with surrounding whitespace should be accepted"
    );

    // Stored username must be trimmed
    let info = svc.user_info(&username, false, 1).await.expect("user_info");
    assert_eq!(info.username, username);

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_register_password_whitespace_preserved() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("spaced_pw");
    // Password with surrounding spaces — must be hashed exactly as typed
    let spaced = " SpacedPass1! ";

    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: spaced.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok, "password with whitespace should be accepted");

    let user_id = approve_user(&state, &username).await;

    // Login with the exact password (including whitespace) must succeed
    let login = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: spaced.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(
        login.ok,
        "login with exact whitespace password should succeed"
    );

    // Login with a trimmed password must fail
    let login = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: "SpacedPass1!".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(!login.ok, "login with trimmed password should fail");

    assert_eq!(user_id, user_id); // silence unused warning if removed later
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_register_concurrent_same_username_race() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("race_user");

    // Two concurrent registrations with the same username: exactly one must
    // win; the other gets the friendly "用户名已存在" (not a 500).
    let req_a = AuthRequest {
        username: username.clone(),
        password: STRONG_PASSWORD.into(),
    };
    let req_b = AuthRequest {
        username: username.clone(),
        password: STRONG_PASSWORD.into(),
    };
    let (r1, r2) = tokio::join!(
        svc.register(&req_a, "127.0.0.1", 1),
        svc.register(&req_b, "127.0.0.1", 1),
    );

    let r1 = r1.expect("register");
    let r2 = r2.expect("register");

    let successes = [r1.ok, r2.ok].into_iter().filter(|ok| *ok).count();
    assert_eq!(successes, 1, "exactly one registration should win the race");

    let loser = if r1.ok { &r2 } else { &r1 };
    assert_eq!(loser.error.as_deref(), Some("用户名已存在"));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── Login ──

#[tokio::test]
async fn test_login_unapproved_user_rejected() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("unapproved");

    register_user(&svc, &username, STRONG_PASSWORD).await;

    // Correct password but the account is pending approval
    let login = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(!login.ok, "unapproved user must not be able to log in");
    assert!(login.token.is_none());

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_approved_user_succeeds() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("approved");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    approve_user(&state, &username).await;

    let login = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(login.ok, "approved user should log in");
    let token = login.token.expect("token should be returned");

    // The token must resolve to the user
    let found = state
        .repos
        .user
        .find_user_by_token(&token)
        .await
        .expect("query")
        .expect("token should resolve");
    assert_eq!(found.username, username);
    assert!(found.approved);

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_username_trimmed() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("trim_login");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    approve_user(&state, &username).await;

    let login = svc
        .login(
            &AuthRequest {
                username: format!("  {}  ", username),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(login.ok, "login should trim the username");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_single_session_enforced() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("single_sess");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    approve_user(&state, &username).await;

    // First login succeeds and creates an active session
    let first = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(first.ok);

    // Second login for a non-admin with an active session must be rejected
    let second = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(
        !second.ok,
        "non-admin with active session must not log in again"
    );
    assert!(
        second
            .error
            .as_deref()
            .is_some_and(|e| e.contains("已在其他设备登录")),
        "error should mention the active session: {:?}",
        second.error
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_admin_multiple_sessions_allowed() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("admin_sess");

    // Create an admin directly (role 3 = auto-approved)
    let hash = password::hash(STRONG_PASSWORD).expect("hash");
    state
        .repos
        .user
        .create_user(1, &username, &hash, 3)
        .await
        .expect("create admin");

    let first = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(first.ok);

    // Admins are exempt from the single-session rule
    let second = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(second.ok, "admin should be allowed multiple sessions");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_enumeration_response_consistency() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("enum_consist");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    approve_user(&state, &username).await;

    // Wrong password for a real user...
    let wrong_pw = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: "TotallyWrong!9".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");

    // ...and a nonexistent user must produce the IDENTICAL response,
    // otherwise an attacker can enumerate valid usernames.
    let ghost = svc
        .login(
            &AuthRequest {
                username: unique_username("ghost_enum"),
                password: "TotallyWrong!9".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");

    assert!(!wrong_pw.ok && !ghost.ok);
    assert_eq!(wrong_pw.error, ghost.error);
    assert_eq!(wrong_pw.error.as_deref(), Some("用户名或密码错误"));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── Logout ──

#[tokio::test]
async fn test_logout() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("logout");

    // Register — non-admin users don't get a token (need approval)
    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok);
    // Non-admin registration doesn't return a token
    assert!(
        reg.token.is_none(),
        "non-admin registration should not return a token"
    );

    // Create a token directly for testing logout
    let user = state
        .repos
        .user
        .find_by_username(1, &username)
        .await
        .expect("find user")
        .expect("user should exist");
    let token = state
        .repos
        .user
        .create_token(user.id)
        .await
        .expect("create token");

    // Logout
    svc.logout(Some(username.as_str()), Some(&token)).await;

    // Token should no longer work — find_user_by_token should return None
    let found = state
        .repos
        .user
        .find_user_by_token(&token)
        .await
        .expect("query");
    assert!(found.is_none(), "token should be invalid after logout");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_logout_without_token_is_noop() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);

    // Logging out without any token must not panic or error
    svc.logout(None, None).await;
    // Logging out with a garbage token is also a no-op
    svc.logout(Some("ghost_user"), Some("not-a-real-token"))
        .await;
}

// ── Token lifecycle: expiry & revocation ──

#[tokio::test]
async fn test_token_expired_is_invalid() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("token_expire");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;
    let token = state
        .repos
        .user
        .create_token(user_id)
        .await
        .expect("create token");

    // Expire it in the DB (what a 7-day-old token looks like)
    sqlx::query("UPDATE auth_tokens SET expires_at = CURRENT_TIMESTAMP - INTERVAL '1 hour' WHERE user_id = $1")
        .bind(user_id)
        .execute(state.repos.video.pool())
        .await
        .expect("expire token");

    let found = state
        .repos
        .user
        .find_user_by_token(&token)
        .await
        .expect("query");
    assert!(found.is_none(), "expired token must not resolve to a user");

    // find_token_detail should report it as not revoked but expired
    let detail = state
        .repos
        .user
        .find_token_detail(&token)
        .await
        .expect("query")
        .expect("token hash still exists");
    assert!(!detail.1, "should not be marked revoked");
    assert!(!detail.2, "should report expired");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_token_revoked_is_invalid() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("token_revoke");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;
    let token = state
        .repos
        .user
        .create_token(user_id)
        .await
        .expect("create token");

    // Revoke (admin kick / password change)
    let n = state
        .repos
        .user
        .revoke_tokens_by_user_id(user_id)
        .await
        .expect("revoke");
    assert!(n >= 1, "at least one token should be revoked");

    let found = state
        .repos
        .user
        .find_user_by_token(&token)
        .await
        .expect("query");
    assert!(found.is_none(), "revoked token must not resolve to a user");

    let detail = state
        .repos
        .user
        .find_token_detail(&token)
        .await
        .expect("query")
        .expect("token hash still exists");
    assert!(detail.1, "should be marked revoked");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_cleanup_expired_tokens_removes_them() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("token_cleanup");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;
    let token = state
        .repos
        .user
        .create_token(user_id)
        .await
        .expect("create token");

    sqlx::query("UPDATE auth_tokens SET expires_at = CURRENT_TIMESTAMP - INTERVAL '1 hour' WHERE user_id = $1")
        .bind(user_id)
        .execute(state.repos.video.pool())
        .await
        .expect("expire token");

    let n = state
        .repos
        .user
        .cleanup_expired_tokens()
        .await
        .expect("cleanup");
    assert!(n >= 1, "expired tokens should be purged");

    let detail = state
        .repos
        .user
        .find_token_detail(&token)
        .await
        .expect("query");
    assert!(detail.is_none(), "purged token hash should be gone");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── User info ──

#[tokio::test]
async fn test_user_info_nonexistent_user() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let ghost = unique_username("ghost_info");

    let info = svc.user_info(&ghost, false, 1).await.expect("user_info");
    assert_eq!(info.id, 0, "nonexistent user should have id 0");
    assert_eq!(info.username, ghost);
    assert!(!info.email_verified);
}

#[tokio::test]
async fn test_user_info_after_email_update() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("info_email");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;

    let before = svc.user_info(&username, false, 1).await.expect("user_info");
    assert!(before.email.is_none(), "no email before update");

    state
        .repos
        .user
        .update_email(user_id, "info@example.com")
        .await
        .expect("update email");

    let after = svc.user_info(&username, false, 1).await.expect("user_info");
    assert_eq!(after.email.as_deref(), Some("info@example.com"));
    assert!(
        !after.email_verified,
        "changing the email must reset verification"
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── Email update & verification ──

#[tokio::test]
async fn test_update_email_unique_conflict() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let user_a = unique_username("email_a");
    let user_b = unique_username("email_b");

    register_user(&svc, &user_a, STRONG_PASSWORD).await;
    register_user(&svc, &user_b, STRONG_PASSWORD).await;
    let id_a = approve_user(&state, &user_a).await;
    let id_b = approve_user(&state, &user_b).await;

    state
        .repos
        .user
        .update_email(id_a, "shared@example.com")
        .await
        .expect("first email bind");

    // Second user binding the same email must hit the unique constraint
    let res = state
        .repos
        .user
        .update_email(id_b, "shared@example.com")
        .await;
    match res {
        Err(sqlx::Error::Database(ref db_err)) => {
            assert_eq!(
                db_err.constraint(),
                Some("idx_users_email_unique"),
                "expected unique constraint violation"
            );
        }
        other => panic!("expected unique violation, got {:?}", other),
    }

    cleanup_test_user(state.repos.video.pool(), &user_a).await;
    cleanup_test_user(state.repos.video.pool(), &user_b).await;
}

#[tokio::test]
async fn test_email_verification_flow_single_use() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("verify_email");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;
    state
        .repos
        .user
        .update_email(user_id, "verify@example.com")
        .await
        .expect("set email");

    let token = state
        .repos
        .user
        .create_email_verification_token(user_id)
        .await
        .expect("create verification token");

    // First use: token is consumed and user id returned
    let found = state
        .repos
        .user
        .find_valid_email_verification_token(&token)
        .await
        .expect("consume");
    assert_eq!(found, Some(user_id));

    // Second use must fail — token is single-use
    let again = state
        .repos
        .user
        .find_valid_email_verification_token(&token)
        .await
        .expect("consume again");
    assert!(again.is_none(), "verification token must be single-use");

    // Garbage token is invalid
    let bogus = state
        .repos
        .user
        .find_valid_email_verification_token("not-a-real-token")
        .await
        .expect("bogus");
    assert!(bogus.is_none());

    // Mark verified and check the flag flips
    state
        .repos
        .user
        .verify_email(user_id)
        .await
        .expect("verify");
    let info = svc.user_info(&username, false, 1).await.expect("user_info");
    assert!(info.email_verified);

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_update_email_invalidates_old_verification_tokens() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("old_verify");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;
    state
        .repos
        .user
        .update_email(user_id, "old@example.com")
        .await
        .expect("set email");

    let token = state
        .repos
        .user
        .create_email_verification_token(user_id)
        .await
        .expect("create token");

    // Changing the email again invalidates all outstanding verification tokens
    state
        .repos
        .user
        .update_email(user_id, "new@example.com")
        .await
        .expect("change email");

    let found = state
        .repos
        .user
        .find_valid_email_verification_token(&token)
        .await
        .expect("consume");
    assert!(
        found.is_none(),
        "old verification token must be invalidated by email change"
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── Avatar ──

#[tokio::test]
async fn test_update_avatar_persisted() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("avatar_user");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;

    state
        .repos
        .user
        .update_avatar(user_id, "/media/avatars/42.png")
        .await
        .expect("update avatar");

    let user = state
        .repos
        .user
        .find_by_username(1, &username)
        .await
        .expect("find")
        .expect("user");
    assert_eq!(user.avatar_url.as_deref(), Some("/media/avatars/42.png"));

    // Token-based lookup must carry the avatar too
    let token = state.repos.user.create_token(user_id).await.expect("token");
    let found = state
        .repos
        .user
        .find_user_by_token(&token)
        .await
        .expect("find by token")
        .expect("user");
    assert_eq!(found.avatar_url.as_deref(), Some("/media/avatars/42.png"));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── Role boundaries ──

#[tokio::test]
async fn test_first_user_becomes_admin_with_env_flag() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    // Create a dedicated tenant that has no users yet, so count == 0.
    let pool = test_pool().await;
    let slug = format!("t_tenant_{}_{}", std::process::id(), unique_username("u"));
    let (tenant_id,): (i64,) =
        sqlx::query_as("INSERT INTO tenants (name, slug) VALUES ($1, $2) RETURNING id")
            .bind(&slug)
            .bind(&slug)
            .fetch_one(&pool)
            .await
            .expect("create temp tenant");

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("first_admin");

    std::env::set_var("ALLOW_FIRST_USER_ADMIN", "true");

    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            tenant_id,
        )
        .await
        .expect("register");

    std::env::remove_var("ALLOW_FIRST_USER_ADMIN");

    assert!(reg.ok, "first user registration should succeed");
    assert!(
        reg.token.is_some(),
        "opt-in first user should get an admin token"
    );

    let user = state
        .repos
        .user
        .find_by_username(tenant_id, &username)
        .await
        .expect("find")
        .expect("user");
    assert!(user.role >= 3, "first user should be promoted to admin");
    assert!(user.approved, "admin is auto-approved");

    cleanup_test_user(&pool, &username).await;
    sqlx::query("DELETE FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .expect("cleanup temp tenant");
}

#[tokio::test]
async fn test_role_boundaries_viewer_vs_admin() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let viewer = unique_username("rb_viewer");
    let admin = unique_username("rb_admin");

    // Normal viewer: role 1
    register_user(&svc, &viewer, STRONG_PASSWORD).await;
    let viewer_id = approve_user(&state, &viewer).await;
    let viewer_row = state
        .repos
        .user
        .find_by_username(1, &viewer)
        .await
        .expect("find")
        .expect("viewer");
    assert_eq!(viewer_row.role, 1);
    assert!(viewer_row.role < 3, "viewer must not be admin");

    // Admin: role 3
    let hash = password::hash(STRONG_PASSWORD).expect("hash");
    state
        .repos
        .user
        .create_user(1, &admin, &hash, 3)
        .await
        .expect("create admin");
    let admin_row = state
        .repos
        .user
        .find_by_username(1, &admin)
        .await
        .expect("find")
        .expect("admin");
    assert!(admin_row.role >= 3);

    // toggle_admin demotes the admin back to role 1
    let toggled = state
        .repos
        .user
        .toggle_admin(admin_row.id)
        .await
        .expect("toggle");
    assert!(toggled);
    let demoted = state
        .repos
        .user
        .get_user_role(admin_row.id)
        .await
        .expect("role");
    assert_eq!(demoted, Some(1));

    // Toggling a viewer promotes them
    let toggled = state
        .repos
        .user
        .toggle_admin(viewer_id)
        .await
        .expect("toggle");
    assert!(toggled);
    let promoted = state
        .repos
        .user
        .get_user_role(viewer_id)
        .await
        .expect("role");
    assert_eq!(promoted, Some(3));

    cleanup_test_user(state.repos.video.pool(), &viewer).await;
    cleanup_test_user(state.repos.video.pool(), &admin).await;
}

// ── Password reset flow ──

#[tokio::test]
async fn test_password_reset_full_flow() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("reset_flow");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;

    // Give the user a live session token, then request a password reset
    let old_token = state
        .repos
        .user
        .create_token(user_id)
        .await
        .expect("create token");
    let reset_token = state
        .repos
        .user
        .create_password_reset_token(user_id)
        .await
        .expect("create reset token");

    // Consume the reset token
    let found = state
        .repos
        .user
        .find_valid_reset_token(&reset_token)
        .await
        .expect("consume");
    assert_eq!(found, Some(user_id));

    // Reset the password
    let new_password = "NewStr0ng!Pass";
    let hash = password::hash(new_password).expect("hash");
    let updated = state
        .repos
        .user
        .update_password_hash(user_id, &hash)
        .await
        .expect("update hash");
    assert!(updated);

    // Invalidate all sessions (handler behaviour after reset)
    state
        .repos
        .user
        .revoke_tokens_by_user_id(user_id)
        .await
        .expect("revoke");
    let stale = state
        .repos
        .user
        .find_user_by_token(&old_token)
        .await
        .expect("query");
    assert!(
        stale.is_none(),
        "old session token must be invalid after reset"
    );

    // Old password no longer works
    let old_login = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(!old_login.ok, "old password must be rejected after reset");

    // New password works
    let new_login = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: new_password.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");
    assert!(new_login.ok, "new password must work after reset");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_password_reset_token_single_use() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("reset_single");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;

    let token = state
        .repos
        .user
        .create_password_reset_token(user_id)
        .await
        .expect("create");

    assert_eq!(
        state
            .repos
            .user
            .find_valid_reset_token(&token)
            .await
            .expect("first use"),
        Some(user_id)
    );
    assert!(
        state
            .repos
            .user
            .find_valid_reset_token(&token)
            .await
            .expect("second use")
            .is_none(),
        "reset token must be single-use"
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_password_reset_token_expired() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("reset_expire");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;

    let token = state
        .repos
        .user
        .create_password_reset_token(user_id)
        .await
        .expect("create");

    // Force expiry in the DB
    sqlx::query("UPDATE password_reset_tokens SET expires_at = CURRENT_TIMESTAMP - INTERVAL '1 hour' WHERE user_id = $1")
        .bind(user_id)
        .execute(state.repos.video.pool())
        .await
        .expect("expire");

    let found = state
        .repos
        .user
        .find_valid_reset_token(&token)
        .await
        .expect("consume");
    assert!(found.is_none(), "expired reset token must be rejected");

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_password_reset_invalid_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("reset_bogus");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;

    // A structurally valid but unknown token
    let bogus = "B".repeat(64);
    let found = state
        .repos
        .user
        .find_valid_reset_token(&bogus)
        .await
        .expect("consume");
    assert!(found.is_none(), "unknown reset token must be rejected");

    // A garbage short string
    let found = state
        .repos
        .user
        .find_valid_reset_token("nope")
        .await
        .expect("consume");
    assert!(found.is_none());

    // The real user must be unaffected
    let token = state
        .repos
        .user
        .create_password_reset_token(user_id)
        .await
        .expect("create");
    assert_eq!(
        state
            .repos
            .user
            .find_valid_reset_token(&token)
            .await
            .expect("consume"),
        Some(user_id)
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_password_reset_second_token_invalidates_first() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("reset_rotate");

    register_user(&svc, &username, STRONG_PASSWORD).await;
    let user_id = approve_user(&state, &username).await;

    let first = state
        .repos
        .user
        .create_password_reset_token(user_id)
        .await
        .expect("first");
    let second = state
        .repos
        .user
        .create_password_reset_token(user_id)
        .await
        .expect("second");

    // Creating a new token must invalidate the previous one
    assert!(
        state
            .repos
            .user
            .find_valid_reset_token(&first)
            .await
            .expect("first use")
            .is_none(),
        "older reset token must be invalidated"
    );
    assert_eq!(
        state
            .repos
            .user
            .find_valid_reset_token(&second)
            .await
            .expect("second use"),
        Some(user_id)
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── HTTP-level auth flow ──

#[tokio::test]
async fn test_http_register_login_logout_flow() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let (app, pool) = build_http_app().await;
    let username = unique_username("http_flow");

    // Register over HTTP
    let res = send(
        &app,
        post_json(
            "/auth/register",
            serde_json::json!({ "username": username, "password": STRONG_PASSWORD }),
        )
        .await,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res).await;
    assert_eq!(body["ok"], true);
    assert!(body.get("token").is_none(), "viewer must not get a token");

    // Approve via repo, then log in over HTTP
    let user_row = sqlx::query_as::<_, (i64,)>("SELECT id FROM users WHERE username = $1")
        .bind(&username)
        .fetch_one(&pool)
        .await
        .expect("fetch user");
    sqlx::query("UPDATE users SET approved = true WHERE id = $1")
        .bind(user_row.0)
        .execute(&pool)
        .await
        .expect("approve");

    let res = send(
        &app,
        post_json(
            "/auth/login",
            serde_json::json!({ "username": username, "password": STRONG_PASSWORD }),
        )
        .await,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .expect("login must set the token cookie")
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("HttpOnly"), "cookie must be HttpOnly");
    assert!(
        cookie.contains("SameSite=Strict"),
        "cookie must be SameSite=Strict"
    );
    let body = read_json(res).await;
    let token = body["token"].as_str().expect("token in body");

    // /auth/user with the token
    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/auth/user")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res).await;
    assert_eq!(body["username"], username);

    // Logout kills the token
    let res = send(
        &app,
        Request::builder()
            .method(Method::POST)
            .uri("/auth/logout")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/auth/user")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "token must be dead after logout"
    );

    RegistrationRepository::new(pool.clone())
        .set_enabled(false)
        .await
        .expect("restore registration flag");
    cleanup_test_user(&pool, &username).await;
}

#[tokio::test]
async fn test_http_forgot_password_response_consistency() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let (app, pool) = build_http_app().await;
    let username = unique_username("forgot_enum");

    // Register a user with a known email
    let res = send(
        &app,
        post_json(
            "/auth/register",
            serde_json::json!({ "username": username, "password": STRONG_PASSWORD }),
        )
        .await,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let user_row = sqlx::query_as::<_, (i64,)>("SELECT id FROM users WHERE username = $1")
        .bind(&username)
        .fetch_one(&pool)
        .await
        .expect("fetch user");
    sqlx::query("UPDATE users SET email = $1 WHERE id = $2")
        .bind("registered@example.com")
        .bind(user_row.0)
        .execute(&pool)
        .await
        .expect("set email");

    // Registered email vs unregistered email: responses must be IDENTICAL,
    // otherwise the endpoint leaks which emails are registered.
    let res_registered = send(
        &app,
        post_json(
            "/auth/forgot-password",
            serde_json::json!({ "email": "registered@example.com" }),
        )
        .await,
    )
    .await;
    assert_eq!(res_registered.status(), StatusCode::OK);
    let body_registered = read_json(res_registered).await;

    let res_unknown = send(
        &app,
        post_json(
            "/auth/forgot-password",
            serde_json::json!({ "email": "nobody@example.com" }),
        )
        .await,
    )
    .await;
    assert_eq!(res_unknown.status(), StatusCode::OK);
    let body_unknown = read_json(res_unknown).await;

    assert_eq!(
        body_registered, body_unknown,
        "forgot-password must not distinguish registered vs unregistered emails"
    );
    assert_eq!(body_registered["ok"], true);

    // Give the background token creation a moment, then verify a reset token
    // was actually minted for the registered user (the response is constant,
    // but the side effect must happen).
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1 AND NOT used",
    )
    .bind(user_row.0)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert!(
        count >= 1,
        "forgot-password should mint a reset token for a registered email"
    );

    RegistrationRepository::new(pool.clone())
        .set_enabled(false)
        .await
        .expect("restore registration flag");
    cleanup_test_user(&pool, &username).await;
}

#[tokio::test]
async fn test_http_reset_password_invalid_token_rejected() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let (app, pool) = build_http_app().await;

    let res = send(
        &app,
        post_json(
            "/auth/reset-password",
            serde_json::json!({ "token": "BogusToken123", "password": "NewStr0ng!Pass" }),
        )
        .await,
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Weak new password is rejected even before token validation
    let res = send(
        &app,
        post_json(
            "/auth/reset-password",
            serde_json::json!({ "token": "BogusToken123", "password": "password" }),
        )
        .await,
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    RegistrationRepository::new(pool)
        .set_enabled(false)
        .await
        .expect("restore registration flag");
}

#[tokio::test]
async fn test_http_admin_endpoint_requires_admin() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let (app, pool) = build_http_app().await;
    let username = unique_username("http_viewer");

    // Register a normal viewer and approve them
    let res = send(
        &app,
        post_json(
            "/auth/register",
            serde_json::json!({ "username": username, "password": STRONG_PASSWORD }),
        )
        .await,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let user_row = sqlx::query_as::<_, (i64,)>("SELECT id FROM users WHERE username = $1")
        .bind(&username)
        .fetch_one(&pool)
        .await
        .expect("fetch user");
    sqlx::query("UPDATE users SET approved = true WHERE id = $1")
        .bind(user_row.0)
        .execute(&pool)
        .await
        .expect("approve");

    let res = send(
        &app,
        post_json(
            "/auth/login",
            serde_json::json!({ "username": username, "password": STRONG_PASSWORD }),
        )
        .await,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res).await;
    let token = body["token"].as_str().expect("token");

    // A viewer hitting an admin endpoint must get 403
    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/admin/users")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Without any token: 401
    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/admin/users")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    RegistrationRepository::new(pool.clone())
        .set_enabled(false)
        .await
        .expect("restore registration flag");
    cleanup_test_user(&pool, &username).await;
}

#[tokio::test]
async fn test_http_malformed_tokens_rejected() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let (app, pool) = build_http_app().await;

    // Wrong length (63 chars)
    let short = "a".repeat(63);
    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/auth/user")
            .header(header::AUTHORIZATION, format!("Bearer {}", short))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Right length but non-alphanumeric
    let weird = format!("{}!", "b".repeat(63));
    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/auth/user")
            .header(header::AUTHORIZATION, format!("Bearer {}", weird))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Well-formed but unknown 64-char token
    let unknown = "c".repeat(64);
    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/auth/user")
            .header(header::AUTHORIZATION, format!("Bearer {}", unknown))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Cookie-based token with wrong length
    let res = send(
        &app,
        Request::builder()
            .method(Method::GET)
            .uri("/auth/user")
            .header(header::COOKIE, "token=short-cookie")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    RegistrationRepository::new(pool)
        .set_enabled(false)
        .await
        .expect("restore registration flag");
}

// ── Invalid credentials ──

#[tokio::test]
async fn test_login_wrong_password() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("wrong_pw");
    let password = STRONG_PASSWORD;

    // Register first
    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok);

    // Login with wrong password
    let login = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: "wrongpassword".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");

    assert!(!login.ok, "login with wrong password should fail");
    assert!(
        login.token.is_none(),
        "wrong password should not return a token"
    );
    assert!(
        login.error.as_deref().is_some_and(|e| !e.is_empty()),
        "error message should be present"
    );

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);

    let login = svc
        .login(
            &AuthRequest {
                username: unique_username("ghost"),
                password: "whatever123".into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("login");

    assert!(!login.ok, "login for nonexistent user should fail");
    assert!(login.token.is_none());
}

// ── Registration validation ──

#[tokio::test]
async fn test_register_short_password() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("short_pw");

    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: "ab".into(), // too short
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");

    assert!(!reg.ok, "registration with short password should fail");
    assert!(reg.token.is_none());
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("dup_user");
    let password = STRONG_PASSWORD;

    // First registration
    let reg1 = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg1.ok, "first registration should succeed");

    // Second registration with same username
    let reg2 = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(!reg2.ok, "duplicate registration should fail");
    assert!(reg2.token.is_none());
    assert_eq!(reg2.error.as_deref(), Some("用户名已存在"));

    cleanup_test_user(state.repos.video.pool(), &username).await;
}

// ── Rate limiting on login ──

#[tokio::test]
async fn test_rate_limiting_on_login() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let svc = auth_service(&state);
    let username = unique_username("rate_limit");

    // Register a user first
    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: STRONG_PASSWORD.into(),
            },
            "127.0.0.1",
            1,
        )
        .await
        .expect("register");
    assert!(reg.ok);

    // Attempt login with wrong password multiple times to trigger rate limit
    // The rate limiter allows 3 attempts per 60s window
    for i in 0..5 {
        let result = svc
            .login(
                &AuthRequest {
                    username: username.clone(),
                    password: "wrongpassword".into(),
                },
                "127.0.0.1",
                1,
            )
            .await;

        match result {
            Ok(resp) => {
                if i < 3 {
                    // First 3 attempts should return a normal error (not rate limited)
                    assert!(!resp.ok, "attempt {}: wrong password should fail", i);
                }
                // After 3 attempts, the AuthError::RateLimited gets mapped to
                // a response by the handler; at the service level it returns Err
            }
            Err(_) => {
                // Rate limited — this is expected after too many attempts
                assert!(
                    i >= 2,
                    "should not be rate limited before 3 attempts, got error at attempt {}",
                    i
                );
                break;
            }
        }
    }

    cleanup_test_user(state.repos.video.pool(), &username).await;
}
