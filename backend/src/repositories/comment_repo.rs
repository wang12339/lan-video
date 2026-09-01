use sqlx::PgPool;

/// 数据库中单条评论的行映射，包含发布者用户名和头像。
#[derive(Debug, sqlx::FromRow)]
pub struct CommentRow {
    /// 评论唯一 ID
    pub id: i64,
    /// 所属视频 ID
    pub video_id: i64,
    /// 发布者用户 ID
    pub user_id: i64,
    /// 发布者用户名（通过 JOIN users 表获取）
    pub username: String,
    /// 发布者头像 URL，未设置时为 `None`
    pub avatar_url: Option<String>,
    /// 评论正文内容
    pub content: String,
    /// 父评论 ID；顶层评论为 `None`，回复为 `Some(parent_id)`
    pub parent_id: Option<i64>,
    /// 评论创建时间
    pub created_at: chrono::NaiveDateTime,
}

/// 评论数据仓库，封装所有与 `comments` 表相关的数据库操作。
///
/// 通过 [`PgPool`] 连接 PostgreSQL，提供评论的增删查及辅助校验方法。
#[derive(Clone)]
pub struct CommentRepository {
    pool: PgPool,
}

impl CommentRepository {
    /// 创建新的评论仓库实例。
    ///
    /// # 参数
    /// - `pool`：PostgreSQL 连接池
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 创建一条新评论（顶层评论或回复）。
    ///
    /// 插入后通过子查询一并返回发布者的用户名和头像，避免额外查询。
    ///
    /// # 参数
    /// - `video_id`：目标视频 ID
    /// - `user_id`：发布者用户 ID
    /// - `content`：评论正文
    /// - `parent_id`：父评论 ID，`None` 表示顶层评论，`Some(id)` 表示回复
    ///
    /// # 返回
    /// 成功时返回包含完整信息的 [`CommentRow`]（含用户名和头像）。
    ///
    /// # 错误
    /// - 视频不存在或外键约束违反时返回 `sqlx::Error`
    pub async fn create_comment(
        &self,
        tenant_id: i64,
        video_id: i64,
        user_id: i64,
        content: &str,
        parent_id: Option<i64>,
    ) -> Result<CommentRow, sqlx::Error> {
        sqlx::query_as::<_, CommentRow>(
            r#"INSERT INTO comments (tenant_id, video_id, user_id, content, parent_id)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, video_id, user_id,
               (SELECT username FROM users WHERE id = $3) AS username,
               (SELECT avatar_url FROM users WHERE id = $3) AS avatar_url,
               content, parent_id, created_at"#,
        )
        .bind(tenant_id)
        .bind(video_id)
        .bind(user_id)
        .bind(content)
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await
    }

