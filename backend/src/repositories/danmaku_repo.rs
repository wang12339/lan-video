use sqlx::PgPool;

use crate::models::danmaku::{DanmakuItemResponse, SendDanmakuRequest};
use crate::util::hashid;

#[derive(Debug, sqlx::FromRow)]
pub struct DanmakuRow {
    pub id: i64,
    pub video_id: i64,
    pub user_id: i64,
    pub text: String,
    #[sqlx(rename = "time")]
    pub time: f64,
    pub color: Option<String>,
    pub font_size: Option<i32>,
    pub created_at: chrono::NaiveDateTime,
}

/// 弹幕数据仓库，封装所有与 `danmaku` 表相关的数据库操作。
#[derive(Clone)]
pub struct DanmakuRepository {
    pool: PgPool,
}

impl DanmakuRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 按出现时间顺序返回某视频的全部弹幕
    pub async fn list_by_video(
        &self,
        video_id: i64,
    ) -> Result<Vec<DanmakuItemResponse>, sqlx::Error> {
        let rows = sqlx::query_as::<_, DanmakuRow>(
            "SELECT id, video_id, user_id, text, \"time\", color, font_size, created_at \
             FROM danmaku WHERE video_id = $1 ORDER BY \"time\" ASC, id ASC",
        )
        .bind(video_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DanmakuItemResponse {
                id: hashid::encode_id(r.id),
                text: r.text,
                time: r.time,
                color: r.color,
                font_size: r.font_size,
            })
            .collect())
    }

    /// 创建一条弹幕，返回新记录的自增 ID
    pub async fn create(
        &self,
        video_id: i64,
        user_id: i64,
        req: &SendDanmakuRequest,
    ) -> Result<i64, sqlx::Error> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO danmaku (video_id, user_id, text, \"time\", color, font_size) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(video_id)
        .bind(user_id)
        .bind(&req.text)
        .bind(req.time)
        .bind(&req.color)
        .bind(req.font_size)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }
}
