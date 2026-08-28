use dashmap::DashMap;
use moka::sync::Cache;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::config::AppConfig;
use crate::metrics::Metrics;
use crate::middleware::rate_limit::RateLimiter;
use crate::models::video::{PagedVideoResponse, VideoItem};
use crate::repositories::comment_repo::CommentRepository;
use crate::repositories::danmaku_repo::DanmakuRepository;
use crate::repositories::plan_repo::PlanRepository;
use crate::repositories::playback_repo::PlaybackRepository;
use crate::repositories::playlist_repo::PlaylistRepository;
use crate::repositories::registration_repo::RegistrationRepository;
use crate::repositories::share_repo::ShareRepository;
use crate::repositories::tag_repo::TagRepository;
use crate::repositories::tenant_repo::TenantRepository;
use crate::repositories::user_repo::UserRepository;
use crate::repositories::video_repo::VideoRepository;
use crate::services::admin_service::AdminService;
use crate::services::auth_service::AuthService;
use crate::services::comment_service::CommentService;
use crate::services::email_service::EmailService;
use crate::services::media_service::MediaService;
use crate::services::plan_service::PlanService;
use crate::services::playback_service::PlaybackService;
use crate::services::playlist_service::PlaylistService;
use crate::services::recommendation_service::{RecommendationService, VideoRecommendation};
use crate::services::search_service::SearchService;
use crate::services::share_service::ShareService;
use crate::services::tag_service::TagService;
use crate::services::task_queue::TaskQueue;
use crate::services::tenant_service::TenantService;
use crate::services::transcoder::Transcoder;
use crate::services::video_service::VideoService;

pub type VideoListCache = Cache<String, PagedVideoResponse>;
pub type RecommendationCache = Cache<String, (Vec<VideoRecommendation>, i64)>;
/// 单视频详情缓存（`GET /videos/{id}` 热路径）：60 秒 TTL。
/// 视频列表/详情查询共享同一失效入口 `AppState::invalidate_caches`。
pub type VideoDetailCache = Cache<i64, VideoItem>;

/// Tracks active playback sessions: key = "username:video_id", value = last heartbeat time
pub struct PlaybackSessionTracker {
    sessions: DashMap<String, Instant>,
}

/// Session timeout: if no heartbeat within this window, session is considered inactive
pub const SESSION_TIMEOUT_SECS: u64 = 120;

impl Default for PlaybackSessionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackSessionTracker {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    #[inline]
    fn make_key(username: &str, video_id: i64) -> String {
        let mut key = String::with_capacity(username.len() + 20);
        key.push_str(username);
        key.push(':');
        key.push_str(&video_id.to_string());
        key
    }

    pub fn start(&self, username: &str, video_id: i64) {
        let key = Self::make_key(username, video_id);
        self.sessions.insert(key, Instant::now());
    }

    pub fn heartbeat(&self, username: &str, video_id: i64) {
        let key = Self::make_key(username, video_id);
        self.sessions.insert(key, Instant::now());
    }

    pub fn stop(&self, username: &str, video_id: i64) {
        let key = Self::make_key(username, video_id);
        self.sessions.remove(&key);
    }

    pub fn is_active(&self, username: &str, video_id: i64) -> bool {
        let key = Self::make_key(username, video_id);
        self.sessions
            .get(&key)
            .map(|entry| entry.elapsed().as_secs() < SESSION_TIMEOUT_SECS)
            .unwrap_or(false)
    }

    pub fn has_any_active(&self, username: &str) -> bool {
        let prefix = format!("{}:", username);
        self.sessions
            .iter()
            .any(|e| e.key().starts_with(&prefix) && e.elapsed().as_secs() < SESSION_TIMEOUT_SECS)
    }

    pub fn evict_expired(&self) {
        self.sessions
            .retain(|_, last| last.elapsed().as_secs() < SESSION_TIMEOUT_SECS);
    }
}

/// Start a background task that periodically cleans up expired playback sessions.
pub fn start_session_cleanup(tracker: std::sync::Arc<PlaybackSessionTracker>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            tracker.evict_expired();
        }
    });
}

#[derive(Clone)]
pub struct RepoLayer {
    pub registration: RegistrationRepository,
    pub user: UserRepository,
    pub video: VideoRepository,
    pub playback: PlaybackRepository,
    pub playlist: PlaylistRepository,
    pub comment: CommentRepository,
    pub danmaku: DanmakuRepository,
    pub share: ShareRepository,
    pub tag: TagRepository,
    pub tenant: TenantRepository,
    pub plan: PlanRepository,
}

#[derive(Clone)]
pub struct ServiceLayer {
    pub video: VideoService,
    pub media: MediaService,
    pub playback: PlaybackService,
    pub playlist: PlaylistService,
    pub auth: AuthService,
    pub email: EmailService,
    pub tag: TagService,
    pub search: SearchService,
    pub recommendation: RecommendationService,
    pub comment: CommentService,
    pub share: ShareService,
    pub admin: AdminService,
    pub tenant: TenantService,
    pub plan: PlanService,
}

#[derive(Clone)]
pub struct AppState {
    pub repos: RepoLayer,
    pub services: ServiceLayer,
    pub config: AppConfig,
    pub rate_limiter: RateLimiter,
    pub ip_rate_limiter: RateLimiter,
    pub video_cache: VideoListCache,
    pub recommendation_cache: RecommendationCache,
    pub video_detail_cache: VideoDetailCache,
    pub playback_sessions: Arc<PlaybackSessionTracker>,
    pub upload_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    pub metrics: Metrics,
    pub redis: Option<redis::aio::ConnectionManager>,
    pub transcoder: Transcoder,
    pub task_queue: TaskQueue,
}

impl AppState {
    pub fn invalidate_caches(&self) {
        self.video_cache.invalidate_all();
        self.recommendation_cache.invalidate_all();
        self.video_detail_cache.invalidate_all();
    }
}
