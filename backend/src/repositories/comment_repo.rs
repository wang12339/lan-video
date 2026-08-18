use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow)]
pub struct CommentRow {
    pub id: i64,
    pub video_id: i64,
    pub user_id: i64,
    pub username: String,
    pub avatar_url: Option<String>,
    pub content: String,
    pub parent_id: Option<i64>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Clone)]
pub struct CommentRepository {
    pool: PgPool,
}

impl CommentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_comment(
        &self,
        video_id: i64,
        user_id: i64,
        content: &str,
        parent_id: Option<i64>,
    ) -> Result<CommentRow, sqlx::Error> {
        sqlx::query_as::<_, CommentRow>(
            r#"INSERT INTO comments (video_id, user_id, content, parent_id)
               VALUES ($1, $2, $3, $4)
               RETURNING id, video_id, user_id,
               (SELECT username FROM users WHERE id = $2) AS username,
               (SELECT avatar_url FROM users WHERE id = $2) AS avatar_url,
               content, parent_id, created_at"#,
        )
        .bind(video_id)
        .bind(user_id)
        .bind(content)
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_comments(
        &self,
        video_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CommentRow>, sqlx::Error> {
        sqlx::query_as::<_, CommentRow>(
            r#"SELECT c.id, c.video_id, c.user_id, u.username, u.avatar_url,
                      c.content, c.parent_id, c.created_at
               FROM comments c
               JOIN users u ON c.user_id = u.id
               WHERE c.video_id = $1 AND c.parent_id IS NULL
               ORDER BY c.created_at DESC, c.id DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(video_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_replies(
        &self,
        parent_id: i64,
        limit: i64,
    ) -> Result<Vec<CommentRow>, sqlx::Error> {
        sqlx::query_as::<_, CommentRow>(
            r#"SELECT c.id, c.video_id, c.user_id, u.username, u.avatar_url,
                      c.content, c.parent_id, c.created_at
               FROM comments c
               JOIN users u ON c.user_id = u.id
               WHERE c.parent_id = $1
               ORDER BY c.created_at ASC, c.id ASC
               LIMIT $2"#,
        )
        .bind(parent_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_comments(&self, video_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM comments WHERE video_id = $1 AND parent_id IS NULL",
        )
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn delete_comment(&self, comment_id: i64, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM comments WHERE id = $1 AND user_id = $2")
            .bind(comment_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_comment_admin(&self, comment_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM comments WHERE id = $1")
            .bind(comment_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn video_exists(&self, video_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM videos WHERE id = $1)")
                .bind(video_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(exists)
    }

    /// Lightweight lookup used to validate a reply target before insert.
    /// Returns the comment's `(video_id, parent_id)` or `None` if missing.
    pub async fn get_comment_meta(
        &self,
        comment_id: i64,
    ) -> Result<Option<(i64, Option<i64>)>, sqlx::Error> {
        sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT video_id, parent_id FROM comments WHERE id = $1",
        )
        .bind(comment_id)
        .fetch_optional(&self.pool)
        .await
    }
}
