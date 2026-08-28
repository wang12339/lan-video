use crate::models::playback::RecentWatchItem;
use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow)]
struct PlaybackRow {
    pub position_ms: i64,
    pub duration_ms: i64,
}

/// 共享行结构：观看历史与收藏列表返回相同字段。
#[derive(Debug, sqlx::FromRow)]
struct HistoryRow {
    video_id: i64,
    title: String,
    cover_url: Option<String>,
    stream_url: String,
    source_type: String,
    category: String,
    position_ms: i64,
    duration_ms: i64,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<HistoryRow> for RecentWatchItem {
    fn from(r: HistoryRow) -> Self {
        RecentWatchItem {
            video_id: r.video_id,
            title: r.title,
            cover_url: r.cover_url,
            stream_url: r.stream_url,
            source_type: r.source_type,
            category: r.category,
            position_ms: r.position_ms,
            duration_ms: r.duration_ms,
            updated_at: r.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

/// 播放历史与用户互动（点赞/收藏）的数据库仓库。
///
/// 封装了与 `playback_history`、`user_likes`、`user_favorites` 表的所有交互操作，
/// 提供播放进度读写、点赞/收藏的切换与查询、以及用户观看统计等功能。
#[derive(Clone)]
pub struct PlaybackRepository {
    pool: PgPool,
}

impl PlaybackRepository {
    /// 创建一个新的 `PlaybackRepository` 实例。
    ///
    /// # 参数
    /// - `pool`: PostgreSQL 连接池。
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 获取底层的 PostgreSQL 连接池引用。
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// 获取指定用户对某个视频的播放进度。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `video_id`: 视频 ID。
    ///
    /// # 返回
    /// 如果存在播放记录，返回 `Some((position_ms, duration_ms))`；否则返回 `None`。
    pub async fn get_playback_data(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<Option<(i64, i64)>, sqlx::Error> {
        let row = sqlx::query_as::<_, PlaybackRow>(
            "SELECT position_ms, duration_ms FROM playback_history WHERE username = $1 AND video_id = $2"
        )
        .bind(username)
        .bind(video_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.position_ms, r.duration_ms)))
    }

    /// 查询指定用户的播放历史列表（分页）。
    ///
    /// 按 `updated_at` 降序排列，返回指定页的数据及总记录数。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `limit`: 每页条数。
    /// - `offset`: 偏移量。
    ///
    /// # 返回
    /// `(items, total)` — 当页的 `RecentWatchItem` 列表和符合条件的总记录数。
    pub async fn find_playback_history_by_username(
        &self,
        username: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RecentWatchItem>, i64), sqlx::Error> {
        let rows = sqlx::query_as::<_, HistoryRow>(
            r#"SELECT h.video_id, v.title, v.cover_url, v.stream_url, v.source_type, v.category,
                      h.position_ms, h.duration_ms, h.updated_at
               FROM playback_history h
               JOIN videos v ON h.video_id = v.id
               WHERE h.username = $1
               ORDER BY h.updated_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(username)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total = if (rows.len() as i64) < limit {
            rows.len() as i64 + offset
        } else {
            let (cnt,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM playback_history WHERE username = $1")
                    .bind(username)
                    .fetch_one(&self.pool)
                    .await?;
            cnt
        };

        Ok((rows.into_iter().map(RecentWatchItem::from).collect(), total))
    }

    /// 插入或更新播放进度记录（upsert）。
    ///
    /// 如果 `(username, video_id)` 已存在则更新 `position_ms`、`duration_ms` 和 `updated_at`，
    /// 否则插入新行。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `video_id`: 视频 ID。
    /// - `position_ms`: 当前播放位置（毫秒）。
    /// - `duration_ms`: 视频总时长（毫秒）。
    pub async fn upsert_playback(
        &self,
        username: &str,
        video_id: i64,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO playback_history (username, video_id, position_ms, duration_ms, updated_at) \
             VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP) \
             ON CONFLICT (username, video_id) DO UPDATE SET \
             position_ms = $3, duration_ms = $4, updated_at = CURRENT_TIMESTAMP"
        )
        .bind(username)
        .bind(video_id)
        .bind(position_ms)
        .bind(duration_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 统计指定用户观看过的视频数量。
    ///
    /// # 参数
    /// - `username`: 用户名。
    ///
    /// # 返回
    /// 该用户在 `playback_history` 中的记录总数。
    pub async fn count_watched_videos(&self, username: &str) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM playback_history WHERE username = $1")
                .bind(username)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    /// 汇总指定用户的累计观看时长。
    ///
    /// 对所有播放记录的 `duration_ms` 求和；若无记录则返回 0。
    ///
    /// # 参数
    /// - `username`: 用户名。
    ///
    /// # 返回
    /// 累计观看时长（毫秒）。
    pub async fn sum_watch_time(&self, username: &str) -> Result<i64, sqlx::Error> {
        let row = sqlx::query_as::<_, (Option<i64>,)>(
            "SELECT COALESCE(SUM(duration_ms), 0) FROM playback_history WHERE username = $1",
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.unwrap_or(0))
    }

    /// 检查用户是否已点赞指定视频。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `video_id`: 视频 ID。
    ///
    /// # 返回
    /// 已点赞返回 `true`，否则返回 `false`。
    pub async fn is_liked(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM user_likes WHERE username = $1 AND video_id = $2)",
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    /// 切换用户对指定视频的点赞状态。
    ///
    /// 若已点赞则取消点赞，若未点赞则添加点赞（原子操作）。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `video_id`: 视频 ID。
    ///
    /// # 返回
    /// 操作后的点赞状态：`true` 表示已点赞，`false` 表示已取消。
    pub async fn toggle_like(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        let (liked,): (bool,) = sqlx::query_as(
            "WITH del AS (
                DELETE FROM user_likes WHERE username = $1 AND video_id = $2 RETURNING 1
            ), ins AS (
                INSERT INTO user_likes (username, video_id)
                SELECT $1, $2 WHERE NOT EXISTS (SELECT 1 FROM del)
                ON CONFLICT DO NOTHING RETURNING 1
            )
            SELECT EXISTS (SELECT 1 FROM ins)",
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(liked)
    }

    /// 检查用户是否已收藏指定视频。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `video_id`: 视频 ID。
    ///
    /// # 返回
    /// 已收藏返回 `true`，否则返回 `false`。
    pub async fn is_favorited(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM user_favorites WHERE username = $1 AND video_id = $2)",
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    /// 切换用户对指定视频的收藏状态。
    ///
    /// 若已收藏则取消收藏，若未收藏则添加收藏（原子操作）。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `video_id`: 视频 ID。
    ///
    /// # 返回
    /// 操作后的收藏状态：`true` 表示已收藏，`false` 表示已取消。
    pub async fn toggle_favorite(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let (favorited,): (bool,) = sqlx::query_as(
            "WITH del AS (
                DELETE FROM user_favorites WHERE username = $1 AND video_id = $2 RETURNING 1
            ), ins AS (
                INSERT INTO user_favorites (username, video_id)
                SELECT $1, $2 WHERE NOT EXISTS (SELECT 1 FROM del)
                ON CONFLICT DO NOTHING RETURNING 1
            )
            SELECT EXISTS (SELECT 1 FROM ins)",
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(favorited)
    }

    /// 查询指定用户的收藏视频列表（分页）。
    ///
    /// 按收藏时间降序排列，返回指定页的数据及总记录数。播放进度从 `playback_history` 左连接获取，
    /// 无播放记录时默认为 0。
    ///
    /// # 参数
    /// - `username`: 用户名。
    /// - `limit`: 每页条数。
    /// - `offset`: 偏移量。
    ///
    /// # 返回
    /// `(items, total)` — 当页的 `RecentWatchItem` 列表和符合条件的总记录数。
    pub async fn find_favorites_by_username(
        &self,
        username: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<crate::models::playback::RecentWatchItem>, i64), sqlx::Error> {
        let (total,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM user_favorites WHERE username = $1")
                .bind(username)
                .fetch_one(&self.pool)
                .await?;

        let rows = sqlx::query_as::<_, HistoryRow>(
            r#"SELECT f.video_id, v.title, v.cover_url, v.stream_url, v.source_type, v.category,
                      COALESCE(h.position_ms, 0) AS position_ms, COALESCE(h.duration_ms, 0) AS duration_ms,
                      f.created_at::timestamptz AS updated_at
               FROM user_favorites f
               JOIN videos v ON f.video_id = v.id
               LEFT JOIN playback_history h ON f.video_id = h.video_id AND h.username = f.username
               WHERE f.username = $1
               ORDER BY f.created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(username)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok((rows.into_iter().map(RecentWatchItem::from).collect(), total))
    }
}
