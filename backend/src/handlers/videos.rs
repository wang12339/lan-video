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
use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::hashid;
use crate::util::pagination::{PaginationParams, DEFAULT_PAGE_SIZE, MAX_PAGE, MAX_PAGE_SIZE};
use crate::util::response::{
    error_response, internal_error_log, CachedResponse, ErrorResponse, SafeJson,
};

use crate::models::danmaku::{DanmakuListResponse, SendDanmakuRequest, SendDanmakuResponse};

const MAX_SEARCH_QUERY_LEN: usize = 200;

#[utoipa::path(
    get,
    path = "/videos",
    tag = "videos",
    summary = "List videos (paginated)",
    description = "Retrieve a paginated list of videos with optional filters. Results are cached for 10 seconds.",
    security(("bearerAuth" = [])),
    params(
        ("page" = Option<i64>, Query, description = "Page number (0-indexed)"),
        ("size" = Option<i64>, Query, description = "Page size (1-1000)"),
        ("query" = Option<String>, Query, description = "Search query — matches title and category (case-insensitive)"),
        ("type" = Option<String>, Query, description = "Filter by source_type (prefix with ! to exclude, e.g. '!external')"),
        ("category" = Option<String>, Query, description = "Filter by category name"),
        ("uploader_id" = Option<String>, Query, description = "Filter by uploader ID (admin only)"),
        ("sort" = Option<String>, Query, description = "Sort order")
    ),
    responses((status = 200, description = "Paginated video list", body = PagedVideoResponse))
)]
pub async fn list_videos(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<VideoQuery>,
) -> Result<CachedResponse<PagedVideoResponse>, (StatusCode, Json<ErrorResponse>)> {
    // /videos 分页是 0 基契约：前端 initialPageParam=0，repo 直接用
    // offset = page * size。这里不能用 1 基的 PaginationParams——它把
    // page=0 clamp 成 1，会让首页跳过最新一页（历史 bug，0 基前端下
    // 首屏永远从第 21 条开始）。
    let page = params.page.unwrap_or(0).clamp(0, MAX_PAGE);
    let size = params
        .size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let query = params.query.as_deref().unwrap_or("");
    let source_type = params.source_type.as_deref().unwrap_or("");
    let category = params.category.as_deref().unwrap_or("");
    let sort = params.sort.as_deref();

    if query.len() > MAX_SEARCH_QUERY_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("查询关键词不能超过 {} 个字符", MAX_SEARCH_QUERY_LEN),
        ));
    }

    // 访客模式/私有化：列表只展示自己上传的内容。非管理员忽略请求中的
    // uploader_id 参数，强制按本人过滤（防止 IDOR 窥探他人列表）；
    // 管理员保留全站视图（管理面板依赖）并允许按 uploader 筛选。
    let uploader_id = if auth_user.is_admin {
        params
            .uploader_id
            .as_deref()
            .and_then(hashid::decode_id_or_numeric)
    } else {
        Some(auth_user.id)
    };
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

#[utoipa::path(
    get,
    path = "/videos/{id}",
    tag = "videos",
    summary = "Get single video details",
    description = "Retrieve details for a single video by ID",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses(
        (status = 200, description = "Video details", body = VideoItem),
        (status = 404, description = "视频不存在")
    )
)]
/// GET /videos/{id}
pub async fn get_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Result<Json<VideoItem>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let cache_key = id;
    // 热路径：详情页每次刷新都会请求；60s 缓存吸收重复查询。
    // 失效由 `AppState::invalidate_caches` 统一处理（更新/删除/上传时全量失效）。
    if let Some(cached) = state.video_detail_cache.get(&cache_key) {
        // 缓存命中也要过归属检查（非管理员只能看自己的视频）
        require_video_owner(&auth_user, cached.uploader_id)?;
        return Ok(Json(cached));
    }
    let video = state
        .services
        .video
        .get_video(id)
        .await
        .map_err(|e| internal_error_log("get_video", &e))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;
    require_video_owner(&auth_user, video.uploader_id)?;
    state.video_detail_cache.insert(cache_key, video.clone());

    Ok(Json(video))
}

