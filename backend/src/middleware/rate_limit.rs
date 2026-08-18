use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use redis::aio::ConnectionManager;

// Per-username: 5 attempts per 60s, 5-minute block after exceeding.
// Note: the attempt that reaches the limit is itself rejected and starts the
// block (count >= max_attempts), so up to max_attempts - 1 attempts succeed.
const RATE_LIMIT_MAX_ATTEMPTS: u32 = 5;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_BLOCK_SECS: u64 = 300;

/// Lua script for atomic Redis rate limiting.
///
/// KEYS[1] = counter key, KEYS[2] = block key
/// ARGV[1] = max_attempts, ARGV[2] = window_secs, ARGV[3] = block_secs
///
/// Returns:
///   count (>=1) if allowed
///   -1 if currently blocked
///   -2 if this request exceeded the limit (and block was applied)
const RATE_LIMIT_SCRIPT: &str = r#"
local counter_key = KEYS[1]
local block_key = KEYS[2]
local max_attempts = tonumber(ARGV[1])
local window_secs = tonumber(ARGV[2])
local block_secs = tonumber(ARGV[3])

-- If currently blocked, reject
if redis.call('EXISTS', block_key) == 1 then
    return -1
end

-- Increment counter and set TTL on first hit
local count = redis.call('INCR', counter_key)
if count == 1 then
    redis.call('EXPIRE', counter_key, window_secs)
end

-- If limit reached, apply block
if count >= max_attempts then
    if block_secs > 0 then
        redis.call('SETEX', block_key, block_secs, 1)
    end
    -- Clear the counter so it resets fresh after the block expires
    redis.call('DEL', counter_key)
    return -2
end

return count
"#;

#[derive(Clone, Copy)]
struct RateLimitEntry {
    count: u32,
    blocked_until: Option<Instant>,
}

