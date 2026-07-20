use crate::repositories::share_repo::{ShareLink, ShareRepository};
use crate::util::response::ErrorResponse;
use axum::{http::StatusCode, Json};
use rand::Rng;

#[derive(Clone)]
pub struct ShareService {
    repo: ShareRepository,
}

pub enum ShareError {
    NotFound,
    Forbidden,
    Invalid(String),
    Internal(String),
}

impl ShareError {
    pub fn into_response(self) -> (StatusCode, Json<ErrorResponse>) {
        match self {
            ShareError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse { error: "分享链接不存在".into() }),
            ),
            ShareError::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse { error: "无权操作".into() }),
            ),
            ShareError::Invalid(msg) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: msg }),
            ),
            ShareError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: msg }),
            ),
        }
    }
}

impl From<sqlx::Error> for ShareError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("share service sqlx error: {}", e);
        ShareError::Internal("数据库错误".into())
    }
}

impl ShareService {
    pub fn new(repo: ShareRepository) -> Self {
        Self { repo }
    }

    /// Create a share link. Owner is the authenticated user.
    /// Returns the raw token (shown once) and the persisted share record.
    pub async fn create_share_link(
        &self,
        video_id: i64,
        user_id: i64,
        expires_in_days: Option<i32>,
    ) -> Result<(String, ShareLink), ShareError> {
        let token = generate_token();
        let expires_at = match expires_in_days {
            Some(days) => chrono::Utc::now()
                .naive_utc()
                .checked_add_signed(chrono::Duration::days(days.clamp(1, 365) as i64)),
            None => chrono::Utc::now()
                .naive_utc()
                .checked_add_signed(chrono::Duration::hours(3)),
        };
        let share = self
            .repo
            .create_share_link(video_id, user_id, &token, expires_at)
            .await?;
        Ok((token, share))
    }

    pub async fn list_my_shares(&self, user_id: i64) -> Result<Vec<ShareLink>, ShareError> {
        let shares = self.repo.list_shares_for_user(user_id).await?;
        Ok(shares)
    }

    pub async fn revoke_my_share(
        &self,
        share_id: i64,
        user_id: i64,
    ) -> Result<(), ShareError> {
        let shares = self.repo.list_shares_for_user(user_id).await?;
        if !shares.iter().any(|s| s.id == share_id) {
            return Err(ShareError::NotFound);
        }
        self.repo.delete_share_link(share_id).await?;
        Ok(())
    }

    pub async fn delete_share_link(
        &self,
        video_id: i64,
        share_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ShareError> {
        let shares = self.repo.list_video_shares(video_id).await?;
        let share = shares
            .iter()
            .find(|s| s.id == share_id)
            .ok_or(ShareError::NotFound)?;
        if !is_admin && share.user_id != user_id {
            return Err(ShareError::Forbidden);
        }
        self.repo.delete_share_link(share_id).await?;
        Ok(())
    }

    pub async fn get_share_video(&self, token: &str) -> Result<ShareLink, ShareError> {
        if !is_valid_share_token(token) {
            return Err(ShareError::Invalid("分享链接格式无效".into()));
        }
        let token_hash = crate::repositories::share_repo::hash_share_token(token);
        let share = self
            .repo
            .is_valid_token_hash(&token_hash)
            .await?
            .ok_or(ShareError::NotFound)?;
        Ok(share)
    }
}

fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            match idx {
                0..10 => (b'0' + idx) as char,
                10..36 => (b'a' + idx - 10) as char,
                36..62 => (b'A' + idx - 36) as char,
                _ => unreachable!(),
            }
        })
        .collect()
}

/// Validate share token format: 32 chars, alphanumeric only
pub fn is_valid_share_token(token: &str) -> bool {
    token.len() == 32 && token.chars().all(|c| c.is_ascii_alphanumeric())
}
