use redis::aio::ConnectionManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

/// First reconnect attempt delay, doubled up to MAX_BACKOFF on failure.
const INITIAL_RETRY_BACKOFF: Duration = Duration::from_secs(5);
const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(30);

/// A cell whose value may be published *after* handles have been handed out.
///
/// `tokio::sync::OnceCell` alone is not enough, because the interesting case is
/// the late write: the value must become visible to every handle that already
/// exists, and a racing writer must not clobber a value that is already there.
/// Cloning shares the single underlying cell, so all observers converge on the
/// first published value.
#[derive(Debug)]
pub struct LateCell<T> {
    inner: Arc<OnceCell<T>>,
}

impl<T> Clone for LateCell<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for LateCell<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LateCell<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(OnceCell::const_new()),
        }
    }

    /// Read the published value, if any.
    #[inline]
    pub fn get(&self) -> Option<&T> {
        self.inner.get()
    }

    /// Publish a value. The first writer wins; later calls are no-ops and
    /// report `false`.
    pub fn publish(&self, value: T) -> bool {
        self.inner.set(value).is_ok()
    }
}

type RedisCell = LateCell<Option<Arc<ConnectionManager>>>;

/// Shared handle to the process-wide Redis connection.
///
/// This is deliberately a *slot* and not an `Option<ConnectionManager>` snapshot.
/// The first connect attempt happens during startup, but Redis may be down at
/// that moment. A snapshot freezes "unavailable" into every holder for the whole
/// process lifetime: the background reconnect succeeds, the log even says
/// "Redis reconnected", yet distributed rate limiting and cross-instance
/// media-auth invalidation stay silently off. With a slot, whatever the reconnect
/// task publishes becomes visible to every existing holder immediately, and the
/// type system makes it impossible to store a stale snapshot again.
#[derive(Clone)]
pub struct SharedRedis {
    cell: RedisCell,
    /// Whether Redis is *wanted* (URL present and parseable), independent of
    /// whether the connection is up yet. When `false` the cell is pre-seeded with
    /// `None` so `resolve()` is a cheap non-blocking read.
    configured: bool,
}

impl std::fmt::Debug for SharedRedis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRedis")
            .field("configured", &self.configured)
            .field("connected", &self.is_connected())
            .finish()
    }
}

impl SharedRedis {
    /// A slot that will never hold a connection (no/invalid `REDIS_URL`).
    pub fn disabled() -> Self {
        let slot = Self {
            cell: RedisCell::new(),
            configured: false,
        };
        slot.cell.publish(None);
        slot
    }

    /// A configured slot that has not connected yet.
    ///
    /// Consumers built from it behave exactly as if Redis were absent, and
    /// transparently start using Redis the moment [`SharedRedis::install`]
    /// publishes a connection. This is the state the process is in between
    /// "startup connect failed" and "background reconnect succeeded".
    pub fn pending() -> Self {
        Self {
            cell: RedisCell::new(),
            configured: true,
        }
    }

    /// Wrap an already-established connection.
    pub fn connected(cm: ConnectionManager) -> Self {
        let slot = Self::pending();
        slot.install(cm);
        slot
    }

    /// Publish a connection into the slot, making it visible to every holder.
    ///
    /// Returns `false` if a connection was already installed: the first writer
    /// wins, so a racing reconnect never replaces an established connection.
    pub fn install(&self, cm: ConnectionManager) -> bool {
        self.cell.publish(Some(Arc::new(cm)))
    }

    /// Whether Redis was configured at all. `true` while still disconnected —
    /// callers that report health must not confuse "not configured" with
    /// "configured but down".
    pub fn is_configured(&self) -> bool {
        self.configured
    }

    /// Current connection, or `None` when unconfigured or not yet connected.
    #[inline]
    pub fn resolve(&self) -> Option<Arc<ConnectionManager>> {
        self.cell.get().cloned().flatten()
    }

    #[inline]
    pub fn is_connected(&self) -> bool {
        self.resolve().is_some()
    }

