use std::sync::Arc;
use std::time::Instant;

use moka::sync::Cache as MokaCache;

const RATE_LIMIT_MAX_ATTEMPTS: u32 = 5;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_BLOCK_SECS: u64 = 600;
const RATE_LIMIT_MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Copy)]
struct RateLimitEntry {
    count: u32,
    blocked_until: Option<Instant>,
}

#[derive(Clone)]
pub struct RateLimiter {
    cache: Arc<MokaCache<std::net::IpAddr, RateLimitEntry>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(
                MokaCache::builder()
                    .max_capacity(RATE_LIMIT_MAX_ENTRIES as u64)
                    .time_to_live(std::time::Duration::from_secs(RATE_LIMIT_WINDOW_SECS))
                    .build(),
            ),
        }
    }

    pub async fn check(&self, ip: std::net::IpAddr) -> Result<(), ()> {
        let now = Instant::now();
        let entry = self.cache.get(&ip).unwrap_or(RateLimitEntry {
            count: 0,
            blocked_until: None,
        });

        if let Some(until) = entry.blocked_until {
            if now < until {
                return Err(());
            }
        }

        let new_count = entry.count + 1;
        let blocked = if new_count >= RATE_LIMIT_MAX_ATTEMPTS {
            Some(now + std::time::Duration::from_secs(RATE_LIMIT_BLOCK_SECS))
        } else {
            None
        };

        self.cache.insert(
            ip,
            RateLimitEntry {
                count: new_count,
                blocked_until: blocked,
            },
        );

        if blocked.is_some() {
            Err(())
        } else {
            Ok(())
        }
    }

    pub async fn reset(&self, ip: std::net::IpAddr) {
        self.cache.invalidate(&ip);
    }
}
