#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! 访客模式集成测试。
//!
//! 覆盖：访客影子账号创建 → 内容归属 → 与真实账号合并 → 影子账号删除。
//! 需要 `DATABASE_URL`，否则整体跳过。

mod integration_test_helpers;

use atmos_video_backend::handlers;
use atmos_video_backend::middleware::auth::AuthUser;
use atmos_video_backend::models::video::VideoQuery;
use atmos_video_backend::services::auth_service::AuthService;
use atmos_video_backend::state::AppState;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::Json;
use integration_test_helpers::*;
use std::sync::Arc;

fn owner_auth_user(id: i64, username: &str) -> AuthUser {
    AuthUser {
        id,
        username: username.to_string(),
        is_admin: false,
        role: 1,
        is_guest: false,
    }
}

/// 创建访客会话并返回 (user_id, username, token)。
async fn create_guest(state: &Arc<AppState>, ip: &str) -> (i64, String, String) {
    let svc = auth_service(state);
    let resp = svc
        .create_guest_session(ip)
        .await
        .expect("create guest session");
    assert!(resp.ok, "guest session should succeed: {:?}", resp.error);
    let token = resp.token.expect("guest token");
    let user = state
        .repos
        .user
        .find_user_by_token(&token)
        .await
        .expect("lookup guest token")
        .expect("guest token should resolve");
    assert!(user.is_guest, "resolved user must be a guest");
    assert!(user.approved, "guest must be auto-approved");
    assert_eq!(user.role, 1);
    (user.id, user.username, token)
}

#[tokio::test]
async fn guest_session_created_and_scoped() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (guest_id, guest_username, _token) = create_guest(&state, "127.0.0.1").await;

    // 访客上传内容归属影子账号：列表只见自己的
    let video_id = create_test_video_owned_by(&state, "guestvid", guest_id).await;

    let resp = handlers::videos::list_videos(
        State(state.clone()),
        Extension(owner_auth_user(guest_id, &guest_username)),
        Query(VideoQuery {
            query: None,
            source_type: None,
            category: None,
            page: None,
            size: None,
            uploader_id: None,
            sort: None,
        }),
    )
    .await
    .expect("guest list should succeed");
    let (_, _, Json(page)) = resp;
    assert_eq!(page.total, 1, "guest should only see own upload");
    assert_eq!(page.items[0].id, video_id);

    // 他人视角看不到访客的视频（404）
    let (other_name, other_id) = create_test_user(&state, "guestoth").await;
    let res = handlers::videos::get_video(
        State(state.clone()),
        Extension(owner_auth_user(other_id, &other_name)),
        axum::extract::Path(video_id.to_string()),
    )
    .await;
    assert!(res.is_err(), "other user must not see guest's video");
    let (status, _) = res.unwrap_err();
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 访客自己能看到
    let res = handlers::videos::get_video(
        State(state.clone()),
        Extension(owner_auth_user(guest_id, &guest_username)),
        axum::extract::Path(video_id.to_string()),
    )
    .await;
    assert!(res.is_ok(), "owner must see own video: {:?}", res.err());

    cleanup_test_video(state.repos.video.pool(), video_id).await;
    cleanup_test_user(state.repos.video.pool(), &other_name).await;
    cleanup_test_user(state.repos.video.pool(), &guest_username).await;
}

#[tokio::test]
async fn guest_content_merges_into_real_account() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let pool = state.repos.video.pool();
    let (guest_id, guest_username, _token) = create_guest(&state, "127.0.0.1").await;
    let video_id = create_test_video_owned_by(&state, "mergvid", guest_id).await;

    // 访客的播放历史（按用户名关联）
    sqlx::query(
        "INSERT INTO playback_history (username, video_id, position_ms, duration_ms) \
         VALUES ($1, $2, 1000, 60000) ON CONFLICT DO NOTHING",
    )
    .bind(&guest_username)
    .bind(video_id)
    .execute(pool)
    .await
    .expect("insert guest playback history");

    // 真实账号注册（直接建行 + token），登录后应合并访客内容
    let (real_name, real_id) = create_test_user(&state, "mergreal").await;

    let merged = state
        .repos
        .user
        .merge_guest_into_user(guest_id, real_id)
        .await
        .expect("merge guest");
    assert_eq!(merged, 1, "one video should be merged");

    // 视频归属迁移
    let row: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT uploader_id FROM videos WHERE id = $1")
            .bind(video_id)
            .fetch_optional(pool)
            .await
            .expect("fetch video");
    assert_eq!(row.expect("video exists").0, Some(real_id));

    // 播放历史用户名迁移
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM playback_history WHERE username = $1 AND video_id = $2",
    )
    .bind(&real_name)
    .bind(video_id)
    .fetch_one(pool)
    .await
    .expect("count history");
    assert_eq!(n.0, 1, "playback history should follow the real username");

    // 影子账号行已删除
    let gone: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE id = $1")
        .bind(guest_id)
        .fetch_one(pool)
        .await
        .expect("count guest rows");
    assert_eq!(gone.0, 0, "guest shadow row must be deleted");

    cleanup_test_video(pool, video_id).await;
    cleanup_test_user(pool, &real_name).await;
}

#[tokio::test]
async fn guest_password_hash_blocks_login() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let (_, guest_username, _) = create_guest(&state, "127.0.0.1").await;

    // 空密码哈希 + 任意密码 → 登录失败（影子账号不可被密码登录）
    let svc = AuthService::new(
        state.repos.user.clone(),
        state.services.playback.clone(),
        state.rate_limiter.clone(),
        state.ip_rate_limiter.clone(),
        test_config(),
    );
    let resp = svc
        .login(
            &atmos_video_backend::models::auth::AuthRequest {
                username: guest_username.clone(),
                password: "whatever-password-1A!".into(),
            },
            "127.0.0.1",
        )
        .await
        .expect("login should not error");
    assert!(!resp.ok, "guest account must not be password-loginable");

    cleanup_test_user(state.repos.video.pool(), &guest_username).await;
}
