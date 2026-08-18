use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{header, StatusCode},
    Extension, Json,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::middleware::auth::AuthUser;
use crate::models::video::{PagedVideoResponse, VideoItem, VideoQuery};
use crate::services::media_service::UPLOAD_TEMP_TTL;
use crate::state::AppState;
use crate::util::hashid;
use crate::util::response::{error_response, internal_error_log, CachedResponse, ErrorResponse};

const MAX_SEARCH_QUERY_LEN: usize = 200;

/// Cap the page number so `page * size` can never overflow i64 (an unbounded
/// `page` previously caused a panic in debug builds / a wrapped negative
/// OFFSET in release).
const MAX_PAGE: i64 = 1_000_000;

/// upload_locks（续传上传的内存锁表）的清理间隔：与 media_service 的
/// 临时文件清扫任务同频。
const UPLOAD_LOCK_CLEANUP_INTERVAL: Duration = Duration::from_secs(3600);

/// 惰性启动 upload_locks 的周期性清理（SECURITY L-07）。续传上传在
/// DashMap 中为每个 hash 创建互斥锁，仅在成功收尾时移除；放弃/失败的上传
/// 会遗留条目导致内存缓慢增长。本任务移除"临时文件不存在或超过 24h 未变化"
/// 的条目（与 media_service 临时文件清扫同一判定标准）。由首次 /videos
/// 列表请求触发，进程内只启动一个任务（OnceLock 幂等）。
fn ensure_upload_lock_cleanup(state: &Arc<AppState>) {
    static CLEANUP_STARTED: OnceLock<()> = OnceLock::new();
    if CLEANUP_STARTED.get().is_some() {
        return;
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let state = state.clone();
    CLEANUP_STARTED.get_or_init(|| {
        std::mem::drop(handle.spawn(async move {
            let mut interval = tokio::time::interval(UPLOAD_LOCK_CLEANUP_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                prune_upload_locks(&state).await;
            }
        }));
    });
}

async fn prune_upload_locks(state: &Arc<AppState>) {
    if state.upload_locks.is_empty() {
        return;
    }
    let root = state.config.media_root.clone();
    let locks = state.upload_locks.clone();
    let stale =
        tokio::task::spawn_blocking(move || stale_upload_lock_keys(&locks, &root, UPLOAD_TEMP_TTL))
            .await
            .unwrap_or_default();
    if stale.is_empty() {
        return;
    }
    for key in &stale {
        state.upload_locks.remove(key);
    }
    tracing::info!(
        removed = stale.len(),
        remaining = state.upload_locks.len(),
        "pruned stale upload lock entries"
    );
}

/// 返回应移除的锁 key：其 `.upload_{hash}` 临时文件缺失，或 mtime 超过
/// `ttl`（与 media_service 临时文件清扫同标准——文件已超龄，锁必然已死）。
/// 对 hash 做格式校验，防止意外 key 拼出目录外路径。竞态说明：entry 创建到
/// 临时文件落盘之间是微秒级窗口，且清理每小时一次；最坏情况是同一 hash
/// 短暂出现两个互斥锁，下一次 chunk 请求会重建串行化，可接受。
fn stale_upload_lock_keys(
    locks: &dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>>,
    media_root: &std::path::Path,
    ttl: Duration,
) -> Vec<String> {
    let now = std::time::SystemTime::now();
    locks
        .iter()
        .filter_map(|entry| {
            let hash = entry.key();
            let valid_hash = hash.len() <= 128
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
            if !valid_hash {
                return Some(hash.clone());
            }
            let path = media_root.join(format!(".upload_{}", hash));
            // 保守判定：文件存在且 mtime 可读且未超龄 → 存活；其余情况（含
            // mtime 读取失败、时钟异常）一律视为死亡并移除锁条目。
            let live = std::fs::metadata(&path)
                .map(|m| {
                    m.is_file()
                        && m.modified()
                            .map(|t| now.duration_since(t).map(|age| age < ttl).unwrap_or(true))
                            .unwrap_or(true)
                })
                .unwrap_or(false);
            if live {
                None
            } else {
                Some(hash.clone())
            }
        })
        .collect()
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub page: Option<i64>,
    pub size: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct SearchResponse {
    pub items: Vec<SearchResultItem>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(serde::Serialize)]
pub struct SearchResultItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rank: f32,
    pub headline: Option<String>,
}

/// GET /videos
pub async fn list_videos(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<VideoQuery>,
) -> Result<CachedResponse<PagedVideoResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 惰性启动 upload_locks 清理任务（进程内仅一次，幂等）
    ensure_upload_lock_cleanup(&state);

    let page = params.page.unwrap_or(0).clamp(0, MAX_PAGE);
    let size = params.size.unwrap_or(20).clamp(1, 1000);
    let query = params.query.as_deref().unwrap_or("");
    let source_type = params.source_type.as_deref().unwrap_or("");
    let category = params.category.as_deref().unwrap_or("");
    let uploader_id = params
        .uploader_id
        .as_deref()
        .and_then(hashid::decode_id_or_numeric);
    let sort = params.sort.as_deref();
    let _auth_user = auth_user;

    if query.len() > MAX_SEARCH_QUERY_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("查询关键词不能超过 {} 个字符", MAX_SEARCH_QUERY_LEN),
        ));
    }

    // Build cache key (without username)
    let cache_key = format!(
        "list_videos:{}:{}:{}:{}:{}:{}:{}",
        page,
        size,
        query,
        source_type,
        category,
        uploader_id.unwrap_or(0),
        sort.unwrap_or("")
    );
    if let Some(resp) = state.video_cache.get(&cache_key) {
        return Ok((
            StatusCode::OK,
            [(
                header::CACHE_CONTROL,
                "public, s-maxage=30, max-age=10".into(),
            )],
            Json(resp),
        ));
    }

    let (items, total) = state
        .services
        .video
        .list_videos_paged(
            page,
            size,
            if query.is_empty() { None } else { Some(query) },
            if source_type.is_empty() {
                None
            } else {
                Some(source_type)
            },
            if category.is_empty() {
                None
            } else {
                Some(category)
            },
            None,
            uploader_id,
            sort,
        )
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;

    let resp = PagedVideoResponse {
        items,
        total,
        page,
        size,
    };
    state.video_cache.insert(cache_key, resp.clone());

    Ok((
        StatusCode::OK,
        [(
            header::CACHE_CONTROL,
            "public, s-maxage=30, max-age=10".into(),
        )],
        Json(resp),
    ))
}

