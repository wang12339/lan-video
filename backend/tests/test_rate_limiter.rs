// Integration tests for RateLimiter
// Run with: cargo test --test test_rate_limiter -- --nocapture

use std::time::Duration;

use atmos_video_backend::middleware::rate_limit::RateLimiter;

/// Test that rate limiting blocks after max attempts
#[tokio::test]
async fn test_rate_limiter_blocks_after_max() {
    let limiter = RateLimiter::new();
    let key = "test_blocks_after_max";

    // First 4 attempts should succeed (max_attempts = 5, block triggers at count >= 5)
    for i in 0..4 {
        assert!(
            limiter.check(key).await.is_ok(),
            "attempt {} should be allowed",
            i + 1
        );
    }

    // 5th attempt should trigger block
    assert!(
        limiter.check(key).await.is_err(),
        "5th attempt should be blocked (rate limited)"
    );

    // Subsequent attempts should also be blocked
    assert!(
        limiter.check(key).await.is_err(),
        "6th attempt should still be blocked"
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

    // Exhaust rate limit (5/60s)
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
    assert!(limiter.check(key).await.is_ok()); // count = 3
    assert!(limiter.check(key).await.is_ok()); // count = 4
    assert!(limiter.check(key).await.is_err()); // count = 5, blocked
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

// ── Concurrency / window / block expiry boundaries ──

/// Concurrent calls on the same key: the atomic counter must guarantee
/// exactly max_attempts - 1 successes regardless of arrival order.
#[tokio::test]
async fn test_rate_limiter_concurrent_same_key() {
    let limiter = RateLimiter::new();
    let key = "test_concurrent_same";

    let mut tasks = Vec::new();
    for _ in 0..8 {
        let l = limiter.clone();
        tasks.push(tokio::spawn(async move {
            l.check_with(key, 3, 60, 600).await.is_ok()
        }));
    }
    let mut ok = 0;
    for t in tasks {
        if t.await.unwrap() {
            ok += 1;
        }
    }
    assert_eq!(
        ok, 2,
        "exactly max_attempts-1 concurrent calls may succeed, got {ok}"
    );
}

/// Concurrent calls on distinct keys must never interfere.
#[tokio::test]
async fn test_rate_limiter_concurrent_distinct_keys() {
    let limiter = RateLimiter::new();

    let mut tasks = Vec::new();
    for i in 0..8 {
        let l = limiter.clone();
        tasks.push(tokio::spawn(async move {
            l.check(&format!("test_concurrent_key_{i}")).await.is_ok()
        }));
    }
    for t in tasks {
        assert!(t.await.unwrap(), "distinct-key call should be allowed");
    }
}

/// After the block duration elapses the key recovers (block + counter reset).
#[tokio::test]
async fn test_rate_limiter_block_auto_expires() {
    let limiter = RateLimiter::new();
    let key = "test_block_expire";

    assert!(limiter.check_with(key, 3, 60, 1).await.is_ok());
    assert!(limiter.check_with(key, 3, 60, 1).await.is_ok());
    assert!(limiter.check_with(key, 3, 60, 1).await.is_err()); // 1s block
    assert!(
        limiter.check_with(key, 3, 60, 1).await.is_err(),
        "still blocked before expiry"
    );

    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        limiter.check_with(key, 3, 60, 1).await.is_ok(),
        "block should expire and renew the budget"
    );
}

/// When the window elapses without a block, the counter resets to a fresh
/// budget (window expiry must renew the allowance).
#[tokio::test]
async fn test_rate_limiter_window_expiry_renews_budget() {
    let limiter = RateLimiter::new();
    let key = "test_window_renew";

    assert!(limiter.check_with(key, 2, 1, 1).await.is_ok()); // count 1
    assert!(limiter.check_with(key, 2, 1, 1).await.is_err()); // blocked, 1s block

    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        limiter.check_with(key, 2, 1, 1).await.is_ok(),
        "window expiry should renew the budget"
    );
}

/// Partially-used window: a single attempt then an expiry restores the full
/// budget (count resets to 0, not 1).
#[tokio::test]
async fn test_rate_limiter_partial_window_use_then_expiry() {
    let limiter = RateLimiter::new();
    let key = "test_partial_window";

    assert!(limiter.check_with(key, 3, 1, 600).await.is_ok()); // count 1
    tokio::time::sleep(Duration::from_millis(1100)).await;

    // Full budget restored after expiry
    assert!(limiter.check_with(key, 3, 1, 600).await.is_ok());
    assert!(limiter.check_with(key, 3, 1, 600).await.is_ok());
    assert!(limiter.check_with(key, 3, 1, 600).await.is_err());
}

/// window_secs = 0: every call starts a fresh window, so it never blocks.
#[tokio::test]
async fn test_rate_limiter_zero_window_never_blocks() {
    let limiter = RateLimiter::new();
    let key = "test_zero_window";

    for _ in 0..50 {
        assert!(
            limiter.check_with(key, 3, 0, 600).await.is_ok(),
            "zero-length window must never accumulate a block"
        );
    }
}

