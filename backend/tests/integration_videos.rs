//! Integration tests for video operations.
//!
//! Requires a running PostgreSQL database. Set `DATABASE_URL` to enable.

mod integration_test_helpers;

use atmos_video_backend::handlers;
use atmos_video_backend::middleware::auth::AuthUser;
use atmos_video_backend::models::video::VideoQuery;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use integration_test_helpers::*;
use std::collections::HashSet;
use std::net::SocketAddr;

// ── Add external video ──

#[tokio::test]
async fn test_add_external_video() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let title = format!("Test Video {}", unique_username("vid"));

    let id = state
        .services
        .video
        .add_external_video(
            &title,
            Some("A test video"),
            Some("test"),
            "https://example.com/video.mp4",
            Some("https://example.com/cover.jpg"),
            None,
        )
        .await
        .expect("add_external_video");

    assert!(id > 0, "video id should be positive");

    // Verify it can be fetched
    let video = state
        .services
        .video
        .get_video(id)
        .await
        .expect("get_video")
        .expect("video should exist");

    assert_eq!(video.title, title);
    assert_eq!(video.source_type, "external");
    assert_eq!(video.stream_url, "https://example.com/video.mp4");
    assert_eq!(
        video.cover_url,
        Some("https://example.com/cover.jpg".into())
    );
    assert_eq!(video.category, "test");

    cleanup_test_video(state.repos.video.pool(), id).await;
}

// ── List videos with pagination ──

#[tokio::test]
async fn test_list_videos_pagination() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let tag = unique_username("page");

    // Insert 3 test videos
    let mut ids = Vec::new();
    for i in 0..3 {
        let id = state
            .services
            .video
            .add_external_video(
                &format!("Pagination Test {} - {}", tag, i),
                Some("pagination test"),
                Some("pagetest"),
                &format!("https://example.com/{}.mp4", i),
                None,
                None,
            )
            .await
            .expect("add video");
        ids.push(id);
    }

    // List with size=2, page=0
    let (items_page0, total) = state
        .services
        .video
        .list_videos_paged(0, 2, Some(&tag), None, None, None, None, None)
        .await
        .expect("list page 0");

    assert!(total >= 3, "total should be at least 3, got {}", total);
    assert_eq!(items_page0.len(), 2, "page 0 should have 2 items");

    // List with size=2, page=1
    let (items_page1, total2) = state
        .services
        .video
        .list_videos_paged(1, 2, Some(&tag), None, None, None, None, None)
        .await
        .expect("list page 1");

    assert_eq!(total, total2, "total should be consistent across pages");
    assert_eq!(items_page1.len(), 1, "page 1 should have 1 item");

    // Ensure no overlap between pages
    let page0_ids: Vec<i64> = items_page0.iter().map(|v| v.id).collect();
    let page1_ids: Vec<i64> = items_page1.iter().map(|v| v.id).collect();
    for id in &page1_ids {
        assert!(
            !page0_ids.contains(id),
            "page 1 id {} should not appear in page 0",
            id
        );
    }

    for id in ids {
        cleanup_test_video(state.repos.video.pool(), id).await;
    }
}

// ── Video search ──

#[tokio::test]
async fn test_video_search() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let unique = unique_username("search");

    let id = state
        .services
        .video
        .add_external_video(
            &format!("Searchable Title {}", unique),
            Some("search test"),
            Some("searchtest"),
            "https://example.com/search.mp4",
            None,
            None,
        )
        .await
        .expect("add video");

    // Search by unique substring
    let (results, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&unique), None, None, None, None, None)
        .await
        .expect("search");

    assert!(
        results.iter().any(|v| v.id == id),
        "search results should contain the video we just created"
    );

    // Search for something that doesn't exist
    let (empty, _) = state
        .services
        .video
        .list_videos_paged(
            0,
            10,
            Some("zzz_nonexistent_query_zzz"),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("empty search");

    assert!(
        empty.is_empty(),
        "search for nonexistent string should return empty"
    );

    cleanup_test_video(state.repos.video.pool(), id).await;
}

// ── Toggle like (atomic CTE) ──

#[tokio::test]
async fn test_toggle_like() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let username = unique_username("like");
    let video_id = create_test_video(&state, "like").await;

    // Initially not liked
    let liked = state
        .services
        .playback
        .is_liked(&username, video_id)
        .await
        .expect("is_liked");
    assert!(!liked, "should not be liked initially");

    // Toggle on → liked
    let liked = state
        .services
        .playback
        .toggle_like(&username, video_id)
        .await
        .expect("toggle_like");
    assert!(liked, "should be liked after first toggle");

    // Verify
    let liked = state
        .services
        .playback
        .is_liked(&username, video_id)
        .await
        .expect("is_liked");
    assert!(liked, "is_liked should return true after toggle on");

    // Toggle off → not liked
    let liked = state
        .services
        .playback
        .toggle_like(&username, video_id)
        .await
        .expect("toggle_like");
    assert!(!liked, "should not be liked after second toggle");

    // Verify
    let liked = state
        .services
        .playback
        .is_liked(&username, video_id)
        .await
        .expect("is_liked");
    assert!(!liked, "is_liked should return false after toggle off");

    // Cleanup
    cleanup_like(state.repos.video.pool(), &username, video_id).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

// ── Toggle favorite (atomic CTE) ──

