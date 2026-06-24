//! Integration tests for video operations.
//!
//! Requires a running PostgreSQL database. Set `DATABASE_URL` to enable.

mod integration_test_helpers;

use integration_test_helpers::*;

// ── Add external video ──

#[tokio::test]
async fn test_add_external_video() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let title = format!("Test Video {}", unique_username("vid"));

    let id = state
        .video_service
        .add_external_video(
            &title,
            Some("A test video"),
            Some("test"),
            "https://example.com/video.mp4",
            Some("https://example.com/cover.jpg"),
        )
        .await
        .expect("add_external_video");

    assert!(id > 0, "video id should be positive");

    // Verify it can be fetched
    let video = state
        .video_service
        .get_video(id)
        .await
        .expect("get_video")
        .expect("video should exist");

    assert_eq!(video.title, title);
    assert_eq!(video.source_type, "external");
    assert_eq!(video.stream_url, "https://example.com/video.mp4");
    assert_eq!(video.cover_url, Some("https://example.com/cover.jpg".into()));
    assert_eq!(video.category, "test");

    cleanup_test_video(&state.db_pool, id).await;
}

// ── List videos with pagination ──

#[tokio::test]
async fn test_list_videos_pagination() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let tag = unique_username("page");

    // Insert 3 test videos
    let mut ids = Vec::new();
    for i in 0..3 {
        let id = state
            .video_service
            .add_external_video(
                &format!("Pagination Test {} - {}", tag, i),
                Some("pagination test"),
                Some("pagetest"),
                &format!("https://example.com/{}.mp4", i),
                None,
            )
            .await
            .expect("add video");
        ids.push(id);
    }

    // List with size=2, page=0
    let (items_page0, total) = state
        .video_service
        .list_videos_paged(0, 2, Some(&tag), None, None, None)
        .await
        .expect("list page 0");

    assert!(total >= 3, "total should be at least 3, got {}", total);
    assert_eq!(items_page0.len(), 2, "page 0 should have 2 items");

    // List with size=2, page=1
    let (items_page1, total2) = state
        .video_service
        .list_videos_paged(1, 2, Some(&tag), None, None, None)
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
        cleanup_test_video(&state.db_pool, id).await;
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
    let unique = unique_username("search");

    let id = state
        .video_service
        .add_external_video(
            &format!("Searchable Title {}", unique),
            Some("search test"),
            Some("searchtest"),
            "https://example.com/search.mp4",
            None,
        )
        .await
        .expect("add video");

    // Search by unique substring
    let (results, _) = state
        .video_service
        .list_videos_paged(0, 10, Some(&unique), None, None, None)
        .await
        .expect("search");

    assert!(
        results.iter().any(|v| v.id == id),
        "search results should contain the video we just created"
    );

    // Search for something that doesn't exist
    let (empty, _) = state
        .video_service
        .list_videos_paged(
            0,
            10,
            Some("zzz_nonexistent_query_zzz"),
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

    cleanup_test_video(&state.db_pool, id).await;
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
        .video_service
        .is_liked(&username, video_id)
        .await
        .expect("is_liked");
    assert!(!liked, "should not be liked initially");

    // Toggle on → liked
    let liked = state
        .video_service
        .toggle_like(&username, video_id)
        .await
        .expect("toggle_like");
    assert!(liked, "should be liked after first toggle");

    // Verify
    let liked = state
        .video_service
        .is_liked(&username, video_id)
        .await
        .expect("is_liked");
    assert!(liked, "is_liked should return true after toggle on");

    // Toggle off → not liked
    let liked = state
        .video_service
        .toggle_like(&username, video_id)
        .await
        .expect("toggle_like");
    assert!(!liked, "should not be liked after second toggle");

    // Verify
    let liked = state
        .video_service
        .is_liked(&username, video_id)
        .await
        .expect("is_liked");
    assert!(!liked, "is_liked should return false after toggle off");

    // Cleanup
    cleanup_like(&state.db_pool, &username, video_id).await;
    cleanup_test_video(&state.db_pool, video_id).await;
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
        .video_service
        .is_favorited(&username, video_id)
        .await
        .expect("is_favorited");
    assert!(!fav, "should not be favorited initially");

    // Toggle on
    let fav = state
        .video_service
        .toggle_favorite(&username, video_id)
        .await
        .expect("toggle_favorite");
    assert!(fav, "should be favorited after first toggle");

    // Verify
    let fav = state
        .video_service
        .is_favorited(&username, video_id)
        .await
        .expect("is_favorited");
    assert!(fav, "is_favorited should return true");

    // Toggle off
    let fav = state
        .video_service
        .toggle_favorite(&username, video_id)
        .await
        .expect("toggle_favorite");
    assert!(!fav, "should not be favorited after second toggle");

    cleanup_favorite(&state.db_pool, &username, video_id).await;
    cleanup_test_video(&state.db_pool, video_id).await;
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
    let position = state
        .video_service
        .get_playback_position(&username, video_id)
        .await
        .expect("get position");
    assert!(position.is_none(), "should have no playback position initially");

    // Update playback
    state
        .video_service
        .update_playback(&username, video_id, 30_000, 120_000)
        .await
        .expect("update_playback");

    // Verify position
    let position = state
        .video_service
        .get_playback_position(&username, video_id)
        .await
        .expect("get position");
    assert_eq!(position, Some(30_000), "position should be 30000ms");

    // Verify duration
    let duration = state
        .video_service
        .get_playback_duration(&username, video_id)
        .await
        .expect("get duration");
    assert_eq!(duration, Some(120_000), "duration should be 120000ms");

    // Update again (upsert)
    state
        .video_service
        .update_playback(&username, video_id, 60_000, 120_000)
        .await
        .expect("update_playback again");

    let position = state
        .video_service
        .get_playback_position(&username, video_id)
        .await
        .expect("get position after update");
    assert_eq!(position, Some(60_000), "position should be updated to 60000ms");

    // Check history list
    let history = state
        .video_service
        .get_playback_history(&username)
        .await
        .expect("get history");

    assert!(
        history.iter().any(|h| h.video_id == video_id),
        "history should contain the video"
    );

    // Cleanup
    cleanup_playback(&state.db_pool, &username).await;
    cleanup_test_video(&state.db_pool, video_id).await;
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
        .video_service
        .get_video(video_id)
        .await
        .expect("get_video")
        .expect("video exists");
    let initial_views = video.views;

    // Increment
    state
        .video_service
        .increment_views(video_id)
        .await
        .expect("increment_views");

    let video = state
        .video_service
        .get_video(video_id)
        .await
        .expect("get_video")
        .expect("video exists");

    assert_eq!(video.views, initial_views + 1, "views should increase by 1");

    cleanup_test_video(&state.db_pool, video_id).await;
}

// ── Helper functions ──

/// Create a test external video and return its ID.
async fn create_test_video(state: &lan_video_backend::state::AppState, prefix: &str) -> i64 {
    state
        .video_service
        .add_external_video(
            &format!("{} Video {}", prefix, unique_username(prefix)),
            Some("integration test"),
            Some("integration"),
            &format!("https://example.com/{}.mp4", unique_username(prefix)),
            None,
        )
        .await
        .expect("create test video")
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
