use crate::db::log_slow_query;
use crate::models::video::VideoItem;
use moka::sync::Cache;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

/// stream_url → video_id 小缓存（media_auth 热路径）。
///
/// 旧格式媒体路径（`/media/{timestamp}_{rand}.mp4`）不含视频 ID，media_auth
/// 需要按 stream_url 反查 videos 表；浏览器 <video> 会发大量 Range 请求，
/// 每次命中此分支都会打一次 DB。缓存只存命中的 (stream_url, video_id)，
/// 30 秒 TTL —— 授权边界不受影响（media_auth 在拿到 video_id 后仍要求
/// 活跃播放会话或绑定该视频的 share token）。
const STREAM_URL_CACHE_TTL: Duration = Duration::from_secs(30);
const STREAM_URL_CACHE_MAX: u64 = 5_000;

/// Tuple type for batch inserting local videos:
/// (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name)
pub type LocalVideoValues<'a> = (
    &'a str,
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<i64>,
    Option<&'a str>,
);

/// Explicit columns for VideoRow — avoids SELECT * fetching unnecessary data
const VIDEO_COLUMNS: &str = "id, title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, created_at, views, duration, uploader_id";
const VIDEO_COLUMNS_PREFIXED: &str = "v.id, v.title, v.description, v.source_type, v.cover_url, v.thumb_url, v.stream_url, v.category, v.file_hash, v.file_size, v.original_name, v.created_at, v.views, v.duration, v.uploader_id";

/// Shared WHERE-clause builder for video list/count queries so the filters
/// can't drift between the two. Values are always bound (never interpolated),
/// so this stays injection-safe.
fn push_video_filters(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    query: Option<String>,
    source_type: Option<String>,
    category: Option<String>,
    uploader_id: Option<i64>,
) {
    if let Some(q) = query {
        builder.push(" AND v.search_vector @@ plainto_tsquery('chinese', ");
        builder.push_bind(q);
        builder.push(")");
    }
    if let Some(t) = source_type {
        if let Some(stripped) = t.strip_prefix('!') {
            builder.push(" AND v.source_type != ");
            builder.push_bind(stripped.to_owned());
        } else {
            builder.push(" AND v.source_type = ");
            builder.push_bind(t);
        }
    }
    if let Some(c) = category {
        builder.push(" AND v.category = ");
        builder.push_bind(c);
    }
    if let Some(uid) = uploader_id {
        builder.push(" AND v.uploader_id = ");
        builder.push_bind(uid);
    }
}

/// Whitelisted ORDER BY clause (leading space included, `ORDER BY` handled by
/// the caller). Unknown sort values fall back to the default ranking — the
/// caller-supplied string is never interpolated into the SQL.
fn push_video_sort_clause(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    sort: Option<&str>,
) {
    match sort {
        Some("views_asc") => builder.push(" v.views ASC, v.id ASC"),
        Some("id") | Some("id_desc") => builder.push(" v.id DESC"),
        Some("id_asc") => builder.push(" v.id ASC"),
        Some("duration") | Some("duration_desc") => builder.push(" v.duration DESC, v.id DESC"),
        Some("duration_asc") => builder.push(" v.duration ASC, v.id ASC"),
        Some("title") | Some("title_asc") => builder.push(" v.title ASC, v.id ASC"),
        Some("title_desc") => builder.push(" v.title DESC, v.id DESC"),
        _ => builder.push(" v.views DESC, v.id DESC"),
    };
}

