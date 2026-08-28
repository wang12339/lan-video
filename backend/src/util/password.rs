use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use std::fmt;
use std::sync::OnceLock;

pub const MIN_PASSWORD_LEN: usize = 8;
pub const MAX_PASSWORD_LEN: usize = 128;

fn argon2_instance() -> &'static Argon2<'static> {
    static INSTANCE: OnceLock<Argon2<'static>> = OnceLock::new();
    INSTANCE.get_or_init(Argon2::default)
}

#[derive(Debug)]
pub enum PasswordError {
    Hash(argon2::password_hash::Error),
    Parse(argon2::password_hash::Error),
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordError::Hash(e) => write!(f, "password hashing failed: {}", e),
            PasswordError::Parse(e) => write!(f, "password hash parse failed: {}", e),
        }
    }
}

impl std::error::Error for PasswordError {}

pub fn hash(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2_instance()
        .hash_password(password.as_bytes(), &salt)
        .map_err(PasswordError::Hash)?;
    Ok(hash.to_string())
}

pub fn verify(password: &str, hash: &str) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(hash).map_err(PasswordError::Parse)?;
    Ok(argon2_instance()
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
