use sqlx::PgPool;
use crate::models::video::VideoItem;
use crate::models::playback::RecentWatchItem;
use std::collections::HashSet;

/// Explicit columns for VideoRow — avoids SELECT * fetching unnecessary data
const VIDEO_COLUMNS: &str = "id, title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, created_at, views, duration";
const VIDEO_COLUMNS_PREFIXED: &str = "v.id, v.title, v.description, v.source_type, v.cover_url, v.thumb_url, v.stream_url, v.category, v.file_hash, v.file_size, v.original_name, v.created_at, v.views, v.duration";

#[derive(Debug, sqlx::FromRow)]
pub struct VideoRow {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub source_type: String,
    pub cover_url: Option<String>,
    pub thumb_url: Option<String>,
    pub stream_url: String,
    pub category: String,
    #[allow(dead_code)]
    pub file_hash: Option<String>,
    #[allow(dead_code)]
    pub file_size: Option<i64>,
    #[allow(dead_code)]
    pub original_name: Option<String>,
    #[allow(dead_code)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub views: i64,
    pub duration: i64,
    #[sqlx(default)]
    pub watch_position: Option<i64>,
}

impl From<VideoRow> for VideoItem {
    fn from(r: VideoRow) -> Self {
        VideoItem {
            id: r.id,
            title: r.title,
            description: r.description,
            source_type: r.source_type,
            cover_url: r.cover_url,
            thumb_url: r.thumb_url,
            stream_url: r.stream_url,
            category: r.category,
            views: r.views,
            duration: r.duration,
            watch_position: r.watch_position,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct PlaybackRow {
    #[allow(dead_code)]
    pub video_id: i64,
    pub position_ms: i64,
    pub duration_ms: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct FileHashRow {
    pub file_hash: Option<String>,
}

#[derive(Clone)]
pub struct VideoRepository {
    pool: PgPool,
}

impl VideoRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Videos ──

    pub async fn find_all(&self, query: Option<&str>) -> Result<Vec<VideoRow>, sqlx::Error> {
        match query {
            Some(q) => {
                let pattern = format!("%{}%", q);
                sqlx::query_as::<_, VideoRow>(
                    &format!("SELECT {} FROM videos WHERE title ILIKE $1 OR category ILIKE $1 ORDER BY id DESC", VIDEO_COLUMNS)
                )
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as::<_, VideoRow>(
                    &format!("SELECT {} FROM videos ORDER BY id DESC", VIDEO_COLUMNS)
                )
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    pub async fn count_all(&self, query: Option<&str>, source_type: Option<&str>, category: Option<&str>) -> Result<i64, sqlx::Error> {
        let mut builder = sqlx::QueryBuilder::new("SELECT COUNT(*) as count FROM videos v WHERE 1=1");

        if let Some(q) = query {
            let pattern = format!("%{}%", q);
            builder.push(" AND (v.title ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR v.category ILIKE ");
            builder.push_bind(pattern);
            builder.push(")");
        }
        if let Some(t) = source_type {
            if let Some(stripped) = t.strip_prefix('!') {
                builder.push(" AND v.source_type != ");
                builder.push_bind(stripped.to_string());
            } else {
                builder.push(" AND v.source_type = ");
                builder.push_bind(t.to_string());
            }
        }
        if let Some(c) = category {
            builder.push(" AND v.category = ");
            builder.push_bind(c);
        }

        builder.build_query_scalar().fetch_one(&self.pool).await
    }

    pub async fn find_all_paged(
        &self,
        page: i64,
        size: i64,
        query: Option<&str>,
        source_type: Option<&str>,
        category: Option<&str>,
        username: Option<&str>,
    ) -> Result<Vec<VideoRow>, sqlx::Error> {
        let offset = page * size;
        let mut builder = sqlx::QueryBuilder::default();

        builder.push(format!("SELECT {}, h.position_ms AS watch_position FROM videos v", VIDEO_COLUMNS_PREFIXED));
        if let Some(u) = username {
            builder.push(" LEFT JOIN playback_history h ON v.id = h.video_id AND h.username = ");
            builder.push_bind(u);
        }
        builder.push(" WHERE 1=1");

        if let Some(q) = query {
            let pattern = format!("%{}%", q);
            builder.push(" AND (v.title ILIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR v.category ILIKE ");
            builder.push_bind(pattern);
            builder.push(")");
        }
        if let Some(t) = source_type {
            if let Some(stripped) = t.strip_prefix('!') {
                builder.push(" AND v.source_type != ");
                builder.push_bind(stripped.to_string());
            } else {
                builder.push(" AND v.source_type = ");
                builder.push_bind(t.to_string());
            }
        }
        if let Some(c) = category {
            builder.push(" AND v.category = ");
            builder.push_bind(c);
        }

        if username.is_some() {
            builder.push(" ORDER BY CASE WHEN h.id IS NOT NULL THEN 1 ELSE 0 END, h.updated_at ASC NULLS LAST, v.id DESC");
        } else {
            builder.push(" ORDER BY v.id DESC");
        }

        builder.push(" LIMIT ");
        builder.push_bind(size);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        builder.build_query_as::<VideoRow>().fetch_all(&self.pool).await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<VideoRow>, sqlx::Error> {
        sqlx::query_as::<_, VideoRow>(&format!("SELECT {} FROM videos WHERE id = $1", VIDEO_COLUMNS))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn save_external_video(
        &self,
        title: &str,
        description: &str,
        category: &str,
        cover_url: Option<&str>,
        stream_url: &str,
    ) -> Result<i64, sqlx::Error> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO videos (title, description, source_type, cover_url, stream_url, category) \
             VALUES ($1, $2, 'external', $3, $4, $5) RETURNING id"
        )
        .bind(title)
        .bind(description)
        .bind(cover_url)
        .bind(stream_url)
        .bind(category)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_video_by_file_hash(&self, hash: &str) -> Result<Option<VideoRow>, sqlx::Error> {
        sqlx::query_as::<_, VideoRow>(&format!("SELECT {} FROM videos WHERE file_hash = $1", VIDEO_COLUMNS))
            .bind(hash)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn find_existing_hashes(&self, hashes: &[String]) -> Result<Vec<String>, sqlx::Error> {
        if hashes.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query_as::<_, FileHashRow>(
            "SELECT file_hash FROM videos WHERE file_hash = ANY($1)"
        )
        .bind(hashes)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().filter_map(|r| r.file_hash).collect())
    }

    /// Find videos without a cover, paginated by id for cursor-based iteration
    pub async fn find_videos_without_cover(&self, after_id: i64, limit: i64) -> Result<Vec<VideoRow>, sqlx::Error> {
        sqlx::query_as::<_, VideoRow>(&format!(
            "SELECT {} FROM videos WHERE cover_url IS NULL AND source_type LIKE 'local%' AND id > $1 ORDER BY id LIMIT $2",
            VIDEO_COLUMNS
        ))
        .bind(after_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Batch check which (name, size) pairs already exist — single query instead of N+1
    pub async fn find_existing_by_name_and_size_batch(&self, files: &[(String, i64)]) -> Result<HashSet<(String, i64)>, sqlx::Error> {
        if files.is_empty() {
            return Ok(HashSet::new());
        }
        let mut builder = sqlx::QueryBuilder::new(
            "SELECT original_name, file_size FROM videos WHERE (original_name, file_size) IN ("
        );
        let mut separated = builder.separated(", ");
        for (name, size) in files {
            separated.push("(");
            separated.push_bind_unseparated(name);
            separated.push_unseparated(", ");
            separated.push_bind_unseparated(size);
            separated.push_unseparated(")");
        }
        separated.push_unseparated(")");

        #[derive(sqlx::FromRow)]
        struct NameSize {
            original_name: Option<String>,
            file_size: Option<i64>,
        }
        let rows: Vec<NameSize> = builder.build_query_as().fetch_all(&self.pool).await?;
        Ok(rows.into_iter()
            .filter_map(|r| Some((r.original_name?, r.file_size?)))
            .collect())
    }

    /// Batch delete videos and their related data in a single transaction
    pub async fn batch_delete_videos(&self, ids: &[i64]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() { return Ok(0); }
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM playback_history WHERE video_id = ANY($1)")
            .bind(ids).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM user_likes WHERE video_id = ANY($1)")
            .bind(ids).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM user_favorites WHERE video_id = ANY($1)")
            .bind(ids).execute(&mut *tx).await?;
        let result = sqlx::query("DELETE FROM videos WHERE id = ANY($1)")
            .bind(ids).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn save_local_video(
        &self,
        title: &str,
        description: &str,
        source_type: &str,
        cover_url: Option<&str>,
        stream_url: &str,
        category: &str,
        file_hash: Option<&str>,
        file_size: Option<i64>,
        original_name: Option<&str>,
        thumb_url: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO videos (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id"
        )
        .bind(title)
        .bind(description)
        .bind(source_type)
        .bind(cover_url)
        .bind(thumb_url)
        .bind(stream_url)
        .bind(category)
        .bind(file_hash)
        .bind(file_size)
        .bind(original_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn update_video(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        category: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        if title.is_none() && description.is_none() && category.is_none() {
            return Ok(());
        }

        let mut builder = sqlx::QueryBuilder::new("UPDATE videos SET ");
        let mut sep = builder.separated(", ");

        if let Some(t) = title { sep.push("title = "); sep.push_bind(t); }
        if let Some(d) = description { sep.push("description = "); sep.push_bind(d); }
        if let Some(c) = category { sep.push("category = "); sep.push_bind(c); }

        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.build().execute(&self.pool).await?;
        Ok(())
    }

    pub async fn delete_video(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM videos WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn increment_views(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET views = views + 1 WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_duration(&self, id: i64, duration_ms: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET duration = $1 WHERE id = $2")
            .bind(duration_ms / 1000)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_playback_history_by_video(
        &self, id: i64
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM playback_history WHERE video_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn update_cover_url(&self, id: i64, cover_url: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET cover_url = $1 WHERE id = $2")
            .bind(cover_url)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_thumb_url(&self, id: i64, thumb_url: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET thumb_url = $1 WHERE id = $2")
            .bind(thumb_url)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Playback History ──

    pub async fn get_playback_position(&self, username: &str, video_id: i64) -> Result<Option<i64>, sqlx::Error> {
        let row = sqlx::query_as::<_, PlaybackRow>(
            "SELECT video_id, position_ms, duration_ms FROM playback_history WHERE username = $1 AND video_id = $2"
        )
        .bind(username)
        .bind(video_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.position_ms))
    }

    pub async fn get_playback_duration(&self, username: &str, video_id: i64) -> Result<Option<i64>, sqlx::Error> {
        let row = sqlx::query_as::<_, PlaybackRow>(
            "SELECT video_id, position_ms, duration_ms FROM playback_history WHERE username = $1 AND video_id = $2"
        )
        .bind(username)
        .bind(video_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.duration_ms))
    }

    pub async fn find_playback_history_by_username(&self, username: &str) -> Result<Vec<RecentWatchItem>, sqlx::Error> {
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
            r#"SELECT h.video_id, v.title, v.cover_url, v.stream_url, v.source_type, v.category,
                      h.position_ms, h.duration_ms, h.updated_at
               FROM playback_history h
               JOIN videos v ON h.video_id = v.id
               WHERE h.username = $1
               ORDER BY h.updated_at DESC
               LIMIT 500"#
        )
        .bind(username)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| RecentWatchItem {
            video_id: r.video_id,
            title: r.title,
            cover_url: r.cover_url,
            stream_url: r.stream_url,
            source_type: r.source_type,
            category: r.category,
            position_ms: r.position_ms,
            duration_ms: r.duration_ms,
            updated_at: r.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }).collect())
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

    pub async fn find_recent_history_with_details(
        &self,
        username: &str,
        limit: i64,
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
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT h.video_id, v.title, v.cover_url, v.stream_url, v.source_type, v.category,
                      h.position_ms, h.duration_ms, h.updated_at
               FROM playback_history h
               JOIN videos v ON h.video_id = v.id
               WHERE h.username = $1
               ORDER BY h.updated_at DESC
               LIMIT $2"#
        )
        .bind(username)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| RecentWatchItem {
            video_id: r.video_id,
            title: r.title,
            cover_url: r.cover_url,
            stream_url: r.stream_url,
            source_type: r.source_type,
            category: r.category,
            position_ms: r.position_ms,
            duration_ms: r.duration_ms,
            updated_at: r.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }).collect())
    }

    pub async fn count_watched_videos(&self, username: &str) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM playback_history WHERE username = $1"
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn sum_watch_time(&self, username: &str) -> Result<i64, sqlx::Error> {
        let row = sqlx::query_as::<_, (Option<i64>,)>(
            "SELECT COALESCE(SUM(duration_ms), 0) FROM playback_history WHERE username = $1"
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.unwrap_or(0))
    }

    pub async fn find_all_local_file_names(&self) -> Result<HashSet<String>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            stream_url: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT stream_url FROM videos WHERE source_type LIKE 'local%'"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.stream_url).collect())
    }

    // ── Likes ──

    pub async fn is_liked(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM user_likes WHERE username = $1 AND video_id = $2)"
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    pub async fn toggle_like(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        // Atomic toggle: DELETE if exists, INSERT if not — single statement, no race condition
        let (liked,): (bool,) = sqlx::query_as(
            "WITH del AS (
                DELETE FROM user_likes WHERE username = $1 AND video_id = $2 RETURNING 1
            ), ins AS (
                INSERT INTO user_likes (username, video_id)
                SELECT $1, $2 WHERE NOT EXISTS (SELECT 1 FROM del)
                ON CONFLICT DO NOTHING RETURNING 1
            )
            SELECT EXISTS (SELECT 1 FROM ins)"
        )
        .bind(username).bind(video_id)
        .fetch_one(&self.pool).await?;
        Ok(liked)
    }

    // ── Favorites ──

    pub async fn is_favorited(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM user_favorites WHERE username = $1 AND video_id = $2)"
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    pub async fn toggle_favorite(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        // Atomic toggle: DELETE if exists, INSERT if not — single statement, no race condition
        let (favorited,): (bool,) = sqlx::query_as(
            "WITH del AS (
                DELETE FROM user_favorites WHERE username = $1 AND video_id = $2 RETURNING 1
            ), ins AS (
                INSERT INTO user_favorites (username, video_id)
                SELECT $1, $2 WHERE NOT EXISTS (SELECT 1 FROM del)
                ON CONFLICT DO NOTHING RETURNING 1
            )
            SELECT EXISTS (SELECT 1 FROM ins)"
        )
        .bind(username).bind(video_id)
        .fetch_one(&self.pool).await?;
        Ok(favorited)
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
