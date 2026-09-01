use sha2::{Digest, Sha256};
use sqlx::PgPool;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_share_token_deterministic() {
        let t1 = hash_share_token("abc123testtoken");
        let t2 = hash_share_token("abc123testtoken");
        assert_eq!(t1, t2, "same token should produce same hash");
    }

    #[test]
    fn test_hash_share_token_different() {
        let t1 = hash_share_token("token_a");
        let t2 = hash_share_token("token_b");
        assert_ne!(t1, t2, "different tokens should produce different hashes");
    }

    #[test]
    fn test_hash_share_token_length() {
        let hash = hash_share_token("short");
        // SHA-256 hex = 64 chars
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_share_token_not_reversible() {
        let hash = hash_share_token("secret_token_value");
        // The hash should not contain the original string
        assert!(!hash.contains("secret"));
    }
}

/// Hash a share token for storage / lookup. The raw token never leaves the
/// creator; we only persist the SHA-256 digest. Compromise of the database
/// therefore does not directly leak usable share tokens.
pub fn hash_share_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShareLink {
    pub id: i64,
    pub video_id: i64,
    pub user_id: i64,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

/// Ownership / tenant context of a video, used to authorize share creation
/// (H-02): a share link may only be created by the video's uploader or an
/// admin, and only within the video's own tenant.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VideoOwnership {
    #[sqlx(default)]
    pub uploader_id: Option<i64>,
    pub tenant_id: i64,
}

#[derive(Clone)]
pub struct ShareRepository {
    pool: PgPool,
}

impl ShareRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Look up a video's uploader and tenant for share authorization.
    /// Returns `None` when the video does not exist — the caller must keep
    /// the existing 404 semantics and never reveal the video to callers who
    /// are not allowed to share it (H-02).
    pub async fn find_video_ownership(
        &self,
        video_id: i64,
    ) -> Result<Option<VideoOwnership>, sqlx::Error> {
        sqlx::query_as::<_, VideoOwnership>(
            "SELECT uploader_id, tenant_id FROM videos WHERE id = $1",
        )
        .bind(video_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn create_share_link(
        &self,
        tenant_id: i64,
        video_id: i64,
        user_id: i64,
        raw_token: &str,
        expires_at: Option<chrono::NaiveDateTime>,
    ) -> Result<ShareLink, sqlx::Error> {
        let token_hash = hash_share_token(raw_token);
        // The share's tenant_id mirrors the video's tenant_id at creation
        // time. The `v.tenant_id = $5` guard is defense-in-depth: the handler
        // (handlers::shares::create_share_link) already enforces the H-02
        // tenant boundary via `find_video_ownership`. INSERT ... SELECT also
        // makes it impossible to create a share for a video that does not
        // exist (0 rows → RowNotFound → caller's 500).
        sqlx::query_as::<_, ShareLink>(
            r#"INSERT INTO share_links (video_id, user_id, token_hash, expires_at, tenant_id)
               SELECT $1, $2, $3, $4, v.tenant_id FROM videos v
               WHERE v.id = $1 AND v.tenant_id = $5
               RETURNING id, video_id, user_id, expires_at, created_at"#,
        )
        .bind(video_id)
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
    }

    /// List share links for videos that the user owns (admin or uploader).
    /// SECURITY: never return the raw token. Return the public share_id and
    /// metadata so the webapp can render a list / revoke UI without ever
    /// touching the secret token.
    pub async fn list_shares_for_user(&self, user_id: i64) -> Result<Vec<ShareLink>, sqlx::Error> {
        sqlx::query_as::<_, ShareLink>(
            r#"SELECT id, video_id, user_id, expires_at, created_at
               FROM share_links
               WHERE user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Resolve a share link by its hashed token (expiry included).
    ///
    /// NOTE (H-02): no `tenant_id` filter here on purpose. This is the
    /// anonymous read path (GET /share/{token} and media_auth), and
    /// `share_links.tenant_id` is currently never populated with a
    /// non-default value (all rows are tenant 1). A one-sided filter would
    /// also diverge from `media_auth` in middleware/auth.rs, which validates
    /// tokens without a tenant condition. Revisit when tenants are actually
    /// provisioned (H-01).
    pub async fn is_valid_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<ShareLink>, sqlx::Error> {
        sqlx::query_as::<_, ShareLink>(
            // expires_at is a naive-UTC TIMESTAMP written by the app; compare
            // against UTC explicitly so the check does not depend on the
            // database session's TimeZone setting.
            r#"SELECT id, video_id, user_id, expires_at, created_at
               FROM share_links
               WHERE token_hash = $1
               AND (expires_at IS NULL OR expires_at > (NOW() AT TIME ZONE 'UTC'))"#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
    }

    /// Delete a share link owned by a specific user.
    pub async fn delete_share_by_owner(
        &self,
        share_id: i64,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM share_links WHERE id = $1 AND user_id = $2")
            .bind(share_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete a share link, with optional ownership check for non-admins.
    pub async fn delete_share_with_auth(
        &self,
        share_id: i64,
        video_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<bool, sqlx::Error> {
        let result = if is_admin {
            sqlx::query("DELETE FROM share_links WHERE id = $1 AND video_id = $2")
                .bind(share_id)
                .bind(video_id)
                .execute(&self.pool)
                .await?
        } else {
            sqlx::query("DELETE FROM share_links WHERE id = $1 AND video_id = $2 AND user_id = $3")
                .bind(share_id)
                .bind(video_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?
        };
        Ok(result.rows_affected() > 0)
    }

    /// Periodic cleanup of expired share links (SH-04).
    /// Returns the number of rows deleted.
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            // expires_at is a naive-UTC TIMESTAMP written by the app; compare
            // against UTC explicitly (see is_valid_token_hash).
            "DELETE FROM share_links
             WHERE expires_at IS NOT NULL AND expires_at < (NOW() AT TIME ZONE 'UTC')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