#[tokio::test]
async fn test_toggle_favorite() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let username = unique_username("fav");
    let video_id = create_test_video(&state, "fav").await;

    // Initially not favorited
    let fav = state
        .services
        .playback
        .is_favorited(&username, video_id)
        .await
        .expect("is_favorited");
    assert!(!fav, "should not be favorited initially");

    // Toggle on
    let fav = state
        .services
        .playback
        .toggle_favorite(&username, video_id)
        .await
        .expect("toggle_favorite");
    assert!(fav, "should be favorited after first toggle");

    // Verify
    let fav = state
        .services
        .playback
        .is_favorited(&username, video_id)
        .await
        .expect("is_favorited");
    assert!(fav, "is_favorited should return true");

    // Toggle off
    let fav = state
        .services
        .playback
        .toggle_favorite(&username, video_id)
        .await
        .expect("toggle_favorite");
    assert!(!fav, "should not be favorited after second toggle");

    cleanup_favorite(state.repos.video.pool(), &username, video_id).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

// ── Playback history ──

#[tokio::test]
async fn test_playback_history() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let username = unique_username("playback");
    let video_id = create_test_video(&state, "playback").await;

    // Initially no history
    let data = state
        .services
        .playback
        .get_playback_data(&username, video_id)
        .await
        .expect("get data");
    assert!(data.is_none(), "should have no playback data initially");

    // Update playback
    state
        .services
        .playback
        .update_playback(&username, video_id, 30_000, 120_000)
        .await
        .expect("update_playback");

    // Verify position
    let (position, duration) = state
        .services
        .playback
        .get_playback_data(&username, video_id)
        .await
        .expect("get data")
        .unwrap();
    assert_eq!(position, 30_000, "position should be 30000ms");
    assert_eq!(duration, 120_000, "duration should be 120000ms");

    // Update again (upsert) — 10 秒节流窗口内的第二次上报会被合并
    state
        .services
        .playback
        .update_playback(&username, video_id, 60_000, 120_000)
        .await
        .expect("update_playback again");

    let (position, _) = state
        .services
        .playback
        .get_playback_data(&username, video_id)
        .await
        .expect("get data after update")
        .unwrap();
    assert_eq!(
        position, 30_000,
        "throttled update within 10s window should keep first persisted value"
    );

    // 窗口过后上报应落库
    tokio::time::sleep(std::time::Duration::from_secs(11)).await;
    state
        .services
        .playback
        .update_playback(&username, video_id, 60_000, 120_000)
        .await
        .expect("update_playback after window");
    let (position, _) = state
        .services
        .playback
        .get_playback_data(&username, video_id)
        .await
        .expect("get data after throttle window")
        .unwrap();
    assert_eq!(
        position, 60_000,
        "position should be updated to 60000ms after throttle window"
    );

    // Check history list
    let history = state
        .services
        .playback
        .get_playback_history(&username, 50)
        .await
        .expect("get history");

    assert!(
        history.iter().any(|h| h.video_id == video_id),
        "history should contain the video"
    );

    // Cleanup
    cleanup_playback(state.repos.video.pool(), &username).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

// ── Increment views ──

#[tokio::test]
async fn test_increment_views() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let video_id = create_test_video(&state, "views").await;

    // Get initial views
    let video = state
        .services
        .video
        .get_video(video_id)
        .await
        .expect("get_video")
        .expect("video exists");
    let initial_views = video.views;

    // Increment
    state
        .services
        .video
        .increment_views(video_id)
        .await
        .expect("increment_views");

    let video = state
        .services
        .video
        .get_video(video_id)
        .await
        .expect("get_video")
        .expect("video exists");

    assert_eq!(video.views, initial_views + 1, "views should increase by 1");

    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

// ── Helper functions ──

/// Create a test external video and return its ID.
async fn create_test_video(state: &atmos_video_backend::state::AppState, prefix: &str) -> i64 {
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    state
        .services
        .video
        .add_external_video(
            &format!("{} Video {}", prefix, unique_username(prefix)),
            Some("integration test"),
            Some("integration"),
            &format!("https://example.com/{}.mp4", unique_username(prefix)),
            None,
            None,
        )
        .await
        .expect("create test video")
}

/// 本地开发库通常没有安装 zhparser 扩展，而迁移 021 的触发器在每次
/// INSERT/UPDATE videos 时调用 to_tsvector('chinese', ...)，会报
/// "text search configuration chinese does not exist"。幂等地从内置 simple
/// 配置复制一个 chinese 配置即可让测试运行；生产库装有 zhparser，此调用
/// 是 no-op。DO 块保证重复执行安全（PG 不支持 CREATE TS CONFIG IF NOT EXISTS，
/// 且已存在时抛的是 unique_violation 而非 duplicate_object）。
async fn ensure_chinese_ts_config(pool: &sqlx::PgPool) {
    sqlx::raw_sql(
        "DO $$ BEGIN
           CREATE TEXT SEARCH CONFIGURATION chinese (COPY = pg_catalog.simple);
         EXCEPTION
           WHEN duplicate_object OR unique_violation THEN NULL;
         END $$",
    )
    .execute(pool)
    .await
    .expect("create chinese text search configuration");
}

/// 直接插入一个已审批的测试用户（uploader 等场景需要真实 users 行）。
/// 返回 (id, username)。
async fn create_test_user(pool: &sqlx::PgPool, prefix: &str) -> (i64, String) {
    let username = unique_username(prefix);
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO users (username, password_hash, approved, role) \
         VALUES ($1, 'test-hash', true, 1) RETURNING id",
    )
    .bind(&username)
    .fetch_one(pool)
    .await
    .expect("create test user");
    (id, username)
}

/// 构造一个 role=1 的测试 AuthUser，用于直接调用需要认证的 handler。
fn test_auth_user(id: i64, username: &str) -> AuthUser {
    AuthUser {
        id,
        username: username.to_string(),
        is_admin: false,
        role: 1,
        tenant_id: 1,
    }
}

/// 列出 media_root 中的文件，用于清理上传测试留下的文件。
fn list_media_files(dir: &std::path::Path) -> HashSet<std::path::PathBuf> {
    let mut set = HashSet::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            set.insert(e.path());
        }
    }
    set
}

