use crate::config::AppConfig;
use crate::middleware::rate_limit::RateLimiter;
use crate::models::auth::{AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse};
use crate::repositories::user_repo::UserRepository;
use crate::services::playback_service::PlaybackService;
use crate::util::password;

/// Token cookie lifetime (7 days in seconds)
pub const COOKIE_MAX_AGE: i64 = 604800;

/// IP rate limit: 30 attempts per minute per IP.
/// Combined with per-username limit for defense in depth.
const IP_RATE_LIMIT_MAX_ATTEMPTS: u32 = 30;
const IP_RATE_LIMIT_WINDOW_SECS: u64 = 60;
const IP_RATE_LIMIT_BLOCK_SECS: u64 = 0;

/// Minimum password length. Stronger policy than the previous 6-char minimum.
const MIN_PASSWORD_LEN: usize = 10;
const MAX_PASSWORD_LEN: usize = 128;

#[derive(Clone)]
pub struct AuthService {
    user_repo: UserRepository,
    playback_service: PlaybackService,
    rate_limiter: RateLimiter,
    ip_rate_limiter: RateLimiter,
    config: AppConfig,
}

impl AuthService {
    pub fn new(
        user_repo: UserRepository,
        playback_service: PlaybackService,
        rate_limiter: RateLimiter,
        ip_rate_limiter: RateLimiter,
        config: AppConfig,
    ) -> Self {
        Self {
            user_repo,
            playback_service,
            rate_limiter,
            ip_rate_limiter,
            config,
        }
    }

    pub async fn register(
        &self,
        req: &AuthRequest,
        client_ip: &str,
    ) -> Result<AuthResponse, AuthError> {
        if !self.config.registration_enabled() {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: registration disabled");
            return Ok(auth_err("注册功能已关闭"));
        }

        self.check_rate_limits(&req.username, client_ip, "register")
            .await?;

        let username = req.username.trim();
        let password = req.password.trim();

        if username.is_empty() || password.is_empty() {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: empty username or password");
            return Ok(auth_err("用户名和密码不能为空"));
        }

        if username.len() < 2 || username.len() > 64 {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: invalid username length");
            return Ok(auth_err("用户名长度需在 2-64 个字符之间"));
        }

        if password.len() < MIN_PASSWORD_LEN || password.len() > MAX_PASSWORD_LEN {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: invalid password length");
            return Ok(auth_err(format!(
                "密码长度需在 {}-{} 个字符之间",
                MIN_PASSWORD_LEN, MAX_PASSWORD_LEN
            )));
        }

        if !is_password_strong_enough(password) {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: weak password (common or trivial)");
            return Ok(auth_err("密码过于简单，请使用更复杂的密码".to_string()));
        }

        // SECURITY (A07-09 / H-09): foot-gun guard. The first-ever registered
        // user previously became admin automatically if REGISTRATION_ENABLED
        // was true on an empty database. We now require an explicit env var
        // (ALLOW_FIRST_USER_ADMIN=true) to opt in to that behaviour. Without
        // it, the first user is a regular viewer that needs admin approval.
        let count = self.user_repo.count_users().await?;
        let is_first_user = count == 0;
        let first_user_admin = std::env::var("ALLOW_FIRST_USER_ADMIN")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        let role: i16 = if is_first_user && first_user_admin {
            3
        } else {
            1
        };

        let user_exists = self
            .user_repo
            .find_by_username(username)
            .await
            .map(|u| u.is_some())
            .unwrap_or(false);
        if user_exists {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: username already exists");
            // Always hash the password to prevent timing side-channel (username enumeration)
            let _ = password::hash(password);
            return Ok(auth_err("注册失败"));
        }

        let hash = password::hash(password)?;

        let user_id = self.user_repo.create_user(username, &hash, role).await?;

        let reset_key = format!("auth:{}", req.username.trim().to_lowercase());
        self.rate_limiter.reset(&reset_key).await;

        tracing::info!(
            username = %sanitize_for_log(username),
            role = %role,
            "user registered"
        );

        if role >= 3 {
            // First user (admin) — auto-approved, return token
            let token = self.user_repo.create_token(user_id).await?;
            Ok(AuthResponse {
                ok: true,
                token: Some(token),
                error: None,
            })
        } else {
            // Regular user — needs admin approval
            Ok(AuthResponse {
                ok: true,
                token: None,
                error: Some("注册成功，请等待管理员审批后再登录".into()),
            })
        }
    }

