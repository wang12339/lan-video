use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserTokenRow {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn count_users(&self) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    pub async fn create_user(&self, username: &str, password_hash: &str, is_admin: bool) -> Result<i64, sqlx::Error> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO users (username, password_hash, is_admin) VALUES ($1, $2, $3) RETURNING id"
        )
        .bind(username)
        .bind(password_hash)
        .bind(is_admin)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserRecord>, sqlx::Error> {
        let user = sqlx::query_as::<_, UserRecord>(
            "SELECT id, username, password_hash, is_admin, created_at FROM users WHERE username = $1"
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<UserRecord>, sqlx::Error> {
        let user = sqlx::query_as::<_, UserRecord>(
            "SELECT id, username, password_hash, is_admin, created_at FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn create_token(&self, user_id: i64) -> Result<String, sqlx::Error> {
        let token = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO auth_tokens (user_id, token) VALUES ($1, $2)")
            .bind(user_id)
            .bind(&token)
            .execute(&self.pool)
            .await?;
        Ok(token)
    }

    pub async fn find_user_by_token(&self, token: &str) -> Result<Option<UserTokenRow>, sqlx::Error> {
        let user = sqlx::query_as::<_, UserTokenRow>(
            r#"SELECT u.id, u.username, u.password_hash, u.is_admin, u.created_at
               FROM auth_tokens t
               JOIN users u ON t.user_id = u.id
               WHERE t.token = $1"#
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn delete_token(&self, token: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM auth_tokens WHERE token = $1")
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
