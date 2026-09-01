//! Integration tests for the playlist feature.
//!
//! Requires a running PostgreSQL database. Set `DATABASE_URL` to enable.

mod integration_test_helpers;

use integration_test_helpers::*;

// ── Create a playlist ──

#[tokio::test]
async fn test_create_playlist() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let (username, _password, user_id, _token) =
        create_test_user_with_credentials(&state, "pl_create").await;

    let p = state
        .services
        .playlist
        .create_playlist(1, user_id, "My Playlist", Some("a description"), Some(true))
        .await
        .expect("create playlist");

    assert!(p.id > 0, "playlist id should be positive");
    assert_eq!(p.name, "My Playlist");
    assert_eq!(p.description, Some("a description".into()));
    assert!(p.is_public);
    assert_eq!(p.user_id, user_id);

    // Cleanup
    let pool = test_pool().await;
    cleanup_playlist(&pool, p.id).await;
    cleanup_test_user(&pool, &username).await;
}

// ── List playlists ──

#[tokio::test]
async fn test_list_playlists() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let (username, _password, user_id, _token) =
        create_test_user_with_credentials(&state, "pl_list").await;

    // Create two playlists
    let p1 = state
        .services
        .playlist
        .create_playlist(1, user_id, "List Test 1", None, Some(false))
        .await
        .expect("create playlist 1");
    let p2 = state
        .services
        .playlist
        .create_playlist(1, user_id, "List Test 2", None, Some(true))
        .await
        .expect("create playlist 2");

    let playlists = state
        .services
        .playlist
        .list_user_playlists(1, user_id)
        .await
        .expect("list playlists");

    assert!(playlists.len() >= 2, "should have at least 2 playlists");
    let ids: Vec<i64> = playlists.iter().map(|(p, _)| p.id).collect();
    assert!(ids.contains(&p1.id), "list should contain playlist 1");
    assert!(ids.contains(&p2.id), "list should contain playlist 2");

    // Each playlist should have item_count = 0
    for (p, count) in &playlists {
        if p.id == p1.id || p.id == p2.id {
            assert_eq!(*count, 0, "new playlist should have 0 items");
        }
    }

    // Cleanup
    let pool = test_pool().await;
    cleanup_playlist(&pool, p1.id).await;
    cleanup_playlist(&pool, p2.id).await;
    cleanup_test_user(&pool, &username).await;
}

// ── Add a video to a playlist ──

#[tokio::test]
async fn test_add_video_to_playlist() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let (username, _password, user_id, _token) =
        create_test_user_with_credentials(&state, "pl_add").await;
    let video_id = create_test_video(&state, "pl_add").await;

    let p = state
        .services
        .playlist
        .create_playlist(1, user_id, "Add Video Test", None, None)
        .await
        .expect("create playlist");

    // Add video
    state
        .services
        .playlist
        .add_video_to_playlist(1, p.id, user_id, video_id)
        .await
        .expect("add video to playlist");

    // Verify count
    let (_, count) = state
        .services
        .playlist
        .get_playlist(1, p.id, user_id, false)
        .await
        .expect("get playlist");
    assert_eq!(count, 1, "playlist should have 1 item");

    // Verify video appears in list
    let videos = state
        .services
        .playlist
        .list_playlist_videos(1, p.id, user_id, false)
        .await
        .expect("list playlist videos");
    assert_eq!(videos.len(), 1);
    assert_eq!(videos[0].id, video_id);

    // Adding the same video again should be a no-op (ON CONFLICT DO NOTHING)
    state
        .services
        .playlist
        .add_video_to_playlist(1, p.id, user_id, video_id)
        .await
        .expect("add duplicate video should be no-op");

    let (_, count) = state
        .services
        .playlist
        .get_playlist(1, p.id, user_id, false)
        .await
        .expect("get playlist after dup");
    assert_eq!(count, 1, "duplicate add should not increase count");

    // Cleanup
    let pool = test_pool().await;
    cleanup_playlist(&pool, p.id).await;
    cleanup_test_video(&pool, video_id).await;
    cleanup_test_user(&pool, &username).await;
}

// ── Remove a video from a playlist ──

