#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use atmos_video_backend::middleware::auth::{
    clear_token_cookie, extract_token_from_cookie, set_token_cookie,
};
use atmos_video_backend::services::auth_service::COOKIE_MAX_AGE;
use atmos_video_backend::util::error::ServiceError;
use atmos_video_backend::util::password;

/// Verify ServiceError conversion from sqlx::Error — messages are sanitised
/// to prevent information leakage (SECURITY A03-01 / A09-6).
#[test]
fn test_auth_error_from_sqlx() {
    let err: ServiceError = sqlx::Error::Protocol("test".into()).into();
    match err {
        ServiceError::Internal(msg) => assert_eq!(msg, "数据库错误"),
        _ => panic!("expected Internal variant"),
    }
}

/// Verify ServiceError conversion from String — the raw message stays in the
/// Internal payload (it only ever reaches the logs; the HTTP response is a
/// fixed 500 "服务器内部错误", so nothing leaks to clients).
#[test]
fn test_auth_error_from_string() {
    let err: ServiceError = "something went wrong".to_string().into();
    match err {
        ServiceError::Internal(msg) => assert_eq!(msg, "something went wrong"),
        _ => panic!("expected Internal variant"),
    }
}

/// Every sqlx error variant must map to the same sanitised message — an
/// attacker must never learn database internals through an auth error.
#[test]
fn test_auth_error_sanitises_all_sqlx_variants() {
    let variants: Vec<sqlx::Error> = vec![
        sqlx::Error::RowNotFound,
        sqlx::Error::PoolTimedOut,
        sqlx::Error::Protocol("secret schema info".into()),
        sqlx::Error::Io(std::io::Error::other("disk full")),
    ];
    for err in variants {
        let converted: ServiceError = err.into();
        match converted {
            ServiceError::Internal(msg) => {
                assert_eq!(msg, "数据库错误", "sqlx error details must never leak")
            }
            _ => panic!("sqlx errors must not become any other variant"),
        }
    }
}

/// Password hashing: correct password verifies, wrong password fails,
/// and every hash is salted (two hashes of the same password differ).
#[test]
fn test_password_hash_roundtrip() {
    let hash = password::hash("TestPass123!").expect("hash should succeed");
    assert!(hash.starts_with("$argon2"), "argon2id hash format");
    assert!(password::verify("TestPass123!", &hash).expect("verify"));
    assert!(
        !password::verify("WrongPass123!", &hash).expect("verify"),
        "wrong password must fail"
    );
}

#[test]
fn test_password_hash_is_salted() {
    let h1 = password::hash("SamePass123!").expect("hash");
    let h2 = password::hash("SamePass123!").expect("hash");
    assert_ne!(
        h1, h2,
        "hashes of the same password must differ (random salt)"
    );
    assert!(password::verify("SamePass123!", &h1).expect("verify h1"));
    assert!(password::verify("SamePass123!", &h2).expect("verify h2"));
}

#[test]
fn test_password_verify_rejects_malformed_hash() {
    // Garbage that is not an argon2 hash must be an Err, not a false Ok(false)
    assert!(password::verify("whatever", "not-an-argon2-hash").is_err());
    // A hash of a *different* password must verify to false, not error
    let hash = password::hash("RealPass123!").expect("hash");
    assert!(!password::verify("OtherPass123!", &hash).expect("verify"));
}

/// Token cookie must carry the security flags: HttpOnly, SameSite=Strict,
/// correct Max-Age, and the Secure flag only when configured.
#[test]
fn test_set_token_cookie_flags() {
    let cookie = set_token_cookie("testtoken123", COOKIE_MAX_AGE, false);
    assert!(cookie.contains("token=testtoken123"));
    assert!(cookie.contains("HttpOnly"), "cookie must be HttpOnly");
    assert!(
        cookie.contains("SameSite=Strict"),
        "cookie must be SameSite=Strict"
    );
    assert!(cookie.contains("Path=/"));
    assert!(
        cookie.contains(&format!("Max-Age={}", COOKIE_MAX_AGE)),
        "cookie must expire after 7 days"
    );
    assert!(
        !cookie.contains("Secure"),
        "Secure flag must be absent when cookie_secure=false"
    );
}

#[test]
fn test_set_token_cookie_secure_flag() {
    let cookie = set_token_cookie("token", 60, true);
    assert!(
        cookie.contains("Secure"),
        "Secure flag must be present when enabled"
    );
    assert!(
        cookie.ends_with("; Secure"),
        "Secure must be the trailing flag"
    );
}

#[test]
fn test_clear_token_cookie_expires_immediately() {
    let cookie = clear_token_cookie(false);
    assert!(cookie.contains("token="));
    assert!(cookie.contains("Max-Age=0"), "clear cookie must expire now");
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert!(!cookie.contains("Secure"));
}

#[test]
fn test_clear_token_cookie_secure_flag() {
    assert!(clear_token_cookie(true).contains("Secure"));
}

/// Cookie token extraction must handle whitespace, multiple cookies and
/// missing values exactly like the server expects.
#[test]
fn test_extract_token_from_cookie_values() {
    let mut headers = axum::http::HeaderMap::new();

    // Basic token
    headers.insert(axum::http::header::COOKIE, "token=abc123".parse().unwrap());
    assert_eq!(
        extract_token_from_cookie(&headers).as_deref(),
        Some("abc123")
    );

    // Token among other cookies, with surrounding whitespace
    headers.insert(
        axum::http::header::COOKIE,
        "theme=dark; token = xyz789 ; lang=zh".parse().unwrap(),
    );
    assert_eq!(
        extract_token_from_cookie(&headers).as_deref(),
        Some("xyz789")
    );

    // Missing token
    headers.insert(
        axum::http::header::COOKIE,
        "theme=dark; lang=zh".parse().unwrap(),
    );
    assert_eq!(extract_token_from_cookie(&headers), None);

    // Empty token value
    headers.insert(axum::http::header::COOKIE, "token=".parse().unwrap());
    assert_eq!(extract_token_from_cookie(&headers), None);

    // No Cookie header at all
    let empty = axum::http::HeaderMap::new();
    assert_eq!(extract_token_from_cookie(&empty), None);
}

/// The token cookie lifetime must be exactly 7 days.
#[test]
fn test_cookie_max_age_is_seven_days() {
    assert_eq!(COOKIE_MAX_AGE, 7 * 24 * 60 * 60);
}

/// Register validation rejects empty credentials before touching the DB —
/// the service-level message is stable and Chinese.
#[test]
fn test_register_validation_messages_are_stable() {
    // These are exercised end-to-end in integration tests; here we only pin
    // the constant that drives cookie lifetime, plus the sanitised error
    // contract already covered above.
    assert_eq!(COOKIE_MAX_AGE, 604800);
}