    /// 获取指定视频的顶层评论列表（分页）。
    ///
    /// 仅返回 `parent_id IS NULL` 的顶层评论，按创建时间倒序排列。
    ///
    /// # 参数
    /// - `tenant_id`：租户 ID（列表与评论双重隔离）
    /// - `video_id`：目标视频 ID
    /// - `limit`：每页返回数量
    /// - `offset`：偏移量（用于分页）
    ///
    /// # 返回
    /// 满足条件的 [`CommentRow`] 列表。
    pub async fn get_comments(
        &self,
        tenant_id: i64,
        video_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CommentRow>, sqlx::Error> {
        sqlx::query_as::<_, CommentRow>(
            r#"SELECT c.id, c.video_id, c.user_id, u.username, u.avatar_url,
                      c.content, c.parent_id, c.created_at
               FROM comments c
               JOIN users u ON c.user_id = u.id
               WHERE c.video_id = $1 AND c.tenant_id = $2 AND c.parent_id IS NULL
               ORDER BY c.id DESC
               LIMIT $3 OFFSET $4"#,
        )
        .bind(video_id)
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    /// 获取指定评论的回复列表。
    ///
    /// 返回所有以 `parent_id` 为父 ID 的子评论，按创建时间正序排列（最早的回复在前）。
    ///
    /// # 参数
    /// - `tenant_id`：父评论所属租户
    /// - `parent_id`：父评论 ID
    /// - `limit`：最大返回数量
    ///
    /// # 返回
    /// 满足条件的 [`CommentRow`] 列表。
    pub async fn get_replies(
        &self,
        tenant_id: i64,
        parent_id: i64,
        limit: i64,
    ) -> Result<Vec<CommentRow>, sqlx::Error> {
        sqlx::query_as::<_, CommentRow>(
            r#"SELECT c.id, c.video_id, c.user_id, u.username, u.avatar_url,
                      c.content, c.parent_id, c.created_at
               FROM comments c
               JOIN users u ON c.user_id = u.id
               WHERE c.parent_id = $1 AND c.tenant_id = $2
               ORDER BY c.created_at ASC, c.id ASC
               LIMIT $3"#,
        )
        .bind(parent_id)
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// 统计指定视频的顶层评论总数。
    ///
    /// 仅计算 `parent_id IS NULL` 的评论，不含回复。
    ///
    /// # 参数
    /// - `video_id`：目标视频 ID
    ///
    /// # 返回
    /// 顶层评论数量。
    pub async fn count_comments(&self, tenant_id: i64, video_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM comments WHERE video_id = $1 AND tenant_id = $2 AND parent_id IS NULL",
        )
        .bind(video_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// 删除自己的评论（普通用户权限）。
    ///
    /// 仅当评论的 `user_id` 匹配时才执行删除，防止越权操作。
    ///
    /// # 参数
    /// - `comment_id`：要删除的评论 ID
    /// - `user_id`：操作者用户 ID，必须是评论的原作者
    ///
    /// # 返回
    /// `true` 表示成功删除，`false` 表示评论不存在或无权删除。
    pub async fn delete_comment(
        &self,
        tenant_id: i64,
        comment_id: i64,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM comments WHERE id = $1 AND user_id = $2 AND tenant_id = $3")
                .bind(comment_id)
                .bind(user_id)
                .bind(tenant_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// 管理员强制删除任意评论。
    ///
    /// 不校验 `user_id`，可删除任何用户的评论。
    ///
    /// # 参数
    /// - `comment_id`：要删除的评论 ID
    ///
    /// # 返回
    /// `true` 表示成功删除，`false` 表示评论不存在。
    pub async fn delete_comment_admin(
        &self,
        tenant_id: i64,
        comment_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM comments WHERE id = $1 AND tenant_id = $2")
            .bind(comment_id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// 检查指定视频在当前租户内是否存在。
    ///
    /// 用于发表评论前校验目标视频的有效性。
    ///
    /// # 参数
    /// - `tenant_id`：请求租户 ID
    /// - `video_id`：待检查的视频 ID
    ///
    /// # 返回
    /// `true` 表示视频存在，`false` 表示不存在。
    pub async fn video_exists(&self, tenant_id: i64, video_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM videos WHERE id = $1 AND tenant_id = $2)")
                .bind(video_id)
                .bind(tenant_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(exists)
    }

    /// 轻量查询评论元信息，用于插入回复前校验目标评论。
    ///
    /// 返回目标评论的 `(video_id, parent_id)`，若评论不存在则返回 `None`。
    ///
    /// # 参数
    /// - `tenant_id`：请求租户 ID
    /// - `comment_id`：待查询的评论 ID
    ///
    /// # 返回
    /// `Some((video_id, parent_id))` 或 `None`（评论不存在）。
    pub async fn get_comment_meta(
        &self,
        tenant_id: i64,
        comment_id: i64,
    ) -> Result<Option<(i64, Option<i64>)>, sqlx::Error> {
        sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT video_id, parent_id FROM comments WHERE id = $1 AND tenant_id = $2",
        )
        .bind(comment_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_comments_cursor(
        &self,
        tenant_id: i64,
        video_id: i64,
        limit: i64,
        before_id: Option<i64>,
    ) -> Result<Vec<CommentRow>, sqlx::Error> {
        match before_id {
            Some(cursor) => {
                sqlx::query_as::<_, CommentRow>(
                    r#"SELECT c.id, c.video_id, c.user_id, u.username, u.avatar_url,
                              c.content, c.parent_id, c.created_at
                       FROM comments c
                       JOIN users u ON c.user_id = u.id
                       WHERE c.video_id = $1 AND c.tenant_id = $2 AND c.parent_id IS NULL AND c.id < $3
                       ORDER BY c.id DESC
                       LIMIT $4"#,
                )
                .bind(video_id)
                .bind(tenant_id)
                .bind(cursor)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as::<_, CommentRow>(
                    r#"SELECT c.id, c.video_id, c.user_id, u.username, u.avatar_url,
                              c.content, c.parent_id, c.created_at
                       FROM comments c
                       JOIN users u ON c.user_id = u.id
                       WHERE c.video_id = $1 AND c.tenant_id = $2 AND c.parent_id IS NULL
                       ORDER BY c.id DESC
                       LIMIT $3"#,
                )
                .bind(video_id)
                .bind(tenant_id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
        }
    }
}
