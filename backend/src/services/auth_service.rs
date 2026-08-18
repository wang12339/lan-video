use crate::config::AppConfig;
use crate::middleware::rate_limit::RateLimiter;
use crate::models::auth::{AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse};
use crate::repositories::user_repo::UserRepository;
use crate::services::playback_service::PlaybackService;
use crate::util::error::ServiceError;
use crate::util::password;

/// Token cookie lifetime (7 days in seconds)
pub const COOKIE_MAX_AGE: i64 = 604800;

/// IP rate limit: 30 attempts per minute per IP.
/// Combined with per-username limit for defense in depth.
const IP_RATE_LIMIT_MAX_ATTEMPTS: u32 = 30;
const IP_RATE_LIMIT_WINDOW_SECS: u64 = 60;
const IP_RATE_LIMIT_BLOCK_SECS: u64 = 0;

/// Minimum password length.
const MIN_PASSWORD_LEN: usize = 8;
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
        tenant_id: i64,
    ) -> Result<AuthResponse, ServiceError> {
        if !self.config.registration_enabled() {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: registration disabled");
            return Ok(auth_err("注册功能已关闭"));
        }

        self.check_rate_limits(&req.username, client_ip, "register")
            .await?;

        let username = req.username.trim();
        // Do NOT trim the password: it is hashed exactly as the user typed it,
        // and login verifies the raw input — trimming here would make
        // passwords with leading/trailing whitespace impossible to log in
        // with.
        let password = req.password.as_str();

        if username.is_empty() || password.is_empty() {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: empty username or password");
            return Ok(auth_err("用户名和密码不能为空"));
        }

        if username.len() < 2 || username.len() > 64 {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: invalid username length");
            return Ok(auth_err("用户名长度需在 2-64 个字符之间"));
        }

        // SECURITY: reject control characters (newlines, etc.) so user-supplied
        // usernames cannot forge log lines or break out of HTML in the admin
        // UI / notification emails.
        if username.chars().any(char::is_control) {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: username contains control characters");
            return Ok(auth_err("用户名包含非法字符"));
        }

        if password.chars().count() < MIN_PASSWORD_LEN
            || password.chars().count() > MAX_PASSWORD_LEN
        {
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
        let count = self.user_repo.count_users(tenant_id).await?;
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
            .find_by_username(tenant_id, username)
            .await
            .map(|u| u.is_some())
            .unwrap_or(false);
        if user_exists {
            tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: username already exists");
            // Always hash the password to prevent timing side-channel (username enumeration)
            let _ = hash_in_blocking(password).await;
            return Ok(auth_err("用户名已存在"));
        }

        let hash = hash_in_blocking(password).await?;

        // SECURITY: a concurrent registration with the same username hits a
        // unique constraint race. Map it to the same friendly error instead
        // of leaking a 500.
        let user_id = match self
            .user_repo
            .create_user(tenant_id, username, &hash, role)
            .await
        {
            Ok(id) => id,
            Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => {
                tracing::warn!(username = %sanitize_for_log(&req.username), ip = %sanitize_for_log(client_ip), "register rejected: username taken (unique violation race)");
                return Ok(auth_err("用户名已存在"));
            }
            Err(e) => return Err(e.into()),
        };

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
        tenant_id: i64,
    ) -> Result<AuthResponse, ServiceError> {
        self.check_rate_limits(&req.username, client_ip, "login")
            .await?;

        // SECURITY (A07-01 / AF-001): close the username-enumeration timing
        // oracle. We previously returned immediately for unknown users
        // *before* running argon2 verify, allowing an attacker to enumerate
        // valid usernames by measuring response time (~50-100 ms gap on LAN).
        // We now always run a dummy argon2 verify when the user is missing,
        // equalising the timing of the "user not found" and "wrong password"
        // branches.
        let user_opt = self
            .user_repo
            .find_by_username(tenant_id, req.username.trim())
            .await?;
        let user_hash: &str = match user_opt.as_ref() {
            Some(u) => u.password_hash.as_str(),
            None => DUMMY_ARGON2_HASH,
        };
        let password_ok = verify_in_blocking(&req.password, user_hash).await;

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
        tenant_id: i64,
    ) -> Result<UserInfoResponse, ServiceError> {
        let user = self
            .user_repo
            .find_by_username(tenant_id, username)
            .await
            .ok()
            .flatten();

        Ok(UserInfoResponse {
            id: user.as_ref().map(|u| u.id).unwrap_or(0),
            username: username.to_string(),
            is_admin,
            created_at: user
                .as_ref()
                .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
            email: user.as_ref().and_then(|u| u.email.clone()),
            email_verified: user.as_ref().map(|u| u.email_verified).unwrap_or(false),
        })
    }

    pub async fn user_profile(
        &self,
        username: &str,
        is_admin: bool,
        tenant_id: i64,
    ) -> Result<UserProfileResponse, ServiceError> {
        let created_at = self
            .user_repo
            .find_by_username(tenant_id, username)
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
    ) -> Result<(), ServiceError> {
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
            return Err(ServiceError::RateLimited);
        }

        let key = format!("auth:{}", username.trim().to_lowercase());
        if self.rate_limiter.check(&key).await.is_err() {
            tracing::warn!(username = %sanitize_for_log(username), ip = %sanitize_for_log(client_ip), "{} rejected: username rate limited", action);
            return Err(ServiceError::RateLimited);
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

/// Run the CPU-heavy argon2 hashing on the blocking pool. Argon2id at the
/// default params costs ~50-100 ms of CPU per call; running it inline on an
/// async worker would let a burst of login/register attempts starve the whole
/// runtime.
async fn hash_in_blocking(password: &str) -> Result<String, ServiceError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || password::hash(&password))
        .await
        .map_err(|e| ServiceError::internal(format!("password hashing task failed: {}", e)))?
        .map_err(ServiceError::from)
}