    /// Establish the shared Redis connection.
    ///
    /// Never fails: an empty or malformed URL yields a disabled slot, and a
    /// failed connect spawns a background task that retries with exponential
    /// backoff until it lands in the slot. A bad URL is a configuration error
    /// and is NOT retried.
    pub async fn init(url: &str) -> Self {
        let url = url.trim();
        if url.is_empty() {
            tracing::info!("Redis disabled: no REDIS_URL configured");
            return Self::disabled();
        }
        let client = match redis::Client::open(url) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Invalid REDIS_URL, Redis disabled: {}", e);
                return Self::disabled();
            }
        };

        let slot = Self::pending();

        match ConnectionManager::new(client.clone()).await {
            Ok(cm) => {
                tracing::info!("Connected to Redis");
                slot.install(cm);
            }
            Err(e) => {
                tracing::warn!(
                    "Redis connection failed at startup (non-fatal, retrying in background): {}",
                    e
                );
                let cell = slot.cell.clone();
                tokio::spawn(async move { retry_connect_until_ready(client, cell).await });
            }
        }

        slot
    }
}

/// Retry connecting in the background until it succeeds. The manager itself
/// reconnects transparently after this point, so only the initial connection
/// (and this bootstrap) needs a retry loop.
async fn retry_connect_until_ready(client: redis::Client, cell: RedisCell) {
    let mut backoff = INITIAL_RETRY_BACKOFF;
    loop {
        tokio::time::sleep(backoff).await;
        // Another task already installed a connection: nothing left to do.
        if cell.get().is_some_and(|cm| cm.is_some()) {
            return;
        }
        match ConnectionManager::new(client.clone()).await {
            Ok(cm) => {
                if cell.publish(Some(Arc::new(cm))) {
                    tracing::info!(
                        "Redis reconnected; distributed rate limiting and shared caches are now active"
                    );
                } else {
                    tracing::debug!("Redis reconnected but another task won the slot");
                }
                return;
            }
            Err(e) => {
                tracing::warn!("Redis reconnect attempt failed, retrying: {}", e);
                backoff = (backoff * 2).min(MAX_RETRY_BACKOFF);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_slot_is_unconfigured_and_never_connects() {
        let slot = SharedRedis::disabled();
        assert!(!slot.is_configured());
        assert!(!slot.is_connected());
        assert!(slot.resolve().is_none());
    }

    #[tokio::test]
    async fn empty_url_yields_disabled_slot() {
        let slot = SharedRedis::init("").await;
        assert!(!slot.is_configured());
        assert!(slot.resolve().is_none());
        // Whitespace-only is treated the same as unset.
        let slot = SharedRedis::init("   ").await;
        assert!(!slot.is_configured());
    }

    #[tokio::test]
    async fn malformed_url_yields_disabled_slot() {
        let slot = SharedRedis::init("not-a-redis-url").await;
        assert!(!slot.is_configured());
        assert!(slot.resolve().is_none());
    }

    // ---- LateCell semantics: the property the Redis slot depends on ----

    #[test]
    fn late_value_is_visible_to_every_existing_handle() {
        let cell = LateCell::<u32>::new();
        let early_a = cell.clone();
        let early_b = cell.clone();

        // Nothing published yet.
        assert!(cell.get().is_none());
        assert!(early_a.get().is_none());
        assert!(early_b.get().is_none());

        // Published *after* the handles were handed out.
        assert!(cell.publish(7));
        assert_eq!(*cell.get().unwrap(), 7);
        assert_eq!(*early_a.get().unwrap(), 7);
        assert_eq!(*early_b.get().unwrap(), 7);
    }

    #[test]
    fn first_publish_wins() {
        let cell = LateCell::<u32>::new();
        assert!(cell.publish(1));
        assert!(!cell.publish(2));
        assert_eq!(*cell.get().unwrap(), 1);
    }

    #[test]
    fn clones_share_one_cell() {
        let cell = LateCell::<String>::new();
        let clone = cell.clone();
        cell.publish("x".to_string());
        assert_eq!(clone.get().map(String::as_str), Some("x"));
    }

    #[test]
    fn pending_slot_is_configured_but_empty() {
        let slot = SharedRedis::pending();
        assert!(slot.is_configured());
        assert!(!slot.is_connected());
        assert!(slot.resolve().is_none());
        // A clone taken before the connection exists sees it afterwards.
        let early = slot.clone();
        assert!(early.resolve().is_none());
    }

    // NOTE: the `init()` connect-failure branch (spawn the background retry and
    // return a pending slot) is deliberately not unit-tested here.
    // `ConnectionManager::new` retries internally with its own backoff, so
    // pointing it at an unreachable address blocks for ~8 minutes before
    // returning. That branch is covered end-to-end instead, with a real
    // reconnect, by `tests/redis_integration.rs`
    // (`rate_limiter_picks_up_a_connection_published_after_construction`).
}