async fn cleanup_like(pool: &sqlx::PgPool, username: &str, video_id: i64) {
    let _ = sqlx::query("DELETE FROM user_likes WHERE username = $1 AND video_id = $2")
        .bind(username)
        .bind(video_id)
        .execute(pool)
        .await;
}

async fn cleanup_favorite(pool: &sqlx::PgPool, username: &str, video_id: i64) {
    let _ = sqlx::query("DELETE FROM user_favorites WHERE username = $1 AND video_id = $2")
        .bind(username)
        .bind(video_id)
        .execute(pool)
        .await;
}

async fn cleanup_playback(pool: &sqlx::PgPool, username: &str) {
    let _ = sqlx::query("DELETE FROM playback_history WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await;
}

// ── Pagination: negative / huge pages ──

#[tokio::test]
async fn test_list_videos_pagination_edge_pages() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let tag = unique_username("edge");

    let mut ids = Vec::new();
    for i in 0..3 {
        let id = state
            .services
            .video
            .add_external_video(
                &format!("Edge Page {} - {}", tag, i),
                Some("edge"),
                Some("edgetest"),
                &format!("https://example.com/edge_{}_{}.mp4", tag, i),
                None,
                None,
            )
            .await
            .expect("add video");
        ids.push(id);
    }

    // page=0 → 正常
    let (items, total) = state
        .services
        .video
        .list_videos_paged(0, 2, Some(&tag), None, None, None, None, None)
        .await
        .expect("page 0");
    assert_eq!(items.len(), 2);
    assert!(total >= 3);

    // 负 page → PostgreSQL 拒绝负 OFFSET，服务层必须返回错误
    // （clamp 是 handler 的职责，handler 测试里验证了 clamp 行为）
    let res = state
        .services
        .video
        .list_videos_paged(-1, 2, Some(&tag), None, None, None, None, None)
        .await;
    assert!(
        res.is_err(),
        "负 page 应返回错误（负 OFFSET），而不是 panic 或返回数据"
    );

    // 超大 page：saturating_mul 保证 OFFSET 不溢出，返回空列表而非 panic
    let (items, total2) = state
        .services
        .video
        .list_videos_paged(
            1_000_000_000_000,
            2,
            Some(&tag),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("huge page should not error");
    assert!(items.is_empty(), "超大 page 应返回空列表");
    assert_eq!(total, total2, "total 不应受 page 影响");

    for id in ids {
        cleanup_test_video(state.repos.video.pool(), id).await;
    }
}

#[tokio::test]
async fn test_list_videos_pagination_size_bounds() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let tag = unique_username("size");

    let mut ids = Vec::new();
    for i in 0..3 {
        let id = state
            .services
            .video
            .add_external_video(
                &format!("Size Bounds {} - {}", tag, i),
                Some("size"),
                Some("sizetest"),
                &format!("https://example.com/size_{}_{}.mp4", tag, i),
                None,
                None,
            )
            .await
            .expect("add video");
        ids.push(id);
    }

    // size=0 → LIMIT 0，返回空列表，total 不受影响
    let (items, total) = state
        .services
        .video
        .list_videos_paged(0, 0, Some(&tag), None, None, None, None, None)
        .await
        .expect("size 0");
    assert!(items.is_empty(), "size=0 应返回空列表");
    assert!(total >= 3);

    // 负 size → LIMIT must not be negative → 错误
    let res = state
        .services
        .video
        .list_videos_paged(0, -1, Some(&tag), None, None, None, None, None)
        .await;
    assert!(res.is_err(), "负 size 应返回错误");

    // 超大 size → 返回全部匹配行
    let (items, total2) = state
        .services
        .video
        .list_videos_paged(0, 100_000, Some(&tag), None, None, None, None, None)
        .await
        .expect("huge size");
    assert_eq!(items.len(), 3, "超大 size 应返回全部 3 条");
    assert_eq!(total, total2);

    for id in ids {
        cleanup_test_video(state.repos.video.pool(), id).await;
    }
}

// ── Sorting: default / views / id / title ──

#[tokio::test]
async fn test_list_videos_sort_default_and_views() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let tag = unique_username("sort");

    // 依次创建 A、B、C；给 B 增加 2 次浏览量
    let id_a = state
        .services
        .video
        .add_external_video(
            &format!("sort_a_{}", tag),
            None,
            None,
            &format!("https://example.com/sa_{}.mp4", tag),
            None,
            None,
        )
        .await
        .expect("video a");
    let id_b = state
        .services
        .video
        .add_external_video(
            &format!("sort_b_{}", tag),
            None,
            None,
            &format!("https://example.com/sb_{}.mp4", tag),
            None,
            None,
        )
        .await
        .expect("video b");
    let id_c = state
        .services
        .video
        .add_external_video(
            &format!("sort_c_{}", tag),
            None,
            None,
            &format!("https://example.com/sc_{}.mp4", tag),
            None,
            None,
        )
        .await
        .expect("video c");
    for _ in 0..2 {
        state
            .services
            .video
            .increment_views(id_b)
            .await
            .expect("increment views b");
    }

    // 默认排序：浏览量降序优先，同分按 id 降序 → B, C, A
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&tag), None, None, None, None, None)
        .await
        .expect("default sort");
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].id, id_b, "默认排序应按浏览量降序，B 应排第一");
    assert_eq!(items[1].id, id_c, "同分（0 浏览）按 id 降序");
    assert_eq!(items[2].id, id_a);

    // views_asc：浏览量升序 → A, C, B
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&tag), None, None, None, None, Some("views_asc"))
        .await
        .expect("views_asc");
    assert_eq!(items[2].id, id_b, "views_asc 时 B 应排最后");

    // id / id_desc：最新创建在前
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&tag), None, None, None, None, Some("id"))
        .await
        .expect("id sort");
    assert_eq!(items[0].id, id_c, "id 排序应最新在前");
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&tag), None, None, None, None, Some("id_desc"))
        .await
        .expect("id_desc");
    assert_eq!(items[0].id, id_c);

    // id_asc：最早创建在前
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&tag), None, None, None, None, Some("id_asc"))
        .await
        .expect("id_asc");
    assert_eq!(items[0].id, id_a, "id_asc 应最早创建在前");

    // 未知排序值 → 回退到默认（浏览量降序）
    let (items, _) = state
        .services
        .video
        .list_videos_paged(
            0,
            10,
            Some(&tag),
            None,
            None,
            None,
            None,
            Some("bogus_sort"),
        )
        .await
        .expect("unknown sort");
    assert_eq!(items[0].id, id_b, "未知排序应回退到默认排序");

    for id in [id_a, id_b, id_c] {
        cleanup_test_video(state.repos.video.pool(), id).await;
    }
}

