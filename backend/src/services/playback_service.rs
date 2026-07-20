use crate::repositories::playback_repo::PlaybackRepository;

#[derive(Clone)]
pub struct PlaybackService {
    repo: PlaybackRepository,
}

impl PlaybackService {
    pub fn new(repo: PlaybackRepository) -> Self {
        Self { repo }
    }

    pub async fn get_playback_data(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<Option<(i64, i64)>, sqlx::Error> {
        self.repo.get_playback_data(username, video_id).await
    }

    pub async fn get_playback_history(
        &self,
        username: &str,
    ) -> Result<Vec<crate::models::playback::RecentWatchItem>, sqlx::Error> {
        self.repo
            .find_playback_history_by_username(username, None)
            .await
    }

    pub async fn update_playback(
        &self,
        username: &str,
        video_id: i64,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<(), sqlx::Error> {
        self.repo
            .upsert_playback(username, video_id, position_ms, duration_ms)
            .await
    }

    pub async fn get_user_profile_data(
        &self,
        username: &str,
    ) -> Result<(i64, i64, Vec<crate::models::playback::RecentWatchItem>), sqlx::Error> {
        let total_videos_watched = self.repo.count_watched_videos(username).await?;
        let total_watch_time = self.repo.sum_watch_time(username).await?;
        let recent_history = self
            .repo
            .find_playback_history_by_username(username, Some(20))
            .await?;
        Ok((total_videos_watched, total_watch_time, recent_history))
    }

    pub async fn toggle_like(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.toggle_like(username, video_id).await
    }

    pub async fn is_liked(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_liked(username, video_id).await
    }

    pub async fn toggle_favorite(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<bool, sqlx::Error> {
        self.repo.toggle_favorite(username, video_id).await
    }

    pub async fn is_favorited(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_favorited(username, video_id).await
    }

    pub async fn get_favorites(
        &self,
        username: &str,
    ) -> Result<Vec<crate::models::playback::RecentWatchItem>, sqlx::Error> {
        self.repo.find_favorites_by_username(username).await
    }
}
