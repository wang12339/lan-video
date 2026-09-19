use sqlx::PgPool;

use crate::models::chat::{ChatMessageRow, NewChatMessage};

/// 聊天消息数据访问层（`chat_messages` 表，迁移 053–055）。
#[derive(Clone)]
pub struct ChatRepository {
    pool: PgPool,
}

impl ChatRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 插入一条消息并返回完整行（含 created_at）。
    ///
    /// `msg_type`：0=文本、1=图片、2=视频（图片/视频消息 `content` 为配文，
    /// 可为空；`image_url`/`video_url` 指向 /media/chat/ 下的文件）。
    pub async fn insert(
        &self,
        user_id: i64,
        username: &str,
        msg: NewChatMessage<'_>,
    ) -> Result<ChatMessageRow, sqlx::Error> {
        let content = msg.content.trim();
        // 非文本消息（图片/视频）允许空配文
        debug_assert!(!content.is_empty() || msg.msg_type != 0);
        sqlx::query_as::<_, ChatMessageRow>(
            "INSERT INTO chat_messages (user_id, username, content, msg_type, image_url, video_url) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             RETURNING id, user_id, username, content, msg_type, image_url, video_url, created_at",
        )
        .bind(user_id)
        .bind(username)
        .bind(content)
        .bind(msg.msg_type)
        .bind(msg.image_url)
        .bind(msg.video_url)
        .fetch_one(&self.pool)
        .await
    }

    /// 历史分页：按 id 倒序游标翻页（`before_id = NULL` 时从最新开始）。
    pub async fn list_paged(
        &self,
        before_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<ChatMessageRow>, sqlx::Error> {
        let limit = limit.clamp(1, 100);
        sqlx::query_as::<_, ChatMessageRow>(
            "SELECT id, user_id, username, content, msg_type, image_url, video_url, created_at FROM chat_messages \
             WHERE ($1::bigint IS NULL OR id < $1) \
             ORDER BY id DESC LIMIT $2",
        )
        .bind(before_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// 按 id 取单条消息（管理员删言前取 image_url/video_url 用）。
    pub async fn find_by_id(&self, id: i64) -> Result<Option<ChatMessageRow>, sqlx::Error> {
        sqlx::query_as::<_, ChatMessageRow>(
            "SELECT id, user_id, username, content, msg_type, image_url, video_url, created_at \
             FROM chat_messages WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// 消息总数（管理后台统计用）。
    pub async fn count(&self) -> Result<i64, sqlx::Error> {
        let (n,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM chat_messages")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    /// 清空全部消息。返回删除条数。
    pub async fn delete_all(&self) -> Result<u64, sqlx::Error> {
        let res = sqlx::query("DELETE FROM chat_messages")
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// 管理员删除单条消息。返回是否删除成功。
    pub async fn delete(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM chat_messages WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
