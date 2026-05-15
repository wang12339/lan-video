// Standalone test to verify password hashing/verification works
// Run with: cargo test --test test_password -- --nocapture

use lan_video_backend::util::password;

#[test]
fn test_password_roundtrip() {
    let password = "admin123";
    let hash = password::hash(password).unwrap();
    let result = password::verify(password, &hash).unwrap();
    assert!(result);
}

#[test]
fn test_password_wrong() {
    let password = "admin123";
    let hash = password::hash(password).unwrap();
    let result = password::verify("wrongpassword", &hash).unwrap();
    assert!(!result);
}
