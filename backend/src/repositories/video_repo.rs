use crate::db::log_slow_query;
use crate::models::video::{ImageExif, VideoItem};
use crate::services::exif_service::ParsedExif;
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

/// EXIF 元数据的入库形态（剥离 `ParsedExif` 与 DB 列类型的耦合）。
///
/// `extracted` 表示“已尝试解析”：即使一个字段都没解析出来也应写 `true`，
/// 否则 `find_images_without_exif` 后台任务会对无 EXIF 的文件反复重试。
#[derive(Debug, Clone, Default)]
pub struct LocalVideoExif {
    pub taken_at: Option<chrono::DateTime<chrono::Utc>>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub aperture: Option<f64>,
    pub shutter: Option<String>,
    pub iso: Option<i32>,
    pub focal_length: Option<f64>,
    pub orientation: Option<i32>,
    pub extracted: bool,
}

impl LocalVideoExif {
    /// 把解析结果转换为入库值。
    ///
    /// 数值/文本字段统一经 `Display` 中转，因此 `ParsedExif` 各字段用
    /// `String`、`f64`、`i32` 等具体类型均可（shutter 以 `"1/250"` 这类
    /// 展示文本存储，光圈/焦距按数值列存储）。
    pub fn from_parsed(exif: &ParsedExif, extracted: bool) -> Self {
        Self {
            taken_at: exif.taken_at,
            lat: exif_to_f64(&exif.lat),
            lon: exif_to_f64(&exif.lon),
            camera: exif_to_text(&exif.camera),
            lens: exif_to_text(&exif.lens),
            aperture: exif_to_f64(&exif.aperture),
            shutter: exif_to_text(&exif.shutter),
            iso: exif_to_i32(&exif.iso),
            focal_length: exif_to_f64(&exif.focal_length),
            orientation: exif_to_i32(&exif.orientation),
            extracted,
        }
    }
}

fn exif_to_text<T: std::fmt::Display>(value: &Option<T>) -> Option<String> {
    value.as_ref().map(ToString::to_string)
}

fn exif_to_f64<T: std::fmt::Display>(value: &Option<T>) -> Option<f64> {
    value.as_ref().and_then(|v| v.to_string().parse().ok())
}

fn exif_to_i32<T: std::fmt::Display>(value: &Option<T>) -> Option<i32> {
    value.as_ref().and_then(|v| v.to_string().parse().ok())
}

/// Tuple type for batch inserting local videos:
/// (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, exif)
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
    LocalVideoExif,
);

/// Explicit columns for VideoRow — avoids SELECT * fetching unnecessary data
const VIDEO_COLUMNS: &str = "id, title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, created_at, views, duration, uploader_id, exif_taken_at, exif_lat, exif_lon, exif_camera, exif_lens, exif_aperture, exif_shutter, exif_iso, exif_focal_length, exif_orientation, exif_extracted";
const VIDEO_COLUMNS_PREFIXED: &str = "v.id, v.title, v.description, v.source_type, v.cover_url, v.thumb_url, v.stream_url, v.category, v.file_hash, v.file_size, v.original_name, v.created_at, v.views, v.duration, v.uploader_id, v.exif_taken_at, v.exif_lat, v.exif_lon, v.exif_camera, v.exif_lens, v.exif_aperture, v.exif_shutter, v.exif_iso, v.exif_focal_length, v.exif_orientation, v.exif_extracted";

fn push_video_filters(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    query: Option<&str>,
    source_type: Option<&str>,
    category: Option<&str>,
    uploader_id: Option<i64>,
    taken_after: Option<chrono::DateTime<chrono::Utc>>,
    taken_before: Option<chrono::DateTime<chrono::Utc>>,
) {
    if let Some(q) = query {
        // 'simple' 是标准 PostgreSQL 自带配置(zhparser 不一定安装);
        // 与 search_service 的 tsquery 配置保持一致,避免未装插件时 500。
        builder.push(" AND v.search_vector @@ plainto_tsquery('simple', ");
        builder.push_bind(q.to_owned());
        builder.push(")");
    }
    if let Some(t) = source_type {
        if let Some(stripped) = t.strip_prefix('!') {
            builder.push(" AND v.source_type != ");
            builder.push_bind(stripped.to_owned());
        } else {
            builder.push(" AND v.source_type = ");
            builder.push_bind(t.to_owned());
        }
    }
    if let Some(c) = category {
        builder.push(" AND v.category = ");
        builder.push_bind(c.to_owned());
    }
    if let Some(uid) = uploader_id {
        builder.push(" AND v.uploader_id = ");
        builder.push_bind(uid);
    }
    // EXIF 拍摄时间范围（半开区间 [after, before)，仅筛选有拍摄时间的图片）
    if let Some(after) = taken_after {
        builder.push(" AND v.exif_taken_at >= ");
        builder.push_bind(after);
    }
    if let Some(before) = taken_before {
        builder.push(" AND v.exif_taken_at < ");
        builder.push_bind(before);
    }
}