#[tokio::test]
async fn test_list_videos_sort_title() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let tag = unique_username("tsort");

    let id_a = state
        .services
        .video
        .add_external_video(
            &format!("aaa_title_{}", tag),
            None,
            None,
            &format!("https://example.com/ta_{}.mp4", tag),
            None,
            None,
        )
        .await
        .expect("video a");
    let id_m = state
        .services
        .video
        .add_external_video(
            &format!("mmm_title_{}", tag),
            None,
            None,
            &format!("https://example.com/tm_{}.mp4", tag),
            None,
            None,
        )
        .await
        .expect("video m");
    let id_z = state
        .services
        .video
        .add_external_video(
            &format!("zzz_title_{}", tag),
            None,
            None,
            &format!("https://example.com/tz_{}.mp4", tag),
            None,
            None,
        )
        .await
        .expect("video z");

    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&tag), None, None, None, None, Some("title_asc"))
        .await
        .expect("title_asc");
    assert_eq!(items[0].id, id_a, "title_asc 应 aaa 在前");
    assert_eq!(items[2].id, id_z, "title_asc 应 zzz 在后");

    let (items, _) = state
        .services
        .video
        .list_videos_paged(
            0,
            10,
            Some(&tag),
            None,
            None,
            None,
            None,
            Some("title_desc"),
        )
        .await
        .expect("title_desc");
    assert_eq!(items[0].id, id_z, "title_desc 应 zzz 在前");

    for id in [id_a, id_m, id_z] {
        cleanup_test_video(state.repos.video.pool(), id).await;
    }
}

// ── Search: empty / long / special-character queries ──

#[tokio::test]
async fn test_video_search_edge_queries() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let unique = unique_username("search_edge");
    let id = create_test_video(&state, "search_edge").await;

    // 空字符串查询 → 不报错，空结果（plainto_tsquery 空查询匹配不到任何行）
    let (items, total) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(""), None, None, None, None, None)
        .await
        .expect("empty query");
    assert!(items.is_empty(), "空查询应返回空列表");
    assert_eq!(total, 0);

    // 纯空白查询
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some("   "), None, None, None, None, None)
        .await
        .expect("whitespace query");
    assert!(items.is_empty(), "纯空白查询应返回空列表");

    // 超长关键词（500 字符）→ 不报错，空结果
    let long = "x".repeat(500);
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&long), None, None, None, None, None)
        .await
        .expect("long query");
    assert!(items.is_empty(), "超长查询应返回空列表");

    // 特殊字符 → 不报错（参数化查询，无注入风险）
    let specials = [
        "%",
        "_",
        "*",
        "'",
        "\"",
        "\\",
        "--",
        ";",
        "&&",
        "|",
        "a%_b",
        "!@#$^&*()",
    ];
    for s in specials {
        let (items, _) = state
            .services
            .video
            .list_videos_paged(0, 10, Some(s), None, None, None, None, None)
            .await
            .unwrap_or_else(|e| panic!("特殊字符查询 {:?} 不应报错: {}", s, e));
        assert!(
            items.is_empty(),
            "特殊字符查询 {:?} 应返回空结果（不命中任何视频）",
            s
        );
    }

    // SQL 注入尝试 → 不应泄露出目标视频
    let injection = format!("{}' OR 1=1 --", unique);
    let (items, _) = state
        .services
        .video
        .list_videos_paged(0, 10, Some(&injection), None, None, None, None, None)
        .await
        .expect("injection query");
    assert!(
        !items.iter().any(|v| v.id == id),
        "注入尝试不应返回目标视频"
    );

    cleanup_test_video(state.repos.video.pool(), id).await;
}

// ── Detail: nonexistent / invalid ids ──

#[tokio::test]
async fn test_get_video_nonexistent_ids() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;

    for bad in [0, -1, -999, 999_999_999_999, i64::MAX] {
        let res = state
            .services
            .video
            .get_video(bad)
            .await
            .expect("get_video should not error");
        assert!(res.is_none(), "id {} 不应存在", bad);
    }
}

#[tokio::test]
async fn test_get_video_handler_invalid_ids() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;

    // 非数字 / 非法 ID → 400
    for bad in ["abc", "12abc", "", "1.5", " 12", "12 "] {
        let res = handlers::videos::get_video(State(state.clone()), Path(bad.to_string())).await;
        match res {
            Err((status, _)) => {
                assert_eq!(status, StatusCode::BAD_REQUEST, "id {:?} 应 400", bad)
            }
            Ok(_) => panic!("id {:?} 应返回错误", bad),
        }
    }

    // 合法数字但视频不存在 → 404
    let res = handlers::videos::get_video(State(state.clone()), Path("999999999999".into())).await;
    let (status, _) = res.expect_err("不存在 id 应 404");
    assert_eq!(status, StatusCode::NOT_FOUND);

    // hashid 编码的合法 id → 200 并返回正确视频
    let id = create_test_video(&state, "hashid").await;
    let hash = atmos_video_backend::util::hashid::encode_id(id);
    let res = handlers::videos::get_video(State(state.clone()), Path(hash)).await;
    let Json(video) = res.expect("hashid 解码应成功");
    assert_eq!(video.id, id, "hashid 解码后应拿到同一视频");

    cleanup_test_video(state.repos.video.pool(), id).await;
}

