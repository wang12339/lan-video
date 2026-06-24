use crate::config::AppConfig;
use crate::models::auth::{
    AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse,
};
use crate::repositories::user_repo::UserRepository;
use crate::services::video_service::VideoService;
use crate::util::password;
use crate::middleware::rate_limit::RateLimiter;

/// Token cookie lifetime (7 days in seconds)
pub const COOKIE_MAX_AGE: i64 = 604800;

#[derive(Clone)]
pub struct AuthService {
    user_repo: UserRepository,
    video_service: VideoService,
    rate_limiter: RateLimiter,
    config: AppConfig,
}

impl AuthService {
    pub fn new(
        user_repo: UserRepository,
        video_service: VideoService,
        rate_limiter: RateLimiter,
        config: AppConfig,
    ) -> Self {
        Self {
            user_repo,
            video_service,
            rate_limiter,
            config,
        }
    }

    pub async fn register(
        &self,
        req: &AuthRequest,
    ) -> Result<AuthResponse, AuthError> {
        if !self.config.registration_enabled {
            return Ok(AuthResponse {
                ok: false,
                token: None,
                error: Some("注册功能已关闭".into()),
            });
        }

        let key = format!("auth:{}", req.username.trim().to_lowercase());
        self.rate_limiter.check(&key).await.map_err(|_| AuthError::RateLimited)?;

        let username = req.username.trim();
        let password = req.password.trim();

        if username.is_empty() || password.is_empty() {
            return Ok(AuthResponse {
                ok: false,
                token: None,
                error: Some("用户名和密码不能为空".into()),
            });
        }

        if username.len() < 2 || username.len() > 64 {
            return Ok(AuthResponse {
                ok: false,
                token: None,
                error: Some("用户名长度需在 2-64 个字符之间".into()),
            });
        }

        if password.len() < 6 || password.len() > 128 {
            return Ok(AuthResponse {
                ok: false,
                token: None,
                error: Some("密码长度需在 6-128 个字符之间".into()),
            });
        }

        let count = self.user_repo.count_users().await?;
        let is_admin = count == 0;

        let user_exists = self
            .user_repo
            .find_by_username(username)
            .await
            .map(|u| u.is_some())
            .unwrap_or(false);
        if user_exists {
            // Always hash the password to prevent timing side-channel (username enumeration)
            let _ = password::hash(password);
            return Ok(AuthResponse {
                ok: false,
                token: None,
                error: Some("注册失败".into()),
            });
        }

        let hash = password::hash(password)?;

        let user_id = self
            .user_repo
            .create_user(username, &hash, is_admin)
            .await?;

        let token = self.user_repo.create_token(user_id).await?;

        self.rate_limiter.reset(&key).await;

        tracing::info!(
            username = %username,
            is_admin = %is_admin,
            "user registered"
        );

        Ok(AuthResponse {
            ok: true,
            token: Some(token),
            error: None,
        })
    }

    pub async fn login(
        &self,
        req: &AuthRequest,
    ) -> Result<AuthResponse, AuthError> {
        let key = format!("auth:{}", req.username.trim().to_lowercase());
        self.rate_limiter.check(&key).await.map_err(|_| AuthError::RateLimited)?;

        let user = self
            .user_repo
            .find_by_username(&req.username)
            .await?;

        let user = match user {
            Some(u) => u,
            None => {
                return Ok(AuthResponse {
                    ok: false,
                    token: None,
                    error: Some("用户名或密码错误".into()),
                });
            }
        };

        if !password::verify(&req.password, &user.password_hash)? {
            return Ok(AuthResponse {
                ok: false,
                token: None,
                error: Some("用户名或密码错误".into()),
            });
        }

        let token = self.user_repo.create_token(user.id).await?;

        self.rate_limiter.reset(&key).await;

        tracing::info!(username = %req.username, "user logged in");

        Ok(AuthResponse {
            ok: true,
            token: Some(token),
            error: None,
        })
    }

    pub async fn logout(&self, token: Option<&str>) {
        if let Some(t) = token {
            let _ = self.user_repo.delete_token(t).await;
        }
    }

    pub async fn user_info(
        &self,
        username: &str,
        is_admin: bool,
    ) -> Result<UserInfoResponse, AuthError> {
        let created_at = self
            .user_repo
            .find_by_username(username)
            .await
            .ok()
            .flatten()
            .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();

        Ok(UserInfoResponse {
            username: username.to_string(),
            is_admin,
            created_at,
        })
    }

    pub async fn user_profile(
        &self,
        username: &str,
        is_admin: bool,
    ) -> Result<UserProfileResponse, AuthError> {
        let (total_watched, total_time, recent) = self
            .video_service
            .get_user_profile_data(username)
            .await
            .unwrap_or((0, 0, vec![]));

        let created_at = self
            .user_repo
            .find_by_username(username)
            .await
            .ok()
            .flatten()
            .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();

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
}

#[derive(Debug)]
pub enum AuthError {
    RateLimited,
    Internal(String),
}

impl From<sqlx::Error> for AuthError {
    fn from(e: sqlx::Error) -> Self {
        AuthError::Internal(e.to_string())
    }
}

impl From<String> for AuthError {
    fn from(e: String) -> Self {
        AuthError::Internal(e)
    }
}
