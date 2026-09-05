#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! 阅后即焚（burn after watch）集成测试 —— 平台全局行为。
//!
//! 需要 PostgreSQL（`DATABASE_URL`）。语义：
//! - 适用于**所有**视频（含存量视频，无 per-video 开关）
//! - 不区分用户：上传者本人观看同样触发
//! - 必须完整观看（播放进度 ≥90%；时长未知时有播放记录即可）
//! - 删除为物理级（主文件/变体/封面/缩略图）+ 数据库级联，不可恢复

mod integration_test_helpers;

use std::net::SocketAddr;
use std::sync::Arc;

use atmos_video_backend::app;
use atmos_video_backend::state::AppState;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use integration_test_helpers::*;
use serde_json::json;
use tower::ServiceExt;

async fn build_test_app() -> axum::Router {
    app::build_router(test_config())
        .await
        .layer(MockConnectInfo(
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        ))
}

async fn send_json(
    app: &axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", t));
    }
    let body = body
        .map(|v| Body::from(v.to_string()))
        .unwrap_or_else(Body::empty);
    let res = app
        .clone()
        .oneshot(
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

struct BurnFixture {
    state: Arc<AppState>,
    app: axum::Router,
    video_id: i64,
    uploader: (String, String, i64),
    viewer: (String, String, i64),
}

/// 普通视频（未做任何阅后即焚设置——全局语义下存量视频同样可焚毁）
async fn setup_video() -> BurnFixture {
    let state = test_app_state().await;
    let app = build_test_app().await;

    let uploader = create_test_user_with_credentials(&state, "burn_owner").await;
    let viewer = create_test_user_with_credentials(&state, "burn_view").await;
    let video_id = create_test_video_owned_by(&state, "burn", uploader.2).await;

    BurnFixture {
        state,
        app,
        video_id,
        uploader: (uploader.0, uploader.3, uploader.2),
        viewer: (viewer.0, viewer.3, viewer.2),
    }
}

fn hash_id(id: i64) -> String {
    atmos_video_backend::util::hashid::encode_id(id)
}

#[tokio::test]
async fn test_burn_requires_watch_progress() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let fx = setup_video().await;
    let id_param = hash_id(fx.video_id);

    // 无任何播放记录 → 403
    let (status, body) = send_json(
        &fx.app,
        Method::POST,
        &format!("/videos/{id_param}/burn"),
        Some(&fx.viewer.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert_eq!(body["error"], json!("需完整观看后才能焚毁"));

    // 进度 50% → 403；视频仍然存在
    fx.state
        .repos
        .playback
        .upsert_playback(1, &fx.viewer.0, fx.video_id, 50_000, 100_000)
        .await
        .expect("seed partial progress");
    let (status, body) = send_json(
        &fx.app,
        Method::POST,
        &format!("/videos/{id_param}/burn"),
        Some(&fx.viewer.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    let (status, _) = send_json(
        &fx.app,
        Method::GET,
        &format!("/videos/{id_param}"),
        Some(&fx.viewer.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "进度不足时视频必须仍存在");

    cleanup_test_user(fx.state.repos.video.pool(), &fx.viewer.0).await;
    cleanup_test_user(fx.state.repos.video.pool(), &fx.uploader.0).await;
    cleanup_test_video(fx.state.repos.video.pool(), fx.video_id).await;
}

#[tokio::test]
async fn test_burn_no_user_distinction_owner_also_burns() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let fx = setup_video().await;
    let id_param = hash_id(fx.video_id);

    // 上传者本人完整观看同样触发焚毁（不区分用户）
    fx.state
        .repos
        .playback
        .upsert_playback(1, &fx.uploader.0, fx.video_id, 100_000, 100_000)
        .await
        .expect("seed full progress for owner");
    let (status, _) = send_json(
        &fx.app,
        Method::POST,
        &format!("/videos/{id_param}/burn"),
        Some(&fx.uploader.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "上传者完整观看后必须焚毁");

    let (status, _) = send_json(
        &fx.app,
        Method::GET,
        &format!("/videos/{id_param}"),
        Some(&fx.uploader.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "焚毁后视频必须 404");

    cleanup_test_user(fx.state.repos.video.pool(), &fx.viewer.0).await;
    cleanup_test_user(fx.state.repos.video.pool(), &fx.uploader.0).await;
    cleanup_test_video(fx.state.repos.video.pool(), fx.video_id).await;
}

#[tokio::test]
async fn test_burn_requires_auth() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let fx = setup_video().await;
    let id_param = hash_id(fx.video_id);

    let (status, _) = send_json(
        &fx.app,
        Method::POST,
        &format!("/videos/{id_param}/burn"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    cleanup_test_user(fx.state.repos.video.pool(), &fx.viewer.0).await;
    cleanup_test_user(fx.state.repos.video.pool(), &fx.uploader.0).await;
    cleanup_test_video(fx.state.repos.video.pool(), fx.video_id).await;
}

#[tokio::test]
async fn test_burn_deletes_video_and_physical_files() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let fx = setup_video().await;
    let id_param = hash_id(fx.video_id);

    // 为该视频创建真实的本地物理文件（主文件 + 变体 + 封面），验证真删除
    let media_root = fx.state.config.media_root.clone();
    let main_rel = format!("burn_main_{}.mp4", fx.video_id);
    let variant_rel = format!("{}_720p.mp4", fx.video_id);
    let cover_rel = format!("burn_cover_{}.jpg", fx.video_id);
    for rel in [&main_rel, &variant_rel, &cover_rel] {
        let p = media_root.join(rel);
        std::fs::write(&p, b"payload").expect("write fake media file");
    }
    sqlx::query("UPDATE videos SET stream_url = $1, cover_url = $2 WHERE id = $3")
        .bind(format!("/media/{}", main_rel))
        .bind(format!("/media/{}", cover_rel))
        .bind(fx.video_id)
        .execute(fx.state.repos.video.pool())
        .await
        .expect("point video at fake local files");
    sqlx::query(
        "INSERT INTO video_variants (tenant_id, video_id, resolution, file_path, file_size, bitrate, created_at) \
         VALUES (1, $1, '720p', $2, 7, NULL, CURRENT_TIMESTAMP) \
         ON CONFLICT (video_id, resolution) DO UPDATE SET file_path = $2",
    )
    .bind(fx.video_id)
    .bind(format!("/media/{}", variant_rel))
    .execute(fx.state.repos.video.pool())
    .await
    .expect("insert fake variant row");

    // 观看者完整观看（95%）→ 204
    fx.state
        .repos
        .playback
        .upsert_playback(1, &fx.viewer.0, fx.video_id, 95_000, 100_000)
        .await
        .expect("seed full progress");
    let (status, body) = send_json(
        &fx.app,
        Method::POST,
        &format!("/videos/{id_param}/burn"),
        Some(&fx.viewer.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "body: {body}");

    // 数据库记录真正被删除：详情 404，二次焚毁 404
    let (status, _) = send_json(
        &fx.app,
        Method::GET,
        &format!("/videos/{id_param}"),
        Some(&fx.viewer.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "焚毁后详情必须 404");
    let (status, _) = send_json(
        &fx.app,
        Method::POST,
        &format!("/videos/{id_param}/burn"),
        Some(&fx.viewer.1),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "重复焚毁必须 404");

    // 物理文件真正被删除（主文件/变体/封面）
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    for rel in [&main_rel, &variant_rel, &cover_rel] {
        assert!(
            !media_root.join(rel).exists(),
            "物理文件 {rel} 必须被焚毁删除"
        );
    }

    // 播放历史级联删除
    let progress = fx
        .state
        .repos
        .playback
        .get_playback_data(1, &fx.viewer.0, fx.video_id)
        .await
        .expect("query playback after burn");
    assert!(progress.is_none(), "焚毁后播放历史必须级联删除");

    cleanup_test_user(fx.state.repos.video.pool(), &fx.viewer.0).await;
    cleanup_test_user(fx.state.repos.video.pool(), &fx.uploader.0).await;
    cleanup_test_video(fx.state.repos.video.pool(), fx.video_id).await;
}