// ── Views increment ──

#[tokio::test]
async fn test_increment_views_multiple_and_missing() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let video_id = create_test_video(&state, "views3").await;

    for _ in 0..3 {
        state
            .services
            .video
            .increment_views(video_id)
            .await
            .expect("increment views");
    }
    let video = state
        .services
        .video
        .get_video(video_id)
        .await
        .expect("get_video")
        .expect("video exists");
    assert_eq!(video.views, 3, "连续递增 3 次后浏览量应为 3");

    // 递增不存在的视频 → no-op，不报错
    state
        .services
        .video
        .increment_views(999_999_999_999)
        .await
        .expect("increment on missing id should be a no-op");

    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

#[tokio::test]
async fn test_increment_views_handler_invalid_id() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let addr: SocketAddr = "127.0.0.1:54321".parse().unwrap();
    let res = handlers::videos::increment_views(
        State(state.clone()),
        Path("abc".to_string()),
        ConnectInfo(addr),
    )
    .await;
    let (status, _) = res.expect_err("非法 id 应返回错误");
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Like / favorite: multiple users + handler invalid ids ──

#[tokio::test]
async fn test_like_multiple_users_independent() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let video_id = create_test_video(&state, "likemulti").await;
    let u1 = unique_username("like_u1");
    let u2 = unique_username("like_u2");

    assert!(state
        .services
        .playback
        .toggle_like(&u1, video_id)
        .await
        .expect("like u1"));
    assert!(state
        .services
        .playback
        .toggle_like(&u2, video_id)
        .await
        .expect("like u2"));
    assert!(state
        .services
        .playback
        .is_liked(&u1, video_id)
        .await
        .expect("check u1"));
    assert!(state
        .services
        .playback
        .is_liked(&u2, video_id)
        .await
        .expect("check u2"));

    // u1 取消点赞不影响 u2
    assert!(!state
        .services
        .playback
        .toggle_like(&u1, video_id)
        .await
        .expect("unlike u1"));
    assert!(!state
        .services
        .playback
        .is_liked(&u1, video_id)
        .await
        .expect("check u1 after unlike"));
    assert!(state
        .services
        .playback
        .is_liked(&u2, video_id)
        .await
        .expect("check u2 unaffected"));

    cleanup_like(state.repos.video.pool(), &u1, video_id).await;
    cleanup_like(state.repos.video.pool(), &u2, video_id).await;
    cleanup_test_video(state.repos.video.pool(), video_id).await;
}

#[tokio::test]
async fn test_toggle_like_missing_video_errors() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let u = unique_username("like_ghost");
    // user_likes.video_id 有外键约束：点赞不存在的视频应报错而非静默成功
    let res = state
        .services
        .playback
        .toggle_like(&u, 999_999_999_999)
        .await;
    assert!(res.is_err(), "对不存在的视频点赞应因外键约束报错");
}

#[tokio::test]
async fn test_favorites_list_and_removal() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let u = unique_username("favlist");
    let v1 = create_test_video(&state, "fav1").await;
    let v2 = create_test_video(&state, "fav2").await;

    assert!(state
        .services
        .playback
        .toggle_favorite(&u, v1)
        .await
        .expect("fav v1"));
    assert!(state
        .services
        .playback
        .toggle_favorite(&u, v2)
        .await
        .expect("fav v2"));

    let favs = state
        .services
        .playback
        .get_favorites(&u)
        .await
        .expect("get favorites");
    assert!(favs.iter().any(|f| f.video_id == v1));
    assert!(favs.iter().any(|f| f.video_id == v2));

    // 取消收藏一个
    assert!(!state
        .services
        .playback
        .toggle_favorite(&u, v1)
        .await
        .expect("unfav v1"));
    let favs = state
        .services
        .playback
        .get_favorites(&u)
        .await
        .expect("get favorites after unfav");
    assert!(
        !favs.iter().any(|f| f.video_id == v1),
        "取消收藏后不应再出现 v1"
    );
    assert!(favs.iter().any(|f| f.video_id == v2));

    cleanup_favorite(state.repos.video.pool(), &u, v1).await;
    cleanup_favorite(state.repos.video.pool(), &u, v2).await;
    cleanup_test_video(state.repos.video.pool(), v1).await;
    cleanup_test_video(state.repos.video.pool(), v2).await;
}

#[tokio::test]
async fn test_like_favorite_handlers_reject_invalid_ids() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let user = test_auth_user(1, "handler_like");

    // toggle_like
    let res = handlers::videos::toggle_like(
        State(state.clone()),
        Extension(user.clone()),
        Path("not-an-id".to_string()),
    )
    .await;
    assert_eq!(
        res.expect_err("非法 id 应返回错误").0,
        StatusCode::BAD_REQUEST
    );

    // get_like_status
    let res = handlers::videos::get_like_status(
        State(state.clone()),
        Extension(user.clone()),
        Path("not-an-id".to_string()),
    )
    .await;
    assert_eq!(
        res.expect_err("非法 id 应返回错误").0,
        StatusCode::BAD_REQUEST
    );

    // toggle_favorite
    let res = handlers::videos::toggle_favorite(
        State(state.clone()),
        Extension(user.clone()),
        Path("not-an-id".to_string()),
    )
    .await;
    assert_eq!(
        res.expect_err("非法 id 应返回错误").0,
        StatusCode::BAD_REQUEST
    );

    // get_favorite_status
    let res = handlers::videos::get_favorite_status(
        State(state.clone()),
        Extension(user.clone()),
        Path("not-an-id".to_string()),
    )
    .await;
    assert_eq!(
        res.expect_err("非法 id 应返回错误").0,
        StatusCode::BAD_REQUEST
    );
}