/// Whitelisted ORDER BY clause (leading space included, `ORDER BY` handled by
/// the caller). Unknown sort values fall back to the default ranking — the
/// caller-supplied string is never interpolated into the SQL.
///
/// Supported: `views_asc`, `id`/`id_desc`, `id_asc`, `duration`/`duration_desc`,
/// `duration_asc`, `title`/`title_asc`, `title_desc`, `taken`/`taken_desc`
/// (EXIF 拍摄时间，NULLS LAST), `taken_asc`.
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
        Some("taken") | Some("taken_desc") => {
            builder.push(" v.exif_taken_at DESC NULLS LAST, v.id DESC")
        }
        Some("taken_asc") => builder.push(" v.exif_taken_at ASC NULLS LAST, v.id ASC"),
        _ => builder.push(" v.views DESC, v.id DESC"),
    };
}

/// 数据库视频行的直接映射（对应 `videos` 表 + 可选的 `watch_position`/`has_variants`）。
///
/// `watch_position` 和 `has_variants` 使用 `#[sqlx(default)]`，
/// 当查询中不包含这些列时（如无用户上下文或旧查询）自动填 `None`/`false`。
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
    #[sqlx(default)]
    pub exif_taken_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    pub exif_lat: Option<f64>,
    #[sqlx(default)]
    pub exif_lon: Option<f64>,
    #[sqlx(default)]
    pub exif_camera: Option<String>,
    #[sqlx(default)]
    pub exif_lens: Option<String>,
    #[sqlx(default)]
    pub exif_aperture: Option<f64>,
    #[sqlx(default)]
    pub exif_shutter: Option<String>,
    #[sqlx(default)]
    pub exif_iso: Option<i32>,
    #[sqlx(default)]
    pub exif_focal_length: Option<f64>,
    #[sqlx(default)]
    pub exif_orientation: Option<i32>,
    /// 是否已尝试过 EXIF 解析（后台补扫任务据此跳过，不进入 API 响应）。
    #[allow(dead_code)]
    #[sqlx(default)]
    pub exif_extracted: bool,
}

impl From<VideoRow> for VideoItem {
    fn from(r: VideoRow) -> Self {
        // 任一 EXIF 字段非空才返回 exif 对象；解析过但无 EXIF 的记录保持 None，
        // 前端据此隐藏"拍摄信息"面板。
        let exif = if r.exif_taken_at.is_some()
            || r.exif_lat.is_some()
            || r.exif_lon.is_some()
            || r.exif_camera.is_some()
            || r.exif_lens.is_some()
            || r.exif_aperture.is_some()
            || r.exif_shutter.is_some()
            || r.exif_iso.is_some()
            || r.exif_focal_length.is_some()
            || r.exif_orientation.is_some()
        {
            Some(ImageExif {
                taken_at: r.exif_taken_at,
                lat: r.exif_lat,
                lon: r.exif_lon,
                camera: r.exif_camera,
                lens: r.exif_lens,
                aperture: r.exif_aperture,
                shutter: r.exif_shutter,
                iso: r.exif_iso,
                focal_length: r.exif_focal_length,
                orientation: r.exif_orientation,
            })
        } else {
            None
        };

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
            exif,
        }
    }
}

/// 仅包含 `file_hash` 的轻量行结构，用于批量哈希存在性检查。
#[derive(Debug, sqlx::FromRow)]
pub struct FileHashRow {
    pub file_hash: Option<String>,
}

/// 上传落库 + 配额扣减的原子操作结果。
#[derive(Debug)]
pub enum SaveVideoOutcome {
    /// 插入成功，且（当上传者存在且文件大小已知时）已在同一事务内扣减配额。
    /// `VideoRow` 体积较大，装箱以缩小枚举体积（clippy::large_enum_variant）。
    Ok(Box<VideoRow>),
    /// 超出上传者存储配额，未写入任何行。
    QuotaExceeded,
    /// 撞上 `uq_videos_uploader_file_hash` 唯一索引（并发重复上传），未写入任何行。
    Duplicate,
    /// 上传者用户行不存在（幽灵上传者），未写入任何行。
    UserNotFound,
}

/// 判断 sqlx 错误是否为 PostgreSQL 唯一约束冲突（SQLSTATE 23505）。
fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|db| db.code())
        .map(|code| code.as_ref() == "23505")
        .unwrap_or(false)
}

/// 视频数据访问层，封装所有 `videos` / `video_variants` / `transcoding_jobs` 相关的
/// SQL 操作。通过 `AppState` 以 `Arc` 形式全局共享，内部维护 `stream_url → VideoRow`
/// 的 Moka 缓存以加速 media_auth 热路径。
#[derive(Debug, Clone)]
pub struct VideoRepository {
    pool: PgPool,
    /// stream_url → VideoRow 缓存（见 `find_by_stream_url` 注释）。
    /// 缓存随 `VideoRepository` 单例共享（AppState 只构造一次）。
    stream_url_cache: Arc<Cache<String, VideoRow>>,
}

impl VideoRepository {
    /// 创建 `VideoRepository` 实例。
    ///
    /// 内部同时初始化 `stream_url → VideoRow` 的 Moka 缓存（30 秒 TTL，
    /// 最大 5000 条），用于加速 media_auth 热路径中的 stream_url 反查。
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

    /// 获取底层数据库连接池引用。
    ///
    /// 供需要直接访问 `PgPool` 的上层调用者使用（如迁移管理、统计查询等）。
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── Videos ──

