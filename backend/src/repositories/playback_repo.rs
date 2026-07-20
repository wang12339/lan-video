use crate::models::playback::RecentWatchItem;
use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow)]
struct PlaybackRow {
    pub position_ms: i64,
    pub duration_ms: i64,
}

#[derive(Clone)]
pub struct PlaybackRepository {
    pool: PgPool,
}

impl PlaybackRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

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

    pub async fn find_playback_history_by_username(
        &self,
        username: &str,
        limit: Option<i64>,
    ) -> Result<Vec<RecentWatchItem>, sqlx::Error> {
        #[derive(Debug, sqlx::FromRow)]
        struct Row {
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
        let limit = limit.unwrap_or(500);
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT h.video_id, v.title, v.cover_url, v.stream_url, v.source_type, v.category,
                      h.position_ms, h.duration_ms, h.updated_at
               FROM playback_history h
               JOIN videos v ON h.video_id = v.id
               WHERE h.username = $1
               ORDER BY h.updated_at DESC
               LIMIT $2"#,
        )
        .bind(username)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| RecentWatchItem {
                video_id: r.video_id,
                title: r.title,
                cover_url: r.cover_url,
                stream_url: r.stream_url,
                source_type: r.source_type,
                category: r.category,
                position_ms: r.position_ms,
                duration_ms: r.duration_ms,
                updated_at: r.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect())
    }

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

    pub async fn count_watched_videos(&self, username: &str) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM playback_history WHERE username = $1")
                .bind(username)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    pub async fn sum_watch_time(&self, username: &str) -> Result<i64, sqlx::Error> {
        let row = sqlx::query_as::<_, (Option<i64>,)>(
            "SELECT COALESCE(SUM(duration_ms), 0) FROM playback_history WHERE username = $1",
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.unwrap_or(0))
    }

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

    pub async fn find_favorites_by_username(
        &self,
        username: &str,
    ) -> Result<Vec<crate::models::playback::RecentWatchItem>, sqlx::Error> {
        #[derive(Debug, sqlx::FromRow)]
        struct Row {
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
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT f.video_id, v.title, v.cover_url, v.stream_url, v.source_type, v.category,
                      COALESCE(h.position_ms, 0) AS position_ms, COALESCE(h.duration_ms, 0) AS duration_ms,
                      f.created_at AS updated_at
               FROM user_favorites f
               JOIN videos v ON f.video_id = v.id
               LEFT JOIN playback_history h ON f.video_id = h.video_id AND h.username = f.username
               WHERE f.username = $1
               ORDER BY f.created_at DESC
               LIMIT 500"#,
        )
        .bind(username)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| RecentWatchItem {
                video_id: r.video_id,
                title: r.title,
                cover_url: r.cover_url,
                stream_url: r.stream_url,
                source_type: r.source_type,
                category: r.category,
                position_ms: r.position_ms,
                duration_ms: r.duration_ms,
                updated_at: r.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect())
    }

    pub async fn delete_playback_history_by_video(&self, id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM playback_history WHERE video_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_likes_by_video(&self, video_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM user_likes WHERE video_id = $1")
            .bind(video_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_favorites_by_video(&self, video_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM user_favorites WHERE video_id = $1")
            .bind(video_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