// ── Handler layer: pagination clamping and query length ──

#[tokio::test]
async fn test_list_videos_handler_clamps_pagination() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let user = test_auth_user(1, "list_handler");

    // 负 page / 负 size → clamp 到 page=0, size=1
    let q = VideoQuery {
        query: None,
        source_type: None,
        category: None,
        page: Some(-5),
        size: Some(-1),
        uploader_id: None,
        sort: None,
    };
    let (status, _, Json(resp)) =
        handlers::videos::list_videos(State(state.clone()), Extension(user.clone()), Query(q))
            .await
            .expect("clamped request 应成功");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp.page, 0, "负 page 应被 clamp 到 0");
    assert_eq!(resp.size, 1, "负 size 应被 clamp 到 1");

    // 超大 page / size → clamp 到上限（MAX_PAGE=1_000_000, size 上限 1000）
    let q = VideoQuery {
        query: None,
        source_type: None,
        category: None,
        page: Some(99_999_999_999),
        size: Some(999_999),
        uploader_id: None,
        sort: None,
    };
    let (_, _, Json(resp)) =
        handlers::videos::list_videos(State(state.clone()), Extension(user.clone()), Query(q))
            .await
            .expect("huge request 应成功");
    assert_eq!(resp.page, 1_000_000, "超大 page 应 clamp 到上限");
    assert_eq!(resp.size, 1000, "超大 size 应 clamp 到 1000");

    // 缺省值 → page=0, size=20
    let q = VideoQuery {
        query: None,
        source_type: None,
        category: None,
        page: None,
        size: None,
        uploader_id: None,
        sort: None,
    };
    let (_, _, Json(resp)) =
        handlers::videos::list_videos(State(state.clone()), Extension(user.clone()), Query(q))
            .await
            .expect("default request");
    assert_eq!(resp.page, 0);
    assert_eq!(resp.size, 20);

    // 查询词超过 200 字符 → 400
    let q = VideoQuery {
        query: Some("x".repeat(201)),
        source_type: None,
        category: None,
        page: None,
        size: None,
        uploader_id: None,
        sort: None,
    };
    let res =
        handlers::videos::list_videos(State(state.clone()), Extension(user.clone()), Query(q))
            .await;
    let (status, _) = res.expect_err("超长查询词应 400");
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Handler layer: search endpoint ──

#[tokio::test]
async fn test_search_videos_handler_edges() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let unique = unique_username("search_h");
    // 注意：必须用同一个 unique 构造标题，create_test_video 内部会生成
    // 另一个独立值，导致搜索不到
    let id = state
        .services
        .video
        .add_external_video(
            &format!("Searchable Title {}", unique),
            Some("search handler test"),
            Some("searchtest"),
            &format!("https://example.com/sh_{}.mp4", unique),
            None,
            None,
        )
        .await
        .expect("add video");

    // 空查询 → 短路空结果（200）
    let q = atmos_video_backend::models::video::SearchQuery {
        q: String::new(),
        page: None,
        size: None,
    };
    let Json(resp) = handlers::videos::search_videos(State(state.clone()), Query(q))
        .await
        .expect("empty q");
    assert_eq!(resp.total, 0);
    assert!(resp.items.is_empty());

    // 纯空白 + 负 page/size → 空结果，且 page/size 被 clamp
    let q = atmos_video_backend::models::video::SearchQuery {
        q: "   ".into(),
        page: Some(-3),
        size: Some(-7),
    };
    let Json(resp) = handlers::videos::search_videos(State(state.clone()), Query(q))
        .await
        .expect("whitespace q");
    assert_eq!(resp.total, 0);
    assert_eq!(resp.page, 0, "负 page 应 clamp 到 0");
    assert_eq!(resp.size, 1, "负 size 应 clamp 到 1");

    // 查询词超过 200 字符 → 400
    let q = atmos_video_backend::models::video::SearchQuery {
        q: "a".repeat(201),
        page: None,
        size: None,
    };
    let res = handlers::videos::search_videos(State(state.clone()), Query(q)).await;
    match res {
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
        Ok(_) => panic!("超长搜索词应 400"),
    }

    // 正常命中
    let q = atmos_video_backend::models::video::SearchQuery {
        q: unique.clone(),
        page: Some(0),
        size: Some(10),
    };
    let Json(resp) = handlers::videos::search_videos(State(state.clone()), Query(q))
        .await
        .expect("matching q");
    assert!(resp.total >= 1, "搜索应命中刚创建的视频");
    assert!(
        resp.items.iter().any(|i| i.id == id),
        "搜索结果应包含目标视频"
    );

    // 特殊字符 → 200 空结果，不报错
    let q = atmos_video_backend::models::video::SearchQuery {
        q: "a%'\"\\--".into(),
        page: None,
        size: None,
    };
    let Json(resp) = handlers::videos::search_videos(State(state.clone()), Query(q))
        .await
        .expect("special chars q");
    assert_eq!(resp.total, 0);

    cleanup_test_video(state.repos.video.pool(), id).await;
}

// ── Upload failure paths ──

