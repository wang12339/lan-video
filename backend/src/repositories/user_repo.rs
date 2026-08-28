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
    /// Creates a new `UserRepository` wrapping the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns a reference to the underlying `PgPool` for direct use by
    /// higher-level services that need raw SQL access (e.g. video queries).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Count all users belonging to `tenant_id`.
    ///
    /// **SQL**: `SELECT COUNT(*) FROM users WHERE tenant_id = $1`
    ///
    /// Uses the composite index `idx_users_tenant_id` (or PK scan filtered
    /// by tenant). The result is a sequential scan of the filtered rows;
    /// fine-grained tenant isolation keeps the working set small per tenant.
    pub async fn count_users(&self, tenant_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Register a new user.
    ///
    /// **SQL**: `INSERT INTO users (tenant_id, username, password_hash, approved, role) … RETURNING id`
    ///
    /// The `approved` flag is set to `true` when `role >= 3` (admin), so
    /// admin accounts are auto-approved while normal users require manual
    /// approval. The uniqueness constraint on `(tenant_id, username)` raises
    /// `sqlx::Error` (unique_violation) if the username is already taken.
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

    /// Look up a user by `(tenant_id, username)`.
    ///
    /// **SQL**: `SELECT … FROM users WHERE tenant_id = $1 AND username = $2`
    ///
    /// Uses the unique composite index on `(tenant_id, username)` for a
    /// single-row index lookup (O(log n)). This is the primary lookup path
    /// for authentication (login).
    pub async fn find_by_username(
        &self,
        tenant_id: i64,
        username: &str,
    ) -> Result<Option<UserRow>, sqlx::Error> {
        let user = log_slow_query("user_repo::find_by_username", || async {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, username, password_hash, approved, role, avatar_url, created_at, email, email_verified, tenant_id FROM users WHERE tenant_id = $1 AND username = $2"
            )
            .bind(tenant_id)
            .bind(username)
            .fetch_optional(&self.pool)
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

    /// Mint a new authentication token for `user_id`.
    ///
    /// **SQL**: `INSERT INTO auth_tokens (user_id, tenant_id, token_hash, expires_at, revoked)
    ///          SELECT id, tenant_id, $2, CURRENT_TIMESTAMP + INTERVAL '7 days', false
    ///          FROM users WHERE id = $1`
    ///
    /// SECURITY (H-01): the token is bound to its user's tenant at creation
    /// time — `tenant_id` is copied from `users.tenant_id` in the same
    /// INSERT..SELECT statement, so a token minted on tenant A can never
    /// authenticate on tenant B. Token is a 256-bit random alphanumeric
    /// string; only its SHA-256 hash is stored. Expires in 7 days. The raw
    /// token is returned to the caller (never stored).
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

    /// Resolve a raw token to the owning `UserRow`.
    ///
    /// **SQL**: `SELECT … FROM auth_tokens t JOIN users u ON t.user_id = u.id
    ///          WHERE t.token_hash = $1 AND t.expires_at > CURRENT_TIMESTAMP AND NOT t.revoked`
    ///
    /// Caching: checks `TOKEN_CACHE` first (10 s TTL, 10k entries). Only
    /// *positive* results are cached — negative lookups always hit the DB so
    /// that `find_token_detail` can distinguish "kicked" from "expired".
    ///
    /// **Index**: `auth_tokens(token_hash)` unique index for the single-row
    /// lookup; the join to `users` uses the PK index on `users(id)`.
    pub async fn find_user_by_token(&self, token: &str) -> Result<Option<UserRow>, sqlx::Error> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Ok(None);
        }
        if let Some(user) = token_cache().get(token) {
            return Ok(Some(user));
        }
        use sha2::{Digest, Sha256};
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let user = log_slow_query("user_repo::find_user_by_token", || async {
            sqlx::query_as::<_, UserRow>(
                r#"SELECT u.id, u.username, u.password_hash, u.approved, u.role, u.avatar_url, u.created_at, u.email, u.email_verified, t.tenant_id
                   FROM auth_tokens t
                   JOIN users u ON t.user_id = u.id
                   WHERE t.token_hash = $1 AND t.expires_at > CURRENT_TIMESTAMP AND NOT t.revoked"#,
            )
            .bind(&token_hash)
            .fetch_optional(&self.pool)
            .await
        })
        .await?;
        if let Some(user) = &user {
            token_cache().insert(token.to_string(), user.clone());
        }
        Ok(user)
    }

    /// Like [`find_user_by_token`] but returns the *full* token state:
    /// `(UserRow, is_revoked, is_valid)`.
    ///
    /// **SQL**: `SELECT … t.revoked, t.expires_at > CURRENT_TIMESTAMP AS valid
    ///          FROM auth_tokens t JOIN users u ON t.user_id = u.id
    ///          WHERE t.token_hash = $1`
    ///
    /// Unlike `find_user_by_token` this query does **not** filter on
    /// `expires_at` or `revoked`, so the caller can distinguish between
    /// a kicked user (`revoked=true`) and an expired token (`valid=false`).
    /// Used by `bearer_auth` to emit specific 401 sub-reasons.
    ///
    /// **Index**: `auth_tokens(token_hash)` unique index, single-row lookup.
    pub async fn find_token_detail(
        &self,
        token: &str,
    ) -> Result<Option<(UserRow, bool, bool)>, sqlx::Error> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Ok(None);
        }
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

    /// Delete (permanently remove) a single token by its raw value.
    ///
    /// **SQL**: `DELETE FROM auth_tokens WHERE token_hash = $1`
    ///
    /// Also evicts the token from `TOKEN_CACHE` so logout takes effect
    /// immediately — unlike `revoke_tokens_by_user_id` which is TTL-bounded.
    ///
    /// **Index**: `auth_tokens(token_hash)` unique index, single-row delete.
    pub async fn delete_token(&self, token: &str) -> Result<bool, sqlx::Error> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Ok(false);
        }
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

    /// Revoke all **active** (non-expired, non-revoked) tokens for `user_id`.
    ///
    /// **SQL**: `UPDATE auth_tokens SET revoked = true
    ///          WHERE user_id = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT revoked`
    ///
    /// Used by admin kick and password reset. Because the caller does not
    /// have the raw tokens, `TOKEN_CACHE` cannot be invalidated precisely —
    /// revocation takes effect within `TOKEN_CACHE_TTL_SECS` seconds.
    ///
    /// **Index**: `auth_tokens(user_id, revoked, expires_at)` composite index
    /// for the filtered update.
    pub async fn revoke_tokens_by_user_id(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("UPDATE auth_tokens SET revoked = true WHERE user_id = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT revoked")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete **all** tokens for `user_id` (including expired/revoked).
    ///
    /// **SQL**: `DELETE FROM auth_tokens WHERE user_id = $1`
    ///
    /// Used during user deletion to fully purge token rows. Unlike
    /// `revoke_tokens_by_user_id` this is a hard delete, not a soft revoke.
    ///
    /// **Index**: `auth_tokens(user_id)` for the filtered delete.
    pub async fn delete_tokens_by_user_id(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM auth_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// List all users in a tenant, newest first, with online status.
    ///
    /// **SQL**: `SELECT u.*, EXISTS(SELECT 1 FROM auth_tokens t
    ///          WHERE t.user_id = u.id AND t.expires_at > CURRENT_TIMESTAMP
    ///          AND NOT t.revoked) AS has_active_token
    ///          FROM users u WHERE u.tenant_id = $1 ORDER BY u.created_at DESC`
    ///
    /// The correlated subquery checks for an active auth token per user.
    /// This is an N+1 pattern but acceptable for admin lists (< 1k users
    /// per tenant). The subquery uses `auth_tokens(user_id, revoked, expires_at)`
    /// composite index.
    ///
    /// **Index**: `users(tenant_id, created_at DESC)` for the ordered scan;
    /// `auth_tokens(user_id, revoked, expires_at)` for the correlated EXISTS.
    pub async fn list_users(&self, tenant_id: i64) -> Result<Vec<UserWithStatus>, sqlx::Error> {
        let users = log_slow_query("user_repo::list_users", || async {
            sqlx::query_as::<_, UserWithStatus>(
                r#"SELECT u.id, u.username, u.approved, u.role >= 3 AS is_admin, u.role, u.avatar_url, u.created_at,
                          EXISTS(SELECT 1 FROM auth_tokens t WHERE t.user_id = u.id AND t.expires_at > CURRENT_TIMESTAMP AND NOT t.revoked) AS has_active_token
                   FROM users u WHERE u.tenant_id = $1 ORDER BY u.created_at DESC"#,
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        })
        .await?;
        Ok(users)
    }

    /// Permanently delete a user and all their tokens.
    ///
    /// **SQL**: Two statements in sequence:
    /// 1. `DELETE FROM auth_tokens WHERE user_id = $1` — purge tokens first
    /// 2. `DELETE FROM users WHERE id = $1` — then the user row
    ///
    /// Token deletion is done first to avoid FK constraint violations if
    /// `auth_tokens.user_id` has a foreign key to `users.id`.
    ///
    /// **Index**: `auth_tokens(user_id)` for the token purge; `users(id)` PK
    /// for the user delete.
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

    /// Check whether `user_id` has any non-expired, non-revoked tokens.
    ///
    /// **SQL**: `SELECT EXISTS(SELECT 1 FROM auth_tokens
    ///          WHERE user_id = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT revoked)`
    ///
    /// Used to detect if a user is currently logged in (e.g. for admin
    /// "kick" UI or login status display).
    ///
    /// **Index**: `auth_tokens(user_id, revoked, expires_at)` composite index
    /// for the filtered existence check.
    pub async fn has_active_tokens(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM auth_tokens WHERE user_id = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT revoked)"
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    /// Update a user's password hash (after password reset or change).
    ///
    /// **SQL**: `UPDATE users SET password_hash = $1 WHERE id = $2`
    ///
    /// Uses PK index on `users(id)` for the single-row update. The caller
    /// should also revoke all tokens after a password change.
    ///
    /// **Index**: `users(id)` PK.
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

    /// Toggle admin status for a user.
    ///
    /// **SQL**: `UPDATE users SET role = CASE WHEN role >= 3 THEN 1 ELSE 3 END,
    ///          approved = CASE WHEN role >= 3 THEN approved ELSE true END
    ///          WHERE id = $1`
    ///
    /// Admins (role >= 3) are demoted to normal user (role 1); normal users
    /// are promoted to admin (role 3) and auto-approved. The `approved`
    /// preservation on demotion prevents accidentally blocking an admin who
    /// was manually approved.
    ///
    /// **Index**: `users(id)` PK.
    pub async fn toggle_admin(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"UPDATE users SET role = CASE WHEN role >= 3 THEN 1 ELSE 3 END, approved = CASE WHEN role >= 3 THEN approved ELSE true END WHERE id = $1"#
        )
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Approve or un-approve a user account.
    ///
    /// **SQL**: `UPDATE users SET approved = $1 WHERE id = $2`
    ///
    /// Approved users can log in and access the platform; unapproved users
    /// are blocked at the `bearer_auth` middleware layer.
    ///
    /// **Index**: `users(id)` PK.
    pub async fn approve_user(&self, user_id: i64, approve: bool) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET approved = $1 WHERE id = $2")
            .bind(approve)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Count unapproved (pending) users in a tenant.
    ///
    /// **SQL**: `SELECT COUNT(*) FROM users WHERE tenant_id = $1 AND approved = false`
    ///
    /// Used by the admin dashboard badge to show how many users need review.
    ///
    /// **Index**: `users(tenant_id, approved)` or the composite tenant index
    /// with a filter on `approved = false`.
    pub async fn count_pending_users(&self, tenant_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE tenant_id = $1 AND approved = false")
                .bind(tenant_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    /// Purge all expired and revoked tokens from the database.
    ///
    /// **SQL**: `DELETE FROM auth_tokens WHERE expires_at <= CURRENT_TIMESTAMP OR revoked`
    ///
    /// Run periodically by a background cleanup task. The OR condition
    /// catches both naturally expired tokens and admin-revoked ones.
    /// The `revoked` column is indexed so the OR scan can use either
    /// `auth_tokens(expires_at)` or `auth_tokens(revoked)` index paths.
    pub async fn cleanup_expired_tokens(&self) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM auth_tokens WHERE expires_at <= CURRENT_TIMESTAMP OR revoked")
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    /// Set a user's role to an arbitrary value (fine-grained admin control).
    ///
    /// **SQL**: `UPDATE users SET role = $1 WHERE id = $2`
    ///
    /// Role values: 1 = normal user, 2 = moderator (future), 3+ = admin.
    /// Prefer `toggle_admin` for simple on/off toggling.
    ///
    /// **Index**: `users(id)` PK.
    pub async fn set_user_role(&self, user_id: i64, role: i16) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
            .bind(role)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Fetch a user's role value.
    ///
    /// **SQL**: `SELECT role FROM users WHERE id = $1`
    ///
    /// Returns `None` if the user does not exist. Used by permission checks
    /// that need the numeric role without loading the full `UserRow`.
    ///
    /// **Index**: `users(id)` PK.
    pub async fn get_user_role(&self, user_id: i64) -> Result<Option<i16>, sqlx::Error> {
        let result: Option<(i16,)> = sqlx::query_as("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(result.map(|(role,)| role))
    }

    /// Update a user's avatar URL.
    ///
    /// **SQL**: `UPDATE users SET avatar_url = $1 WHERE id = $2`
    ///
    /// The URL is typically a relative path under `/media/avatars/` generated
    /// by the upload handler. Set to `NULL` to remove the avatar.
    ///
    /// **Index**: `users(id)` PK.
    pub async fn update_avatar(&self, user_id: i64, avatar_url: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET avatar_url = $1 WHERE id = $2")
            .bind(avatar_url)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Change a user's email address and reset verification status.
    ///
    /// **SQL** (in a transaction):
    /// 1. `UPDATE users SET email = $1, email_verified = false WHERE id = $2`
    /// 2. `UPDATE email_verification_tokens SET used = true WHERE user_id = $1 AND NOT used`
    ///
    /// Both statements run in a single transaction. Setting `email_verified`
    /// to `false` forces the user to re-verify the new email. Invalidating
    /// old verification tokens prevents stale tokens from verifying the
    /// wrong address.
    ///
    /// **Index**: `users(id)` PK; `email_verification_tokens(user_id, used)`
    /// for the token invalidation.
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

    /// Mark a user's email as verified.
    ///
    /// **SQL**: `UPDATE users SET email_verified = true WHERE id = $1`
    ///
    /// Called after the user clicks the verification link. Verified emails
    /// are eligible for password reset and notification delivery.
    ///
    /// **Index**: `users(id)` PK.
    pub async fn verify_email(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET email_verified = true WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Look up a user by email address (cross-tenant).
    ///
    /// **SQL**: `SELECT … FROM users WHERE email = $1`
    ///
    /// Used for password reset flow: the user enters their email and we
    /// need to find the account. This is a **cross-tenant** lookup — if
    /// multiple tenants share the same email, the first match is returned.
    ///
    /// **Index**: `users(email)` index for the single-column lookup. If no
    /// email index exists, this falls back to a sequential scan (consider
    /// adding `CREATE INDEX idx_users_email ON users(email)` if this
    /// becomes a hot path).
    pub async fn find_by_email(&self, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, approved, role, avatar_url, created_at, email, email_verified, tenant_id FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
    }

    /// Mint a new password-reset token for `user_id`.
    ///
    /// **SQL** (in a transaction):
    /// 1. `UPDATE password_reset_tokens SET used = true
    ///     WHERE user_id = $1 AND NOT used` — invalidate prior tokens
    /// 2. `INSERT INTO password_reset_tokens (user_id, token_hash, expires_at)
    ///     VALUES ($1, $2, CURRENT_TIMESTAMP + INTERVAL '1 hour')`
    ///
    /// Only one active reset token per user is allowed; the UPDATE marks any
    /// existing unused token as used before inserting a new one. Token
    /// expires in 1 hour. The raw token is returned; only its SHA-256 hash
    /// is stored.
    ///
    /// **Index**: `password_reset_tokens(user_id, used)` for the invalidation
    /// update; `password_reset_tokens(token_hash)` for later lookups.
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

    /// Atomically find and consume a password-reset token (single-use).
    ///
    /// **SQL**: `UPDATE password_reset_tokens SET used = true WHERE id = (
    ///          SELECT id FROM password_reset_tokens
    ///          WHERE token_hash = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT used
    ///          FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING user_id`
    ///
    /// The `FOR UPDATE SKIP LOCKED` pattern prevents concurrent requests
    /// from consuming the same token — the second caller's subquery skips
    /// the locked row and finds nothing. This is a race-safe single-use
    /// consume operation.
    ///
    /// **Index**: `password_reset_tokens(token_hash)` for the subquery
    /// lookup; the `FOR UPDATE` takes a row-level lock on the matched row.
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

    /// Mark a password-reset token as used (explicit consume, distinct from
    /// `find_valid_reset_token` which consumes atomically via CTE).
    ///
    /// **SQL**: `UPDATE password_reset_tokens SET used = true WHERE token_hash = $1`
    ///
    /// Used when the caller needs a simple "mark as used" without the
    /// CTE-based atomic lookup. Rows affected > 0 indicates success.
    ///
    /// **Index**: `password_reset_tokens(token_hash)` for the single-row update.
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

    /// Fetch a user's email address (returns `None` if not set).
    ///
    /// **SQL**: `SELECT email FROM users WHERE id = $1 AND email IS NOT NULL`
    ///
    /// The `IS NOT NULL` guard avoids returning NULL rows — callers get
    /// `None` rather than `Some("")`. Used before sending verification or
    /// reset emails.
    ///
    /// **Index**: `users(id)` PK.
    pub async fn get_email(&self, user_id: i64) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT email FROM users WHERE id = $1 AND email IS NOT NULL")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(e,)| e))
    }

    /// Mint a new email-verification token for `user_id`.
    ///
    /// **SQL** (in a transaction):
    /// 1. `UPDATE email_verification_tokens SET used = true
    ///     WHERE user_id = $1 AND NOT used` — invalidate prior tokens
    /// 2. `INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
    ///     VALUES ($1, $2, CURRENT_TIMESTAMP + INTERVAL '24 hours')`
    ///
    /// Same single-active-token pattern as password reset. Token expires in
    /// 24 hours. The raw token is returned for embedding in a verification
    /// email link.
    ///
    /// **Index**: `email_verification_tokens(user_id, used)` for the
    /// invalidation update; `email_verification_tokens(token_hash)` for
    /// later lookups.
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

    /// Atomically find and consume an email-verification token (single-use).
    ///
    /// **SQL**: `UPDATE email_verification_tokens SET used = true WHERE id = (
    ///          SELECT id FROM email_verification_tokens
    ///          WHERE token_hash = $1 AND expires_at > CURRENT_TIMESTAMP AND NOT used
    ///          FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING user_id`
    ///
    /// Same `FOR UPDATE SKIP LOCKED` race-safe pattern as
    /// `find_valid_reset_token`. The CTE-style UPDATE prevents double-use
    /// under concurrent requests.
    ///
    /// **Index**: `email_verification_tokens(token_hash)` for the subquery;
    /// row-level lock on the matched row.
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

    /// Mark an email-verification token as used (explicit consume).
    ///
    /// **SQL**: `UPDATE email_verification_tokens SET used = true WHERE token_hash = $1`
    ///
    /// Simpler alternative to `find_valid_email_verification_token` when the
    /// caller already holds the user_id and just needs to mark the token used.
    ///
    /// **Index**: `email_verification_tokens(token_hash)` for the single-row
    /// update.
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

    /// Count current active PostgreSQL connections.
    ///
    /// **SQL**: `SELECT count(*)::int FROM pg_stat_activity`
    ///
    /// Queries the `pg_stat_activity` system view (not a regular table).
    /// Used by the admin health dashboard to monitor connection pool
    /// utilization against `DB_MAX_CONNECTIONS`. No user-table indexes
    /// are involved — this is a server-wide system catalog query.
    pub async fn count_active_connections(&self) -> Result<i32, sqlx::Error> {
        let (count,): (i32,) = sqlx::query_as("SELECT count(*)::int FROM pg_stat_activity")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}
