use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::repositories::playback_repo::PlaybackRepository;

/// 同一用户对同一视频的进度写入节流窗口。
/// 客户端每次心跳/进度变化都会上报 POST /playback/history，
/// 服务端在此窗口内合并写库，避免高频 UPSERT。
const WRITE_THROTTLE: Duration = Duration::from_secs(10);

/// 节流表条目在超过该时长后视为过期（对应会话超时 120s）。
const ENTRY_TTL: Duration = Duration::from_secs(120);

/// 节流表条目上限，超过后惰性清理过期条目，防止内存无限增长。
const MAX_TRACKED_WRITES: usize = 20_000;

#[derive(Clone)]
pub struct PlaybackService {
    repo: PlaybackRepository,
    /// username:video_id -> 最近一次实际写库时间
    last_writes: Arc<DashMap<String, Instant>>,
}

impl PlaybackService {
    pub fn new(repo: PlaybackRepository) -> Self {
        Self {
            repo,
            last_writes: Arc::new(DashMap::new()),
        }
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
        limit: i64,
    ) -> Result<Vec<crate::models::playback::RecentWatchItem>, sqlx::Error> {
        self.repo
            .find_playback_history_by_username(username, Some(limit))
            .await
    }

    pub async fn update_playback(
        &self,
        username: &str,
        video_id: i64,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<(), sqlx::Error> {
        if !self.should_write(username, video_id) {
            return Ok(());
        }
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

impl PlaybackService {
    /// 返回 true 表示应写入数据库：该 key 在节流窗口内没有写过。
    /// 原子地更新记录时间（DashMap::entry 持写锁），锁在 await 前释放。
    fn should_write(&self, username: &str, video_id: i64) -> bool {
        let key = format!("{}:{}", username, video_id);
        let now = Instant::now();
        // 新条目首次出现时立即放行（or_insert 的时间戳回溯到窗口外），
        // 否则第一笔进度写入会被节流窗口吞掉。
        let backdated = now
            .checked_sub(WRITE_THROTTLE + Duration::from_millis(1))
            .unwrap_or(now);
        {
            let mut last = self.last_writes.entry(key).or_insert(backdated);
            if now.duration_since(*last) < WRITE_THROTTLE {
                return false;
            }
            *last = now;
        }
        // 惰性清理：条目数超限时回收过期条目，防止内存无限增长
        if self.last_writes.len() > MAX_TRACKED_WRITES {
            self.last_writes
                .retain(|_, last| last.elapsed() < ENTRY_TTL);
        }
        true
    }
}