#[tokio::test]
async fn test_upload_video_wrong_file_type() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let pool = state.repos.video.pool();
    let (user_id, username) = create_test_user(pool, "upload_bad").await;
    std::fs::create_dir_all(&state.config.media_root).unwrap();

    // 伪装成 mp4 的文本文件 → 验证失败
    let tmp = std::env::temp_dir().join(format!("upload_bad_{}.mp4", unique_username("f")));
    std::fs::write(&tmp, b"this is definitely not a video file").unwrap();
    let res = state
        .services
        .media
        .upload_video_file("bad.mp4", &tmp, "local", user_id)
        .await;
    let err = res.expect_err("伪装成 mp4 的文本文件应上传失败");
    assert!(
        format!("{}", err).contains("文件验证失败"),
        "错误信息应为验证失败: {}",
        err
    );
    assert!(!tmp.exists(), "失败的临时文件应被服务清理");

    // 空文件 → 验证失败
    let tmp2 = std::env::temp_dir().join(format!("upload_empty_{}.mp4", unique_username("f")));
    std::fs::write(&tmp2, b"").unwrap();
    let res = state
        .services
        .media
        .upload_video_file("empty.mp4", &tmp2, "local", user_id)
        .await;
    assert!(res.is_err(), "空文件应上传失败");
    assert!(!tmp2.exists(), "失败的临时文件应被服务清理");

    // 上传者不存在 → 存储配额读取失败
    let tmp3 = std::env::temp_dir().join(format!("upload_ghost_{}.mp4", unique_username("f")));
    std::fs::write(&tmp3, b"some content").unwrap();
    let res = state
        .services
        .media
        .upload_video_file("ghost.mp4", &tmp3, "local", 999_999_999_999)
        .await;
    assert!(res.is_err(), "上传者不存在应报错");
    assert!(!tmp3.exists(), "失败后临时文件应被清理");

    // 所有失败路径都不应产生视频行
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM videos WHERE uploader_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count");
    assert_eq!(count, 0, "上传失败不应产生任何视频行");

    cleanup_test_user(pool, &username).await;
}

#[tokio::test]
async fn test_upload_video_duplicate_hash_rejected() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let pool = state.repos.video.pool();
    let (user_id, username) = create_test_user(pool, "upload_dup").await;
    std::fs::create_dir_all(&state.config.media_root).unwrap();

    // 最小合法 MP4（ftyp + isom 主品牌）
    let mp4: Vec<u8> = vec![
        0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70, //
        0x69, 0x73, 0x6F, 0x6D, 0x00, 0x00, 0x02, 0x00, //
        0x69, 0x73, 0x6F, 0x6D, 0x69, 0x73, 0x6F, 0x32, //
        0x6D, 0x70, 0x34, 0x31, 0x00, 0x00, 0x00, 0x00, //
    ];
    let tag = unique_username("dupfile");
    let fname = format!("dup_{}.mp4", tag);
    let tmp = std::env::temp_dir().join(&fname);
    std::fs::write(&tmp, &mp4).unwrap();

    let before = list_media_files(&state.config.media_root);

    let id = state
        .services
        .media
        .upload_video_file(&fname, &tmp, "local", user_id)
        .await
        .expect("首次上传应成功");
    assert!(id > 0);

    // 相同内容再次上传 → 重复拒绝
    let tmp2 = std::env::temp_dir().join(format!("dup2_{}", tag));
    std::fs::write(&tmp2, &mp4).unwrap();
    let res = state
        .services
        .media
        .upload_video_file(&fname, &tmp2, "local", user_id)
        .await;
    let err = res.expect_err("相同文件重复上传应被拒绝");
    assert!(
        format!("{}", err).contains("重复"),
        "错误信息应指明重复: {}",
        err
    );
    assert!(!tmp2.exists(), "重复上传的临时文件应被清理");

    // 清理：视频行 + media_root 中新出现的文件
    cleanup_test_video(pool, id).await;
    let after = list_media_files(&state.config.media_root);
    for f in after.difference(&before) {
        let _ = std::fs::remove_file(f);
    }
    cleanup_test_user(pool, &username).await;
}

#[tokio::test]
async fn test_upload_resume_handler_size_limits() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let user = test_auth_user(1, "resume_user");
    let gb = 1024i64 * 1024 * 1024;

    let mk_headers = |size: &str, hash: &str| {
        let mut h = HeaderMap::new();
        h.insert("x-upload-size", size.parse().unwrap());
        h.insert("x-upload-hash", hash.parse().unwrap());
        h
    };

    // 超大文件（> 50GB）→ 400
    let h = mk_headers(&(50 * gb + 1).to_string(), "resumehash1");
    let res = handlers::admin::upload_resume(
        State(state.clone()),
        Extension(user.clone()),
        h,
        Bytes::new(),
    )
    .await;
    let (status, _) = res.expect_err("超过 50GB 应被拒绝");
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // size = 0 / 负数 → 400
    for bad_size in ["0", "-5"] {
        let h = mk_headers(bad_size, "resumehash2");
        let res = handlers::admin::upload_resume(
            State(state.clone()),
            Extension(user.clone()),
            h,
            Bytes::new(),
        )
        .await;
        let (status, _) = res.expect_err(&format!("size={} 应被拒绝", bad_size));
        assert_eq!(status, StatusCode::BAD_REQUEST, "size={}", bad_size);
    }

    // 缺少 x-upload-hash → 400
    let mut h = HeaderMap::new();
    h.insert("x-upload-size", "1024".parse().unwrap());
    let res = handlers::admin::upload_resume(
        State(state.clone()),
        Extension(user.clone()),
        h,
        Bytes::new(),
    )
    .await;
    let (status, _) = res.expect_err("缺少 x-upload-hash 应 400");
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 非法 hash 字符 → 400
    let h = mk_headers("1024", "bad;chars!");
    let res = handlers::admin::upload_resume(
        State(state.clone()),
        Extension(user.clone()),
        h,
        Bytes::new(),
    )
    .await;
    let (status, _) = res.expect_err("非法 hash 应 400");
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 合法 size + 空 body → 进度查询（200，received=0）
    let h = mk_headers("1024", "resumehash3");
    let res = handlers::admin::upload_resume(
        State(state.clone()),
        Extension(user.clone()),
        h,
        Bytes::new(),
    )
    .await;
    let Ok((status, Json(body))) = res else {
        panic!("合法 size 的空 body 应返回进度");
    };
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["received"], 0);
}

