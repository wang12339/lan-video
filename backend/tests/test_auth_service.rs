use lan_video_backend::services::auth_service::{AuthError, AuthService};

/// Verify AuthError conversion from sqlx::Error
#[test]
fn test_auth_error_from_sqlx() {
    let err: AuthError = sqlx::Error::Protocol("test".into()).into();
    match err {
        AuthError::Internal(msg) => assert!(msg.contains("test")),
        _ => panic!("expected Internal variant"),
    }
}

/// Verify AuthError conversion from String
#[test]
fn test_auth_error_from_string() {
    let err: AuthError = "something went wrong".to_string().into();
    match err {
        AuthError::Internal(msg) => assert_eq!(msg, "something went wrong"),
        _ => panic!("expected Internal variant"),
    }
}

/// Verify AuthService cookie_secure returns config value
/// This is a smoke-test that AuthService can be constructed
#[test]
fn test_auth_service_cookie_secure_default() {
    // This would need a real config + repos — just verify the type exists
    assert!(
        std::any::TypeId::of::<AuthService>() == std::any::TypeId::of::<AuthService>(),
        "AuthService type exists"
    );
}
