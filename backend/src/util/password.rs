use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

pub fn hash(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("hashing error: {}", e))?;
    Ok(hash.to_string())
}

pub fn verify(password: &str, hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hash).map_err(|e| format!("parse error: {}", e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "test_password_123";
        let hash = hash(password).expect("hashing should succeed");
        assert!(
            hash.starts_with("$argon2"),
            "hash should start with argon2 identifier"
        );
        assert!(
            verify(password, &hash).unwrap(),
            "should verify correct password"
        );
        assert!(
            !verify("wrong_password", &hash).unwrap(),
            "should reject wrong password"
        );
    }

    #[test]
    fn test_verify_invalid_hash() {
        let result = verify("password", "invalid_hash");
        assert!(
            result.is_err(),
            "should return error for invalid hash format"
        );
    }

    #[test]
    fn test_empty_password() {
        let hash = hash("").expect("hashing empty string should succeed");
        assert!(verify("", &hash).unwrap(), "should verify empty password");
        assert!(
            !verify("not_empty", &hash).unwrap(),
            "should reject non-empty for empty hash"
        );
    }

    #[test]
    fn test_hash_determinism() {
        // Same password should produce different hashes each time (different salt)
        let pw = "same_password";
        let h1 = hash(pw).unwrap();
        let h2 = hash(pw).unwrap();
        assert_ne!(h1, h2, "hashes should differ due to random salt");
        assert!(verify(pw, &h1).unwrap(), "h1 should verify");
        assert!(verify(pw, &h2).unwrap(), "h2 should verify");
    }
}
