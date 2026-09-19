use crate::repositories::playlist_repo::{PlaylistRepository, PlaylistRow, PlaylistVideoRow};
use crate::util::error::ServiceError;

/// Playlist-related business logic.
/// Encapsulates validation, ownership checks, visibility rules, and
/// FK-violation mapping so that handlers stay thin and consistent.
#[derive(Clone)]
pub struct PlaylistService {
    repo: PlaylistRepository,
}

/// Maximum number of characters in a playlist name.
/// Postgres VARCHAR(200) counts characters, not bytes, so use char count
/// (byte length would wrongly reject multi-byte names such as Chinese).
const MAX_PLAYLIST_NAME_LEN: usize = 200;

impl PlaylistService {
    pub fn new(repo: PlaylistRepository) -> Self {
        Self { repo }
    }

    // ---------------------------------------------------------------------------
    // Validation helpers
    // ---------------------------------------------------------------------------

    /// Validate a playlist name: non-blank and at most 200 characters.
    fn validate_playlist_name(name: &str) -> Result<(), ServiceError> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > MAX_PLAYLIST_NAME_LEN {
            return Err(ServiceError::bad_request("名称长度 1-200 字符"));
        }
        Ok(())
    }

    /// Check whether a sqlx error is a foreign-key violation on a named constraint.
    fn check_fk_violation(e: &sqlx::Error, constraint: &str) -> bool {
        matches!(
            e,
            sqlx::Error::Database(db)
                if db.code().as_deref() == Some("23503")
                    && db.constraint() == Some(constraint)
        )
    }

    // ---------------------------------------------------------------------------
    // Authorization helpers
    // ---------------------------------------------------------------------------

    /// Fetch a playlist by id; return `NotFound` if it does not exist.
    async fn get_or_err(&self, playlist_id: i64) -> Result<PlaylistRow, ServiceError> {
        self.repo
            .get_playlist(playlist_id)
            .await?
            .ok_or_else(|| ServiceError::not_found("播放列表不存在"))
    }

    /// Load a playlist the user is allowed to read: owner, admin, or public.
    /// Non-public playlists of other users are reported as `NotFound` so their
    /// existence is not leaked (same behavior as GET /playlists/{id}).
    pub async fn get_visible(
        &self,
        playlist_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<PlaylistRow, ServiceError> {
        let p = self.get_or_err(playlist_id).await?;
        if p.is_public || p.user_id == user_id || is_admin {
            Ok(p)
        } else {
            Err(ServiceError::not_found("播放列表不存在"))
        }
    }

    /// Verify the requesting user owns the playlist. Returns the playlist row
    /// on success so callers don't have to fetch it again.
    async fn verify_ownership(
        &self,
        playlist_id: i64,
        user_id: i64,
    ) -> Result<PlaylistRow, ServiceError> {
        let p = self.get_or_err(playlist_id).await?;
        if p.user_id != user_id {
            return Err(ServiceError::forbidden("无权修改此播放列表"));
        }
        Ok(p)
    }

    // ---------------------------------------------------------------------------
    // Playlist CRUD
    // ---------------------------------------------------------------------------

    /// Create a new playlist for the given user.
    pub async fn create_playlist(
        &self,
        user_id: i64,
        name: &str,
        description: Option<&str>,
        is_public: Option<bool>,
    ) -> Result<PlaylistRow, ServiceError> {
        Self::validate_playlist_name(name)?;

        let p = self
            .repo
            .create_playlist(user_id, name, description, is_public.unwrap_or(false))
            .await?;
        Ok(p)
    }

    /// Get a single playlist (with item count) that is visible to the user.
    ///
    /// Returns `(PlaylistRow, item_count)`.
    pub async fn get_playlist(
        &self,
        playlist_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(PlaylistRow, i64), ServiceError> {
        let p = self.get_visible(playlist_id, user_id, is_admin).await?;

        let count = self
            .repo
            .count_playlist_items(playlist_id)
            .await
            .unwrap_or_else(|e| {
                tracing::error!(playlist_id, "count_playlist_items failed: {}", e);
                0
            });

        Ok((p, count))
    }

    /// List all playlists belonging to a user, each with its item count.
    pub async fn list_user_playlists(
        &self,
        user_id: i64,
    ) -> Result<Vec<(PlaylistRow, i64)>, ServiceError> {
        let playlists = self.repo.list_user_playlists_with_counts(user_id).await?;
        Ok(playlists)
    }

    /// Update a playlist. Only the owner may call this.
    pub async fn update_playlist(
        &self,
        playlist_id: i64,
        user_id: i64,
        name: Option<&str>,
        description: Option<&str>,
        is_public: Option<bool>,
    ) -> Result<(), ServiceError> {
        self.verify_ownership(playlist_id, user_id).await?;

        if let Some(n) = name {
            Self::validate_playlist_name(n)?;
        }

        self.repo
            .update_playlist(playlist_id, name, description, is_public)
            .await?;
        Ok(())
    }

    /// Delete a playlist. Only the owner may call this.
    pub async fn delete_playlist(
        &self,
        playlist_id: i64,
        user_id: i64,
    ) -> Result<(), ServiceError> {
        self.verify_ownership(playlist_id, user_id).await?;
        self.repo.delete_playlist(playlist_id).await?;
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Playlist items
    // ---------------------------------------------------------------------------

    /// List videos in a playlist (visible to the requesting user).
    pub async fn list_playlist_videos(
        &self,
        playlist_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<Vec<PlaylistVideoRow>, ServiceError> {
        // Consistent with GET /playlists/{id}: owner, admin, or the public can
        // read a playlist's videos. Only the owner may modify it.
        self.get_visible(playlist_id, user_id, is_admin).await?;

        let videos = self.repo.list_playlist_videos(playlist_id).await?;
        Ok(videos)
    }

    /// Add a video to a playlist. Only the owner may call this.
    ///
    /// Returns `Ok(())` whether the video was newly inserted or already present
    /// (ON CONFLICT DO NOTHING).
    pub async fn add_video_to_playlist(
        &self,
        playlist_id: i64,
        user_id: i64,
        video_id: i64,
    ) -> Result<(), ServiceError> {
        self.verify_ownership(playlist_id, user_id).await?;

        self.repo
            .add_video(playlist_id, video_id)
            .await
            .map_err(|e| {
                if Self::check_fk_violation(&e, "playlist_items_video_id_fkey") {
                    return ServiceError::not_found("视频不存在");
                }
                if Self::check_fk_violation(&e, "playlist_items_playlist_id_fkey") {
                    return ServiceError::not_found("播放列表不存在");
                }
                ServiceError::from(e)
            })?;

        Ok(())
    }

    /// Remove a video from a playlist. Only the owner may call this.
    pub async fn remove_video_from_playlist(
        &self,
        playlist_id: i64,
        user_id: i64,
        video_id: i64,
    ) -> Result<(), ServiceError> {
        self.verify_ownership(playlist_id, user_id).await?;
        self.repo.remove_video(playlist_id, video_id).await?;
        Ok(())
    }

    /// Reorder videos inside a playlist. Only the owner may call this.
    ///
    /// `video_ids` must contain **exactly** the set of video IDs currently in
    /// the playlist (no more, no fewer) — the service validates this to prevent
    /// accidental drops or phantom insertions.
    ///
    /// Positions are assigned sequentially starting from 0 in the order given.
    pub async fn reorder_playlist(
        &self,
        playlist_id: i64,
        user_id: i64,
        video_ids: &[i64],
    ) -> Result<(), ServiceError> {
        self.verify_ownership(playlist_id, user_id).await?;

        // Fetch current video IDs to validate the caller supplied the full set.
        let current_videos = self.repo.list_playlist_videos(playlist_id).await?;
        let mut current_ids: Vec<i64> = current_videos.iter().map(|v| v.id).collect();
        current_ids.sort_unstable();

        let mut provided_ids: Vec<i64> = video_ids.to_vec();
        provided_ids.sort_unstable();

        if current_ids != provided_ids {
            return Err(ServiceError::bad_request(
                "排序列表必须包含播放列表中所有且仅包含现有视频",
            ));
        }

        self.repo.reorder_videos(playlist_id, video_ids).await?;
        Ok(())
    }
}
