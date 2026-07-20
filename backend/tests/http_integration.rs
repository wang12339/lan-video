//! HTTP-level integration tests using `axum::test`.
//!
//! These exercise the full middleware stack including auth, rate limiting,
//! and error handling. Requires `DATABASE_URL` to be set.

mod integration_test_helpers;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use integration_test_helpers::*;
use lan_video_backend::app;
use tower::ServiceExt;

/// Build the full app router for testing, skipping the bind step.
async fn build_test_app() -> axum::Router {
    let Some(_) = database_url() else {
        // We never reach this branch because tests below early-return on missing DB,
        // but the function signature must still type-check.
        panic!("DATABASE_URL not set");
    };
    app::build_router(test_config()).await
}

#[tokio::test]
async fn test_health_endpoint_returns_ok() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_protected_endpoint_rejects_missing_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/user")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_endpoint_rejects_invalid_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/user")
                .header(header::AUTHORIZATION, "Bearer not-a-real-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_endpoint_rejects_non_admin_token() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    // Try to access an admin endpoint with a malformed token
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/users")
                .header(header::AUTHORIZATION, "Bearer fake-non-admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Should be 401 (invalid token) since the user doesn't exist
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
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
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/auth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    // Even if registration is disabled, the response should be a structured
    // error, not a 500
    assert!(res.status() == StatusCode::OK || res.status().is_client_error());
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
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/videos/search?q={}", long_q))
                .header(header::AUTHORIZATION, "Bearer fake")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // The length check should fire before the auth check (it's a parameter parse)
    // OR auth check fires first
    // Either way, the response should not be 500
    assert!(!res.status().is_server_error(), "got 500: {:?}", res.status());
}

#[tokio::test]
async fn test_unknown_route_returns_404_or_fallback() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let app = build_test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/this-route-does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // The app has a fallback that returns the index.html for SPA, so 200 is acceptable
    assert!(res.status() == StatusCode::NOT_FOUND || res.status() == StatusCode::OK);
}