/// GET /videos/{id}
pub async fn get_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<VideoItem>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    // 热路径：详情页每次刷新都会请求；60s 缓存吸收重复查询。
    // 失效由 `AppState::invalidate_caches` 统一处理（更新/删除/上传时全量失效）。
    if let Some(cached) = state.video_detail_cache.get(&id) {
        return Ok(Json(cached));
    }
    let video = state
        .services
        .video
        .get_video(id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;
    state.video_detail_cache.insert(id, video.clone());

    Ok(Json(video))
}

/// GET /videos/{id}/variants — available transcoded resolutions for playback
pub async fn get_video_variants(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<VideoVariantResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let variants = state
        .repos
        .video
        .list_variants(id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    Ok(Json(
        variants
            .into_iter()
            .map(|v| {
                let file_name = std::path::Path::new(&v.file_path)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                VideoVariantResponse {
                    resolution: v.resolution,
                    url: format!("/media/{}", file_name),
                    file_size: v.file_size,
                    bitrate: v.bitrate,
                    codec: v.codec,
                }
            })
            .collect(),
    ))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoVariantResponse {
    pub resolution: String,
    pub url: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
    pub codec: Option<String>,
}

/// GET /videos/{id}/hls — Get HLS playlist URL for adaptive streaming
pub async fn get_hls_playlist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;

    let hls_dir = state
        .config
        .media_root
        .join("hls")
        .join(video_id.to_string());
    let master_playlist = hls_dir.join("master.m3u8");

    if master_playlist.exists() {
        Ok(Json(serde_json::json!({
            "status": "ready",
            "masterUrl": format!("/media/hls/{}/master.m3u8", video_id),
        })))
    } else {
        Ok(Json(serde_json::json!({
            "status": "not_available",
            "message": "HLS 流尚未生成，请先转码",
        })))
    }
}

/// POST /videos/{id}/like
pub async fn toggle_like(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let liked = state
        .services
        .playback
        .toggle_like(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    tracing::info!(user = %auth_user.username, video_id = id, liked = liked, "toggle like");
    Ok(Json(serde_json::json!({"liked": liked})))
}

/// GET /videos/{id}/like
pub async fn get_like_status(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let liked = state
        .services
        .playback
        .is_liked(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(serde_json::json!({"liked": liked})))
}

/// POST /videos/{id}/favorite
pub async fn toggle_favorite(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let favorited = state
        .services
        .playback
        .toggle_favorite(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    tracing::info!(user = %auth_user.username, video_id = id, favorited = favorited, "toggle favorite");
    Ok(Json(serde_json::json!({"favorited": favorited})))
}

/// GET /videos/{id}/favorite
pub async fn get_favorite_status(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let favorited = state
        .services
        .playback
        .is_favorited(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(serde_json::json!({"favorited": favorited})))
}

/// GET /videos/favorites
pub async fn list_favorites(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<crate::models::playback::RecentWatchItem>>, (StatusCode, Json<ErrorResponse>)>
{
    let favorites = state
        .services
        .playback
        .get_favorites(&auth_user.username)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(favorites))
}

/// POST /videos/{id}/view
pub async fn increment_views(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let rate_key = format!("view:{}:{}", addr.ip(), id);
    if state
        .ip_rate_limiter
        .check_with(&rate_key, 30, 60, 300)
        .await
        .is_err()
    {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "请求过于频繁",
        ));
    }
    state
        .services
        .video
        .increment_views(id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// GET /videos/search
pub async fn search_videos(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<ErrorResponse>)> {
    let page = params.page.unwrap_or(0).clamp(0, MAX_PAGE);
    let size = params.size.unwrap_or(20).clamp(1, 100);

    if params.q.len() > MAX_SEARCH_QUERY_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("搜索关键词不能超过 {} 个字符", MAX_SEARCH_QUERY_LEN),
        ));
    }

    // Empty/whitespace query: nothing matches a tsquery, short-circuit instead
    // of running a pointless full scan.
    if params.q.trim().is_empty() {
        return Ok(Json(SearchResponse {
            items: Vec::new(),
            total: 0,
            page,
            size,
        }));
    }

    let (results, total) = state
        .services
        .search
        .full_text_search(&params.q, page, size)
        .await
        .map_err(|e| internal_error_log("search_videos", &e))?;

    let items: Vec<SearchResultItem> = results
        .into_iter()
        .map(|r| SearchResultItem {
            id: r.video_id,
            title: r.title,
            description: r.description,
            category: r.category,
            rank: r.rank,
            headline: r.headline,
        })
        .collect();

    Ok(Json(SearchResponse {
        items,
        total,
        page,
        size,
    }))
}

