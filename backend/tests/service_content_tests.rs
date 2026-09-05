#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! 单元测试：评论 / 分享 / 标签 / 播放列表 服务的纯逻辑部分。
//!
//! 说明：本文件是 integration test（`tests/` 目录下独立编译的 crate），
//! 只能访问 crate 中标记为 `pub` 的项。因此：
//! - 直接测试所有公开的纯函数（`is_valid_share_token`、`hash_share_token`、
//!   错误映射 `into_response`、DTO 的 serde 契约等）；
//! - `comment_service::sanitize_content`、`tag_service::normalize_tag_name` 等
//!   私有纯函数无法直接调用（改为不可修改源码），只能通过真实 DB 的服务方法
//!   端到端间接验证，此类测试标 `#[ignore]`（需要 PostgreSQL）。
//!
//! 运行（仅非 ignore 测试，无需 DB）：
//!   cargo test --test service_content_tests

use atmos_video_backend::models::playlist::{
    AddVideoRequest, CreatePlaylistRequest, PlaylistListResponse, PlaylistResponse,
    PlaylistVideoItem, UpdatePlaylistRequest,
};
use atmos_video_backend::models::tag::TagResponse as HandlerTagResponse;
use atmos_video_backend::repositories::share_repo::hash_share_token;
use atmos_video_backend::repositories::tag_repo::Tag;
use atmos_video_backend::services::comment_service::CommentService;
use atmos_video_backend::services::share_service::{is_valid_share_token, ShareService};
use atmos_video_backend::services::tag_service::{
    CreateTagRequest, TagResponse as ServiceTagResponse, TagService, UpdateTagRequest,
};
use atmos_video_backend::util::error::ServiceError;
use axum::http::StatusCode;
use axum::Json;

/// `ServiceError` 已实现 `Debug`，可直接用 `.expect()`；此辅助函数保留以输出更友好的消息。
fn expect_share_ok<T>(r: Result<T, ServiceError>, msg: &str) -> T {
    match r {
        Ok(v) => v,
        Err(ServiceError::NotFound(m)) => panic!("{msg}: 分享不存在 ({m})"),
        Err(ServiceError::Forbidden(m)) => panic!("{msg}: 无权操作 ({m})"),
        Err(ServiceError::BadRequest(m)) => panic!("{msg}: 无效参数 {m}"),
        Err(ServiceError::Internal(m)) => panic!("{msg}: 内部错误 {m}"),
        Err(ServiceError::RateLimited) => panic!("{msg}: 限流"),
        Err(ServiceError::Conflict(m)) => panic!("{msg}: 资源冲突 {m}"),
        Err(ServiceError::Duplicate(m)) => panic!("{msg}: 资源重复 {m}"),
        Err(ServiceError::QuotaExceeded(m)) => panic!("{msg}: 配额超限 {m}"),
        Err(ServiceError::Validation(m)) => panic!("{msg}: 验证失败 {m}"),
    }
}

// ──────────────────────────── 评论服务 ────────────────────────────

fn assert_error_mapping(e: ServiceError, expected_status: StatusCode, expected_body: &str) {
    let (status, Json(body)) = e.into_tuple();
    assert_eq!(status, expected_status);
    assert_eq!(body.error, expected_body);
}

#[test]
fn comment_error_not_found_maps_to_404() {
    assert_error_mapping(
        ServiceError::NotFound("评论不存在".into()),
        StatusCode::NOT_FOUND,
        "评论不存在",
    );
}

#[test]
fn comment_error_forbidden_maps_to_403() {
    assert_error_mapping(
        ServiceError::Forbidden("无权操作".into()),
        StatusCode::FORBIDDEN,
        "无权操作",
    );
}

#[test]
fn comment_error_invalid_passes_message_through_as_400() {
    assert_error_mapping(
        ServiceError::BadRequest("评论内容 1-2000 字符".to_string()),
        StatusCode::BAD_REQUEST,
        "评论内容 1-2000 字符",
    );
    // 空消息也应返回 400 而不是其他状态
    assert_error_mapping(
        ServiceError::BadRequest(String::new()),
        StatusCode::BAD_REQUEST,
        "",
    );
}