/// 访客模式/私有化：视频详情仅上传者本人（或管理员）可见。
/// 他人视频一律 404 —— 不暴露"存在但无权"的信息。
fn require_video_owner(
    auth_user: &AuthUser,
    uploader_id: Option<i64>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if auth_user.is_admin || uploader_id == Some(auth_user.id) {
        Ok(())
    } else {
        Err(error_response(StatusCode::NOT_FOUND, "视频不存在"))
    }
}

#[utoipa::path(
    get,
    path = "/videos/{id}/variants",
    tag = "videos",
    summary = "List transcoded variants for a video",
    description = "返回视频可用的转码分片（分辨率、播放地址、大小等）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses((status = 200, description = "Variant list", body = [VideoVariantResponse]))
)]
/// GET /videos/{id}/variants — available transcoded resolutions for playback
pub async fn get_video_variants(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Result<Json<Vec<VideoVariantResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    // 变体 URL 会暴露转码文件名，同样受归属检查约束
    let video = state
        .repos
        .video
        .find_by_id(id)
        .await
        .map_err(|e| internal_error_log("get_video_variants", &e))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;
    require_video_owner(&auth_user, video.uploader_id)?;
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

#[utoipa::path(
    get,
    path = "/videos/{id}/hls",
    tag = "videos",
    summary = "Get HLS playback status",
    description = "返回视频 HLS 主播放列表是否已生成；未生成时提示先转码",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses((status = 200, description = "HLS availability", body = serde_json::Value))
)]
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

#[utoipa::path(
    post,
    path = "/videos/{id}/like",
    tag = "videos",
    summary = "Toggle like",
    description = "Toggle like status for the current user on this video",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses((status = 200, description = "New like status", body = serde_json::Value))
)]
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

#[utoipa::path(
    get,
    path = "/videos/{id}/like",
    tag = "videos",
    summary = "Get like status",
    description = "Check if the current user has liked this video",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses((status = 200, description = "Like status", body = serde_json::Value))
)]
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

#[utoipa::path(
    post,
    path = "/videos/{id}/favorite",
    tag = "videos",
    summary = "Toggle favorite",
    description = "Toggle favorite status for the current user on this video",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses((status = 200, description = "New favorite status", body = serde_json::Value))
)]
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

#[utoipa::path(
    get,
    path = "/videos/{id}/favorite",
    tag = "videos",
    summary = "Get favorite status",
    description = "Check if the current user has favorited this video",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses((status = 200, description = "Favorite status", body = serde_json::Value))
)]
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

#[utoipa::path(
    post,
    path = "/videos/{id}/burn",
    tag = "videos",
    summary = "Burn video or image after view",
    description = "阅后即焚（平台全局行为）：永久删除该视频/图片（物理文件 + 数据库记录）。视频要求调用者播放进度 ≥ 90%，未完整观看返回 403；图片无进度要求，仅上传者本人或管理员可焚毁。",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses(
        (status = 204, description = "Video permanently deleted"),
        (status = 403, description = "Caller has not fully watched the video"),
        (status = 404, description = "Video not found")
    )
)]
/// POST /videos/{id}/burn — 阅后即焚：永久删除视频或图片
///
/// 平台全局行为：适用于所有视频/图片、所有用户（含上传者与存量内容）。
/// - 视频：请求者需有 ≥90% 的播放进度。
/// - 图片：无片长/进度要求，拥有者或管理员在查看结束后调用即可
///   （前端在关闭图片查看器时触发）。
///
/// 删除为物理级（主文件/变体/封面/缩略图）加数据库级联，不可恢复。
pub async fn burn_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let id = hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    state
        .services
        .video
        .burn_after_watch(&auth_user.username, auth_user.id, auth_user.is_admin, id)
        .await
        .map_err(|e| match e {
            // 用户可见的校验失败（400/403/404）原样透传；其余记日志转 500
            ServiceError::BadRequest(_)
            | ServiceError::Forbidden(_)
            | ServiceError::NotFound(_) => e.into_tuple(),
            _ => internal_error_log("burn_video", &e),
        })?;
    state.invalidate_caches();
    tracing::info!(user = %auth_user.username, video_id = id, "video burned after watch");
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/videos/favorites",
    tag = "videos",
    summary = "List current user's favorites (paginated)",
    description = "返回当前用户收藏的视频列表（分页）",
    security(("bearerAuth" = [])),
    params(
        ("page" = Option<i64>, Query, description = "Page number (1-indexed)"),
        ("size" = Option<i64>, Query, description = "Page size (1-100)")
    ),
    responses((status = 200, description = "Paginated favorite video list", body = serde_json::Value))
)]
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