/// GET /videos/search/suggest
pub async fn search_suggest(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    if params.q.len() > MAX_SEARCH_QUERY_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("搜索关键词不能超过 {} 个字符", MAX_SEARCH_QUERY_LEN),
        ));
    }

    // Empty/whitespace query: nothing matches a tsquery, short-circuit instead
    // of running a pointless full scan.
    if params.q.trim().is_empty() {
        return Ok(Json(Vec::new()));
    }

    let suggestions = state
        .services
        .search
        .search_suggest(&params.q, 10)
        .await
        .map_err(|e| internal_error_log("search_suggest", &e))?;

    Ok(Json(suggestions))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("atmos_{}_{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_stale_upload_lock_keys_prunes_dead_entries() {
        let dir = test_dir("lockprune");
        let locks: dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>> = dashmap::DashMap::new();
        locks.insert("alive".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        locks.insert("dead".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        locks.insert("old".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        std::fs::write(dir.join(".upload_alive"), b"x").unwrap();
        std::fs::write(dir.join(".upload_old"), b"y").unwrap();
        std::fs::File::open(dir.join(".upload_old"))
            .unwrap()
            .set_modified(
                std::time::SystemTime::now()
                    .checked_sub(std::time::Duration::from_secs(25 * 60 * 60))
                    .unwrap(),
            )
            .unwrap();

        let stale = stale_upload_lock_keys(&locks, &dir, UPLOAD_TEMP_TTL);
        assert!(
            stale.contains(&"dead".to_string()),
            "临时文件缺失的条目应移除"
        );
        assert!(
            stale.contains(&"old".to_string()),
            "临时文件超龄的条目应移除"
        );
        assert!(
            !stale.contains(&"alive".to_string()),
            "临时文件新鲜的条目应保留"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_stale_upload_lock_keys_defensive_hash_format() {
        let dir = test_dir("lockprune2");
        let locks: dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>> = dashmap::DashMap::new();
        locks.insert("../evil".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        locks.insert(
            "valid_hash_1".to_string(),
            Arc::new(tokio::sync::Mutex::new(())),
        );

        let stale = stale_upload_lock_keys(&locks, &dir, UPLOAD_TEMP_TTL);
        assert!(
            stale.contains(&"../evil".to_string()),
            "非 hash 格式的 key 应被防御性移除"
        );
        assert!(
            stale.contains(&"valid_hash_1".to_string()),
            "临时文件不存在时合法 key 也应收敛"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
