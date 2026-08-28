use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{header, StatusCode},
    Extension, Json,
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::playback::{PagedRecentWatchResponse, PaginationQuery};
use crate::models::video::{
    PagedVideoResponse, SearchQuery, SearchResponse, SearchResultItem, VideoItem, VideoQuery,
    VideoVariantResponse,
};
use crate::services::media_service::sweeper;
use crate::state::AppState;
use crate::util::hashid;
use crate::util::pagination::PaginationParams;
use crate::util::error::ServiceError;
use crate::util::response::{error_response, internal_error_log, CachedResponse, ErrorResponse, SafeJson};

use crate::models::danmaku::{DanmakuListResponse, SendDanmakuRequest, SendDanmakuResponse};

const MAX_SEARCH_QUERY_LEN: usize = 200;

pub async fn list_videos(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<VideoQuery>,
) -> Result<CachedResponse<PagedVideoResponse>, (StatusCode, Json<ErrorResponse>)> {
    sweeper::ensure_upload_lock_cleanup(&state);

    let pagination = PaginationParams::new(params.page, params.size);
    let page = pagination.page;
    let size = pagination.page_size;
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

    let cache_key = format!(
        "lv:{}:{}:{}:{}:{}:{}:{}",
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
                "public, s-maxage=30, max-age=10".to_string(),
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
            (!query.is_empty()).then_some(query),
            (!source_type.is_empty()).then_some(source_type),
            (!category.is_empty()).then_some(category),
            None,
            uploader_id,
            sort,
        )
        .await
        .map_err(|e| internal_error_log("list_videos", &e))?;

    let resp = PagedVideoResponse {
        items,
        total,
        page,
        size,
    };

    Ok((
        StatusCode::OK,
        [(
            header::CACHE_CONTROL,
            "public, s-maxage=30, max-age=10".to_string(),
        )],
        {
            state.video_cache.insert(cache_key, resp.clone());
            Json(resp)
        },
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
        .map_err(|e| internal_error_log("get_video", &e))?
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
        .map_err(|e| internal_error_log("get_video_variants", &e))?;
    Ok(Json(
        variants
            .into_iter()
            .map(|v| {
                let file_name = std::path::Path::new(&v.file_path)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
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

pub async fn get_hls_playlist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;

    let master_playlist = state
        .config
        .media_root
        .join(format!("hls/{}/master.m3u8", video_id));

    let exists = tokio::fs::metadata(&master_playlist).await.is_ok();
    Ok(Json(if exists {
        serde_json::json!({
            "status": "ready",
            "masterUrl": format!("/media/hls/{}/master.m3u8", video_id),
        })
    } else {
        serde_json::json!({
            "status": "not_available",
            "message": "HLS 流尚未生成，请先转码",
        })
    }))
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
        .map_err(|e| internal_error_log("toggle_like", &e))?;
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
        .map_err(|e| internal_error_log("get_like_status", &e))?;
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
        .map_err(|e| internal_error_log("toggle_favorite", &e))?;
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
        .map_err(|e| internal_error_log("get_favorite_status", &e))?;
    Ok(Json(serde_json::json!({"favorited": favorited})))
}

/// GET /videos/favorites
pub async fn list_favorites(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<PagedRecentWatchResponse>, (StatusCode, Json<ErrorResponse>)> {
    let pagination = PaginationParams::new(params.page, params.size);
    let page = pagination.page;
    let size = pagination.page_size;
    let offset = pagination.offset();

    let (items, total) = state
        .services
        .playback
        .get_favorites(&auth_user.username, size, offset)
        .await
        .map_err(|e| internal_error_log("list_favorites", &e))?;
    Ok(Json(PagedRecentWatchResponse {
        items,
        total,
        page,
        size,
    }))
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
        .map_err(|e| internal_error_log("increment_views", &e))?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn search_videos(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<ErrorResponse>)> {
    let pagination = PaginationParams::new(params.page, params.size);
    let page = pagination.page;
    let size = pagination.page_size;

    if params.q.len() > MAX_SEARCH_QUERY_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("搜索关键词不能超过 {} 个字符", MAX_SEARCH_QUERY_LEN),
        ));
    }

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

    let items = results
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

/// GET /videos/{id}/danmaku
///
/// 返回某视频的全部弹幕（按出现时间升序）。该路由位于统一的 `bearer_auth`
/// 之下，调用方需携带有效令牌。
pub async fn list_danmaku(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<String>,
) -> Result<Json<DanmakuListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;

    let items = state
        .repos
        .danmaku
        .list_by_video(video_id)
        .await
        .map_err(|e| ServiceError::into_tuple(e.into()))?;

    Ok(Json(DanmakuListResponse { items }))
}

/// POST /videos/{id}/danmaku
///
/// 发送一条弹幕。调用方需登录（由 `bearer_auth` 保证 `AuthUser` 存在）。
pub async fn create_danmaku(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<String>,
    SafeJson(req): SafeJson<SendDanmakuRequest>,
) -> Result<(StatusCode, Json<SendDanmakuResponse>), (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;

    let mut req = req;
    req.text = req.text.trim().to_string();
    if req.text.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "弹幕内容不能为空"));
    }
    if req.text.len() > 200 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "弹幕内容不能超过 200 个字符",
        ));
    }

    let id = state
        .repos
        .danmaku
        .create(video_id, auth_user.id, &req)
        .await
        .map_err(|e| ServiceError::into_tuple(e.into()))?;

    Ok((
        StatusCode::CREATED,
        Json(SendDanmakuResponse {
            id: hashid::encode_id(id),
        }),
    ))
}
