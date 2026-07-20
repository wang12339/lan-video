use sqlx::PgPool;

use crate::db::log_slow_query;

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub approved: bool,
    pub role: i16,
    pub avatar_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserWithStatus {
    pub id: i64,
    pub username: String,
    pub approved: bool,
    pub is_admin: bool,
    pub role: i16,
    pub avatar_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub has_active_token: bool,
}

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn count_users(&self) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    pub async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        role: i16,
    ) -> Result<i64, sqlx::Error> {
        let approved = role >= 3;
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO users (username, password_hash, approved, role) VALUES ($1, $2, $3, $4) RETURNING id"
        )
        .bind(username)
        .bind(password_hash)
        .bind(approved)
        .bind(role)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserRow>, sqlx::Error> {
        let pool = self.pool.clone();
        let username = username.to_string();
        let user = log_slow_query("user_repo::find_by_username", || async {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, username, password_hash, approved, role, avatar_url, created_at FROM users WHERE username = $1"
            )
            .bind(&username)
            .fetch_optional(&pool)
            .await
        })
        .await?;
        Ok(user)
    }

    /// Generate a cryptographically secure 256-bit token (64 hex chars).
    /// Stores SHA-256(token) in the DB; the raw token is returned to the caller.
    pub async fn create_token(&self, user_id: i64) -> Result<String, sqlx::Error> {
        use rand::Rng;
        use sha2::{Digest, Sha256};
        let token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        sqlx::query("INSERT INTO auth_tokens (user_id, token_hash, expires_at, revoked) VALUES ($1, $2, CURRENT_TIMESTAMP + INTERVAL '7 days', false)")
            .bind(user_id)
            .bind(&token_hash)
            .execute(&self.pool)
            .await?;
        Ok(token)
    }

    pub async fn find_user_by_token(&self, token: &str) -> Result<Option<UserRow>, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let pool = self.pool.clone();
        let user = log_slow_query("user_repo::find_user_by_token", || async {
            sqlx::query_as::<_, UserRow>(
                r#"SELECT u.id, u.username, u.password_hash, u.approved, u.role, u.avatar_url, u.created_at
                   FROM auth_tokens t
                   JOIN users u ON t.user_id = u.id
                   WHERE t.token_hash = $1 AND t.expires_at > CURRENT_TIMESTAMP AND NOT t.revoked"#,
            )
            .bind(&token_hash)
            .fetch_optional(&pool)
            .await
        })
        .await?;
        Ok(user)
    }

    /// Like find_user_by_token but also returns whether the token was revoked.
    /// Returns (UserRow, is_revoked) when the token hash matches (regardless of expiry/revoked).
    pub async fn find_token_detail(
        &self,
        token: &str,
    ) -> Result<Option<(UserRow, bool, bool)>, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let row = sqlx::query_as::<_, (i64, String, String, bool, i16, Option<String>, chrono::DateTime<chrono::Utc>, bool, bool)>(
            r#"SELECT u.id, u.username, u.password_hash, u.approved, u.role, u.avatar_url, u.created_at,
                      t.revoked, t.expires_at > CURRENT_TIMESTAMP AS valid
               FROM auth_tokens t
               JOIN users u ON t.user_id = u.id
               WHERE t.token_hash = $1"#,
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(
                id,
                username,
                password_hash,
                approved,
                role,
                avatar_url,
                created_at,
                revoked,
                valid,
            )| {
                (
                    UserRow {
                        id,
                        username,
                        password_hash,
                        approved,
                        role,
                        avatar_url,
                        created_at,
                    },
                    revoked,
                    valid,
                )
            },
        ))
    }

    pub async fn delete_token(&self, token: &str) -> Result<bool, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let result = sqlx::query("DELETE FROM auth_tokens WHERE token_hash = $1")
            .bind(&token_hash)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn revoke_tokens_by_user_id(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("UPDATE auth_tokens SET revoked = true WHERE user_id = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT revoked")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_tokens_by_user_id(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM auth_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_users(&self) -> Result<Vec<UserWithStatus>, sqlx::Error> {
        let pool = self.pool.clone();
        let users = log_slow_query("user_repo::list_users", || async {
            sqlx::query_as::<_, UserWithStatus>(
                r#"SELECT u.id, u.username, u.approved, u.role >= 3 AS is_admin, u.role, u.avatar_url, u.created_at,
                          EXISTS(SELECT 1 FROM auth_tokens t WHERE t.user_id = u.id AND t.expires_at > CURRENT_TIMESTAMP AND NOT t.revoked) AS has_active_token
                   FROM users u ORDER BY u.created_at DESC"#,
            )
            .fetch_all(&pool)
            .await
        })
        .await?;
        Ok(users)
    }

    pub async fn delete_user(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM auth_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn has_active_tokens(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM auth_tokens WHERE user_id = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT revoked)"
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    pub async fn update_password_hash(
        &self,
        user_id: i64,
        hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(hash)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn toggle_admin(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"UPDATE users SET role = CASE WHEN role >= 3 THEN 1 ELSE 3 END, approved = CASE WHEN role >= 3 THEN approved ELSE true END WHERE id = $1"#
        )
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn approve_user(&self, user_id: i64, approve: bool) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET approved = $1 WHERE id = $2")
            .bind(approve)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_pending_users(&self) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE approved = false")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    pub async fn cleanup_expired_tokens(&self) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM auth_tokens WHERE expires_at <= CURRENT_TIMESTAMP OR revoked")
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    pub async fn set_user_role(&self, user_id: i64, role: i16) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
            .bind(role)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_user_role(&self, user_id: i64) -> Result<Option<i16>, sqlx::Error> {
        let result: Option<(i16,)> = sqlx::query_as("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(result.map(|(role,)| role))
    }

    pub async fn update_avatar(&self, user_id: i64, avatar_url: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET avatar_url = $1 WHERE id = $2")
            .bind(avatar_url)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
