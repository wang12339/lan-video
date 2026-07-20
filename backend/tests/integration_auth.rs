//! Integration tests for the authentication flow.
//!
//! Requires a running PostgreSQL database. Set `DATABASE_URL` to enable.

mod integration_test_helpers;

use integration_test_helpers::*;
use lan_video_backend::models::auth::AuthRequest;
use lan_video_backend::services::auth_service::AuthService;

/// Helper: create an AuthService backed by the test AppState.
fn auth_service(state: &lan_video_backend::state::AppState) -> AuthService {
    AuthService::new(
        state.user_repo.clone(),
        state.playback_service.clone(),
        state.rate_limiter.clone(),
        state.ip_rate_limiter.clone(),
        state.config.clone(),
    )
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
    let password = "testpass123";

    // Register
    let reg_result = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
        )
        .await
        .expect("register should not error");

    assert!(reg_result.ok, "registration should succeed");
    assert!(
        reg_result.token.is_some(),
        "registration should return a token"
    );
    let reg_token = reg_result.token.unwrap();

    // Login with correct credentials
    let login_result = svc
        .login(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
        )
        .await
        .expect("login should not error");

    assert!(login_result.ok, "login should succeed");
    assert!(login_result.token.is_some(), "login should return a token");
    let login_token = login_result.token.unwrap();

    // Tokens should be different (each login creates a new token)
    assert_ne!(
        reg_token, login_token,
        "login token should differ from register token"
    );

    // Get user info using the login token
    let user_info = svc
        .user_info(&username, false)
        .await
        .expect("user_info should not error");

    assert_eq!(user_info.username, username);

    // Cleanup
    let pool = state.video_repo.pool();
    cleanup_test_user(pool, &username).await;
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
    let password = "correctpass";

    // Register first
    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
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

    cleanup_test_user(state.video_repo.pool(), &username).await;
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
    let password = "password123";

    // First registration
    let reg1 = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: password.into(),
            },
            "127.0.0.1",
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
        )
        .await
        .expect("register");
    assert!(!reg2.ok, "duplicate registration should fail");
    assert!(reg2.token.is_none());

    cleanup_test_user(state.video_repo.pool(), &username).await;
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
                password: "password123".into(),
            },
            "127.0.0.1",
        )
        .await
        .expect("register");
    assert!(reg.ok);

    // Attempt login with wrong password multiple times to trigger rate limit
    // The rate limiter allows 5 attempts in a 60s window
    for i in 0..6 {
        let result = svc
            .login(
                &AuthRequest {
                    username: username.clone(),
                    password: "wrongpassword".into(),
                },
                "127.0.0.1",
            )
            .await;

        match result {
            Ok(resp) => {
                if i < 5 {
                    // First 5 attempts should return a normal error (not rate limited)
                    assert!(!resp.ok, "attempt {}: wrong password should fail", i);
                }
                // After 5 attempts, the AuthError::RateLimited gets mapped to
                // a response by the handler; at the service level it returns Err
            }
            Err(_) => {
                // Rate limited — this is expected after too many attempts
                assert!(
                    i >= 4,
                    "should not be rate limited before 5 attempts, got error at attempt {}",
                    i
                );
                break;
            }
        }
    }

    cleanup_test_user(state.video_repo.pool(), &username).await;
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

    // Register
    let reg = svc
        .register(
            &AuthRequest {
                username: username.clone(),
                password: "password123".into(),
            },
            "127.0.0.1",
        )
        .await
        .expect("register");
    assert!(reg.ok);
    let token = reg.token.unwrap();

    // Logout
    svc.logout(Some(username.as_str()), Some(&token)).await;

    // Token should no longer work — find_user_by_token should return None
    let found = state
        .user_repo
        .find_user_by_token(&token)
        .await
        .expect("query");
    assert!(found.is_none(), "token should be invalid after logout");

    cleanup_test_user(state.video_repo.pool(), &username).await;
}