#[test]
fn comment_error_internal_maps_to_500_without_leaking_detail() {
    // 内部消息只记日志，响应必须为通用文案（不泄露内部错误细节）
    assert_error_mapping(
        ServiceError::Internal("panic: null byte in password".to_string()),
        StatusCode::INTERNAL_SERVER_ERROR,
        "服务器内部错误",
    );
}

#[test]
fn comment_error_from_sqlx_error_is_internal_db_error() {
    for sqlx_err in [
        sqlx::Error::RowNotFound,
        sqlx::Error::PoolTimedOut,
        sqlx::Error::Protocol("mock".into()),
    ] {
        let (status, Json(body)) = ServiceError::from(sqlx_err).into_tuple();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "服务器内部错误", "DB 错误必须映射为通用 500");
    }
}

#[test]
fn comment_error_from_string_is_internal() {
    let (status, Json(body)) = ServiceError::from("自定义内部错误".to_string()).into_tuple();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.error, "服务器内部错误");
}

// 私有函数 sanitize_content / resolve_parent_comment 无法从集成测试直接调用，
// 其行为（XSS 清洗、长度限制、回复层级归一化）由文件末尾的 #[ignore]
// DB 测试端到端验证。

// ──────────────────────────── 分享服务 ────────────────────────────

#[test]
fn share_token_32_alnum_is_valid() {
    let token = "AbC123xYz0123456789abcdefghijklm";
    assert_eq!(token.len(), 32);
    assert!(is_valid_share_token(token));
}

#[test]
fn share_token_rejects_short_tokens() {
    for token in [
        "",
        "a",
        "abcdefghijklmnopqrstuvwxy",
        "a".repeat(31).as_str(),
    ] {
        assert!(!is_valid_share_token(token), "{token:?} 应被拒绝");
    }
}

#[test]
fn share_token_rejects_long_tokens() {
    assert!(!is_valid_share_token(&"a".repeat(33)));
    assert!(!is_valid_share_token(&"a".repeat(64)));
}

#[test]
fn share_token_rejects_special_characters() {
    for token in [
        "abcdefghijklmnopqrstuvwxyz012345-",
        "abcdefghijklmnopqrstuvwxyz012345_",
        "abcdefghijklmnopqrstuvwxyz012345.",
        "abcdefghijklmnopqrstuvwxyz01234+",
        "abcdefghijklmnopqrstuvwxyz01234 ",
    ] {
        assert!(!is_valid_share_token(token), "{token:?} 含非法字符应被拒绝");
    }
}

#[test]
fn share_token_rejects_non_ascii_bytes() {
    // 30 ASCII + 1 个双字节字符：len()==32（字节数）但含非 ASCII
    let token = format!("{}é", "a".repeat(30));
    assert_eq!(token.len(), 32);
    assert!(!is_valid_share_token(&token));
    // 31 ASCII + 中文（3 字节 → 字节数 34）
    assert!(!is_valid_share_token(&format!("{}你", "a".repeat(31))));
}

#[test]
fn share_token_alphabet_covers_digits_lower_and_upper() {
    // 全数字、全小写、全大写都是合法 token 字符集
    assert!(is_valid_share_token(&"0".repeat(32)));
    assert!(is_valid_share_token(&"a".repeat(32)));
    assert!(is_valid_share_token(&"Z".repeat(32)));
}

#[test]
fn share_error_mapping() {
    let (status, Json(body)) = ServiceError::NotFound("分享链接不存在".into()).into_tuple();
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body.error, "分享链接不存在");

    let (status, Json(body)) = ServiceError::Forbidden("无权操作".into()).into_tuple();
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body.error, "无权操作");

    let (status, Json(body)) =
        ServiceError::BadRequest("分享链接格式无效".to_string()).into_tuple();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body.error, "分享链接格式无效");

    let (status, Json(body)) = ServiceError::Internal("secret".to_string()).into_tuple();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.error, "服务器内部错误", "内部细节不得泄露给客户端");
}