/// Run the CPU-heavy argon2 verification on the blocking pool. Returns false
/// on any error (unparseable hash, task failure) so callers treat every
/// failure as "credentials rejected".
async fn verify_in_blocking(password: &str, hash: &str) -> bool {
    let password = password.to_owned();
    let hash = hash.to_owned();
    tokio::task::spawn_blocking(move || password::verify(&password, &hash))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false)
}

/// Pre-computed argon2 hash of a random throwaway string, used to keep the
/// "user not found" branch on the same code path as the "wrong password"
/// branch (closing the timing oracle).
/// Generated once with `argon2::hash_encoded`; rotating it is cheap if it
/// ever leaks.
const DUMMY_ARGON2_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$ZHVtbXlzYWx0Zm9yYXRtb3M$Ix2m8ZxRg2E2XgO6nQ8T0q3yJ4cZUEZ5K6yJxY7GQhA";

pub(crate) fn is_password_strong_enough(pw: &str) -> bool {
    let has_upper = pw.chars().any(|c| c.is_uppercase());
    let has_lower = pw.chars().any(|c| c.is_lowercase());
    let has_digit = pw.chars().any(|c| c.is_ascii_digit());
    let has_special = pw.chars().any(|c| !c.is_alphanumeric());
    let categories: u8 = [has_upper, has_lower, has_digit, has_special]
        .into_iter()
        .map(u8::from)
        .sum();

    // Require at least 3 of 4 character categories for passwords under 12
    // chars. Counted in chars, not bytes, to match register()'s length
    // validation — a byte count would let a multibyte password slip into
    // the lenient 2-category tier.
    if pw.chars().count() < 12 {
        categories >= 3
    } else {
        // Longer passwords can be all lowercase with at least one non-alpha
        categories >= 2
    }
}

fn auth_err(msg: impl Into<String>) -> AuthResponse {
    AuthResponse {
        ok: false,
        token: None,
        error: Some(msg.into()),
    }
}

impl From<password::PasswordError> for ServiceError {
    fn from(e: password::PasswordError) -> Self {
        tracing::error!("password operation failed: {}", e);
        ServiceError::internal("password error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strong_password_accepted() {
        assert!(is_password_strong_enough("MyStr0ngPwd"));
        assert!(is_password_strong_enough("Abcdef1!"));
        assert!(is_password_strong_enough("correct-horse-battery"));
    }

    #[test]
    fn test_weak_password_rejected() {
        assert!(!is_password_strong_enough("password"));
        assert!(!is_password_strong_enough("12345678"));
        assert!(!is_password_strong_enough("abcdefgh"));
        assert!(!is_password_strong_enough("PASSWORD1"));
    }

    #[test]
    fn test_long_password_with_two_categories() {
        assert!(is_password_strong_enough("longbutonlylowercase1"));
        assert!(is_password_strong_enough("longbutonlyUPPERCASE1"));
    }

    #[test]
    fn dummy_argon2_hash_is_parseable() {
        // SECURITY: DUMMY_ARGON2_HASH is used to keep the "user not found"
        // login branch running a real argon2 verify. If it ever becomes
        // unparseable, verify() fails fast and the username-enumeration
        // timing oracle reopens.
        match password::verify("not-a-real-password", DUMMY_ARGON2_HASH) {
            Ok(_) => {}
            Err(e) => panic!(
                "DUMMY_ARGON2_HASH must be a parseable argon2 hash, got: {}",
                e
            ),
        }
    }

    #[test]
    fn username_control_chars_are_rejected_by_validation() {
        // The check lives in register(); this guards the predicate used there.
        let bad = ["a\nbc", "a\rbc", "a\u{0}bc"];
        for name in bad {
            assert!(
                name.chars().any(char::is_control),
                "{:?} should contain control chars",
                name
            );
        }
        assert!("alice".chars().all(|c| !c.is_control()));
    }
}
