use moka::sync::Cache as MokaCache;

use crate::config::AppConfig;
use crate::middleware::rate_limit::RateLimiter;
use crate::repositories::user_repo::UserRepository;
use crate::services::video_service::VideoService;

pub type VideoListCache = MokaCache<String, String>;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: UserRepository,
    pub video_service: VideoService,
    pub config: AppConfig,
    pub rate_limiter: RateLimiter,
    pub video_cache: VideoListCache,
}