/// block_secs = 0: the block expires immediately, so the next call recovers.
#[tokio::test]
async fn test_rate_limiter_zero_block_instant_recovery() {
    let limiter = RateLimiter::new();
    let key = "test_zero_block";

    assert!(limiter.check_with(key, 2, 60, 0).await.is_ok()); // count 1
    assert!(limiter.check_with(key, 2, 60, 0).await.is_err()); // blocked, until = now+0
    assert!(
        limiter.check_with(key, 2, 60, 0).await.is_ok(),
        "zero-duration block should allow immediate recovery"
    );
}

/// Separate RateLimiter instances are fully independent, even for the same key.
#[tokio::test]
async fn test_rate_limiter_separate_instances_independent() {
    let l1 = RateLimiter::new();
    let l2 = RateLimiter::new();

    for _ in 0..5 {
        let _ = l1.check("shared_key").await;
    }
    assert!(l1.check("shared_key").await.is_err());
    assert!(
        l2.check("shared_key").await.is_ok(),
        "a second instance must not share state"
    );
}

/// Empty, Unicode and very long keys must work; exhausting one key must not
/// affect the others.
#[tokio::test]
async fn test_rate_limiter_exotic_keys() {
    let limiter = RateLimiter::new();
    let empty = "";
    let unicode = "中文键🎵";
    let long = &"k".repeat(5000);

    assert!(limiter.check(empty).await.is_ok());
    assert!(limiter.check(unicode).await.is_ok());
    assert!(limiter.check(long).await.is_ok());

    for _ in 0..5 {
        let _ = limiter.check(unicode).await;
    }
    assert!(limiter.check(unicode).await.is_err(), "unicode key blocked");
    assert!(
        limiter.check(empty).await.is_ok(),
        "empty key must be unaffected"
    );
    assert!(
        limiter.check(long).await.is_ok(),
        "long key must be unaffected"
    );
}

/// Keys containing control characters (user-supplied) must not panic and must
/// be rate-limited independently (log_safe path).
#[tokio::test]
async fn test_rate_limiter_control_char_key() {
    let limiter = RateLimiter::new();
    let key = "user\nwith\r\ncontrol\ttabs";

    assert!(limiter.check(key).await.is_ok());
    assert!(limiter.check(key).await.is_ok());
    assert!(limiter.check(key).await.is_ok());
    assert!(limiter.check(key).await.is_ok());
    assert!(limiter.check(key).await.is_err());
    assert!(limiter.check("clean_key").await.is_ok());
}

/// cleanup_expired must keep live (blocked) entries.
#[tokio::test]
async fn test_rate_limiter_cleanup_keeps_live_blocked_entry() {
    let limiter = RateLimiter::new();
    let key = "test_cleanup_live";

    for _ in 0..5 {
        let _ = limiter.check(key).await;
    }
    assert!(limiter.check(key).await.is_err());

    limiter.cleanup_expired();
    assert!(
        limiter.check(key).await.is_err(),
        "an actively blocked entry must survive cleanup"
    );
}

/// cleanup_expired must drop entries whose window AND block have expired,
/// restoring a fresh budget afterwards.
#[tokio::test]
async fn test_rate_limiter_cleanup_removes_stale_entry() {
    let limiter = RateLimiter::new();
    let key = "test_cleanup_stale";

    assert!(limiter.check_with(key, 3, 1, 1).await.is_ok());
    assert!(limiter.check_with(key, 3, 1, 1).await.is_ok());
    assert!(limiter.check_with(key, 3, 1, 1).await.is_err());

    tokio::time::sleep(Duration::from_millis(1100)).await;
    limiter.cleanup_expired();
    assert!(
        limiter.check_with(key, 3, 1, 1).await.is_ok(),
        "stale entry should have been removed by cleanup"
    );
}

/// reset mid-block then re-exhaust: reset must fully clear the state.
#[tokio::test]
async fn test_rate_limiter_reset_mid_block_then_reblocks() {
    let limiter = RateLimiter::new();
    let key = "test_reset_reblock";

    for _ in 0..5 {
        let _ = limiter.check(key).await;
    }
    assert!(limiter.check(key).await.is_err());

    limiter.reset(key).await;
    assert!(limiter.check(key).await.is_ok());
    assert!(limiter.check(key).await.is_ok());
    assert!(limiter.check(key).await.is_ok());
    assert!(limiter.check(key).await.is_ok());
    assert!(
        limiter.check(key).await.is_err(),
        "should re-block after reset and re-exhaustion"
    );
}

/// Exact boundary: with max_attempts = 5 exactly 4 calls may succeed.
#[tokio::test]
async fn test_rate_limiter_max_attempts_exact_boundary() {
    let limiter = RateLimiter::new();
    let key = "test_max_boundary";

    for _ in 0..4 {
        assert!(
            limiter.check_with(key, 5, 60, 600).await.is_ok(),
            "attempts below max must succeed"
        );
    }
    assert!(
        limiter.check_with(key, 5, 60, 600).await.is_err(),
        "the attempt that reaches max must be blocked"
    );
}
