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

#[test]
fn test_password_unicode() {
    let password = "密码测试123";
    let hash = password::hash(password).unwrap();
    assert!(password::verify(password, &hash).unwrap());
    assert!(!password::verify("wrong", &hash).unwrap());
}

#[test]
fn test_password_special_characters() {
    let password = "p@$$w0rd!#%^&*()_+-=[]{}|;':\",./<>?";
    let hash = password::hash(password).unwrap();
    assert!(password::verify(password, &hash).unwrap());
}

#[test]
fn test_password_long() {
    let password = "a".repeat(128);
    let hash = password::hash(&password).unwrap();
    assert!(password::verify(&password, &hash).unwrap());
    assert!(!password::verify("short", &hash).unwrap());
}

#[test]
fn test_password_empty() {
    let hash = password::hash("").unwrap();
    assert!(password::verify("", &hash).unwrap());
    assert!(!password::verify("not_empty", &hash).unwrap());
}

#[test]
fn test_password_whitespace_sensitive() {
    let hash = password::hash("password").unwrap();
    // Leading/trailing whitespace should make it different
    assert!(!password::verify(" password", &hash).unwrap());
    assert!(!password::verify("password ", &hash).unwrap());
    assert!(!password::verify(" password ", &hash).unwrap());
}

#[test]
fn test_password_case_sensitive() {
    let hash = password::hash("Password").unwrap();
    assert!(!password::verify("password", &hash).unwrap());
    assert!(!password::verify("PASSWORD", &hash).unwrap());
    assert!(password::verify("Password", &hash).unwrap());
}

#[test]
fn test_password_invalid_hash_format() {
    let result = password::verify("password", "not_a_valid_hash");
    assert!(result.is_err());
}

#[test]
fn test_password_different_hashes_still_verify() {
    let password = "test_password";
    let hash1 = password::hash(password).unwrap();
    let hash2 = password::hash(password).unwrap();
    // Different salts produce different hashes
    assert_ne!(hash1, hash2);
    // But both verify correctly
    assert!(password::verify(password, &hash1).unwrap());
    assert!(password::verify(password, &hash2).unwrap());
}

#[test]
fn test_password_hash_format() {
    let hash = password::hash("test").unwrap();
    assert!(
        hash.starts_with("$argon2"),
        "hash should use argon2 algorithm"
    );
}