#[utoipa::path(
    post,
    path = "/videos/{id}/view",
    tag = "videos",
    summary = "Increment view count",
    description = "Increment the view counter for a video. Rate-limited to 30 views per IP per 60 seconds per video.",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses(
        (status = 200, description = "View recorded", body = serde_json::Value),
        (status = 429, description = "Rate limited")
    )
)]
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

#[utoipa::path(
    get,
    path = "/videos/search",
    tag = "videos",
    summary = "Full-text search videos",
    description = "Search videos using PostgreSQL full-text search with ranking. Supports Chinese tokenization.",
    security(("bearerAuth" = [])),
    params(
        ("q" = String, Query, description = "Search query"),
        ("page" = Option<i64>, Query, description = "Page number (0-indexed)"),
        ("size" = Option<i64>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Search results", body = SearchResponse),
        (status = 400, description = "Bad request")
    )
)]
pub async fn search_videos(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
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

    // 访客模式/私有化：搜索仅命中自己上传的视频（管理员搜全站）
    let owner_id = (!auth_user.is_admin).then_some(auth_user.id);
    let (results, total) = state
        .services
        .search
        .full_text_search(owner_id, &params.q, page - 1, size)
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

#[utoipa::path(
    get,
    path = "/videos/search/suggest",
    tag = "videos",
    summary = "Search suggestions",
    description = "Get search suggestions based on partial query",
    security(("bearerAuth" = [])),
    params(
        ("q" = String, Query, description = "Partial search query"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("size" = Option<i64>, Query, description = "Max suggestions")
    ),
    responses(
        (status = 200, description = "Search suggestions", body = [String]),
        (status = 400, description = "Bad request")
    )
)]
/// GET /videos/search/suggest
pub async fn search_suggest(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
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

    let owner_id = (!auth_user.is_admin).then_some(auth_user.id);
    let suggestions = state
        .services
        .search
        .search_suggest(owner_id, &params.q, 10)
        .await
        .map_err(|e| internal_error_log("search_suggest", &e))?;

    Ok(Json(suggestions))
}

#[utoipa::path(
    get,
    path = "/videos/{id}/danmaku",
    tag = "videos",
    summary = "List danmaku for a video",
    description = "返回视频的全部弹幕（按出现时间升序）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses(
        (status = 200, description = "Danmaku list", body = serde_json::Value),
        (status = 400, description = "Bad request")
    )
)]
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

#[utoipa::path(
    post,
    path = "/videos/{id}/danmaku",
    tag = "videos",
    summary = "Send a danmaku",
    description = "发送一条弹幕（需登录）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID")),
    responses(
        (status = 201, description = "Danmaku created", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 404, description = "视频不存在")
    )
)]
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

    // 弹幕归属校验:视频必须存在,否则会向不存在的视频写入"幽灵弹幕"。
    match state.repos.video.find_by_id(video_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return Err(error_response(StatusCode::NOT_FOUND, "视频不存在")),
        Err(e) => return Err(internal_error_log("danmaku: find video", &e)),
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