    /// 按可选筛选条件统计视频总数。
    ///
    /// # SQL
    /// ```sql
    /// SELECT COUNT(*) as count FROM videos v WHERE 1=1
    ///   [AND v.search_vector @@ plainto_tsquery('simple', $query)]
    ///   [AND v.source_type = $source_type]   -- 前缀 `!` 表示排除
    ///   [AND v.category = $category]
    ///   [AND v.uploader_id = $uploader_id]
    /// ```
    ///
    /// # 用途
    /// 与 [`find_all_paged`] 配合，计算分页的总页数。
    ///
    /// # 性能
    /// - 全文搜索使用 `search_vector` 列的 GIN 索引（迁移 039）
    /// - 所有可选筛选均使用绑定参数，不会注入
    pub async fn count_all(
        &self,
        query: Option<&str>,
        source_type: Option<&str>,
        category: Option<&str>,
        uploader_id: Option<i64>,
        taken_after: Option<chrono::DateTime<chrono::Utc>>,
        taken_before: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<i64, sqlx::Error> {
        let mut builder =
            sqlx::QueryBuilder::new("SELECT COUNT(*) as count FROM videos v WHERE 1=1");
        push_video_filters(
            &mut builder,
            query,
            source_type,
            category,
            uploader_id,
            taken_after,
            taken_before,
        );
        builder.build_query_scalar().fetch_one(&self.pool).await
    }

    /// 分页查询视频列表，支持全文搜索、筛选、排序与播放进度关联。
    ///
    /// # SQL
    /// ```sql
    /// -- 带用户时，LEFT JOIN playback_history 获取上次播放位置：
    /// SELECT {VIDEO_COLUMNS_PREFIXED}, h.position_ms AS watch_position
    /// FROM videos v
    /// LEFT JOIN playback_history h ON v.id = h.video_id AND h.username = $username
    /// WHERE 1=1 [AND ... filters ...]
    /// ORDER BY CASE WHEN h.id IS NOT NULL THEN 1 ELSE 0 END,
    ///          h.updated_at DESC NULLS LAST,
    ///          {sort_clause}
    /// LIMIT $size OFFSET $offset
    ///
    /// -- 无用户时：
    /// SELECT {VIDEO_COLUMNS_PREFIXED}, NULL::bigint AS watch_position
    /// FROM videos v
    /// WHERE 1=1 [AND ... filters ...]
    /// ORDER BY {sort_clause}
    /// LIMIT $size OFFSET $offset
    /// ```
    ///
    /// # 排序选项 (`sort`)
    /// `views_asc`, `id`/`id_desc`, `id_asc`, `duration`/`duration_desc`,
    /// `duration_asc`, `title`/`title_asc`, `title_desc`,
    /// `taken`/`taken_desc`/`taken_asc`（按 EXIF 拍摄时间，NULLS LAST 置底）；
    /// 默认 `v.views DESC, v.id DESC`。
    ///
    /// # 性能
    /// - `OFFSET` 使用 `saturating_mul` 防止溢出
    /// - `sort` 从白名单匹配，不直接拼接 SQL
    /// - 有用户时，已观看视频优先展示（`h.updated_at DESC NULLS LAST`）
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
        taken_after: Option<chrono::DateTime<chrono::Utc>>,
        taken_before: Option<chrono::DateTime<chrono::Utc>>,
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
            query,
            source_type,
            category,
            uploader_id,
            taken_after,
            taken_before,
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

    /// 根据视频 ID 获取单条视频记录。
    ///
    /// # SQL
    /// ```sql
    /// SELECT {VIDEO_COLUMNS} FROM videos WHERE id = $1
    /// ```
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

    /// 根据 ID 列表批量获取视频记录。
    ///
    /// # SQL
    /// ```sql
    /// SELECT {VIDEO_COLUMNS} FROM videos WHERE id = ANY($1)
    /// ```
    ///
    /// # 性能
    /// - `ANY($1)` 利用主键索引，单次查询替代 N+1
    /// - 空列表时提前返回空 Vec，不发 DB 请求
    pub async fn find_all_by_ids(&self, ids: &[i64]) -> Result<Vec<VideoRow>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, VideoRow>(&format!(
            "SELECT {} FROM videos WHERE id = ANY($1) ORDER BY array_position($1, id)",
            VIDEO_COLUMNS
        ))
        .bind(ids)
        .fetch_all(&self.pool)
        .await
    }

    /// 保存外部链接视频（`source_type = 'external'`）。
    ///
    /// # SQL
    /// ```sql
    /// INSERT INTO videos (title, description, source_type, cover_url, stream_url, category, uploader_id)
    /// VALUES ($1, $2, 'external', $3, $4, $5, $6)
    /// RETURNING id
    /// ```
    ///
    /// # 返回
    /// 新插入视频的自增 `id`。
    #[allow(clippy::too_many_arguments)]
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

    /// 原子递增用户的已用存储量，返回更新后的值。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE users
    /// SET storage_used_bytes = COALESCE(storage_used_bytes, 0) + $1
    /// WHERE id = $2
    /// RETURNING storage_used_bytes
    /// ```
    ///
    /// # 用途
    /// 文件上传成功后调用，用于强制执行每用户存储配额（`UPLOAD_QUOTA_BYTES`）。
    ///
    /// # 安全
    /// 原子 `UPDATE` + `RETURNING` 保证并发安全（A04 H2 存储配额强制）。
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