#[tokio::test]
async fn test_remove_video_from_playlist() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let (username, _password, user_id, _token) =
        create_test_user_with_credentials(&state, "pl_rm").await;
    let video_id = create_test_video(&state, "pl_rm").await;

    let p = state
        .services
        .playlist
        .create_playlist(1, user_id, "Remove Video Test", None, None)
        .await
        .expect("create playlist");

    // Add then remove
    state
        .services
        .playlist
        .add_video_to_playlist(1, p.id, user_id, video_id)
        .await
        .expect("add video");

    state
        .services
        .playlist
        .remove_video_from_playlist(1, p.id, user_id, video_id)
        .await
        .expect("remove video");

    let (_, count) = state
        .services
        .playlist
        .get_playlist(1, p.id, user_id, false)
        .await
        .expect("get playlist");
    assert_eq!(count, 0, "playlist should be empty after removal");

    // Removing a video not in the playlist should succeed (no-op)
    state
        .services
        .playlist
        .remove_video_from_playlist(1, p.id, user_id, video_id)
        .await
        .expect("remove non-existent video should be no-op");

    // Cleanup
    let pool = test_pool().await;
    cleanup_playlist(&pool, p.id).await;
    cleanup_test_video(&pool, video_id).await;
    cleanup_test_user(&pool, &username).await;
}

// ── Delete a playlist ──

#[tokio::test]
async fn test_delete_playlist() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let (username, _password, user_id, _token) =
        create_test_user_with_credentials(&state, "pl_del").await;
    let video_id = create_test_video(&state, "pl_del").await;

    let p = state
        .services
        .playlist
        .create_playlist(1, user_id, "Delete Test", None, None)
        .await
        .expect("create playlist");

    // Add a video so we can verify cascade or item cleanup
    state
        .services
        .playlist
        .add_video_to_playlist(1, p.id, user_id, video_id)
        .await
        .expect("add video");

    // Delete the playlist
    state
        .services
        .playlist
        .delete_playlist(1, p.id, user_id)
        .await
        .expect("delete playlist");

    // Verify it's gone
    let res = state
        .services
        .playlist
        .get_playlist(1, p.id, user_id, false)
        .await;
    assert!(res.is_err(), "deleted playlist should not be found");

    // Cleanup (video still exists, playlist_items cascade-deleted)
    let pool = test_pool().await;
    cleanup_test_video(&pool, video_id).await;
    cleanup_test_user(&pool, &username).await;
}

// ── Authorization: non-owner cannot modify playlist ──

#[tokio::test]
async fn test_non_owner_cannot_modify_playlist() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };

    let state = test_app_state().await;
    let (owner_name, _owner_pw, owner_id, _owner_token) =
        create_test_user_with_credentials(&state, "pl_owner").await;
    let (other_name, _other_pw, other_id, _other_token) =
        create_test_user_with_credentials(&state, "pl_other").await;
    let video_id = create_test_video(&state, "pl_auth").await;

    let p = state
        .services
        .playlist
        .create_playlist(1, owner_id, "Owner's Playlist", None, Some(true))
        .await
        .expect("create playlist");

    // Non-owner cannot add video
    let res = state
        .services
        .playlist
        .add_video_to_playlist(1, p.id, other_id, video_id)
        .await;
    assert!(res.is_err(), "non-owner should not be able to add video");

    // Owner can add video
    state
        .services
        .playlist
        .add_video_to_playlist(1, p.id, owner_id, video_id)
        .await
        .expect("owner should be able to add video");

    // Non-owner cannot remove video
    let res = state
        .services
        .playlist
        .remove_video_from_playlist(1, p.id, other_id, video_id)
        .await;
    assert!(res.is_err(), "non-owner should not be able to remove video");

    // Non-owner cannot update playlist
    let res = state
        .services
        .playlist
        .update_playlist(1, p.id, other_id, Some("hacked"), None, None)
        .await;
    assert!(
        res.is_err(),
        "non-owner should not be able to update playlist"
    );

    // Non-owner cannot delete playlist
    let res = state
        .services
        .playlist
        .delete_playlist(1, p.id, other_id)
        .await;
    assert!(
        res.is_err(),
        "non-owner should not be able to delete playlist"
    );

    // Non-owner cannot reorder playlist
    let res = state
        .services
        .playlist
        .reorder_playlist(1, p.id, other_id, &[video_id])
        .await;
    assert!(
        res.is_err(),
        "non-owner should not be able to reorder playlist"
    );

    // Owner can still read it
    let (fetched, count) = state
        .services
        .playlist
        .get_playlist(1, p.id, owner_id, false)
        .await
        .expect("owner should be able to read");
    assert_eq!(fetched.id, p.id);
    assert_eq!(count, 1);

    // Cleanup
    let pool = test_pool().await;
    cleanup_playlist(&pool, p.id).await;
    cleanup_test_video(&pool, video_id).await;
    cleanup_test_user(&pool, &owner_name).await;
    cleanup_test_user(&pool, &other_name).await;
}

// ── Cleanup helpers ──

/// Delete a playlist and its items directly via SQL.
async fn cleanup_playlist(pool: &sqlx::PgPool, playlist_id: i64) {
    let _ = sqlx::query("DELETE FROM playlist_items WHERE playlist_id = $1")
        .bind(playlist_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM playlists WHERE id = $1")
        .bind(playlist_id)
        .execute(pool)
        .await;
}
