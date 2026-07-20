use dashmap::DashMap;
use moka::sync::Cache;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::config::AppConfig;
use crate::metrics::Metrics;
use crate::middleware::rate_limit::RateLimiter;
use crate::models::video::PagedVideoResponse;
use crate::repositories::comment_repo::CommentRepository;
use crate::repositories::playback_repo::PlaybackRepository;
use crate::repositories::playlist_repo::PlaylistRepository;
use crate::repositories::registration_repo::RegistrationRepository;
use crate::repositories::share_repo::ShareRepository;
use crate::repositories::tag_repo::TagRepository;
use crate::repositories::user_repo::UserRepository;
use crate::repositories::video_repo::VideoRepository;
use crate::services::admin_service::AdminService;
use crate::services::auth_service::AuthService;
use crate::services::comment_service::CommentService;
use crate::services::media_service::MediaService;
use crate::services::playback_service::PlaybackService;
use crate::services::recommendation_service::RecommendationService;
use crate::services::search_service::SearchService;
use crate::services::share_service::ShareService;
use crate::services::tag_service::TagService;
use crate::services::task_queue::TaskQueue;
use crate::services::transcoder::Transcoder;
use crate::services::video_service::VideoService;

pub type VideoListCache = Cache<String, PagedVideoResponse>;

/// Tracks active playback sessions: key = "username:video_id", value = last heartbeat time
pub struct PlaybackSessionTracker {
    sessions: DashMap<String, Instant>,
}

/// Session timeout: if no heartbeat within this window, session is considered inactive
pub const SESSION_TIMEOUT_SECS: u64 = 30;

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

    pub fn start(&self, username: &str, video_id: i64) {
        let key = format!("{}:{}", username, video_id);
        self.sessions.insert(key, Instant::now());
    }

    pub fn heartbeat(&self, username: &str, video_id: i64) {
        let key = format!("{}:{}", username, video_id);
        self.sessions.insert(key, Instant::now());
    }

    pub fn stop(&self, username: &str, video_id: i64) {
        let key = format!("{}:{}", username, video_id);
        self.sessions.remove(&key);
    }

    pub fn is_active(&self, username: &str, video_id: i64) -> bool {
        let key = format!("{}:{}", username, video_id);
        self.sessions
            .get(&key)
            .map(|entry| entry.elapsed().as_secs() < SESSION_TIMEOUT_SECS)
            .unwrap_or(false)
    }

    /// Returns true if the user has any active playback session for any video.
    /// Used by the media middleware to gate access to non-video assets
    /// (thumbnails, covers, avatars) without binding to a specific video.
    pub fn has_any_active(&self, username: &str) -> bool {
        let prefix = format!("{}:", username);
        self.sessions
            .iter()
            .any(|e| e.key().starts_with(&prefix) && e.elapsed().as_secs() < SESSION_TIMEOUT_SECS)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub registration_repo: RegistrationRepository,
    pub user_repo: UserRepository,
    pub video_repo: VideoRepository,
    pub playback_repo: PlaybackRepository,
    pub playlist_repo: PlaylistRepository,
    pub comment_repo: CommentRepository,
    pub share_repo: ShareRepository,
    pub tag_repo: TagRepository,
    pub video_service: VideoService,
    pub media_service: MediaService,
    pub playback_service: PlaybackService,
    pub auth_service: AuthService,
    pub tag_service: TagService,
    pub search_service: SearchService,
    pub recommendation_service: RecommendationService,
    pub comment_service: CommentService,
    pub share_service: ShareService,
    pub admin_service: AdminService,
    pub config: AppConfig,
    pub rate_limiter: RateLimiter,
    pub ip_rate_limiter: RateLimiter,
    pub video_cache: VideoListCache,
    pub playback_sessions: Arc<PlaybackSessionTracker>,
    pub upload_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    pub metrics: Metrics,
    pub transcoder: Transcoder,
    pub task_queue: TaskQueue,
}
