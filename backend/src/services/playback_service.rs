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

/// 播放服务，负责管理视频播放进度、点赞、收藏等与用户播放交互相关的业务逻辑。
///
/// 内置写入节流机制：同一用户对同一视频的进度上报会在 [`WRITE_THROTTLE`] 窗口内合并，
/// 避免高频 UPSERT 对数据库造成压力。
#[derive(Clone)]
pub struct PlaybackService {
    repo: PlaybackRepository,
    /// username:video_id -> 最近一次实际写库时间
    last_writes: Arc<DashMap<String, Instant>>,
}

impl PlaybackService {
    /// 创建一个新的 [`PlaybackService`] 实例。
    ///
    /// # 参数
    /// * `repo` - 播放数据仓储，用于访问底层数据库。
    pub fn new(repo: PlaybackRepository) -> Self {
        Self {
            repo,
            last_writes: Arc::new(DashMap::new()),
        }
    }

    /// 获取指定用户对指定视频的播放进度。
    ///
    /// 返回 `Ok(Some((position_ms, duration_ms)))` 表示存在历史记录，
    /// `Ok(None)` 表示该用户从未播放过该视频。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `video_id` - 视频 ID。
    pub async fn get_playback_data(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<Option<(i64, i64)>, sqlx::Error> {
        self.repo.get_playback_data(username, video_id).await
    }

    /// 获取用户的播放历史记录（最近观看列表，分页）。
    ///
    /// 返回按最近播放时间倒序排列的视频列表，每项包含视频基本信息及播放进度。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `limit` - 每页条数。
    /// * `offset` - 偏移量。
    pub async fn get_playback_history(
        &self,
        username: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<crate::models::playback::RecentWatchItem>, i64), sqlx::Error> {
        self.repo
            .find_playback_history_by_username(username, limit, offset)
            .await
    }

    /// 更新用户的视频播放进度。
    ///
    /// 内置节流逻辑：同一用户对同一视频在 [`WRITE_THROTTLE`]（10 秒）窗口内的
    /// 重复上报会被静默忽略，以减少数据库写入压力。首次上报不受节流限制。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `video_id` - 视频 ID。
    /// * `position_ms` - 当前播放位置（毫秒）。
    /// * `duration_ms` - 视频总时长（毫秒）。
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

    /// 获取用户个人主页所需的播放统计数据。
    ///
    /// 一次性返回三项数据，用于在用户资料页展示：
    /// 1. 已观看视频总数
    /// 2. 累计观看时长（毫秒）
    /// 3. 最近 20 条播放历史记录
    ///
    /// # 参数
    /// * `username` - 用户名。
    pub async fn get_user_profile_data(
        &self,
        username: &str,
    ) -> Result<(i64, i64, Vec<crate::models::playback::RecentWatchItem>), sqlx::Error> {
        let total_videos_watched = self.repo.count_watched_videos(username).await?;
        let total_watch_time = self.repo.sum_watch_time(username).await?;
        let (recent_history, _) = self
            .repo
            .find_playback_history_by_username(username, 20, 0)
            .await?;
        Ok((total_videos_watched, total_watch_time, recent_history))
    }

    /// 切换用户对视频的点赞状态。
    ///
    /// 若用户已点赞则取消点赞，否则添加点赞。返回切换后的点赞状态：
    /// `true` 表示当前已点赞，`false` 表示当前未点赞。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `video_id` - 视频 ID。
    pub async fn toggle_like(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.toggle_like(username, video_id).await
    }

    /// 查询用户是否已点赞指定视频。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `video_id` - 视频 ID。
    pub async fn is_liked(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_liked(username, video_id).await
    }

    /// 切换用户对视频的收藏状态。
    ///
    /// 若用户已收藏则取消收藏，否则添加收藏。返回切换后的收藏状态：
    /// `true` 表示当前已收藏，`false` 表示当前未收藏。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `video_id` - 视频 ID。
    pub async fn toggle_favorite(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<bool, sqlx::Error> {
        self.repo.toggle_favorite(username, video_id).await
    }

    /// 查询用户是否已收藏指定视频。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `video_id` - 视频 ID。
    pub async fn is_favorited(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_favorited(username, video_id).await
    }

    /// 获取用户的收藏列表（分页）。
    ///
    /// 返回用户收藏的视频信息，包含视频基本信息及播放进度。
    ///
    /// # 参数
    /// * `username` - 用户名。
    /// * `limit` - 每页条数。
    /// * `offset` - 偏移量。
    pub async fn get_favorites(
        &self,
        username: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<crate::models::playback::RecentWatchItem>, i64), sqlx::Error> {
        self.repo
            .find_favorites_by_username(username, limit, offset)
            .await
    }
}

impl PlaybackService {
    /// 返回 true 表示应写入数据库：该 key 在节流窗口内没有写过。
    /// 原子地更新记录时间（DashMap::entry 持写锁），锁在 await 前释放。
    fn should_write(&self, username: &str, video_id: i64) -> bool {
        let key = format!("{}:{video_id}", username);
        let now = Instant::now();
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
        if self.last_writes.len() > MAX_TRACKED_WRITES {
            self.last_writes
                .retain(|_, last| last.elapsed() < ENTRY_TTL);
        }
        true
    }

    pub fn active_throttle_entries(&self) -> usize {
        self.last_writes.len()
    }
}