    pub async fn login(
        &self,
        req: &AuthRequest,
        client_ip: &str,
    ) -> Result<AuthResponse, AuthError> {
        self.check_rate_limits(&req.username, client_ip, "login")
            .await?;

        // SECURITY (A07-01 / AF-001): close the username-enumeration timing
        // oracle. We previously returned immediately for unknown users
        // *before* running argon2 verify, allowing an attacker to enumerate
        // valid usernames by measuring response time (~50-100 ms gap on LAN).
        // We now always run a dummy argon2 verify when the user is missing,
        // equalising the timing of the "user not found" and "wrong password"
        // branches.
        let user_opt = self.user_repo.find_by_username(req.username.trim()).await?;
        let user_hash: &str = match user_opt.as_ref() {
            Some(u) => u.password_hash.as_str(),
            None => DUMMY_ARGON2_HASH,
        };
        let password_ok = password::verify(&req.password, user_hash).unwrap_or(false);

        let user = match user_opt {
            Some(u) if password_ok => u,
            _ => {
                // Generic error: do not reveal whether the user exists
                tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "failed login");
                return Ok(auth_err("用户名或密码错误"));
            }
        };

        if !user.approved {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "failed login: not approved");
            return Ok(auth_err("用户名或密码错误"));
        }

        // Reject login if user already has an active session (admins exempt)
        if user.role < 3 && self.user_repo.has_active_tokens(user.id).await? {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "failed login: already logged in elsewhere");
            return Ok(auth_err("该用户已在其他设备登录，请先退出后再试"));
        }

        let token = self.user_repo.create_token(user.id).await?;

        let reset_key = format!("auth:{}", req.username.trim().to_lowercase());
        self.rate_limiter.reset(&reset_key).await;

        tracing::info!(username = %sanitize_for_log(&req.username), "user logged in");

        Ok(AuthResponse {
            ok: true,
            token: Some(token),
            error: None,
        })
    }

    pub async fn logout(&self, username: Option<&str>, token: Option<&str>) {
        if let Some(t) = token {
            let _ = self.user_repo.delete_token(t).await;
        }
        tracing::info!(
            username = %sanitize_for_log(username.unwrap_or("<unknown>")),
            "user logged out"
        );
    }

    pub async fn user_info(
        &self,
        username: &str,
        is_admin: bool,
    ) -> Result<UserInfoResponse, AuthError> {
        let user = self
            .user_repo
            .find_by_username(username)
            .await
            .ok()
            .flatten();

        Ok(UserInfoResponse {
            id: user.as_ref().map(|u| u.id).unwrap_or(0),
            username: username.to_string(),
            is_admin,
            created_at: user
                .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
        })
    }

    pub async fn user_profile(
        &self,
        username: &str,
        is_admin: bool,
    ) -> Result<UserProfileResponse, AuthError> {
        let created_at = self
            .user_repo
            .find_by_username(username)
            .await
            .ok()
            .flatten()
            .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();

        let (total_watched, total_time, recent) = self
            .playback_service
            .get_user_profile_data(username)
            .await
            .unwrap_or((0, 0, vec![]));

        Ok(UserProfileResponse {
            username: username.to_string(),
            is_admin,
            created_at,
            total_videos_watched: total_watched,
            total_watch_time_ms: total_time,
            recent_history: recent,
        })
    }

    pub fn cookie_secure(&self) -> bool {
        self.config.cookie_secure
    }

    /// Shared rate limit check: IP-level + per-username.
    /// SECURITY: per-username keying enables an account-DoS where an attacker
    /// who knows a username can lock it out for 10 minutes. The per-username
    /// rate limit uses a short window (3 attempts per minute) and short block
    /// (300s) to limit the impact. IP-level rate limiting provides the primary
    /// brute-force defense.
    async fn check_rate_limits(
        &self,
        username: &str,
        client_ip: &str,
        action: &str,
    ) -> Result<(), AuthError> {
        let ip_key = format!("auth:ip:{}", client_ip);
        if self
            .ip_rate_limiter
            .check_with(
                &ip_key,
                IP_RATE_LIMIT_MAX_ATTEMPTS,
                IP_RATE_LIMIT_WINDOW_SECS,
                IP_RATE_LIMIT_BLOCK_SECS,
            )
            .await
            .is_err()
        {
            tracing::warn!(username = %sanitize_for_log(username), ip = %sanitize_for_log(client_ip), "{} rejected: IP rate limited", action);
            return Err(AuthError::RateLimited);
        }

        let key = format!("auth:{}", username.trim().to_lowercase());
        if self.rate_limiter.check(&key).await.is_err() {
            tracing::warn!(username = %sanitize_for_log(username), ip = %sanitize_for_log(client_ip), "{} rejected: username rate limited", action);
            return Err(AuthError::RateLimited);
        }
        Ok(())
    }
}