#[derive(Debug, Clone, sqlx::FromRow)]
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
    pub uploader_id: Option<i64>,
    #[sqlx(default)]
    pub watch_position: Option<i64>,
    #[sqlx(default)]
    pub has_variants: bool,
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
            has_variants: r.has_variants,
            uploader_id: r.uploader_id,
            created_at: r.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct FileHashRow {
    pub file_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VideoRepository {
    pool: PgPool,
    /// stream_url → VideoRow 缓存（见 `find_by_stream_url` 注释）。
    /// 缓存随 `VideoRepository` 单例共享（AppState 只构造一次）。
    stream_url_cache: Arc<Cache<String, VideoRow>>,
}

impl VideoRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            stream_url_cache: Arc::new(
                Cache::builder()
                    .time_to_live(STREAM_URL_CACHE_TTL)
                    .max_capacity(STREAM_URL_CACHE_MAX)
                    .build(),
            ),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── Videos ──

    pub async fn count_all(
        &self,
        query: Option<&str>,
        source_type: Option<&str>,
        category: Option<&str>,
        uploader_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let mut builder =
            sqlx::QueryBuilder::new("SELECT COUNT(*) as count FROM videos v WHERE 1=1");
        push_video_filters(
            &mut builder,
            query.map(str::to_owned),
            source_type.map(str::to_owned),
            category.map(str::to_owned),
            uploader_id,
        );
        builder.build_query_scalar().fetch_one(&self.pool).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn find_all_paged(
        &self,
        page: i64,
        size: i64,
        query: Option<&str>,
        source_type: Option<&str>,
        category: Option<&str>,
        username: Option<&str>,
        uploader_id: Option<i64>,
        sort: Option<&str>,
    ) -> Result<Vec<VideoRow>, sqlx::Error> {
        // Defense in depth: handlers already clamp `page`, but saturating
        // multiplication guarantees `OFFSET` can never overflow/wrap.
        let offset = page.saturating_mul(size);
        let mut builder = sqlx::QueryBuilder::default();

        if let Some(uname) = username {
            builder.push(format!(
                "SELECT {}, h.position_ms AS watch_position FROM videos v",
                VIDEO_COLUMNS_PREFIXED
            ));
            builder.push(" LEFT JOIN playback_history h ON v.id = h.video_id AND h.username = ");
            builder.push_bind(uname);
        } else {
            builder.push(format!(
                "SELECT {}, NULL::bigint AS watch_position FROM videos v",
                VIDEO_COLUMNS_PREFIXED
            ));
        }
        builder.push(" WHERE 1=1");
        push_video_filters(
            &mut builder,
            query.map(str::to_owned),
            source_type.map(str::to_owned),
            category.map(str::to_owned),
            uploader_id,
        );

        match username {
            Some(_uname) => {
                // Watched videos first (most recently watched first), then the
                // caller's chosen sort for everything else.
                builder.push(
                    " ORDER BY CASE WHEN h.id IS NOT NULL THEN 1 ELSE 0 END, h.updated_at DESC NULLS LAST",
                );
                push_video_sort_clause(&mut builder, sort);
            }
            None => {
                builder.push(" ORDER BY");
                push_video_sort_clause(&mut builder, sort);
            }
        }

        builder.push(" LIMIT ");
        builder.push_bind(size);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        builder
            .build_query_as::<VideoRow>()
            .fetch_all(&self.pool)
            .await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<VideoRow>, sqlx::Error> {
        log_slow_query("video_repo::find_by_id", || async {
            sqlx::query_as::<_, VideoRow>(&format!(
                "SELECT {} FROM videos WHERE id = $1",
                VIDEO_COLUMNS
            ))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
        })
        .await
    }

    pub async fn find_all_by_ids(&self, ids: &[i64]) -> Result<Vec<VideoRow>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, VideoRow>(&format!(
            "SELECT {} FROM videos WHERE id = ANY($1)",
            VIDEO_COLUMNS
        ))
        .bind(ids)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn save_external_video(
        &self,
        title: &str,
        description: &str,
        category: &str,
        cover_url: Option<&str>,
        stream_url: &str,
        uploader_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO videos (title, description, source_type, cover_url, stream_url, category, uploader_id) \
             VALUES ($1, $2, 'external', $3, $4, $5, $6) RETURNING id"
        )
        .bind(title)
        .bind(description)
        .bind(cover_url)
        .bind(stream_url)
        .bind(category)
        .bind(uploader_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Atomically increment a user's storage counter and return the new value.
    /// SECURITY (A04 H2): used to enforce per-user upload quotas.
    pub async fn increment_storage_used(
        &self,
        user_id: i64,
        bytes: i64,
    ) -> Result<i64, sqlx::Error> {
        let (new_value,): (i64,) = sqlx::query_as(
            "UPDATE users SET storage_used_bytes = COALESCE(storage_used_bytes, 0) + $1 \
             WHERE id = $2 RETURNING storage_used_bytes",
        )
        .bind(bytes)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(new_value)
    }

    /// Decrement a user's storage counter (capped at 0).
    pub async fn decrement_storage_used(
        &self,
        user_id: i64,
        bytes: i64,
    ) -> Result<i64, sqlx::Error> {
        let (new_value,): (i64,) = sqlx::query_as(
            "UPDATE users SET storage_used_bytes = GREATEST(0, COALESCE(storage_used_bytes, 0) - $1) \
             WHERE id = $2 RETURNING storage_used_bytes",
        )
        .bind(bytes)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(new_value)
    }

    /// Read the current storage counter for a user.
    pub async fn get_storage_used(&self, user_id: i64) -> Result<i64, sqlx::Error> {
        let (val,): (i64,) =
            sqlx::query_as("SELECT COALESCE(storage_used_bytes, 0) FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(val)
    }

    pub async fn find_video_by_file_hash(
        &self,
        hash: &str,
    ) -> Result<Option<VideoRow>, sqlx::Error> {
        sqlx::query_as::<_, VideoRow>(&format!(
            "SELECT {} FROM videos WHERE file_hash = $1",
            VIDEO_COLUMNS
        ))
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
    }

    /// Find a video by its stream_url path (e.g. "/media/1783103442300_1_5019736047878146210.mp4")
    /// SECURITY: Used by media_auth to resolve video_id from legacy path format
    ///
    /// 结果按 stream_url 缓存 30 秒（media_auth 的 Range 请求风暴下避免
    /// 重复 DB 查询）。只缓存命中项：未注册路径每次仍查库（此类路径本就
    /// 稀少，且缓存 None 会让"上传后立即可播"出现 30 秒假阴性）。
    pub async fn find_by_stream_url(
        &self,
        stream_url: &str,
    ) -> Result<Option<VideoRow>, sqlx::Error> {
        if let Some(row) = self.stream_url_cache.get(stream_url) {
            return Ok(Some(row));
        }
        let row = sqlx::query_as::<_, VideoRow>(&format!(
            "SELECT {} FROM videos WHERE stream_url = $1",
            VIDEO_COLUMNS
        ))
        .bind(stream_url)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(v) = &row {
            self.stream_url_cache
                .insert(stream_url.to_string(), v.clone());
        }
        Ok(row)
    }

    pub async fn find_existing_hashes(
        &self,
        hashes: &[String],
    ) -> Result<Vec<String>, sqlx::Error> {
        if hashes.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query_as::<_, FileHashRow>(
            "SELECT file_hash FROM videos WHERE file_hash = ANY($1)",
        )
        .bind(hashes)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().filter_map(|r| r.file_hash).collect())
    }

    /// Find videos without a cover, paginated by id for cursor-based iteration
    pub async fn find_videos_without_cover(
        &self,
        after_id: i64,
        limit: i64,
    ) -> Result<Vec<VideoRow>, sqlx::Error> {
        sqlx::query_as::<_, VideoRow>(&format!(
            "SELECT {} FROM videos WHERE (cover_url IS NULL OR thumb_url IS NULL) AND source_type LIKE 'local%' AND id > $1 ORDER BY id LIMIT $2",
            VIDEO_COLUMNS
        ))
        .bind(after_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Batch check which (name, size) pairs already exist — single query instead of N+1
    pub async fn find_existing_by_name_and_size_batch(
        &self,
        files: &[(String, i64)],
    ) -> Result<HashSet<(String, i64)>, sqlx::Error> {
        if files.is_empty() {
            return Ok(HashSet::new());
        }
        let mut builder = sqlx::QueryBuilder::new(
            "SELECT original_name, file_size FROM videos WHERE (original_name, file_size) IN (",
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
        Ok(rows
            .into_iter()
            .filter_map(|r| Some((r.original_name?, r.file_size?)))
            .collect())
    }

    /// Batch delete videos and their related data in a single transaction.
    /// Also decrements the storage quota for each affected uploader.
    pub async fn batch_delete_videos(&self, ids: &[i64]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM playback_history WHERE video_id = ANY($1)")
            .bind(ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM user_likes WHERE video_id = ANY($1)")
            .bind(ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM user_favorites WHERE video_id = ANY($1)")
            .bind(ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM comments WHERE video_id = ANY($1)")
            .bind(ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM video_tags WHERE video_id = ANY($1)")
            .bind(ids)
            .execute(&mut *tx)
            .await?;
        let result = sqlx::query("DELETE FROM videos WHERE id = ANY($1)")
            .bind(ids)
            .execute(&mut *tx)
            .await?;

        // Decrement storage quota for all affected uploaders in one statement
        // instead of one UPDATE per uploader inside the transaction.
        sqlx::query(
            "UPDATE users SET storage_used_bytes = GREATEST(0, COALESCE(storage_used_bytes, 0) - sub.total_bytes) \
             FROM (SELECT uploader_id, SUM(file_size) AS total_bytes \
                   FROM videos WHERE id = ANY($1) AND uploader_id IS NOT NULL \
                   GROUP BY uploader_id) AS sub \
             WHERE users.id = sub.uploader_id",
        )
        .bind(ids)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_video_cascade(&self, id: i64) -> Result<bool, sqlx::Error> {
        let rows = self.batch_delete_videos(&[id]).await?;
        Ok(rows > 0)
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
        uploader_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO videos (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, uploader_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id"
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
        .bind(uploader_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    /// Batch insert local videos in a single query for better performance during scan
    pub async fn batch_save_local_videos(
        &self,
        videos: &[LocalVideoValues<'_>],
    ) -> Result<u64, sqlx::Error> {
        if videos.is_empty() {
            return Ok(0);
        }
        let mut builder = sqlx::QueryBuilder::new(
            "INSERT INTO videos (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name) ",
        );
        builder.push_values(videos, |mut b, v| {
            b.push_bind(v.0)
                .push_bind(v.1)
                .push_bind(v.2)
                .push_bind(v.3)
                .push_bind(v.4)
                .push_bind(v.5)
                .push_bind(v.6)
                .push_bind(v.7)
                .push_bind(v.8)
                .push_bind(v.9);
        });
        let result = builder.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn update_video(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        category: Option<&str>,
    ) -> Result<u64, sqlx::Error> {
        if title.is_none() && description.is_none() && category.is_none() {
            return Ok(0);
        }

        let mut builder = sqlx::QueryBuilder::new("UPDATE videos SET ");
        let mut sep = builder.separated(", ");

        if let Some(t) = title {
            sep.push("title = ");
            sep.push_bind_unseparated(t);
        }
        if let Some(d) = description {
            sep.push("description = ");
            sep.push_bind_unseparated(d);
        }
        if let Some(c) = category {
            sep.push("category = ");
            sep.push_bind_unseparated(c);
        }

        builder.push(" WHERE id = ");
        builder.push_bind(id);
        let result = builder.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
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

    pub async fn count_by_type(&self) -> Result<Vec<(String, i64)>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            source_type: String,
            count: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT source_type, COUNT(*)::bigint as count FROM videos GROUP BY source_type ORDER BY count DESC"
        )
        .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| (r.source_type, r.count)).collect())
    }

    pub async fn count_by_category(&self) -> Result<Vec<(String, i64)>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            category: String,
            count: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT cat as category, COUNT(*)::bigint as count FROM (SELECT COALESCE(NULLIF(category,''), '未分类') as cat FROM videos) t GROUP BY cat ORDER BY count DESC"
        )
        .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| (r.category, r.count)).collect())
    }

    pub async fn total_views(&self) -> Result<i64, sqlx::Error> {
        let (total,): (i64,) = sqlx::query_as("SELECT COALESCE(SUM(views), 0)::bigint FROM videos")
            .fetch_one(&self.pool)
            .await?;
        Ok(total)
    }

    pub async fn total_duration_secs(&self) -> Result<i64, sqlx::Error> {
        let (total,): (i64,) =
            sqlx::query_as("SELECT COALESCE(SUM(duration), 0)::bigint FROM videos")
                .fetch_one(&self.pool)
                .await?;
        Ok(total)
    }

    pub async fn batch_update_category(
        &self,
        ids: &[i64],
        category: &str,
    ) -> Result<i64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query("UPDATE videos SET category = $1 WHERE id = ANY($2)")
            .bind(category)
            .bind(ids)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() as i64)
    }

    pub async fn find_all_local_file_names(&self) -> Result<HashSet<String>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            stream_url: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT stream_url FROM videos WHERE source_type LIKE 'local%'",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.stream_url).collect())
    }

    // ── Variant / transcode helpers ──

    pub async fn delete_variant_record(
        &self,
        video_id: i64,
        resolution: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM video_variants WHERE video_id = $1 AND resolution = $2")
            .bind(video_id)
            .bind(resolution)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count_variants(&self, video_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM video_variants WHERE video_id = $1")
                .bind(video_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    pub async fn clear_has_variants(&self, video_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET has_variants = false WHERE id = $1")
            .bind(video_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_variants(&self, video_id: i64) -> Result<Vec<VideoVariantRow>, sqlx::Error> {
        sqlx::query_as::<_, VideoVariantRow>(
            r#"SELECT resolution, file_path, file_size, bitrate, codec
               FROM video_variants
               WHERE video_id = $1
               ORDER BY CASE resolution
                   WHEN '2160p' THEN 1 WHEN '1080p' THEN 2 WHEN '720p' THEN 3
                   WHEN '480p' THEN 4 WHEN '360p' THEN 5 ELSE 6 END"#,
        )
        .bind(video_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn cancel_transcode_jobs(&self, video_id: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE transcoding_jobs SET status = 'failed', error_message = 'Cancelled by admin' \
             WHERE video_id = $1 AND status IN ('pending', 'processing')",
        )
        .bind(video_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct VideoVariantRow {
    pub resolution: String,
    pub file_path: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
    pub codec: Option<String>,
}
