use sqlx::PgPool;
use crate::models::video::VideoItem;
use crate::models::playback::{PlaybackHistoryResponse, RecentWatchItem};
use std::collections::HashSet;

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
    pub file_hash: Option<String>,
    pub file_size: Option<i64>,
    pub original_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
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
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct PlaybackRow {
    pub video_id: i64,
    pub position_ms: i64,
    pub duration_ms: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct FileHashRow {
    pub file_hash: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct StreamUrlRow {
    pub id: i64,
    pub stream_url: String,
}

#[derive(Debug, sqlx::FromRow)]
struct CountRow {
    pub count: i64,
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
                    "SELECT * FROM videos WHERE title ILIKE $1 OR category ILIKE $1 ORDER BY id DESC"
                )
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as::<_, VideoRow>(
                    "SELECT * FROM videos ORDER BY id DESC"
                )
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    /// Build WHERE clause and bound values for source_type filter (supports ! prefix for negation)
    /// Returns (clause, bound_value, has_filter)
    fn build_type_filter(source_type: Option<&str>) -> (String, Option<String>, bool) {
        match source_type {
            Some(t) if t.starts_with('!') => {
                let negated = &t[1..];
                let val = negated.to_string();
                if negated.contains('%') {
                    (format!("source_type NOT LIKE $1"), Some(val), true)
                } else {
                    (format!("source_type != $1"), Some(val), true)
                }
            }
            Some(t) => {
                let val = t.to_string();
                if t.contains('%') {
                    (format!("source_type LIKE $1"), Some(val), true)
                } else {
                    (format!("source_type = $1"), Some(val), true)
                }
            }
            None => (String::new(), None, false),
        }
    }

    pub async fn count_all(&self, query: Option<&str>, source_type: Option<&str>) -> Result<i64, sqlx::Error> {
        let has_query = query.is_some();
        let has_type = source_type.is_some();
        let pattern = query.map(|q| format!("%{}%", q));
        let (type_val, _) = source_type.map_or((None, false), |t| {
            if t.starts_with('!') {
                (Some(t[1..].to_string()), true)
            } else {
                (Some(t.to_string()), true)
            }
        });

        let type_op = |idx: u32| -> String {
            match source_type {
                Some(t) if t.starts_with('!') => format!("source_type != ${}", idx),
                Some(_) => format!("source_type = ${}", idx),
                None => unreachable!(),
            }
        };

        let sql = match (has_query, has_type) {
            (false, false) => "SELECT COUNT(*) as count FROM videos".to_string(),
            (true, false) => {
                "SELECT COUNT(*) as count FROM videos WHERE (title ILIKE $1 OR category ILIKE $1)".to_string()
            }
            (false, true) => {
                format!("SELECT COUNT(*) as count FROM videos WHERE {}", type_op(1))
            }
            (true, true) => {
                format!(
                    "SELECT COUNT(*) as count FROM videos WHERE (title ILIKE $1 OR category ILIKE $1) AND {}",
                    type_op(2)
                )
            }
        };

        let mut q = sqlx::query_as::<_, CountRow>(&sql);
        match (has_query, has_type) {
            (false, false) => {}
            (true, false) => { q = q.bind(pattern.as_deref().unwrap_or_default()); }
            (false, true) => { q = q.bind(type_val.as_deref().unwrap_or_default()); }
            (true, true) => {
                q = q.bind(pattern.as_deref().unwrap_or_default());
                q = q.bind(type_val.as_deref().unwrap_or_default());
            }
        }
        let result = q.fetch_one(&self.pool).await?;
        Ok(result.count)
    }

    pub async fn find_all_paged(
        &self,
        page: i64,
        size: i64,
        query: Option<&str>,
        source_type: Option<&str>,
        username: Option<&str>,
    ) -> Result<Vec<VideoRow>, sqlx::Error> {
        let offset = page * size;
        let has_query = query.is_some();
        let pattern = query.map(|q| format!("%{}%", q));
        let (_, type_val, has_type) = Self::build_type_filter(source_type);

        // SQL helpers
        let order_clause = |user_param: Option<u32>| -> String {
            match user_param {
                Some(_) => "ORDER BY CASE WHEN h.id IS NOT NULL THEN 1 ELSE 0 END, h.updated_at ASC NULLS LAST, v.id DESC".into(),
                None => "ORDER BY id DESC".into(),
            }
        };
        let from_clause = |user_param: Option<u32>| -> String {
            match user_param {
                Some(idx) => format!("FROM videos v LEFT JOIN playback_history h ON v.id = h.video_id AND h.username = ${idx}"),
                None => "FROM videos".into(),
            }
        };

        // Parameter index offset: 0 = no username, 1 = username is $1
        let u = if username.is_some() { 1u32 } else { 0u32 };

        // Build SQL
        let sql = match (has_query, has_type, u) {
            (false, false, 0) => {
                format!("SELECT * {} {} LIMIT $1 OFFSET $2",
                    from_clause(None), order_clause(None))
            }
            (false, false, 1) => {
                format!("SELECT v.* {} {} LIMIT $2 OFFSET $3",
                    from_clause(Some(1)), order_clause(Some(1)))
            }
            (true, false, 0) => {
                format!("SELECT * {} WHERE (title ILIKE $1 OR category ILIKE $1) {} LIMIT $2 OFFSET $3",
                    from_clause(None), order_clause(None))
            }
            (true, false, 1) => {
                format!("SELECT v.* {} WHERE (v.title ILIKE $2 OR v.category ILIKE $2) {} LIMIT $3 OFFSET $4",
                    from_clause(Some(1)), order_clause(Some(1)))
            }
            (false, true, 0) => {
                let t_op = if source_type.map_or(false, |t| t.starts_with('!')) { "!=" } else { "=" };
                format!("SELECT * {} WHERE source_type {} $1 {} LIMIT $2 OFFSET $3",
                    from_clause(None), t_op, order_clause(None))
            }
            (false, true, 1) => {
                let t_op = if source_type.map_or(false, |t| t.starts_with('!')) { "!=" } else { "=" };
                format!("SELECT v.* {} WHERE v.source_type {} $2 {} LIMIT $3 OFFSET $4",
                    from_clause(Some(1)), t_op, order_clause(Some(1)))
            }
            (true, true, 0) => {
                let t_op = if source_type.map_or(false, |t| t.starts_with('!')) { "!=" } else { "=" };
                format!("SELECT * {} WHERE (title ILIKE $1 OR category ILIKE $1) AND source_type {} $2 {} LIMIT $3 OFFSET $4",
                    from_clause(None), t_op, order_clause(None))
            }
            (true, true, 1) => {
                let t_op = if source_type.map_or(false, |t| t.starts_with('!')) { "!=" } else { "=" };
                format!("SELECT v.* {} WHERE (v.title ILIKE $2 OR v.category ILIKE $2) AND v.source_type {} $3 {} LIMIT $4 OFFSET $5",
                    from_clause(Some(1)), t_op, order_clause(Some(1)))
            }
            _ => unreachable!(),
        };

        let mut q = sqlx::query_as::<_, VideoRow>(&sql);

        // Bind: username first if present
        if let Some(u_name) = username {
            q = q.bind(u_name);
        }

        // Bind: pattern, type, limit, offset in the right positions
        match (has_query, has_type) {
            (false, false) => { q = q.bind(size).bind(offset); }
            (true, false) => {
                q = q.bind(pattern.as_deref().unwrap_or_default());
                q = q.bind(size).bind(offset);
            }
            (false, true) => {
                q = q.bind(type_val.as_deref().unwrap_or_default());
                q = q.bind(size).bind(offset);
            }
            (true, true) => {
                q = q.bind(pattern.as_deref().unwrap_or_default());
                q = q.bind(type_val.as_deref().unwrap_or_default());
                q = q.bind(size).bind(offset);
            }
        }

        q.fetch_all(&self.pool).await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<VideoRow>, sqlx::Error> {
        sqlx::query_as::<_, VideoRow>("SELECT * FROM videos WHERE id = $1")
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
        sqlx::query_as::<_, VideoRow>("SELECT * FROM videos WHERE file_hash = $1")
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

    pub async fn find_stream_urls_by_null_file_hash(&self) -> Result<Vec<(i64, String)>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i64,
            stream_url: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, stream_url FROM videos WHERE file_hash IS NULL"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.id, r.stream_url)).collect())
    }

    pub async fn update_file_hash(&self, id: i64, hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET file_hash = $1 WHERE id = $2")
            .bind(hash)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_stream_urls_by_null_file_size(&self) -> Result<Vec<(i64, String)>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i64,
            stream_url: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, stream_url FROM videos WHERE file_size IS NULL AND source_type LIKE 'local%'"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.id, r.stream_url)).collect())
    }

    pub async fn update_file_size(&self, id: i64, size: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET file_size = $1 WHERE id = $2")
            .bind(size)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

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
        // Build dynamic update
        let mut sets = Vec::new();
        let mut param_idx = 1;

        if title.is_some() { sets.push(format!("title = ${}", param_idx)); param_idx += 1; }
        if description.is_some() { sets.push(format!("description = ${}", param_idx)); param_idx += 1; }
        if category.is_some() { sets.push(format!("category = ${}", param_idx)); param_idx += 1; }

        if sets.is_empty() {
            return Ok(());
        }

        let sql = format!("UPDATE videos SET {} WHERE id = ${}", sets.join(", "), param_idx);
        let mut q = sqlx::query(&sql);
        if let Some(t) = title { q = q.bind(t); }
        if let Some(d) = description { q = q.bind(d); }
        if let Some(c) = category { q = q.bind(c); }
        q = q.bind(id);
        q.execute(&self.pool).await?;
        Ok(())
    }

    pub async fn delete_video(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM videos WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
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

    pub async fn delete_videos(&self, ids: &[i64]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query("DELETE FROM videos WHERE id = ANY($1)")
            .bind(ids)
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

    pub async fn find_existing_by_name_and_size(
        &self,
        name: &str,
        size: i64,
    ) -> Result<bool, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) as count FROM videos WHERE original_name = $1 AND file_size = $2 AND source_type LIKE 'local%'"
        )
        .bind(name)
        .bind(size)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
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

    pub async fn find_playback_history_by_username(&self, username: &str) -> Result<Vec<PlaybackHistoryResponse>, sqlx::Error> {
        let rows = sqlx::query_as::<_, PlaybackRow>(
            "SELECT video_id, position_ms, duration_ms FROM playback_history WHERE username = $1 ORDER BY updated_at DESC"
        )
        .bind(username)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| PlaybackHistoryResponse {
            video_id: r.video_id,
            position_ms: r.position_ms,
            duration_ms: r.duration_ms,
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
}
