use std::sync::OnceLock;
use std::time::Duration;

use moka::sync::Cache;
use sqlx::PgPool;

use crate::db::log_slow_query;

/// Cache for `find_user_by_token` results (M-08).
///
/// bearer_auth / media_auth call find_user_by_token on every request; without
/// a cache each one costs a SHA-256 hash plus a 2-table JOIN.
///
/// Invalidation: logout (`delete_token`) has the raw token and invalidates
/// precisely. Admin kick / password reset revoke by user_id and cannot
/// enumerate the raw tokens held in this cache, so those revocations take
/// effect within at most `TOKEN_CACHE_TTL_SECS` seconds. We accept that
/// TTL-bounded revocation delay (same tradeoff as the media_auth cache) —
/// revocation immediacy is prioritised by the fact that we only ever cache
/// VALID (Some) results: a revoked token keeps authenticating for at most
/// `TOKEN_CACHE_TTL_SECS`, never longer. Negative results are never cached,
/// so the find_token_detail "kicked / expired" differentiation in bearer_auth
/// keeps working unchanged.
///
/// The cached UserRow carries tenant_id (H-01 binding) and role/approved;
/// role/approval changes are likewise TTL-bounded, which is acceptable.
static TOKEN_CACHE: OnceLock<Cache<String, UserRow>> = OnceLock::new();

const TOKEN_CACHE_TTL_SECS: u64 = 10;
const TOKEN_CACHE_CAPACITY: u64 = 10_000;