#[test]
fn share_error_from_sqlx_is_generic_500() {
    let (status, Json(body)) = ServiceError::from(sqlx::Error::RowNotFound).into_tuple();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.error, "服务器内部错误");
}

#[test]
fn share_token_hash_is_64_hex_chars() {
    let hash = hash_share_token("AbC123xYz0123456789abcdefghijklmn");
    assert_eq!(hash.len(), 64);
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "哈希必须为小写 hex"
    );
}

#[test]
fn share_token_hash_is_deterministic() {
    let token = "AbC123xYz0123456789abcdefghijklmn";
    assert_eq!(hash_share_token(token), hash_share_token(token));
}

#[test]
fn share_token_hash_differs_for_different_tokens() {
    // 仅 1 字符差异的 token 也必须产生不同哈希
    assert_ne!(
        hash_share_token(&"a".repeat(32)),
        hash_share_token(&format!("{}b", "a".repeat(31)))
    );
}

#[test]
fn share_token_hash_does_not_leak_raw_token() {
    let raw = "AbC123xYz0123456789abcdefghijklmn";
    let hash = hash_share_token(raw);
    assert!(!hash.contains(&raw[..8]), "哈希不得包含原文片段");
}

#[test]
fn share_token_hash_matches_known_sha256_vector() {
    // 固定 SHA-256 向量，锁定算法（一旦哈希算法更换此测试即失败）
    assert_eq!(
        hash_share_token("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// generate_token 与 create_share_link 的过期时间计算为私有逻辑，
// 需通过 DB 端到端验证，见文件末尾 #[ignore] 测试。

// ──────────────────────────── 标签服务 ────────────────────────────

#[test]
fn tag_response_from_repo_tag_maps_all_fields() {
    let tag = Tag {
        id: 42,
        name: "Rust".to_string(),
        color: Some("#FF5733".to_string()),
        usage_count: 7,
    };
    let resp = ServiceTagResponse::from(tag);
    assert_eq!(resp.id, 42);
    assert_eq!(resp.name, "Rust");
    assert_eq!(resp.color.as_deref(), Some("#FF5733"));
    assert_eq!(resp.usage_count, 7);
}

#[test]
fn tag_response_handles_missing_color() {
    let tag = Tag {
        id: 1,
        name: "无颜色".to_string(),
        color: None,
        usage_count: 0,
    };
    let resp = ServiceTagResponse::from(tag);
    assert!(resp.color.is_none());
}

#[test]
fn create_tag_request_deserializes_with_color() {
    let req: CreateTagRequest =
        serde_json::from_str(r##"{"name":"Rust","color":"#FF5733"}"##).unwrap();
    assert_eq!(req.name, "Rust");
    assert_eq!(req.color.as_deref(), Some("#FF5733"));
}

#[test]
fn create_tag_request_deserializes_without_color() {
    let req: CreateTagRequest = serde_json::from_str(r#"{"name":"Rust"}"#).unwrap();
    assert_eq!(req.name, "Rust");
    assert!(req.color.is_none());
}

#[test]
fn create_tag_request_rejects_missing_name() {
    assert!(serde_json::from_str::<CreateTagRequest>(r##"{"color":"#FF5733"}"##).is_err());
    assert!(serde_json::from_str::<CreateTagRequest>(r#"{}"#).is_err());
}

#[test]
fn update_tag_request_accepts_partial_fields() {
    let req: UpdateTagRequest = serde_json::from_str(r#"{"name":"Rust2"}"#).unwrap();
    assert_eq!(req.name.as_deref(), Some("Rust2"));
    assert!(req.color.is_none());

    let req: UpdateTagRequest = serde_json::from_str(r##"{"color":"#000000"}"##).unwrap();
    assert!(req.name.is_none());
    assert_eq!(req.color.as_deref(), Some("#000000"));

    // 全空对象合法（表示不修改任何字段）
    let req: UpdateTagRequest = serde_json::from_str(r#"{}"#).unwrap();
    assert!(req.name.is_none() && req.color.is_none());
}

#[test]
fn service_tag_response_serializes_with_snake_case() {
    // 服务层 TagResponse 未声明 rename_all —— 序列化为 snake_case。
    // 该类型仅供服务内部流转，对外由 handlers::tags::TagResponse 转换
    // （见 test_handler_tag_response_camel_case）。
    let resp = ServiceTagResponse {
        id: 1,
        name: "Rust".to_string(),
        color: None,
        usage_count: 3,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json.get("usage_count").and_then(|v| v.as_i64()), Some(3));
    assert!(json.get("usageCount").is_none(), "服务层应为 snake_case");
}

#[test]
fn handler_tag_response_serializes_with_camel_case() {
    // handlers::tags::TagResponse 带 rename_all="camelCase"，与前端
    // webapp/src/api/tags.ts 的 usageCount 字段一致 —— 用测试锁定契约。
    let resp = HandlerTagResponse {
        id: 1,
        name: "Rust".to_string(),
        color: Some("#FF5733".to_string()),
        usage_count: 3,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json.get("usageCount").and_then(|v| v.as_i64()), Some(3));
    assert!(json.get("usage_count").is_none());
}

// normalize_tag_name / validate_color / validate_tag_name / dedupe_tag_ids
// 均为私有函数，无法直接从集成测试调用；其行为（归一化、颜色格式、
// 长度与控制字符校验、去重排序）由文件末尾 #[ignore] DB 测试端到端验证。

// ──────────────────────────── 播放列表 ────────────────────────────

fn sample_playlist_response() -> PlaylistResponse {
    PlaylistResponse {
        id: 5,
        name: "我的收藏".to_string(),
        description: Some("好视频".to_string()),
        is_public: true,
        cover_url: Some("https://example.com/c.jpg".to_string()),
        item_count: 12,
        created_at: "2026-08-13 10:00:00".to_string(),
        updated_at: "2026-08-13 12:00:00".to_string(),
    }
}

#[test]
fn playlist_response_serializes_with_camel_case() {
    let json = serde_json::to_value(sample_playlist_response()).unwrap();
    let obj = json.as_object().unwrap();
    // 契约字段：前端 webapp 依赖 camelCase
    for key in [
        "id",
        "name",
        "description",
        "isPublic",
        "coverUrl",
        "itemCount",
        "createdAt",
        "updatedAt",
    ] {
        assert!(obj.contains_key(key), "缺少字段 {key}");
    }
    // 不存在 snake_case 变体
    for bad in [
        "is_public",
        "cover_url",
        "item_count",
        "created_at",
        "updated_at",
    ] {
        assert!(!obj.contains_key(bad), "不应序列化出 {bad}");
    }
    assert_eq!(obj.get("itemCount").and_then(|v| v.as_i64()), Some(12));
    assert_eq!(obj.get("isPublic").and_then(|v| v.as_bool()), Some(true));
    // Option::None 序列化为 null，前端反序列化可容忍
    let mut none_cover = sample_playlist_response();
    none_cover.cover_url = None;
    let json = serde_json::to_value(none_cover).unwrap();
    assert!(json.get("coverUrl").is_some());
    assert_eq!(json.get("coverUrl").and_then(|v| v.as_str()), None);
}

#[test]
fn playlist_list_response_serializes() {
    let resp = PlaylistListResponse {
        playlists: vec![sample_playlist_response()],
    };
    let json = serde_json::to_value(resp).unwrap();
    let playlists = json.get("playlists").and_then(|v| v.as_array()).unwrap();
    assert_eq!(playlists.len(), 1);
}

#[test]
fn playlist_video_item_serializes_with_camel_case() {
    let item = PlaylistVideoItem {
        id: 9,
        title: "视频".to_string(),
        description: "描述".to_string(),
        source_type: "external".to_string(),
        cover_url: None,
        stream_url: "https://example.com/v.mp4".to_string(),
        category: "fixture".to_string(),
        views: 100,
        duration: 3000,
    };
    let json = serde_json::to_value(item).unwrap();
    for key in [
        "id",
        "title",
        "description",
        "sourceType",
        "coverUrl",
        "streamUrl",
        "category",
        "views",
        "duration",
    ] {
        assert!(json.get(key).is_some(), "缺少字段 {key}");
    }
    assert!(json.get("source_type").is_none());
}

#[test]
fn create_playlist_request_deserializes_partial() {
    let req: CreatePlaylistRequest = serde_json::from_str(r#"{"name":"歌单"}"#).unwrap();
    assert_eq!(req.name, "歌单");
    assert!(req.description.is_none() && req.is_public.is_none());

    let req: CreatePlaylistRequest =
        serde_json::from_str(r#"{"name":"歌单","description":"d","is_public":true}"#).unwrap();
    assert_eq!(req.description.as_deref(), Some("d"));
    assert_eq!(req.is_public, Some(true));
}

#[test]
fn create_playlist_request_rejects_missing_name() {
    assert!(serde_json::from_str::<CreatePlaylistRequest>(r#"{}"#).is_err());
    assert!(serde_json::from_str::<CreatePlaylistRequest>(r#"{"is_public":true}"#).is_err());
}

#[test]
fn update_playlist_request_deserializes_empty_and_partial() {
    // 全空对象合法：PUT 时表示不修改任何字段
    let req: UpdatePlaylistRequest = serde_json::from_str(r#"{}"#).unwrap();
    assert!(req.name.is_none() && req.description.is_none() && req.is_public.is_none());

    let req: UpdatePlaylistRequest = serde_json::from_str(r#"{"name":"改名"}"#).unwrap();
    assert_eq!(req.name.as_deref(), Some("改名"));
}

#[test]
fn add_video_request_deserializes() {
    // 服务端 struct 无 rename_all，字段名为 snake_case
    let req: AddVideoRequest = serde_json::from_str(r#"{"video_id":123}"#).unwrap();
    assert_eq!(req.video_id, 123);

    assert!(serde_json::from_str::<AddVideoRequest>(r#"{"videoId":123}"#).is_err());
    assert!(serde_json::from_str::<AddVideoRequest>(r#"{}"#).is_err());
}

// handlers::playlists::is_valid_playlist_name 为私有函数，无法直接测试。
// 名称长度边界（≤200 字符）与 DB VARCHAR(200) 的一致性由文件末尾
// #[ignore] DB 测试验证。

// ──────────────────────── #[ignore] DB 端到端测试 ────────────────────────
// 以下测试依赖真实 PostgreSQL（DATABASE_URL + 已应用 migrations），默认不运行。
// 取消 #[ignore] 并设置 DATABASE_URL 后可执行：
//   DATABASE_URL="postgres://..." cargo test --test service_content_tests -- --ignored

use atmos_video_backend::repositories::comment_repo::CommentRepository;
use atmos_video_backend::repositories::playlist_repo::PlaylistRepository;
use atmos_video_backend::repositories::share_repo::ShareRepository;
use atmos_video_backend::repositories::tag_repo::TagRepository;
use atmos_video_backend::repositories::user_repo::UserRepository;
use atmos_video_backend::repositories::video_repo::VideoRepository;
use chrono::Duration;
use sqlx::PgPool;

async fn db_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("需要设置 DATABASE_URL 才能运行 DB 测试");
    PgPool::connect(&url).await.expect("连接测试数据库失败")
}

/// 创建 (user_id, video_id) 测试夹具，结束前用 `cleanup_fixture` 清理。
async fn db_fixture(pool: &PgPool, tag: &str) -> (i64, i64) {
    let user_repo = UserRepository::new(pool.clone());
    let username = format!("svc_test_{}_{}", std::process::id(), tag);
    let user_id = user_repo
        .create_user(1, &username, "unused-hash", 3)
        .await
        .expect("create fixture user");
    let video_repo = VideoRepository::new(pool.clone());
    let video_id = video_repo
        .save_external_video(
            1,
            &format!("svc_test_video_{tag}"),
            "fixture",
            "fixture",
            None,
            &format!("https://example.com/{tag}.mp4"),
            Some(user_id),
        )
        .await
        .expect("create fixture video");
    (user_id, video_id)
}

async fn db_cleanup(pool: &PgPool, user_id: i64, video_id: i64) {
    let _ = sqlx::query("DELETE FROM comments WHERE video_id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM share_links WHERE video_id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "DELETE FROM video_tags WHERE video_id IN (SELECT id FROM videos WHERE uploader_id = $1)",
    )
    .bind(user_id)
    .execute(pool)
    .await;
    let _ = sqlx::query("DELETE FROM videos WHERE id = $1")
        .bind(video_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

#[ignore]
#[tokio::test]
async fn db_comment_sanitize_strips_html_and_trims() {
    let pool = db_pool().await;
    let (user_id, video_id) = db_fixture(&pool, "cmt_sanitize").await;
    let svc = CommentService::new(
        CommentRepository::new(pool.clone()),
        VideoRepository::new(pool.clone()),
    );

    let row = svc
        .create_comment(
            1,
            video_id,
            user_id,
            "  <script>alert(1)</script>你好<b>世界</b>  ",
            None,
            false,
        )
        .await
        .expect("create comment");
    assert_eq!(
        row.content, "你好世界",
        "HTML 必须被剥离，首尾空白必须被修剪"
    );

    db_cleanup(&pool, user_id, video_id).await;
}

#[ignore]
#[tokio::test]
async fn db_comment_length_limit_enforced() {
    let pool = db_pool().await;
    let (user_id, video_id) = db_fixture(&pool, "cmt_len").await;
    let svc = CommentService::new(
        CommentRepository::new(pool.clone()),
        VideoRepository::new(pool.clone()),
    );

    // 2001 字符 → 拒绝
    let err = svc
        .create_comment(1, video_id, user_id, &"a".repeat(2001), None, false)
        .await
        .unwrap_err();
    assert!(matches!(err, ServiceError::BadRequest(_)), "{err:?}");

    // 空/纯空白 → 拒绝
    assert!(matches!(
        svc.create_comment(1, video_id, user_id, "   ", None, false)
            .await,
        Err(ServiceError::BadRequest(_))
    ));

    // 恰好 2000 字符 → 通过
    let row = svc
        .create_comment(1, video_id, user_id, &"a".repeat(2000), None, false)
        .await
        .expect("2000 字符应通过");
    assert_eq!(row.content.len(), 2000);

    db_cleanup(&pool, user_id, video_id).await;
}

#[ignore]
#[tokio::test]
async fn db_comment_reply_depth_normalized_to_two_levels() {
    let pool = db_pool().await;
    let (user_id, video_id) = db_fixture(&pool, "cmt_depth").await;
    let svc = CommentService::new(
        CommentRepository::new(pool.clone()),
        VideoRepository::new(pool.clone()),
    );

    let root = svc
        .create_comment(1, video_id, user_id, "root", None, false)
        .await
        .expect("root comment");
    let reply = svc
        .create_comment(1, video_id, user_id, "reply", Some(root.id), false)
        .await
        .expect("reply");
    assert_eq!(reply.parent_id, Some(root.id));

    // 对回复再回复 → 父节点必须被归一化到顶层评论（线程最多两层）
    let nested = svc
        .create_comment(1, video_id, user_id, "nested", Some(reply.id), false)
        .await
        .expect("nested reply");
    assert_eq!(
        nested.parent_id,
        Some(root.id),
        "第三层回复必须归一化到根评论"
    );

    // 父评论不存在 → 明确报错
    assert!(matches!(
        svc.create_comment(1, video_id, user_id, "x", Some(999_999_999), false)
            .await,
        Err(ServiceError::BadRequest(_))
    ));

    db_cleanup(&pool, user_id, video_id).await;
}

#[ignore]
#[tokio::test]
async fn db_comment_rejects_parent_from_other_video() {
    let pool = db_pool().await;
    let (user_id, video_a) = db_fixture(&pool, "cmt_va").await;
    let (_, video_b) = db_fixture(&pool, "cmt_vb").await;
    let svc = CommentService::new(
        CommentRepository::new(pool.clone()),
        VideoRepository::new(pool.clone()),
    );

    let comment_on_a = svc
        .create_comment(1, video_a, user_id, "on A", None, false)
        .await
        .expect("comment on video A");

    // 在视频 B 下回复视频 A 的评论 → 拒绝
    let err = svc
        .create_comment(1, video_b, user_id, "cross", Some(comment_on_a.id), true)
        .await
        .unwrap_err();
    assert!(matches!(err, ServiceError::BadRequest(_)));

    // 不存在的视频 → 拒绝
    assert!(matches!(
        svc.create_comment(1, 999_999_999, user_id, "no video", None, true)
            .await,
        Err(ServiceError::BadRequest(_))
    ));

    db_cleanup(&pool, user_id, video_a).await;
    db_cleanup(&pool, user_id, video_b).await;
}

#[ignore]
#[tokio::test]
async fn db_share_link_expiry_math_default_and_clamped() {
    let pool = db_pool().await;
    let (user_id, video_id) = db_fixture(&pool, "share").await;
    let svc = ShareService::new(ShareRepository::new(pool.clone()));

    // 默认（无参数）：固定 3 小时
    let (token, share) = expect_share_ok(
        svc.create_share_link(1, video_id, user_id, None).await,
        "create share",
    );
    assert_eq!(token.len(), 32, "token 必须为 32 位");
    assert!(token.chars().all(|c| c.is_ascii_alphanumeric()));
    let expires = share.expires_at.expect("必须有过期时间");
    assert_eq!(
        expires.signed_duration_since(share.created_at),
        Duration::hours(3),
        "默认过期时间应为 3 小时"
    );

    // 1 天
    let (_, share) = expect_share_ok(
        svc.create_share_link(1, video_id, user_id, Some(1)).await,
        "create 1-day share",
    );
    assert_eq!(
        share
            .expires_at
            .unwrap()
            .signed_duration_since(share.created_at),
        Duration::days(1)
    );

    // clamp：0 / 负数 → 1 天；超大值 → 365 天
    for clamped in [0, -10] {
        let (_, share) = expect_share_ok(
            svc.create_share_link(1, video_id, user_id, Some(clamped))
                .await,
            "clamped share",
        );
        assert_eq!(
            share
                .expires_at
                .unwrap()
                .signed_duration_since(share.created_at),
            Duration::days(1),
            "{clamped} 天应被钳制为 1 天"
        );
    }
    let (_, share) = expect_share_ok(
        svc.create_share_link(1, video_id, user_id, Some(999)).await,
        "clamped share",
    );
    assert_eq!(
        share
            .expires_at
            .unwrap()
            .signed_duration_since(share.created_at),
        Duration::days(365),
        "999 天应被钳制为 365 天"
    );

    // 无效格式 token → 拒绝（无需 DB 命中）
    let err = svc.get_share_video("bad-token").await.unwrap_err();
    assert!(matches!(err, ServiceError::BadRequest(_)));

    db_cleanup(&pool, user_id, video_id).await;
}

#[ignore]
#[tokio::test]
async fn db_share_link_create_then_lookup_and_revoke() {
    let pool = db_pool().await;
    let (user_id, video_id) = db_fixture(&pool, "share2").await;
    let svc = ShareService::new(ShareRepository::new(pool.clone()));

    let (token, _) = expect_share_ok(
        svc.create_share_link(1, video_id, user_id, Some(30)).await,
        "create share",
    );

    // 用原始 token 查询 → 命中
    let share = expect_share_ok(svc.get_share_video(&token).await, "lookup by token");
    assert_eq!(share.video_id, video_id);

    // 篡改一个字符 → 查不到（哈希查找，不是原文比对）
    let mut tampered: Vec<char> = token.chars().collect();
    tampered[0] = if tampered[0] == 'a' { 'b' } else { 'a' };
    let tampered: String = tampered.into_iter().collect();
    assert!(matches!(
        svc.get_share_video(&tampered).await,
        Err(ServiceError::NotFound(_))
    ));

    // 非本人撤销 → NotFound；本人撤销 → 成功
    assert!(matches!(
        svc.revoke_my_share(share.id, user_id + 1).await,
        Err(ServiceError::NotFound(_))
    ));
    expect_share_ok(
        svc.revoke_my_share(share.id, user_id).await,
        "revoke own share",
    );

    db_cleanup(&pool, user_id, video_id).await;
}

#[ignore]
#[tokio::test]
async fn db_tag_name_normalized_color_validated_and_deduped() {
    let pool = db_pool().await;
    let (user_id, video_id) = db_fixture(&pool, "tags").await;
    let tag_repo = TagRepository::new(pool.clone());
    let svc = TagService::new(tag_repo.clone(), VideoRepository::new(pool.clone()));

    // 名称归一化 + 颜色 trim 后才落库
    let tag = svc
        .create_tag(
            1,
            CreateTagRequest {
                name: "  Rust   Lang  ".to_string(),
                color: Some("  #FF5733  ".to_string()),
            },
        )
        .await
        .expect("create tag");
    assert_eq!(tag.name, "Rust Lang", "内部连续空白必须折叠为单个空格");
    assert_eq!(tag.color.as_deref(), Some("#FF5733"), "颜色必须 trim");

    // 重复名 → 唯一约束冲突
    let err = svc
        .create_tag(
            1,
            CreateTagRequest {
                name: "Rust Lang".to_string(),
                color: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ServiceError::Duplicate(_)), "{err}");

    // 非法颜色 → 校验失败
    let err = svc
        .create_tag(
            1,
            CreateTagRequest {
                name: "bad-color-tag".to_string(),
                color: Some("GGGGGG".to_string()),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ServiceError::Validation(_)), "{err}");

    // 空名 / 超长名 → 校验失败
    assert!(svc
        .create_tag(
            1,
            CreateTagRequest {
                name: "   ".to_string(),
                color: None,
            }
        )
        .await
        .is_err());
    assert!(svc
        .create_tag(
            1,
            CreateTagRequest {
                name: "a".repeat(101),
                color: None,
            }
        )
        .await
        .is_err());

    // 加标签：重复 id 去重后成功；不存在的 id → 报错；超量 → 报错
    let id = tag.id;
    svc.add_tags_to_video(1, video_id, &[id, id, id], user_id, false)
        .await
        .expect("dedupe then add");
    let err = svc
        .add_tags_to_video(1, video_id, &[999_999], user_id, false)
        .await
        .unwrap_err();
    assert!(matches!(err, ServiceError::NotFound(_)), "{err}");
    let too_many: Vec<i32> = (1..=101).collect();
    let err = svc
        .add_tags_to_video(1, video_id, &too_many, user_id, false)
        .await
        .unwrap_err();
    assert!(matches!(err, ServiceError::Validation(_)), "{err}");

    // 空列表直接成功（幂等）
    svc.add_tags_to_video(1, video_id, &[], user_id, false)
        .await
        .expect("empty ok");

    db_cleanup(&pool, user_id, video_id).await;
}

#[ignore]
#[tokio::test]
async fn db_playlist_name_boundary_matches_varchar_200() {
    let pool = db_pool().await;
    let (user_id, video_id) = db_fixture(&pool, "plist").await;
    let repo = PlaylistRepository::new(pool.clone());

    // 200 字符（中文按字符计数）→ DB 层可存
    let ok_name = "名".repeat(200);
    let playlist = repo
        .create_playlist(1, user_id, &ok_name, None, true)
        .await
        .expect("200 字符名称应可存储");
    assert_eq!(playlist.name.chars().count(), 200);

    // 201 字符 → DB VARCHAR(200) 拒绝（与 handler 的 is_valid_playlist_name 边界一致）
    assert!(repo
        .create_playlist(1, user_id, &"名".repeat(201), None, true)
        .await
        .is_err());

    // 空名在 DB 层不被拦截（验证校验责任在 handler/service 层）
    let empty = repo
        .create_playlist(1, user_id, "", None, false)
        .await
        .expect("DB 允许空名，handler 负责拦截");
    assert!(empty.name.is_empty());

    // 清理（删视频会级联 playlist_items，先删 playlist 再删其余）
    let _ = sqlx::query("DELETE FROM playlists WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await;
    db_cleanup(&pool, user_id, video_id).await;
}