    /// 递减用户的已用存储量（下限为 0），返回更新后的值。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE users
    /// SET storage_used_bytes = GREATEST(0, COALESCE(storage_used_bytes, 0) - $1)
    /// WHERE id = $2
    /// RETURNING storage_used_bytes
    /// ```
    ///
    /// # 用途
    /// 视频删除后调用，回收配额。`GREATEST(0, ...)` 防止计数器变为负数。
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

    /// 读取用户的当前已用存储量。
    ///
    /// # SQL
    /// ```sql
    /// SELECT COALESCE(storage_used_bytes, 0) FROM users WHERE id = $1
    /// ```
    ///
    /// # 用途
    /// 上传前检查是否超出配额。`COALESCE` 处理 `NULL` 情况（旧账户未初始化时）。
    pub async fn get_storage_used(&self, user_id: i64) -> Result<i64, sqlx::Error> {
        let (val,): (i64,) =
            sqlx::query_as("SELECT COALESCE(storage_used_bytes, 0) FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(val)
    }

    /// 根据文件哈希查找视频（用于上传去重）。
    ///
    /// # SQL
    /// ```sql
    /// SELECT {VIDEO_COLUMNS} FROM videos WHERE file_hash = $1
    /// ```
    ///
    /// # 用途
    /// 上传时检查同一文件是否已存在，避免重复存储。
    ///
    /// # 性能
    /// - `file_hash` 列有索引，单行查找效率高
    /// - 返回 `None` 表示该文件尚未上传
    pub async fn find_video_by_file_hash(
        &self,
        uploader_id: i64,
        hash: &str,
    ) -> Result<Option<VideoRow>, sqlx::Error> {
        // 私有模型（访客模式）：重复判定只看同一上传者，避免共享同一行
        // 让阅后即焚/删除误删他人的内容。
        sqlx::query_as::<_, VideoRow>(&format!(
            "SELECT {} FROM videos WHERE file_hash = $1 AND uploader_id = $2",
            VIDEO_COLUMNS
        ))
        .bind(hash)
        .bind(uploader_id)
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
        let cache_key = stream_url.to_string();
        if let Some(row) = self.stream_url_cache.get(&cache_key) {
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
            self.stream_url_cache.insert(cache_key, v.clone());
        }
        Ok(row)
    }

    /// 批量检查哪些文件哈希已存在于数据库中。
    ///
    /// # SQL
    /// ```sql
    /// SELECT file_hash FROM videos WHERE file_hash = ANY($1)
    /// ```
    ///
    /// # 用途
    /// 批量上传扫描时，一次查询筛出所有已存在文件，避免逐个 N+1 查询。
    ///
    /// # 性能
    /// - 空列表提前返回，不发 DB 请求
    /// - `ANY($1)` 利用 `file_hash` 索引
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

    /// 查找缺少封面图的本地视频（游标分页，供后台封面生成任务使用）。
    ///
    /// 封面生成属于全局后台任务，需遍历所有视频补齐封面。此方法仅供内部使用，
    /// 不对外暴露。
    ///
    /// # SQL
    /// ```sql
    /// SELECT {VIDEO_COLUMNS} FROM videos
    /// WHERE (cover_url IS NULL OR thumb_url IS NULL)
    ///   AND source_type LIKE 'local%'
    ///   AND id > $1
    /// ORDER BY id
    /// LIMIT $2
    /// ```
    ///
    /// # 性能
    /// - 使用 `id > $1` 游标分页（优于 `OFFSET`，适合持续迭代）
    /// - 条件索引 `idx_videos_no_cover` 加速过滤
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

    /// 查找尚未尝试解析 EXIF 的本地图片（供后台补扫任务使用）。
    ///
    /// # SQL
    /// ```sql
    /// SELECT id, stream_url FROM videos
    /// WHERE source_type = 'local_image'
    ///   AND exif_extracted = FALSE
    ///   AND deleted_at IS NULL
    /// ORDER BY id
    /// LIMIT $1
    /// ```
    ///
    /// # 返回
    /// `(id, stream_url)` 列表；调用方解析成功与否都应调用
    /// [`update_video_exif`](Self::update_video_exif) 把 `exif_extracted`
    /// 置 TRUE，避免无 EXIF 的文件被反复扫描。
    pub async fn find_images_without_exif(
        &self,
        limit: i64,
    ) -> Result<Vec<(i64, String)>, sqlx::Error> {
        sqlx::query_as::<_, (i64, String)>(
            "SELECT id, stream_url FROM videos WHERE source_type = 'local_image' AND exif_extracted = FALSE AND deleted_at IS NULL ORDER BY id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// 全量列出本地视频的 `(id, stream_url)`,供后台
    /// “视频记录 ↔ 物理文件”一致性清扫使用(文件丢失的记录会被删除)。
    pub async fn list_local_video_media(&self) -> Result<Vec<(i64, String)>, sqlx::Error> {
        log_slow_query("video_repo::list_local_video_media", || async {
            sqlx::query_as::<_, (i64, String)>(
                "SELECT id, stream_url FROM videos WHERE source_type = 'local_video'",
            )
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// 批量检查 (文件名, 文件大小) 组合是否已存在（用于上传去重）。
    ///
    /// # SQL
    /// ```sql
    /// SELECT original_name, file_size FROM videos
    /// WHERE (original_name, file_size) IN (
    ///   ($name1, $size1), ($name2, $size2), ...
    /// )
    /// ```
    ///
    /// # 用途
    /// 大批量上传时，单次查询筛出所有可能重复的文件（基于文件名 + 大小
    /// 的快速初筛，完整去重还需后续比较 `file_hash`）。
    ///
    /// # 性能
    /// - 单条查询替代 N+1，利用 `(original_name, file_size)` 复合索引
    /// - 空列表提前返回
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

    /// 在单个事务中批量删除视频及其所有关联数据。
    ///
    /// # SQL（按执行顺序）
    /// ```sql
    /// BEGIN;
    ///   DELETE FROM playback_history WHERE video_id = ANY($1);
    ///   DELETE FROM user_likes       WHERE video_id = ANY($1);
    ///   DELETE FROM user_favorites   WHERE video_id = ANY($1);
    ///   DELETE FROM comments         WHERE video_id = ANY($1);
    ///   DELETE FROM video_tags       WHERE video_id = ANY($1);
    ///   DELETE FROM videos           WHERE id = ANY($1);
    ///   UPDATE users SET storage_used_bytes = GREATEST(0, ...) -- 按 uploader_id 聚合回收;
    /// COMMIT;
    /// ```
    ///
    /// # 用途
    /// 管理员批量删除视频时使用，保证所有关联表的一致性。
    ///
    /// # 安全
    /// - 事务保证原子性：要么全部删除，要么全部回滚
    /// - 级联删除顺序：先删依赖表（history/likes/favorites/comments/tags），再删主表
    /// - 最后统一回收各 uploader 的存储配额（单条 UPDATE，非逐个更新）
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
        // Decrement storage quota for all affected uploaders BEFORE deleting
        // the videos, since the subquery reads FROM videos.
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

        let result = sqlx::query("DELETE FROM videos WHERE id = ANY($1)")
            .bind(ids)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }

    /// 删除单个视频及其所有关联数据（级联删除）。
    ///
    /// # SQL
    /// 内部委托给 [`batch_delete_videos`]，等价于传入 `&[id]`。
    ///
    /// # 返回
    /// - `true` — 成功删除（至少影响 1 行）
    /// - `false` — ID 不存在（0 行受影响）
    pub async fn delete_video_cascade(&self, id: i64) -> Result<bool, sqlx::Error> {
        let rows = self.batch_delete_videos(&[id]).await?;
        Ok(rows > 0)
    }

    /// 保存一条本地视频记录（包含文件元数据）。
    ///
    /// # SQL
    /// ```sql
    /// INSERT INTO videos (
    ///   title, description, source_type, cover_url, thumb_url,
    ///   stream_url, category, file_hash, file_size, original_name, uploader_id
    /// ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
    /// RETURNING id
    /// ```
    ///
    /// # 用途
    /// 单个文件上传完成后调用，记录视频元数据和文件信息。
    ///
    /// # 返回
    /// 新插入视频的自增 `id`。
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
            "INSERT INTO videos (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, uploader_id, exif_extracted) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, FALSE) RETURNING id"
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

    /// 保存本地视频并**原子地**校验/扣减上传者存储配额。
    ///
    /// 与 [`save_local_video`] + [`increment_storage_used`] 的组合不同，这里把
    /// 「读配额 → 校验 → INSERT → 记账」放进同一个事务：
    /// 1. `SELECT storage_used_bytes FROM users WHERE id = $1 FOR UPDATE`
    ///    —— 行锁把同一用户的并发上传串行化，消除 TOCTOU 超卖；
    /// 2. `quota_bytes <= 0` 表示不限量；否则要求
    ///    `used + file_size <= quota_bytes`（`file_size` 为空按 0 计）；
    /// 3. `INSERT ... RETURNING {VIDEO_COLUMNS}`；
    /// 4. `UPDATE users SET storage_used_bytes = COALESCE(storage_used_bytes, 0) + $file_size`
    ///    —— 与 INSERT 同事务，**失败即整体回滚**，不会出现"视频已入库但未计费"。
    ///
    /// 唯一索引 `uq_videos_uploader_file_hash`（迁移 058）的 23505 冲突映射为
    /// [`SaveVideoOutcome::Duplicate`]；上传者不存在映射为
    /// [`SaveVideoOutcome::UserNotFound`]，均不会留下半成品行。
    #[allow(clippy::too_many_arguments)]
    pub async fn save_local_video_with_quota(
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
        quota_bytes: i64,
    ) -> Result<SaveVideoOutcome, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        if let Some(uid) = uploader_id {
            // 行锁：同一 uploader 的并发上传在这里排队，配额判断不会读到
            // 对方尚未提交的旧值。
            let used: Option<i64> =
                sqlx::query_scalar("SELECT storage_used_bytes FROM users WHERE id = $1 FOR UPDATE")
                    .bind(uid)
                    .fetch_optional(&mut *tx)
                    .await?;
            let Some(used) = used else {
                tx.rollback().await?;
                return Ok(SaveVideoOutcome::UserNotFound);
            };
            let size = file_size.unwrap_or(0);
            if quota_bytes > 0 && used.saturating_add(size) > quota_bytes {
                tx.rollback().await?;
                return Ok(SaveVideoOutcome::QuotaExceeded);
            }
        }

        let insert = sqlx::query_as::<_, VideoRow>(&format!(
            "INSERT INTO videos (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, uploader_id, exif_extracted) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, FALSE) RETURNING {}",
            VIDEO_COLUMNS
        ))
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
        .fetch_one(&mut *tx)
        .await;

        let row = match insert {
            Ok(row) => row,
            Err(e) if is_unique_violation(&e) => {
                tx.rollback().await?;
                return Ok(SaveVideoOutcome::Duplicate);
            }
            Err(e) => {
                tx.rollback().await?;
                return Err(e);
            }
        };

        if let (Some(uid), Some(size)) = (uploader_id, file_size) {
            if size > 0 {
                sqlx::query(
                    "UPDATE users SET storage_used_bytes = COALESCE(storage_used_bytes, 0) + $1 \
                     WHERE id = $2",
                )
                .bind(size)
                .bind(uid)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(SaveVideoOutcome::Ok(Box::new(row)))
    }

    /// 批量插入本地视频记录（目录扫描时使用，单条 INSERT 替代 N+1）。
    ///
    /// # SQL
    /// ```sql
    /// INSERT INTO videos (title, description, source_type, cover_url, thumb_url,
    ///                     stream_url, category, file_hash, file_size, original_name,
    ///                     exif_taken_at, ..., exif_orientation, exif_extracted)
    /// VALUES ($1, $2, ...), ($3, $4, ...), ...
    /// ```
    ///
    /// # 用途
    /// 目录扫描发现新文件时，一次批量插入所有新视频，显著减少 DB 往返。
    ///
    /// # 返回
    /// 实际插入的行数。空列表提前返回 0。
    pub async fn batch_save_local_videos(
        &self,
        videos: &[LocalVideoValues<'_>],
    ) -> Result<u64, sqlx::Error> {
        if videos.is_empty() {
            return Ok(0);
        }
        let mut builder = sqlx::QueryBuilder::new(
            "INSERT INTO videos (title, description, source_type, cover_url, thumb_url, stream_url, category, file_hash, file_size, original_name, exif_taken_at, exif_lat, exif_lon, exif_camera, exif_lens, exif_aperture, exif_shutter, exif_iso, exif_focal_length, exif_orientation, exif_extracted) ",
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
                .push_bind(v.9)
                .push_bind(v.10.taken_at)
                .push_bind(v.10.lat)
                .push_bind(v.10.lon)
                .push_bind(v.10.camera.as_deref())
                .push_bind(v.10.lens.as_deref())
                .push_bind(v.10.aperture)
                .push_bind(v.10.shutter.as_deref())
                .push_bind(v.10.iso)
                .push_bind(v.10.focal_length)
                .push_bind(v.10.orientation)
                .push_bind(v.10.extracted);
        });
        let result = builder.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// 选择性更新视频的标题、描述或分类。
    ///
    /// # SQL
    /// 动态拼接，只更新非 `None` 的字段：
    /// ```sql
    /// UPDATE videos SET title = $1 [, description = $2] [, category = $3]
    /// WHERE id = $id
    /// ```
    ///
    /// # 用途
    /// 视频编辑页面保存修改时调用。
    ///
    /// # 性能
    /// - 所有字段均为 `None` 时直接返回 0，不发 DB 请求
    /// - 使用 `QueryBuilder` 动态拼接，避免 COALESCE 的写放大
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

    /// 原子递增视频的播放次数。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET views = views + 1 WHERE id = $1
    /// ```
    ///
    /// # 用途
    /// 每次用户开始播放时调用（由 playback service 触发）。
    ///
    /// # 性能
    /// - 原子操作，无需先 SELECT 再 UPDATE
    /// - 使用主键索引
    pub async fn increment_views(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET views = views + 1 WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 更新视频时长（毫秒转秒后存储）。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET duration = $1 WHERE id = $2
    /// ```
    ///
    /// # 用途
    /// 视频上传/转码完成后，从媒体文件的元数据中提取时长并更新。
    ///
    /// # 注意
    /// 入参为毫秒，存储为秒（`duration_ms / 1000`，整数除法截断）。
    pub async fn update_duration(&self, id: i64, duration_ms: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET duration = $1 WHERE id = $2")
            .bind(duration_ms / 1000)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 更新视频的封面图 URL。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET cover_url = $1 WHERE id = $2
    /// ```
    ///
    /// # 用途
    /// 封面提取任务完成后，将生成的封面图路径写入数据库。
    pub async fn update_cover_url(&self, id: i64, cover_url: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET cover_url = $1 WHERE id = $2")
            .bind(cover_url)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 仅标记"已尝试解析 EXIF"（无 EXIF / 文件缺失），不写任何 exif 列。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET exif_extracted = TRUE WHERE id = $1
    /// ```
    ///
    /// # 用途
    /// 与 [`Self::update_video_exif`] 的 `exif_extracted = TRUE` 收尾语义一致，
    /// 但不写 exif 列：图片没有 EXIF、或文件在补扫前已被删除时调用，避免后台
    /// 补扫任务对同一个文件反复重试。
    pub async fn mark_exif_extracted(&self, id: i64) -> Result<(), sqlx::Error> {
        log_slow_query("video_repo::mark_exif_extracted", || async {
            sqlx::query("UPDATE videos SET exif_extracted = TRUE WHERE id = $1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map(|_| ())
        })
        .await
    }

    /// 写入 EXIF 解析结果（10 个 exif 列 + `exif_extracted = TRUE`）。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET
    ///   exif_taken_at = $2, exif_lat = $3, ..., exif_orientation = $11,
    ///   exif_extracted = TRUE
    /// WHERE id = $1
    /// ```
    ///
    /// # 用途
    /// 图片/视频上传或后台补扫完成 EXIF 解析后调用。无论是否解析出字段都
    /// 应调用（`ParsedExif` 全空表示“无 EXIF”），把 `exif_extracted` 置 TRUE
    /// 以结束后台补扫。
    pub async fn update_video_exif(&self, id: i64, exif: &ParsedExif) -> Result<(), sqlx::Error> {
        let e = LocalVideoExif::from_parsed(exif, true);
        sqlx::query(
            "UPDATE videos SET exif_taken_at = $2, exif_lat = $3, exif_lon = $4, exif_camera = $5, \
             exif_lens = $6, exif_aperture = $7, exif_shutter = $8, exif_iso = $9, \
             exif_focal_length = $10, exif_orientation = $11, exif_extracted = TRUE WHERE id = $1",
        )
        .bind(id)
        .bind(e.taken_at)
        .bind(e.lat)
        .bind(e.lon)
        .bind(e.camera.as_deref())
        .bind(e.lens.as_deref())
        .bind(e.aperture)
        .bind(e.shutter.as_deref())
        .bind(e.iso)
        .bind(e.focal_length)
        .bind(e.orientation)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 更新视频的缩略图 URL。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET thumb_url = $1 WHERE id = $2
    /// ```
    ///
    /// # 用途
    /// 缩略图生成完成后写入数据库，供列表页展示使用。
    pub async fn update_thumb_url(&self, id: i64, thumb_url: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET thumb_url = $1 WHERE id = $2")
            .bind(thumb_url)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 按视频来源类型分组统计数量。
    ///
    /// # SQL
    /// ```sql
    /// SELECT source_type, COUNT(*)::bigint as count
    /// FROM videos
    /// GROUP BY source_type
    /// ORDER BY count DESC
    /// ```
    ///
    /// # 用途
    /// 管理后台仪表盘展示各来源（local/external/...）的视频数量分布。
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

    /// 按视频分类分组统计数量（空分类归入"未分类"）。
    ///
    /// # SQL
    /// ```sql
    /// SELECT cat as category, COUNT(*)::bigint as count
    /// FROM (
    ///   SELECT COALESCE(NULLIF(category,''), '未分类') as cat FROM videos
    /// ) t
    /// GROUP BY cat
    /// ORDER BY count DESC
    /// ```
    ///
    /// # 用途
    /// 管理后台仪表盘展示各分类的视频数量。空字符串与 `NULL` 统一显示为"未分类"。
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

    /// 获取所有视频的总播放次数。
    ///
    /// # SQL
    /// ```sql
    /// SELECT COALESCE(SUM(views), 0)::bigint FROM videos
    /// ```
    ///
    /// # 用途
    /// 管理后台仪表盘展示全站累计播放量。`COALESCE` 处理空表返回 `NULL` 的情况。
    pub async fn total_views(&self) -> Result<i64, sqlx::Error> {
        let (total,): (i64,) = sqlx::query_as("SELECT COALESCE(SUM(views), 0)::bigint FROM videos")
            .fetch_one(&self.pool)
            .await?;
        Ok(total)
    }

    /// 获取所有视频的总时长（秒）。
    ///
    /// # SQL
    /// ```sql
    /// SELECT COALESCE(SUM(duration), 0)::bigint FROM videos
    /// ```
    ///
    /// # 用途
    /// 管理后台仪表盘展示全站视频总时长。`COALESCE` 处理空表返回 `NULL` 的情况。
    pub async fn total_duration_secs(&self) -> Result<i64, sqlx::Error> {
        let (total,): (i64,) =
            sqlx::query_as("SELECT COALESCE(SUM(duration), 0)::bigint FROM videos")
                .fetch_one(&self.pool)
                .await?;
        Ok(total)
    }

    /// 批量更新多个视频的分类。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET category = $1 WHERE id = ANY($2)
    /// ```
    ///
    /// # 用途
    /// 管理后台批量修改视频分类。
    ///
    /// # 返回
    /// 实际更新的行数。空列表提前返回 0。
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

    /// 获取所有本地视频的 stream_url 集合。
    ///
    /// # SQL
    /// ```sql
    /// SELECT stream_url FROM videos WHERE source_type LIKE 'local%'
    /// ```
    ///
    /// # 用途
    /// 目录扫描时，将数据库中的文件路径与磁盘上的实际文件比对，
    /// 识别出磁盘上已删除但数据库中仍存在的"孤儿"记录。
    ///
    /// # 返回
    /// `HashSet<String>` 方便 O(1) 包含检查。
    pub async fn find_all_local_file_names(&self) -> Result<HashSet<String>, sqlx::Error> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT stream_url FROM videos WHERE source_type LIKE 'local%'",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    // ── Variant / transcode helpers ──

    /// 列出指定视频所有转码变体的文件路径（`/media/...`）。
    ///
    /// # SQL
    /// ```sql
    /// SELECT file_path FROM video_variants WHERE video_id = $1
    /// ```
    ///
    /// # 用途
    /// 阅后即焚删除时收集需要清理的物理文件（变体文件不属于主记录的
    /// stream_url，必须单独取回）。
    pub async fn list_variant_file_paths(&self, video_id: i64) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT file_path FROM video_variants WHERE video_id = $1")
                .bind(video_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(p,)| p).collect())
    }

    /// 删除指定视频的某个分辨率转码变体记录。
    ///
    /// # SQL
    /// ```sql
    /// DELETE FROM video_variants WHERE video_id = $1 AND resolution = $2
    /// ```
    ///
    /// # 用途
    /// 管理员删除某个分辨率的转码文件时调用，同步清理数据库记录。
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

    /// 统计指定视频的转码变体数量。
    ///
    /// # SQL
    /// ```sql
    /// SELECT COUNT(*) FROM video_variants WHERE video_id = $1
    /// ```
    ///
    /// # 用途
    /// 判断视频是否还有可用的多分辨率变体，决定是否清除 `has_variants` 标记。
    pub async fn count_variants(&self, video_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM video_variants WHERE video_id = $1")
                .bind(video_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    /// 清除视频的 `has_variants` 标记（当所有转码变体被删除后）。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE videos SET has_variants = false WHERE id = $1
    /// ```
    ///
    /// # 用途
    /// 所有分辨率变体被删除后调用，告知播放器不再尝试加载多分辨率流。
    pub async fn clear_has_variants(&self, video_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE videos SET has_variants = false WHERE id = $1")
            .bind(video_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 列出指定视频的所有转码变体（按分辨率从高到低排序）。
    ///
    /// # SQL
    /// ```sql
    /// SELECT resolution, file_path, file_size, bitrate, codec
    /// FROM video_variants
    /// WHERE video_id = $1
    /// ORDER BY CASE resolution
    ///   WHEN '2160p' THEN 1 WHEN '1080p' THEN 2 WHEN '7200p' THEN 3
    ///   WHEN '480p' THEN 4 WHEN '360p' THEN 5 ELSE 6
    /// END
    /// ```
    ///
    /// # 用途
    /// 播放器请求多分辨率源时返回可用列表；管理页面展示转码状态。
    ///
    /// # 返回
    /// 按分辨率从高到低排序的变体列表（2160p → 1080p → 720p → ...）。
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

    /// 取消指定视频的所有待处理/进行中的转码任务。
    ///
    /// # SQL
    /// ```sql
    /// UPDATE transcoding_jobs
    /// SET status = 'failed', error_message = 'Cancelled by admin'
    /// WHERE video_id = $1 AND status IN ('pending', 'processing')
    /// ```
    ///
    /// # 用途
    /// 管理员删除视频或手动取消转码时调用，防止后台任务继续处理已删除的视频。
    ///
    /// # 返回
    /// 实际被取消的任务数量（`rows_affected`）。
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

    // ── Trending ──

    /// 按当前时间重算所有 `views > 0` 视频的热度分数。
    ///
    /// # SQL（keyset 分批，每批 1000 行）
    /// ```sql
    /// UPDATE videos SET trending_score = calculate_trending_score(views, created_at)
    /// WHERE id IN (
    ///     SELECT id FROM videos WHERE views > 0 AND id > $1 ORDER BY id LIMIT $2
    /// )
    /// RETURNING id
    /// ```
    ///
    /// # 用途
    /// 迁移 037 的触发器只在 INSERT/UPDATE views/created_at 时计算一次分数，
    /// 而 `calculate_trending_score` 依赖 `CURRENT_TIMESTAMP`，分数会随
    /// 时间衰减失效。后台任务定期调用本方法即可让热度随时间自然衰减。
    ///
    /// # 性能
    /// 单条 `UPDATE ... WHERE views > 0` 在数据量大时会长时间持锁；这里用
    /// keyset（`id > last_id`）分批，每批独立提交，锁持有时间可控。
    ///
    /// # 返回
    /// 实际更新的行数（所有批次之和）。
    pub async fn recompute_trending_scores(&self) -> Result<u64, sqlx::Error> {
        const BATCH_SIZE: i64 = 1000;
        let mut last_id: i64 = 0;
        let mut total: u64 = 0;
        loop {
            let ids: Vec<i64> = sqlx::query_scalar(
                r#"
                UPDATE videos
                   SET trending_score = calculate_trending_score(views, created_at)
                 WHERE id IN (
                     SELECT id FROM videos
                      WHERE views > 0 AND id > $1
                      ORDER BY id
                      LIMIT $2
                 )
                RETURNING id
                "#,
            )
            .bind(last_id)
            .bind(BATCH_SIZE)
            .fetch_all(&self.pool)
            .await?;
            if ids.is_empty() {
                break;
            }
            total += ids.len() as u64;
            last_id = ids.into_iter().max().unwrap_or(last_id);
        }
        Ok(total)
    }
}

/// 视频转码变体行结构（对应 `video_variants` 表）。
#[derive(Debug, sqlx::FromRow)]
pub struct VideoVariantRow {
    pub resolution: String,
    pub file_path: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
    pub codec: Option<String>,
}