fn token_cache() -> &'static Cache<String, UserRow> {
    TOKEN_CACHE.get_or_init(|| {
        Cache::builder()
            .time_to_live(Duration::from_secs(TOKEN_CACHE_TTL_SECS))
            .max_capacity(TOKEN_CACHE_CAPACITY)
            .build()
    })
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub approved: bool,
    pub role: i16,
    pub avatar_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub email: Option<String>,
    pub email_verified: bool,
    /// Tenant this user (or, on token queries, the token) belongs to.
    /// On token lookups it mirrors `auth_tokens.tenant_id` (H-01 binding).
    pub tenant_id: i64,
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

    pub async fn count_users(&self, tenant_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    pub async fn create_user(
        &self,
        tenant_id: i64,
        username: &str,
        password_hash: &str,
        role: i16,
    ) -> Result<i64, sqlx::Error> {
        let approved = role >= 3;
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO users (tenant_id, username, password_hash, approved, role) VALUES ($1, $2, $3, $4, $5) RETURNING id"
        )
        .bind(tenant_id)
        .bind(username)
        .bind(password_hash)
        .bind(approved)
        .bind(role)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_by_username(
        &self,
        tenant_id: i64,
        username: &str,
    ) -> Result<Option<UserRow>, sqlx::Error> {
        let pool = self.pool.clone();
        let username = username.to_string();
        let user = log_slow_query("user_repo::find_by_username", || async {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, username, password_hash, approved, role, avatar_url, created_at, email, email_verified, tenant_id FROM users WHERE tenant_id = $1 AND username = $2"
            )
            .bind(tenant_id)
            .bind(&username)
            .fetch_optional(&pool)
            .await
        })
        .await?;
        Ok(user)
    }

    /// Generate a 256-bit (64 char) cryptographically secure token from the
    /// OS CSPRNG directly (not seeded PRNG state).
    fn generate_random_token() -> String {
        use rand::distributions::Alphanumeric;
        use rand::Rng;
        rand::rngs::OsRng
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect()
    }

    /// Generate a cryptographically secure 256-bit token (64 hex chars).
    /// Stores SHA-256(token) in the DB; the raw token is returned to the caller.
    ///
    /// SECURITY (H-01): the token is bound to its user's tenant at creation
    /// time (`tenant_id` copied from `users.tenant_id` in the same statement),
    /// so a token minted on tenant A can never authenticate on tenant B.
    pub async fn create_token(&self, user_id: i64) -> Result<String, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token = Self::generate_random_token();
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        sqlx::query(
            "INSERT INTO auth_tokens (user_id, tenant_id, token_hash, expires_at, revoked)
             SELECT id, tenant_id, $2, CURRENT_TIMESTAMP + INTERVAL '7 days', false
             FROM users WHERE id = $1",
        )
        .bind(user_id)
        .bind(&token_hash)
        .execute(&self.pool)
        .await?;
        Ok(token)
    }

    pub async fn find_user_by_token(&self, token: &str) -> Result<Option<UserRow>, sqlx::Error> {
        if let Some(user) = token_cache().get(token) {
            return Ok(Some(user));
        }
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let pool = self.pool.clone();
        let user = log_slow_query("user_repo::find_user_by_token", || async {
            sqlx::query_as::<_, UserRow>(
                r#"SELECT u.id, u.username, u.password_hash, u.approved, u.role, u.avatar_url, u.created_at, u.email, u.email_verified, t.tenant_id
                   FROM auth_tokens t
                   JOIN users u ON t.user_id = u.id
                   WHERE t.token_hash = $1 AND t.expires_at > CURRENT_TIMESTAMP AND NOT t.revoked"#,
            )
            .bind(&token_hash)
            .fetch_optional(&pool)
            .await
        })
        .await?;
        // Only positive results are cached (see TOKEN_CACHE docs): revocation
        // by user_id (admin kick, password reset) cannot invalidate here, so
        // the short TTL bounds how long a revoked token keeps working.
        if let Some(user) = &user {
            token_cache().insert(token.to_string(), user.clone());
        }
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
        let row = sqlx::query_as::<_, (i64, i64, String, String, bool, i16, Option<String>, chrono::DateTime<chrono::Utc>, Option<String>, bool, bool, bool)>(
            r#"SELECT u.id, t.tenant_id, u.username, u.password_hash, u.approved, u.role, u.avatar_url, u.created_at,
                      u.email, u.email_verified,
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
                tenant_id,
                username,
                password_hash,
                approved,
                role,
                avatar_url,
                created_at,
                email,
                email_verified,
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
                        email,
                        email_verified,
                        tenant_id,
                    },
                    revoked,
                    valid,
                )
            },
        ))
    }

    pub async fn delete_token(&self, token: &str) -> Result<bool, sqlx::Error> {
        // Logout has the raw token, so the token cache can be invalidated
        // precisely — logout takes effect immediately, unlike user_id-based
        // revocations which are TTL-bounded (see TOKEN_CACHE docs).
        token_cache().invalidate(token);
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

    pub async fn list_users(&self, tenant_id: i64) -> Result<Vec<UserWithStatus>, sqlx::Error> {
        let pool = self.pool.clone();
        let users = log_slow_query("user_repo::list_users", || async {
            sqlx::query_as::<_, UserWithStatus>(
                r#"SELECT u.id, u.username, u.approved, u.role >= 3 AS is_admin, u.role, u.avatar_url, u.created_at,
                          EXISTS(SELECT 1 FROM auth_tokens t WHERE t.user_id = u.id AND t.expires_at > CURRENT_TIMESTAMP AND NOT t.revoked) AS has_active_token
                   FROM users u WHERE u.tenant_id = $1 ORDER BY u.created_at DESC"#,
            )
            .bind(tenant_id)
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

    pub async fn count_pending_users(&self, tenant_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE tenant_id = $1 AND approved = false")
                .bind(tenant_id)
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

    pub async fn update_email(&self, user_id: i64, email: &str) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // 更新邮箱并重置验证状态
        let result =
            sqlx::query("UPDATE users SET email = $1, email_verified = false WHERE id = $2")
                .bind(email)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;

        // 失效旧的邮箱验证令牌
        sqlx::query(
            "UPDATE email_verification_tokens SET used = true WHERE user_id = $1 AND NOT used",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn verify_email(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET email_verified = true WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
        let pool = self.pool.clone();
        let email = email.to_string();
        let user = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, approved, role, avatar_url, created_at, email, email_verified, tenant_id FROM users WHERE email = $1"
        )
        .bind(&email)
        .fetch_optional(&pool)
        .await?;
        Ok(user)
    }

    pub async fn create_password_reset_token(&self, user_id: i64) -> Result<String, sqlx::Error> {
        use sha2::{Digest, Sha256};

        let mut tx = self.pool.begin().await?;

        // 先使旧令牌失效
        sqlx::query("UPDATE password_reset_tokens SET used = true WHERE user_id = $1 AND NOT used")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        let token = Self::generate_random_token();
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        sqlx::query(
            "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, CURRENT_TIMESTAMP + INTERVAL '1 hour')"
        )
        .bind(user_id)
        .bind(&token_hash)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(token)
    }

    pub async fn find_valid_reset_token(&self, token: &str) -> Result<Option<i64>, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let row: Option<(i64,)> = sqlx::query_as(
            "UPDATE password_reset_tokens SET used = true WHERE id = (SELECT id FROM password_reset_tokens WHERE token_hash = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT used FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING user_id"
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }

    pub async fn consume_reset_token(&self, token: &str) -> Result<bool, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let result =
            sqlx::query("UPDATE password_reset_tokens SET used = true WHERE token_hash = $1")
                .bind(&token_hash)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_email(&self, user_id: i64) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT email FROM users WHERE id = $1 AND email IS NOT NULL")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(e,)| e))
    }

    pub async fn create_email_verification_token(
        &self,
        user_id: i64,
    ) -> Result<String, sqlx::Error> {
        use sha2::{Digest, Sha256};

        let mut tx = self.pool.begin().await?;

        // 先使旧令牌失效
        sqlx::query(
            "UPDATE email_verification_tokens SET used = true WHERE user_id = $1 AND NOT used",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        let token = Self::generate_random_token();
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        sqlx::query(
            "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, CURRENT_TIMESTAMP + INTERVAL '24 hours')"
        )
        .bind(user_id)
        .bind(&token_hash)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(token)
    }

    pub async fn find_valid_email_verification_token(
        &self,
        token: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let row: Option<(i64,)> = sqlx::query_as(
            "UPDATE email_verification_tokens SET used = true WHERE id = (SELECT id FROM email_verification_tokens WHERE token_hash = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT used FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING user_id"
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }

    pub async fn consume_email_verification_token(&self, token: &str) -> Result<bool, sqlx::Error> {
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let result =
            sqlx::query("UPDATE email_verification_tokens SET used = true WHERE token_hash = $1")
                .bind(&token_hash)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_active_connections(&self) -> Result<i32, sqlx::Error> {
        let (count,): (i32,) = sqlx::query_as("SELECT count(*)::int FROM pg_stat_activity")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}
