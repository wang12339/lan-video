#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Standalone test to verify password hashing/verification works
// Run with: cargo test --test test_password -- --nocapture

use atmos_video_backend::util::password;

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

#[test]
fn test_password_hash_structure() {
    // A PHC-formatted hash: $argon2id$v=19$<params>$<salt>$<hash>
    let hash = password::hash("structure").unwrap();
    assert!(hash.starts_with("$argon2id"), "default should be argon2id");
    assert!(
        hash.contains("$v=19$"),
        "should embed argon2 version: {hash}"
    );
    assert!(hash.contains("$m="), "should embed memory cost: {hash}");
    assert!(hash.contains(",t="), "should embed iteration count: {hash}");
    assert!(hash.contains(",p="), "should embed parallelism: {hash}");
    assert_eq!(hash.split('$').count(), 6, "PHC string has 6 '$' sections");
}

#[test]
fn test_password_empty_hash_still_argon2() {
    let hash = password::hash("").unwrap();
    assert!(
        hash.starts_with("$argon2id"),
        "empty password must still produce a valid argon2 hash"
    );
    assert!(password::verify("", &hash).unwrap());
    assert!(!password::verify("not_empty", &hash).unwrap());
}

#[test]
fn test_password_very_long_roundtrip() {
    // 4096 chars — argon2 has no practical input length limit
    let password = "x".repeat(4096);
    let hash = password::hash(&password).unwrap();
    assert!(password::verify(&password, &hash).unwrap());
    assert!(!password::verify("y", &hash).unwrap());
}

#[test]
fn test_password_null_byte_roundtrip() {
    // NUL bytes are legal in Rust strings and must be hashed as-is
    let password = "pass\u{0}word\u{0}";
    let hash = password::hash(password).unwrap();
    assert!(password::verify(password, &hash).unwrap());
    assert!(!password::verify("password", &hash).unwrap());
}

#[test]
fn test_password_hashes_embed_distinct_salts() {
    use argon2::password_hash::PasswordHash;

    let hash1 = password::hash("saltcheck").unwrap();
    let hash2 = password::hash("saltcheck").unwrap();

    let p1 = PasswordHash::new(&hash1).unwrap();
    let p2 = PasswordHash::new(&hash2).unwrap();

    let salt1 = p1.salt.map(|s| s.to_string());
    let salt2 = p2.salt.map(|s| s.to_string());
    assert!(salt1.is_some(), "hash should embed a salt: {hash1}");
    assert!(salt2.is_some(), "hash should embed a salt: {hash2}");
    assert_ne!(salt1, salt2, "each hash must use a fresh random salt");
    assert_eq!(
        p1.params.to_string(),
        p2.params.to_string(),
        "cost parameters must be identical across hashes"
    );
}

#[test]
fn test_password_verify_wrong_password_is_ok_false_not_error() {
    let hash = password::hash("correct_password").unwrap();
    let result = password::verify("wrong_password", &hash);
    assert!(
        matches!(result, Ok(false)),
        "wrong password must be Ok(false), not an error: {result:?}"
    );
}

#[test]
fn test_password_verify_empty_hash_string() {
    assert!(password::verify("password", "").is_err());
}

#[test]
fn test_password_verify_whitespace_hash_string() {
    assert!(password::verify("password", "   ").is_err());
}

#[test]
fn test_password_verify_algorithm_prefix_only() {
    // NOTE: password-hash's parser is lenient — a bare algorithm tag parses
    // and fails at verification (Ok(false)) rather than at parsing (Err).
    // Either way it must never verify.
    let result = password::verify("password", "$argon2id");
    assert!(
        !matches!(result, Ok(true)),
        "incomplete PHC string must never verify: {result:?}"
    );
}

#[test]
fn test_password_verify_invalid_base64_salt() {
    // Structured PHC string with an invalid base64 salt must fail parsing
    let result = password::verify(
        "password",
        "$argon2id$v=19$m=19456,t=2,p=1$!!!$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    );
    assert!(result.is_err(), "invalid base64 salt should be rejected");
}

#[test]
fn test_password_verify_wrong_algorithm_prefix() {
    // A well-formed hash with the algorithm tag swapped must not verify
    let hash = password::hash("password").unwrap();
    let mut tampered = hash.clone();
    tampered.replace_range(0.."$argon2id".len(), "$argon2d");
    let result = password::verify("password", &tampered);
    assert!(
        result.is_err() || matches!(result, Ok(false)),
        "hash from another algorithm family must not verify: {result:?}"
    );
}

#[test]
fn test_password_verify_truncated_hash() {
    let hash = password::hash("password").unwrap();
    let truncated = &hash[..hash.len() / 2];
    // Lenient parser: a truncated string verifies false rather than erroring —
    // either way it must never verify.
    let result = password::verify("password", truncated);
    assert!(
        !matches!(result, Ok(true)),
        "truncated hash must never verify: {result:?}"
    );
}

#[test]
fn test_password_verify_tampered_hash_payload() {
    let hash = password::hash("password").unwrap();
    let mut chars: Vec<char> = hash.chars().collect();
    *chars.last_mut().unwrap() = if *chars.last().unwrap() == 'A' {
        'B'
    } else {
        'A'
    };
    let tampered: String = chars.into_iter().collect();
    assert_ne!(tampered, hash);
    let result = password::verify("password", &tampered);
    assert!(
        !matches!(result, Ok(true)),
        "tampered hash must never verify: {result:?}"
    );
}
