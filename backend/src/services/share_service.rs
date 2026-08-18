use crate::repositories::share_repo::{ShareLink, ShareRepository};
use crate::util::error::ServiceError;
use rand::Rng;

#[derive(Clone)]
pub struct ShareService {
    repo: ShareRepository,
}

impl ShareService {
    pub fn new(repo: ShareRepository) -> Self {
        Self { repo }
    }

    /// Create a share link. Owner is the authenticated user.
    /// Returns the raw token (shown once) and the persisted share record.
    ///
    /// SECURITY (H-02): the authorization boundary (only the video's
    /// uploader or an admin may create a share) is enforced by the caller —
    /// `handlers::shares::create_share_link` checks `VideoOwnership` before
    /// invoking this method. Do not call this method directly from an
    /// unauthenticated or non-uploader path.
    pub async fn create_share_link(
        &self,
        video_id: i64,
        user_id: i64,
        expires_in_days: Option<i32>,
    ) -> Result<(String, ShareLink), ServiceError> {
        let token = generate_token();
        let base = chrono::Utc::now().naive_utc();
        // Inputs are clamped (1..=365 days, or the fixed 3h default), so the
        // addition can never overflow; the fallback path is dead weight.
        let expires_at = match expires_in_days {
            Some(days) => base + chrono::Duration::days(days.clamp(1, 365) as i64),
            None => base + chrono::Duration::hours(3),
        };
        let share = self
            .repo
            .create_share_link(video_id, user_id, &token, Some(expires_at))
            .await?;
        Ok((token, share))
    }

    pub async fn list_my_shares(&self, user_id: i64) -> Result<Vec<ShareLink>, ServiceError> {
        let shares = self.repo.list_shares_for_user(user_id).await?;
        Ok(shares)
    }

    pub async fn revoke_my_share(&self, share_id: i64, user_id: i64) -> Result<(), ServiceError> {
        let deleted = self.repo.delete_share_by_owner(share_id, user_id).await?;
        if !deleted {
            return Err(ServiceError::not_found("分享链接不存在"));
        }
        Ok(())
    }

    pub async fn delete_share_link(
        &self,
        video_id: i64,
        share_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        let deleted = self
            .repo
            .delete_share_with_auth(share_id, video_id, user_id, is_admin)
            .await?;
        if !deleted {
            return Err(ServiceError::not_found("分享链接不存在"));
        }
        Ok(())
    }

    pub async fn get_share_video(&self, token: &str) -> Result<ShareLink, ServiceError> {
        if !is_valid_share_token(token) {
            return Err(ServiceError::bad_request("分享链接格式无效"));
        }
        let token_hash = crate::repositories::share_repo::hash_share_token(token);
        let share = self
            .repo
            .is_valid_token_hash(&token_hash)
            .await?
            .ok_or_else(|| ServiceError::not_found("分享链接不存在"))?;
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