/// Strip control characters and newlines from user-supplied values before they
/// are written to logs, so an attacker cannot forge log lines.
fn sanitize_for_log(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// Pre-computed argon2 hash of a random throwaway string, used to keep the
/// "user not found" branch on the same code path as the "wrong password"
/// branch (closing the timing oracle).
/// Generated once with `argon2::hash_encoded`; rotating it is cheap if it
/// ever leaks.
const DUMMY_ARGON2_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$ZHVtbXlzYWx0Zm9yYXRtb3M$Ix2m8ZxRg2E2XgO6nQ8T0q3yJ4cZUEZ5K6yJxY7GQhA";

/// A small, hardcoded list of the most common weak passwords.
/// This is a defense-in-depth check: the user already needs a 10+ char
/// password, so trivial short passwords wouldn't pass length check anyway.
/// This list catches patterns like "password1234" or "qwertyuiop".
const COMMON_WEAK_PASSWORDS: &[&str] = &[
    "password", "password1", "password12", "password123", "password1234",
    "qwerty", "qwerty12", "qwerty123", "qwerty1234", "qwertyuiop",
    "12345678", "123456789", "1234567890",
    "iloveyou", "admin1234", "admin12345", "admin123456",
    "letmein12", "welcome12", "welcome123",
    "abcdefgh", "abcdefghi", "abcdefghij",
    "11111111", "00000000", "12341234", "abcd1234",
    "asdfghjk", "asdfghjkl", "zxcvbnm12", "zxcvbn123",
    "football1", "baseball1", "dragon123", "monkey123",
];

/// Check that the password is not in the common weak-password list
/// and contains at least two character classes (digits, letters, symbols).
fn is_password_strong_enough(pw: &str) -> bool {
    let lower = pw.to_ascii_lowercase();
    if COMMON_WEAK_PASSWORDS.iter().any(|w| lower == *w) {
        return false;
    }
    let has_digit = pw.chars().any(|c| c.is_ascii_digit());
    let has_alpha = pw.chars().any(|c| c.is_ascii_alphabetic());
    let has_symbol = pw.chars().any(|c| !c.is_ascii_alphanumeric());
    // Require at least two of: digit, letter, symbol.
    (has_digit as u8 + has_alpha as u8 + has_symbol as u8) >= 2
}

fn auth_err(msg: impl Into<String>) -> AuthResponse {
    AuthResponse {
        ok: false,
        token: None,
        error: Some(msg.into()),
    }
}

#[derive(Debug)]
pub enum AuthError {
    RateLimited,
    Internal(String),
}

impl From<sqlx::Error> for AuthError {
    fn from(_e: sqlx::Error) -> Self {
        // SECURITY (A03-01 / A09-6): never leak raw sqlx error to clients
        AuthError::Internal("database error".into())
    }
}

impl From<String> for AuthError {
    fn from(_e: String) -> Self {
        AuthError::Internal("internal error".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_common_password() {
        assert!(!is_password_strong_enough("password12"));
        assert!(!is_password_strong_enough("admin12345"));
        assert!(!is_password_strong_enough("welcome123"));
    }

    #[test]
    fn test_rejects_single_class_password() {
        assert!(!is_password_strong_enough("abcdefghij"));
        assert!(!is_password_strong_enough("1234567890"));
    }

    #[test]
    fn test_accepts_mixed_password() {
        assert!(is_password_strong_enough("MyStr0ngPwd"));
        assert!(is_password_strong_enough("Hunter22X!"));
        assert!(is_password_strong_enough("z9k.m3P2z8a"));
    }

    #[test]
    fn test_case_insensitive_common_check() {
        assert!(!is_password_strong_enough("password12"));
        assert!(!is_password_strong_enough("PASSWORD12"));
        assert!(!is_password_strong_enough("Welcome123"));
    }
}
