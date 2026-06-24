use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

const RATE_LIMIT_MAX_ATTEMPTS: u32 = 5;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_BLOCK_SECS: u64 = 600;

#[derive(Clone, Copy)]
struct RateLimitEntry {
    count: u32,
    blocked_until: Option<Instant>,
}

#[derive(Clone)]
pub struct RateLimiter {
    /// (expires_at, entry) — expires_at for TTL-based cleanup
    cache: Arc<DashMap<String, (Instant, RateLimitEntry)>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }

    /// Atomically check and increment rate limit for the given key.
    /// Returns Ok(()) if allowed, Err(()) if rate-limited.
    pub async fn check(&self, key: &str) -> Result<(), ()> {
        self.check_with(key, RATE_LIMIT_MAX_ATTEMPTS, RATE_LIMIT_WINDOW_SECS, RATE_LIMIT_BLOCK_SECS).await
    }

    /// Like `check` but with custom parameters.
    pub async fn check_with(&self, key: &str, max_attempts: u32, window_secs: u64, block_secs: u64) -> Result<(), ()> {
        let now = Instant::now();

        // Lazily purge expired entries every call (cheap scan on DashMap)
        self.cache.retain(|_, (expires_at, entry)| {
            if now >= *expires_at && entry.blocked_until.is_none() {
                return false; // expired window, not blocked
            }
            if let Some(until) = entry.blocked_until {
                if now >= until && now >= *expires_at {
                    return false; // block expired and window expired
                }
            }
            true
        });

        // Atomic read-modify-write via DashMap::entry — holds write guard
        let mut slot = self.cache.entry(key.to_string()).or_insert_with(|| {
            (
                now + Duration::from_secs(window_secs),
                RateLimitEntry {
                    count: 0,
                    blocked_until: None,
                },
            )
        });

        let (expires_at, entry) = slot.value_mut();

        // If blocked, reject
        if let Some(until) = entry.blocked_until {
            if now < until {
                return Err(());
            }
            // Block expired — reset count and clear block
            entry.count = 0;
            entry.blocked_until = None;
            *expires_at = now + Duration::from_secs(window_secs);
        }

        // If window expired, reset
        if now >= *expires_at {
            entry.count = 0;
            *expires_at = now + Duration::from_secs(window_secs);
        }

        entry.count += 1;

        if entry.count >= max_attempts {
            entry.blocked_until = Some(now + Duration::from_secs(block_secs));
            Err(())
        } else {
            Ok(())
        }
    }

    pub async fn reset(&self, key: &str) {
        self.cache.remove(key);
    }
}