/// Rate limiter with optional Redis persistence.
///
/// When Redis is configured, rate-limit state survives server restarts.
/// If Redis becomes unreachable at runtime, the limiter transparently falls
/// back to in-memory counting and logs a warning.
#[derive(Clone)]
pub struct RateLimiter {
    /// Always present — in-memory fallback (and sole backend when Redis is off)
    cache: Arc<DashMap<String, (Instant, RateLimitEntry)>>,
    /// Optional Redis connection for persistent rate limiting.
    /// `ConnectionManager` is internally Arc'd and cheap to clone.
    redis: Option<ConnectionManager>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    /// Create a memory-only rate limiter (no Redis persistence).
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            redis: None,
        }
    }

    /// Create a rate limiter backed by Redis for persistence.
    /// Falls back to in-memory counting if Redis is unreachable.
    pub fn with_redis(redis: ConnectionManager) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            redis: Some(redis),
        }
    }

    /// Atomically check and increment rate limit for the given key.
    /// Returns Ok(()) if allowed, Err(()) if rate-limited.
    pub async fn check(&self, key: &str) -> Result<(), ()> {
        self.check_with(
            key,
            RATE_LIMIT_MAX_ATTEMPTS,
            RATE_LIMIT_WINDOW_SECS,
            RATE_LIMIT_BLOCK_SECS,
        )
        .await
    }

    /// Like `check` but with custom parameters.
    pub async fn check_with(
        &self,
        key: &str,
        max_attempts: u32,
        window_secs: u64,
        block_secs: u64,
    ) -> Result<(), ()> {
        if let Some(redis) = &self.redis {
            match self
                .check_redis(redis, key, max_attempts, window_secs, block_secs)
                .await
            {
                Ok(result) => return result,
                Err(redis_err) => {
                    tracing::warn!(
                        key = %log_safe(key),
                        error = %redis_err,
                        "Redis rate limit failed, falling back to in-memory"
                    );
                }
            }
        }
        self.check_memory(key, max_attempts, window_secs, block_secs)
            .await
    }

    /// Redis-backed rate limiting via Lua script (atomic).
    /// Returns Ok(Ok(())) if allowed, Ok(Err(())) if rate-limited,
    /// Err(msg) if Redis communication failed.
    async fn check_redis(
        &self,
        redis: &ConnectionManager,
        key: &str,
        max_attempts: u32,
        window_secs: u64,
        block_secs: u64,
    ) -> Result<Result<(), ()>, String> {
        let counter_key = format!("rl:c:{}", key);
        let block_key = format!("rl:b:{}", key);

        // ConnectionManager is Clone-safe for concurrent use
        let mut conn = redis.clone();
        let result: i64 = redis::Script::new(RATE_LIMIT_SCRIPT)
            .key(&counter_key)
            .key(&block_key)
            .arg(max_attempts as i64)
            .arg(window_secs as i64)
            .arg(block_secs as i64)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| format!("{e}"))?;

        match result {
            -1 => {
                tracing::warn!(key = %log_safe(key), "rate limited: blocked (Redis)");
                Ok(Err(()))
            }
            -2 => {
                tracing::warn!(
                    key = %log_safe(key),
                    max = max_attempts,
                    block_secs = block_secs,
                    "rate limit exceeded, blocking (Redis)"
                );
                Ok(Err(()))
            }
            _ => Ok(Ok(())),
        }
    }

    /// In-memory rate limiting (original DashMap-based logic).
    async fn check_memory(
        &self,
        key: &str,
        max_attempts: u32,
        window_secs: u64,
        block_secs: u64,
    ) -> Result<(), ()> {
        let now = Instant::now();

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
                tracing::warn!(key = %log_safe(key), "rate limited: blocked until {:?}", until);
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

        entry.count = entry.count.saturating_add(1);

        // The attempt that reaches the limit is itself rejected: a client is
        // allowed max_attempts - 1 successful calls before the block kicks in.
        // (Deliberately `>=`, not `>` — existing callers and tests rely on it.)
        if entry.count >= max_attempts {
            entry.blocked_until = Some(now + Duration::from_secs(block_secs));
            tracing::warn!(
                key = %log_safe(key),
                count = entry.count,
                max = max_attempts,
                block_secs = block_secs,
                "rate limit exceeded, blocking"
            );
            Err(())
        } else {
            Ok(())
        }
    }

    pub async fn reset(&self, key: &str) {
        // Clear Redis keys if present
        if let Some(redis) = &self.redis {
            let counter_key = format!("rl:c:{}", key);
            let block_key = format!("rl:b:{}", key);
            let mut conn = redis.clone();
            let _: Result<i64, _> = redis::cmd("DEL")
                .arg(&counter_key)
                .arg(&block_key)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    tracing::warn!(key = %log_safe(key), "Redis DEL error during reset: {}", e);
                });
        }
        self.cache.remove(key);
    }

    /// Remove expired entries to prevent memory leak.
    /// Redis keys expire automatically via TTL — only the in-memory cache is cleaned.
    /// Should be called periodically (e.g., every 5 minutes).
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.cache.retain(|_, (expires_at, entry)| {
            // Keep if not expired OR if blocked and block hasn't expired
            if now < *expires_at {
                return true;
            }
            if let Some(until) = entry.blocked_until {
                now < until
            } else {
                false
            }
        });
    }
}

