use dashmap::DashMap;
use moka::sync::Cache;
use std::sync::Arc;
use std::time::Instant;

use crate::config::AppConfig;
use crate::metrics::Metrics;
use crate::middleware::rate_limit::RateLimiter;
use crate::models::chat::ChatEvent;
use crate::models::video::{PagedVideoResponse, VideoItem};
use crate::repositories::chat_repo::ChatRepository;
use crate::repositories::comment_repo::CommentRepository;
use crate::repositories::danmaku_repo::DanmakuRepository;
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
use crate::services::email_service::EmailService;
use crate::services::media_service::MediaService;
use crate::services::playback_service::PlaybackService;
use crate::services::playlist_service::PlaylistService;
use crate::services::recommendation_service::{RecommendationService, VideoRecommendation};
use crate::services::search_service::SearchService;
use crate::services::share_service::ShareService;
use crate::services::tag_service::TagService;
use crate::services::task_queue::TaskQueue;
use crate::services::transcoder::Transcoder;
use crate::services::video_service::VideoService;

pub type VideoListCache = Cache<String, PagedVideoResponse>;
pub type RecommendationCache = Cache<String, (Vec<VideoRecommendation>, i64)>;
/// 单视频详情缓存（`GET /videos/{id}` 热路径）：60 秒 TTL。
/// 视频列表/详情查询共享同一失效入口 `AppState::invalidate_caches`。
pub type VideoDetailCache = Cache<i64, VideoItem>;

/// Tracks active playback sessions: key = "username:video_id", value = last heartbeat time
///
/// # Why this is process-local
///
/// Unlike the HLS in-flight lock, this tracker is deliberately *not* backed by
/// Redis, even in multi-instance deployments. The reasons:
///
/// 1. **It is a liveness signal, not an authorization decision.** It answers
///    "is this browser currently playing this video?", which `media_auth` then
///    combines with the real checks (path-derived `video_id`, ownership, or a
///    share token bound to that specific video). Making the liveness half
///    distributed does not strengthen the authorization half.
/// 2. **It sits on the hottest path in the server.** A `<video>` element issues
///    a `Range` request per segment; `media_auth` runs on every one of them. A
///    Redis round trip there would add a network hop and a new failure mode to
///    every media byte served.
/// 3. **The TTL already bounds staleness.** A session dies 120 s after the last
///    heartbeat regardless of which instance holds it, so a session started on
///    instance A is honoured on instance B for as long as A keeps heartbeating
///    — and no longer. The worst case is a viewer who is throttled to one
///    instance by the load balancer, which is a correct outcome anyway.
///
/// The cost is that a session started on A is invisible to B if the client is
/// load-balanced across instances and A then dies; the client re-starts its
/// session, which is cheap. Sessions are *not* used for revocation: kicking a
/// user invalidates their tokens in the database and, when Redis is available,
/// the shared media-auth cache — see `middleware::auth::media`.
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

    /// Key is `username:video_id`.
    #[inline]
    fn make_key(username: &str, video_id: i64) -> String {
        let mut key = String::with_capacity(username.len() + 22);
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
        let prefix = format!("{username}:");
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
    pub chat: ChatRepository,
}

/// 公共聊天室：单房间。
///
/// - `tx`：tokio broadcast，向所有在线连接广播事件（容量 256，
///   慢消费者会收到 `Lagged` 由连接任务自行处理——聊天允许丢实时事件，
///   历史以 DB 为准）
/// - `users`：在线成员表（user_id → username），用于在线人数/名单广播
pub struct ChatHub {
    room: ChatRoom,
}

pub struct ChatRoom {
    tx: tokio::sync::broadcast::Sender<Arc<ChatEvent>>,
    users: DashMap<i64, String>,
}

const CHAT_BROADCAST_CAPACITY: usize = 256;
/// 在线名单广播上限，防止大量访客名刷爆消息帧
const CHAT_ONLINE_NAMES_MAX: usize = 50;

impl Default for ChatHub {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatHub {
    pub fn new() -> Self {
        Self {
            room: ChatRoom {
                tx: tokio::sync::broadcast::channel(CHAT_BROADCAST_CAPACITY).0,
                users: DashMap::new(),
            },
        }
    }

    /// 加入房间：登记成员并返回接收器（含加入瞬间的广播订阅）。
    pub fn join(
        &self,
        user_id: i64,
        username: &str,
    ) -> tokio::sync::broadcast::Receiver<Arc<ChatEvent>> {
        let room = &self.room;
        // 必须先订阅再登记广播：否则加入者会错过自己的 join 在线广播，
        // 单人在线时前端永远显示 0 人
        let rx = room.tx.subscribe();
        room.users.insert(user_id, username.to_string());
        Self::broadcast_online(room);
        rx
    }

    /// 离开房间：移除成员并广播名单变化。幂等（未在房内时仅静默返回）。
    pub fn leave(&self, user_id: i64) {
        let room = &self.room;
        if room.users.remove(&user_id).is_some() {
            Self::broadcast_online(room);
        }
    }

    /// 向房间广播事件。
    pub fn broadcast(&self, event: ChatEvent) {
        let room = &self.room;
        // 无接收者时 send 返回 Err——纯属正常（没人在线），忽略
        let _ = room.tx.send(Arc::new(event));
    }

    fn broadcast_online(room: &ChatRoom) {
        let names: Vec<String> = room
            .users
            .iter()
            .take(CHAT_ONLINE_NAMES_MAX)
            .map(|u| u.value().clone())
            .collect();
        let count = room.users.len();
        let _ = room.tx.send(Arc::new(ChatEvent::Online { count, names }));
    }
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
    /// 公共聊天室（在线成员 + 广播通道）
    pub chat_hub: Arc<ChatHub>,
    pub metrics: Metrics,
    /// Shared Redis slot (resolved per use, so a late reconnect is picked up).
    pub redis: crate::services::redis::SharedRedis,
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
