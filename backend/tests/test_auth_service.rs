use lan_video_backend::services::auth_service::{AuthError, AuthService};

/// Verify AuthError conversion from sqlx::Error — messages are sanitised
/// to prevent information leakage (SECURITY A03-01 / A09-6).
#[test]
fn test_auth_error_from_sqlx() {
    let err: AuthError = sqlx::Error::Protocol("test".into()).into();
    match err {
        AuthError::Internal(msg) => assert_eq!(msg, "database error"),
        _ => panic!("expected Internal variant"),
    }
}

/// Verify AuthError conversion from String — messages are sanitised.
#[test]
fn test_auth_error_from_string() {
    let err: AuthError = "something went wrong".to_string().into();
    match err {
        AuthError::Internal(msg) => assert_eq!(msg, "internal error"),
        _ => panic!("expected Internal variant"),
    }
}