// ── Update / delete ──

#[tokio::test]
async fn test_update_video_fields() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let id = create_test_video(&state, "upd").await;

    let ok = state
        .services
        .video
        .update_video(id, Some("新标题"), Some("新描述"), Some("newcat"))
        .await
        .expect("update");
    assert!(ok, "更新存在的视频应返回 true");

    let video = state
        .services
        .video
        .get_video(id)
        .await
        .expect("get_video")
        .expect("video exists");
    assert_eq!(video.title, "新标题");
    assert_eq!(video.description, "新描述");
    assert_eq!(video.category, "newcat");

    // 全部 None → 视为 no-op，返回 false
    let ok = state
        .services
        .video
        .update_video(id, None, None, None)
        .await
        .expect("noop update");
    assert!(!ok, "无字段更新应返回 false");

    // 不存在的视频 → false
    let ok = state
        .services
        .video
        .update_video(999_999_999_999, Some("x"), None, None)
        .await
        .expect("update missing");
    assert!(!ok, "更新不存在的视频应返回 false");

    cleanup_test_video(state.repos.video.pool(), id).await;
}

#[tokio::test]
async fn test_delete_video_single_and_batch() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let id1 = create_test_video(&state, "del1").await;
    let id2 = create_test_video(&state, "del2").await;

    // 删除不存在的视频 → false
    assert!(!state
        .services
        .video
        .delete_video(999_999_999_999)
        .await
        .expect("delete missing"));

    // 删除存在的视频 → true，行消失
    assert!(state.services.video.delete_video(id1).await.expect("del1"));
    assert!(
        state
            .services
            .video
            .get_video(id1)
            .await
            .expect("get_video")
            .is_none(),
        "删除后视频应不存在"
    );

    // 重复删除 → false
    assert!(!state
        .services
        .video
        .delete_video(id1)
        .await
        .expect("del1 again"));

    // 批量删除
    let id3 = create_test_video(&state, "del3").await;
    let deleted = state
        .services
        .video
        .delete_videos(&[id2, id3])
        .await
        .expect("batch delete");
    assert_eq!(deleted, 2, "批量删除应删掉 2 条");
    assert!(state
        .services
        .video
        .get_video(id2)
        .await
        .expect("q")
        .is_none());
    assert!(state
        .services
        .video
        .get_video(id3)
        .await
        .expect("q")
        .is_none());
}

// ── Recommendations: latest / trending ordering ──

#[tokio::test]
async fn test_recommendations_latest_and_trending_ordering() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    ensure_chinese_ts_config(state.repos.video.pool()).await;
    let pool = state.repos.video.pool();
    let tag = unique_username("trend");

    let mut ids = Vec::new();
    for i in 0..3 {
        let id = state
            .services
            .video
            .add_external_video(
                &format!("Trend {} {}", tag, i),
                Some("trend"),
                Some("trendtest"),
                &format!("https://example.com/tr_{}_{}.mp4", tag, i),
                None,
                None,
            )
            .await
            .expect("add video");
        ids.push(id);
    }
    // 给第一个视频 50 次浏览，使其 trending 分显著高于其余
    for _ in 0..50 {
        state
            .services
            .video
            .increment_views(ids[0])
            .await
            .expect("increment views");
    }
    // 错开 created_at，避免同一毫秒导致的排序不确定：
    // ids[2] 最新，ids[1] 一小时前，ids[0] 两小时前
    sqlx::query(
        "UPDATE videos SET created_at = CURRENT_TIMESTAMP - $2 * interval '1 hour' WHERE id = $1",
    )
    .bind(ids[0])
    .bind(2i64)
    .execute(pool)
    .await
    .expect("backdate v0");
    sqlx::query(
        "UPDATE videos SET created_at = CURRENT_TIMESTAMP - $2 * interval '1 hour' WHERE id = $1",
    )
    .bind(ids[1])
    .bind(1i64)
    .execute(pool)
    .await
    .expect("backdate v1");

    // 最新视频：created_at 降序，且只比较本测试创建的 3 个视频的
    // 相对顺序（数据库里可能有之前运行留下的脏数据）
    let recent = state
        .services
        .recommendation
        .get_recent_videos(500)
        .await
        .expect("recent");
    let mine: Vec<_> = recent.iter().filter(|r| ids.contains(&r.id)).collect();
    assert_eq!(mine.len(), 3, "三个测试视频都应出现在 recent 列表");
    assert_eq!(mine[0].id, ids[2], "最新创建的视频应排最前");
    assert_eq!(mine[1].id, ids[1]);
    assert_eq!(mine[2].id, ids[0]);
    assert!(recent.iter().any(|r| r.reason == "最新上传"));

    // 热门视频：trending_score 降序 → 浏览量最高的 ids[0] 应排在本组视频最前
    let trending = state
        .services
        .recommendation
        .get_trending_videos(500)
        .await
        .expect("trending");
    let mine: Vec<_> = trending.iter().filter(|r| ids.contains(&r.id)).collect();
    assert!(!mine.is_empty(), "测试视频应出现在 trending 列表");
    assert_eq!(mine[0].id, ids[0], "浏览量最高的视频应排 trending 第一");
    assert!(trending.iter().any(|r| r.reason == "热门推荐"));

    for id in ids {
        cleanup_test_video(pool, id).await;
    }
}
