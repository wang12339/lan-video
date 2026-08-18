use redis::aio::ConnectionManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

static REDIS: OnceCell<Option<Arc<ConnectionManager>>> = OnceCell::const_new();

/// First reconnect attempt delay, doubled up to MAX_BACKOFF on failure.
const INITIAL_RETRY_BACKOFF: Duration = Duration::from_secs(5);
const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(30);

/// Establish the shared Redis connection manager.
///
/// If Redis is down at startup the function returns `None` (non-fatal) but a
/// background task keeps retrying with exponential backoff, so Redis support
/// comes up automatically once the server is reachable again. A bad URL is a
/// configuration error and is NOT retried.
pub async fn init_redis(url: &str) -> Option<Arc<ConnectionManager>> {
    let url = url.trim();
    if url.is_empty() {
        tracing::info!("Redis disabled: no REDIS_URL configured");
        return None;
    }
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Invalid REDIS_URL, Redis disabled: {}", e);
            return None;
        }
    };

    // Another init_redis call may have raced us; don't create a second manager.
    if let Some(existing) = get_redis().await {
        return Some(existing);
    }

    match ConnectionManager::new(client.clone()).await {
        Ok(cm) => {
            tracing::info!("Connected to Redis");
            let cm = Arc::new(cm);
            let _ = REDIS.set(Some(cm.clone()));
            Some(cm)
        }
        Err(e) => {
            tracing::warn!(
                "Redis connection failed at startup (non-fatal, retrying in background): {}",
                e
            );
            tokio::spawn(async move { retry_connect_until_ready(client).await });
            None
        }
    }
}

/// Retry connecting in the background until it succeeds. The manager itself
/// reconnects transparently after this point, so only initial connection
/// (and this bootstrap) needs a retry loop.
async fn retry_connect_until_ready(client: redis::Client) {
    let mut backoff = INITIAL_RETRY_BACKOFF;
    loop {
        tokio::time::sleep(backoff).await;
        match ConnectionManager::new(client.clone()).await {
            Ok(cm) => {
                tracing::info!("Redis reconnected");
                let cm = Arc::new(cm);
                match REDIS.set(Some(cm.clone())) {
                    Ok(()) => return,
                    // Someone else initialized it meanwhile.
                    Err(_) => return,
                }
            }
            Err(e) => {
                tracing::warn!("Redis reconnect attempt failed, retrying: {}", e);
                backoff = (backoff * 2).min(MAX_RETRY_BACKOFF);
            }
        }
    }
}

pub async fn get_redis() -> Option<Arc<ConnectionManager>> {
    REDIS.get().cloned().flatten()
}
