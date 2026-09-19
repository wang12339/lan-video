//! OpenAPI 3.1 规范：由 utoipa 从各 handler 的 `#[utoipa::path]` 注解与
//! models 的 `ToSchema` 派生自动生成，不再手工维护整份 JSON。
//!
//! 新增/修改接口时：
//! 1. 给 handler 加 `#[utoipa::path(...)]` 注解（方法、路径、tag、security、
//!    请求体/响应体）；
//! 2. 请求/响应结构体加 `#[derive(utoipa::ToSchema)]`；
//! 3. 在本文件 `ApiDoc` 的 `paths(...)`/`components(schemas(...))` 中登记。
//!
//! `tests/openapi_route_tests.rs` 会把生成结果里的路径与方法逐条与真实
//! axum 路由对比，漏登记会直接测试失败。
//!
//! 同一 handler 服务多条路径时（如 `list_tags` 同时服务 `/tags` 与
//! `/admin/tags`），在 handler 所在模块补一个薄包装函数并单独注解，因为
//! 一个函数只能携带一条 `#[utoipa::path]`。

use std::sync::OnceLock;

use serde_json::Value;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

/// 注册三种鉴权方案，供各 path 的 `security(...)` 引用。
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let Some(components) = openapi.components.as_mut() else {
            return;
        };
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("opaque")
                    .description(Some(
                        "Opaque 256-bit alphanumeric token returned by /auth/login or /auth/register",
                    ))
                    .build(),
            ),
        );
        components.add_security_scheme(
            "adminAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("opaque")
                    .description(Some("Requires admin privileges (role >= 3)"))
                    .build(),
            ),
        );
        components.add_security_scheme(
            "metricsToken",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("opaque")
                    .description(Some(
                        "Dedicated read-only METRICS_TOKEN for /metrics* (404 when unset)",
                    ))
                    .build(),
            ),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "ATMOS API",
        version = "0.1.0",
        description = "ATMOS Video — REST API",
        license(name = "MIT")
    ),
    servers(
        (url = "/", description = "Same-origin (reverse proxy)"),
        (url = "http://localhost:8082", description = "Development server")
    ),
    paths(
        // server / 监控
        crate::handlers::server::server_info,
        crate::handlers::server::health,
        crate::handlers::server::metrics,
        crate::handlers::server::metrics_prometheus,
        crate::handlers::server::openapi_spec,
        crate::handlers::server::docs_redirect,
        // auth
        crate::handlers::auth::register,
        crate::handlers::auth::login,
        crate::handlers::auth::guest_session,
        crate::handlers::auth::logout,
        crate::handlers::auth::user_info,
        crate::handlers::auth::user_profile,
        crate::handlers::auth::upload_avatar,
        crate::handlers::auth::forgot_password,
        crate::handlers::auth::reset_password,
        crate::handlers::auth::reset_password_get,
        crate::handlers::auth::update_email,
        crate::handlers::auth::send_verification_email,
        crate::handlers::auth::verify_email_get,
        crate::handlers::auth::verify_email,
        // auth gateway SSO
        crate::handlers::gateway::status,
        crate::handlers::gateway::start,
        crate::handlers::gateway::callback,
        crate::handlers::gateway::exchange,
        // users (self-service)
        crate::handlers::shares::list_my_shares,
        crate::handlers::shares::revoke_my_share,
        // videos
        crate::handlers::videos::list_videos,
        crate::handlers::videos::get_video,
        crate::handlers::videos::get_video_variants,
        crate::handlers::videos::get_hls_playlist,
        crate::handlers::videos::toggle_like,
        crate::handlers::videos::get_like_status,
        crate::handlers::videos::toggle_favorite,
        crate::handlers::videos::get_favorite_status,
        crate::handlers::videos::burn_video,
        crate::handlers::videos::list_favorites,
        crate::handlers::videos::increment_views,
        crate::handlers::videos::search_videos,
        crate::handlers::videos::search_suggest,
        crate::handlers::videos::list_danmaku,
        crate::handlers::videos::create_danmaku,
        // tags
        crate::handlers::tags::list_tags,
        crate::handlers::tags::list_tags_admin,
        crate::handlers::tags::create_tag,
        crate::handlers::tags::get_tag,
        crate::handlers::tags::update_tag,
        crate::handlers::tags::delete_tag,
        crate::handlers::tags::get_popular_tags,
        crate::handlers::tags::add_tags_to_video,
        crate::handlers::tags::remove_tags_from_video,
        crate::handlers::tags::remove_tag_from_video,
        crate::handlers::tags::get_video_tags,
        // comments
        crate::handlers::comments::list_comments,
        crate::handlers::comments::list_replies,
        crate::handlers::comments::create_comment,
        crate::handlers::comments::delete_comment,
        // playback
        crate::handlers::playback::get_playback_history_for_video,
        crate::handlers::playback::list_playback_history,
        crate::handlers::playback::update_playback_history,
        crate::handlers::playback::start_playback_session,
        crate::handlers::playback::playback_session_heartbeat,
        crate::handlers::playback::stop_playback_session,
        // playlists
        crate::handlers::playlists::list_my_playlists,
        crate::handlers::playlists::create_playlist,
        crate::handlers::playlists::get_playlist,
        crate::handlers::playlists::update_playlist,
        crate::handlers::playlists::delete_playlist,
        crate::handlers::playlists::list_playlist_videos,
        crate::handlers::playlists::add_video_to_playlist,
        crate::handlers::playlists::remove_video_from_playlist,
        // recommendations
        crate::handlers::recommendations::get_recommendations,
        crate::handlers::recommendations::get_similar_videos,
        crate::handlers::recommendations::get_trending_videos,
        crate::handlers::recommendations::get_recent_videos,
        // shares
        crate::handlers::shares::create_share_link,
        crate::handlers::shares::get_share_video,
        crate::handlers::shares::delete_share_link,
        // chat
        crate::handlers::chat::get_chat_history,
        crate::handlers::chat::ws_chat,
        crate::handlers::chat::upload_chat_image,
        crate::handlers::chat::upload_chat_video,
        crate::handlers::chat::admin_delete_chat_message,
        crate::handlers::chat::admin_chat_stats,
        crate::handlers::chat::admin_clear_chat_messages,
        // admin: perf / system / logs
        crate::handlers::admin::admin_performance::get_performance_metrics,
        crate::handlers::admin::admin_performance::reset_performance_metrics,
        crate::handlers::admin::admin_system::track_action,
        crate::handlers::admin::admin_system::get_stats,
        crate::handlers::admin::admin_system::get_registration_enabled,
        crate::handlers::admin::admin_system::set_registration_enabled,
        crate::handlers::admin::admin_system::system_info,
        crate::handlers::admin::admin_logs::get_logs,
        crate::handlers::admin::admin_logs::clear_logs,
        // admin: users
        crate::handlers::admin::admin_user::list_users,
        crate::handlers::admin::admin_user::pending_user_count,
        crate::handlers::admin::admin_user::delete_user,
        crate::handlers::admin::admin_user::reset_user_password,
        crate::handlers::admin::admin_user::toggle_user_admin,
        crate::handlers::admin::admin_user::approve_user,
        crate::handlers::admin::admin_user::kick_user,
        // admin: videos
        crate::handlers::admin::admin_video::add_external_video,
        crate::handlers::admin::admin_video::upload_video,
        crate::handlers::admin::admin_video::upload_status,
        crate::handlers::admin::admin_video::upload_resume,
        crate::handlers::admin::admin_video::check_hashes,
        crate::handlers::admin::admin_video::check_files,
        crate::handlers::admin::admin_video::scan_media,
        crate::handlers::admin::admin_video::update_video,
        crate::handlers::admin::admin_video::delete_video,
        crate::handlers::admin::admin_video::delete_videos,
        crate::handlers::admin::admin_video::upload_cover,
        crate::handlers::admin::admin_video::backfill_thumbnails,
        crate::handlers::admin::admin_video::backfill_exif,
        crate::handlers::admin::admin_video::batch_update_category,
        // admin: transcode / HLS
        crate::handlers::admin::admin_transcode::transcode_video,
        crate::handlers::admin::admin_transcode::transcode_status,
        crate::handlers::admin::admin_transcode::delete_variant,
        crate::handlers::admin::admin_transcode::cancel_transcode,
        crate::handlers::admin::admin_transcode::transcode_to_hls,
        crate::handlers::admin::admin_transcode::hls_status
    ),
    components(schemas(
        crate::util::response::ErrorResponse,
        crate::models::admin::LogEntry,
        crate::models::admin::LogQuery,
        crate::models::admin::TrackRequest,
        crate::models::admin::RegistrationToggleRequest,
        crate::models::admin::TranscodeRequest,
        crate::models::admin::TranscodeResponse,
        crate::models::admin::TranscodeStatusResponse,
        crate::models::admin::VariantInfo,
        crate::models::admin::JobInfo,
        crate::models::admin::AdminResetPasswordRequest,
        crate::models::admin::ApproveRequest,
        crate::models::auth::AuthRequest,
        crate::models::auth::AuthResponse,
        crate::models::auth::UserInfoResponse,
        crate::models::auth::UserProfileResponse,
        crate::models::auth::ForgotPasswordRequest,
        crate::models::auth::ForgotPasswordResponse,
        crate::models::auth::ResetPasswordRequest,
        crate::models::auth::ResetPasswordToken,
        crate::models::auth::UpdateEmailRequest,
        crate::models::auth::SendVerificationEmailResponse,
        crate::models::auth::VerifyEmailRequest,
        crate::models::chat::ChatHistoryResponse,
        crate::models::chat::ChatMessageItem,
        crate::models::chat::ChatEvent,
        crate::models::comment::CreateCommentRequest,
        crate::models::comment::CommentQuery,
        crate::models::comment::CommentResponse,
        crate::models::comment::CommentListResponse,
        crate::models::danmaku::DanmakuItemResponse,
        crate::models::danmaku::DanmakuListResponse,
        crate::models::danmaku::SendDanmakuRequest,
        crate::models::danmaku::SendDanmakuResponse,
        crate::models::playback::ListQuery,
        crate::models::playback::PaginationQuery,
        crate::models::playback::PagedRecentWatchResponse,
        crate::models::playback::SessionRequest,
        crate::models::playback::PlaybackHistoryRequest,
        crate::models::playback::PlaybackHistoryResponse,
        crate::models::playback::RecentWatchItem,
        crate::models::playlist::CreatePlaylistRequest,
        crate::models::playlist::UpdatePlaylistRequest,
        crate::models::playlist::AddVideoRequest,
        crate::models::playlist::ReorderRequest,
        crate::models::playlist::PlaylistResponse,
        crate::models::playlist::PlaylistListResponse,
        crate::models::playlist::PlaylistVideoItem,
        crate::models::recommendation::RecommendationResponse,
        crate::models::recommendation::RecommendationItem,
        crate::models::server::ServerInfo,
        crate::models::server::HealthCheckResponse,
        crate::models::server::CheckStatus,
        crate::models::server::SystemInfo,
        crate::models::server::DiskUsage,
        crate::models::server::MemoryUsage,
        crate::models::server::MetricsResponse,
        crate::models::share::CreateShareRequest,
        crate::models::share::CreateShareResponse,
        crate::models::share::ShareListItem,
        crate::models::tag::CreateTagRequest,
        crate::models::tag::UpdateTagRequest,
        crate::models::tag::TagResponse,
        crate::models::tag::TagListResponse,
        crate::models::tag::TagQuery,
        crate::models::video::SearchQuery,
        crate::models::video::SearchResponse,
        crate::models::video::SearchResultItem,
        crate::models::video::VideoVariantResponse,
        crate::models::video::ImageExif,
        crate::models::video::VideoItem,
        crate::models::video::PagedVideoResponse,
        crate::models::video::VideoQuery,
        crate::models::video::ExternalVideoRequest,
        crate::models::video::VideoUpdateRequest,
        crate::models::video::IdResponse,
        crate::models::video::OkResponse,
        crate::models::video::CheckHashesResponse,
        crate::models::video::CheckHashesRequest,
        crate::models::video::FileCheckItem,
        crate::models::video::CheckFilesResponse
    )),
    modifiers(&SecurityAddon),
)]
struct ApiDoc;

/// 生成并缓存 OpenAPI 文档（首次调用构建，之后复用）。
///
/// 派生出的文档只含字符串键与普通数值，序列化不会失败；这里 fail-fast
/// 以便模型定义出错时立刻暴露。
#[allow(clippy::expect_used)]
pub fn spec() -> &'static Value {
    static SPEC: OnceLock<Value> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI document must serialize")
    })
}
