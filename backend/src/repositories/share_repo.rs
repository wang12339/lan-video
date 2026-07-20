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

#[derive(Clone)]
pub struct ShareRepository {
    pool: PgPool,
}

impl ShareRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_share_link(
        &self,
        video_id: i64,
        user_id: i64,
        raw_token: &str,
        expires_at: Option<chrono::NaiveDateTime>,
    ) -> Result<ShareLink, sqlx::Error> {
        let token_hash = hash_share_token(raw_token);
        sqlx::query_as::<_, ShareLink>(
            r#"INSERT INTO share_links (video_id, user_id, token_hash, expires_at)
               VALUES ($1, $2, $3, $4)
               RETURNING id, video_id, user_id, expires_at, created_at"#,
        )
        .bind(video_id)
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<ShareLink>, sqlx::Error> {
        sqlx::query_as::<_, ShareLink>(
            r#"SELECT id, video_id, user_id, expires_at, created_at
               FROM share_links
               WHERE token_hash = $1"#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete_share_link(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM share_links WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_video_shares(&self, video_id: i64) -> Result<Vec<ShareLink>, sqlx::Error> {
        sqlx::query_as::<_, ShareLink>(
            r#"SELECT id, video_id, user_id, expires_at, created_at
               FROM share_links
               WHERE video_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(video_id)
        .fetch_all(&self.pool)
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

    pub async fn is_valid_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<ShareLink>, sqlx::Error> {
        sqlx::query_as::<_, ShareLink>(
            r#"SELECT id, video_id, user_id, expires_at, created_at
               FROM share_links
               WHERE token_hash = $1
               AND (expires_at IS NULL OR expires_at > NOW())"#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
    }

    /// Periodic cleanup of expired share links (SH-04).
    /// Returns the number of rows deleted.
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM share_links WHERE expires_at IS NOT NULL AND expires_at < NOW()",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