/// Keys can embed user-supplied input (usernames, IPs) — strip control
/// characters before they reach the log so an attacker cannot forge log lines.
fn log_safe(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// Start a background task that periodically cleans up expired entries from both rate limiters.
pub fn start_cleanup_task(limiter: RateLimiter, ip_limiter: RateLimiter) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await; // Every 5 minutes
            limiter.cleanup_expired();
            ip_limiter.cleanup_expired();
            tracing::debug!("rate limiter cleanup completed");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_attempts_up_to_limit_then_blocks() {
        let limiter = RateLimiter::new();
        // The attempt that reaches the limit is itself rejected: with
        // max_attempts = 3 the third call is blocked.
        for _ in 0..RATE_LIMIT_MAX_ATTEMPTS - 1 {
            assert!(limiter.check("user").await.is_ok());
        }
        assert!(limiter.check("user").await.is_err());
        assert!(limiter.check("user").await.is_err());
    }

    #[tokio::test]
    async fn keys_are_independent() {
        let limiter = RateLimiter::new();
        for _ in 0..RATE_LIMIT_MAX_ATTEMPTS - 1 {
            assert!(limiter.check("a").await.is_ok());
        }
        assert!(limiter.check("a").await.is_err());
        assert!(limiter.check("b").await.is_ok());
    }

    #[tokio::test]
    async fn block_expires_and_resets_counter() {
        let limiter = RateLimiter::new();
        let key = "expire";
        assert!(limiter.check_with(key, 2, 1, 1).await.is_ok());
        assert!(limiter.check_with(key, 2, 1, 1).await.is_err());
        tokio::time::sleep(Duration::from_millis(1100)).await;
        // After the block expires the counter is reset: a fresh budget.
        assert!(limiter.check_with(key, 2, 1, 1).await.is_ok());
        assert!(limiter.check_with(key, 2, 1, 1).await.is_err());
    }

    #[tokio::test]
    async fn window_expiry_resets_budget() {
        let limiter = RateLimiter::new();
        let key = "window";
        assert!(limiter.check_with(key, 2, 1, 0).await.is_ok());
        // Exceeded: rejected, but block_secs=0 means the block expires
        // immediately and the next request starts a fresh window.
        assert!(limiter.check_with(key, 2, 1, 0).await.is_err());
        assert!(limiter.check_with(key, 2, 1, 0).await.is_ok());
    }

    #[tokio::test]
    async fn reset_clears_state() {
        let limiter = RateLimiter::new();
        let key = "reset";
        for _ in 0..RATE_LIMIT_MAX_ATTEMPTS - 1 {
            assert!(limiter.check(key).await.is_ok());
        }
        assert!(limiter.check(key).await.is_err());
        limiter.reset(key).await;
        assert!(limiter.check(key).await.is_ok());
    }

    #[tokio::test]
    async fn max_attempts_one_rejects_immediately() {
        let limiter = RateLimiter::new();
        // With max_attempts = 1 even the first call is rejected (and starts
        // the block), matching the `>=` semantics used everywhere.
        assert!(limiter.check_with("k", 1, 60, 60).await.is_err());
        assert!(limiter.check_with("k", 1, 60, 60).await.is_err());
    }

    #[tokio::test]
    async fn keys_are_exact_strings() {
        let limiter = RateLimiter::new();
        // max_attempts = 2: exactly one call succeeds, the second is rejected
        assert!(limiter.check_with("user", 2, 60, 60).await.is_ok());
        assert!(limiter.check_with("user", 2, 60, 60).await.is_err());
        // Different case and a different prefix are unrelated keys
        assert!(limiter.check_with("User", 2, 60, 60).await.is_ok());
        assert!(limiter.check_with("us", 2, 60, 60).await.is_ok());
        assert!(limiter.check_with("", 2, 60, 60).await.is_ok());
    }

    #[tokio::test]
    async fn cleanup_removes_expired_plain_entries() {
        let limiter = RateLimiter::new();
        assert!(limiter.check_with("k", 10, 1, 0).await.is_ok());
        assert!(!limiter.cache.is_empty());
        tokio::time::sleep(Duration::from_millis(1100)).await;
        limiter.cleanup_expired();
        assert!(limiter.cache.is_empty());
    }

    #[tokio::test]
    async fn cleanup_keeps_blocked_entries_until_block_expires() {
        let limiter = RateLimiter::new();
        assert!(limiter.check_with("k", 2, 1, 2).await.is_ok());
        assert!(limiter.check_with("k", 2, 1, 2).await.is_err());
        // After the window (1s) expires the block (2s) is still active: the
        // entry must survive cleanup so the client stays blocked.
        tokio::time::sleep(Duration::from_millis(1100)).await;
        limiter.cleanup_expired();
        assert!(!limiter.cache.is_empty());
        // Once the block expires too, cleanup must reclaim the entry.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        limiter.cleanup_expired();
        assert!(limiter.cache.is_empty());
    }

    #[test]
    fn log_safe_strips_control_characters() {
        assert_eq!(log_safe("alice"), "alice");
        assert_eq!(log_safe("a\nb"), "a?b");
        assert_eq!(log_safe("a\rb"), "a?b");
        assert_eq!(log_safe("a\tb"), "a?b");
        assert_eq!(log_safe("\u{1b}[31m"), "?[31m");
        assert_eq!(log_safe("a\u{7f}b"), "a?b");
        assert_eq!(log_safe("\u{0}"), "?");
        // Non-ASCII printable text is preserved
        assert_eq!(log_safe("用户"), "用户");
        // A fully-printable fake log line is NOT mangled (attacker-controlled
        // content only gets neutralised when it is actually a control char)
        assert_eq!(
            log_safe("2024-01-01 OK user=alice"),
            "2024-01-01 OK user=alice"
        );
    }
}
