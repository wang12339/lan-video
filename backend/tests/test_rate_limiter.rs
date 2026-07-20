// Integration tests for RateLimiter
// Run with: cargo test --test test_rate_limiter -- --nocapture

use lan_video_backend::middleware::rate_limit::RateLimiter;

/// Test that rate limiting blocks after max attempts
#[tokio::test]
async fn test_rate_limiter_blocks_after_max() {
    let limiter = RateLimiter::new();
    let key = "test_blocks_after_max";

    // First 2 attempts should succeed (max_attempts = 3, block triggers at count >= 3)
    for i in 0..2 {
        assert!(
            limiter.check(key).await.is_ok(),
            "attempt {} should be allowed",
            i + 1
        );
    }

    // 3rd attempt should trigger block
    assert!(
        limiter.check(key).await.is_err(),
        "3rd attempt should be blocked (rate limited)"
    );

    // Subsequent attempts should also be blocked
    assert!(
        limiter.check(key).await.is_err(),
        "4th attempt should still be blocked"
    );
}

/// Test that check_with custom parameters work correctly
#[tokio::test]
async fn test_rate_limiter_check_with_custom_params() {
    let limiter = RateLimiter::new();
    let key = "test_custom_params";

    // Use custom params: max 3 attempts, 60s window, 600s block
    for i in 0..2 {
        assert!(
            limiter.check_with(key, 3, 60, 600).await.is_ok(),
            "attempt {} should be allowed with custom max=3",
            i + 1
        );
    }

    // 3rd attempt should trigger block
    assert!(
        limiter.check_with(key, 3, 60, 600).await.is_err(),
        "3rd attempt should be blocked with custom max=3"
    );
}

/// Test that different keys are independent
#[tokio::test]
async fn test_rate_limiter_independent_keys() {
    let limiter = RateLimiter::new();

    // Exhaust rate limit on key_a
    for _ in 0..5 {
        let _ = limiter.check("key_a").await;
    }
    assert!(
        limiter.check("key_a").await.is_err(),
        "key_a should be blocked"
    );

    // key_b should still be allowed
    assert!(
        limiter.check("key_b").await.is_ok(),
        "key_b should not be affected by key_a's rate limit"
    );
}

/// Test that reset clears the rate limit
#[tokio::test]
async fn test_rate_limiter_reset() {
    let limiter = RateLimiter::new();
    let key = "test_reset";

    // Exhaust rate limit
    for _ in 0..5 {
        let _ = limiter.check(key).await;
    }
    assert!(
        limiter.check(key).await.is_err(),
        "should be blocked before reset"
    );

    // Reset
    limiter.reset(key).await;

    // Should be allowed again
    assert!(
        limiter.check(key).await.is_ok(),
        "should be allowed after reset"
    );
}

/// Test that reset on non-existent key is a no-op
#[tokio::test]
async fn test_rate_limiter_reset_nonexistent() {
    let limiter = RateLimiter::new();
    // Should not panic
    limiter.reset("nonexistent_key").await;
    // Should still work normally
    assert!(limiter.check("nonexistent_key").await.is_ok());
}

/// Test that max_attempts=1 blocks immediately on first attempt
#[tokio::test]
async fn test_rate_limiter_max_attempts_one() {
    let limiter = RateLimiter::new();
    let key = "test_max_one";

    assert!(
        limiter.check_with(key, 1, 60, 600).await.is_err(),
        "with max_attempts=1, first attempt should be blocked"
    );
}

/// Test that max_attempts=0 blocks immediately
#[tokio::test]
async fn test_rate_limiter_max_attempts_zero() {
    let limiter = RateLimiter::new();
    let key = "test_max_zero";

    assert!(
        limiter.check_with(key, 0, 60, 600).await.is_err(),
        "with max_attempts=0, should always be blocked"
    );
}

/// Basic single-threaded DashMap atomicity test
/// Verifies that sequential operations see consistent state
#[tokio::test]
async fn test_rate_limiter_consistency() {
    let limiter = RateLimiter::new();
    let key = "test_consistency";

    // Interleave operations on the same key
    assert!(limiter.check(key).await.is_ok()); // count = 1
    assert!(limiter.check(key).await.is_ok()); // count = 2
    limiter.reset(key).await; // cleared
    assert!(limiter.check(key).await.is_ok()); // count = 1 (fresh)
    assert!(limiter.check(key).await.is_ok()); // count = 2
    assert!(limiter.check(key).await.is_err()); // count = 3, blocked
}

/// Test that check_with with very large max_attempts never blocks
#[tokio::test]
async fn test_rate_limiter_large_max_attempts() {
    let limiter = RateLimiter::new();
    let key = "test_large_max";

    for _ in 0..100 {
        assert!(
            limiter.check_with(key, 1000, 60, 600).await.is_ok(),
            "should not block with large max_attempts"
        );
    }
}

/// Test that the default constructor works
#[tokio::test]
async fn test_rate_limiter_default() {
    let limiter = RateLimiter::default();
    assert!(limiter.check("test_default").await.is_ok());
}
