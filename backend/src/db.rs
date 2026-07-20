use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Threshold above which a SQL query is considered slow and logged as a warning.
pub const SLOW_QUERY_THRESHOLD: Duration = Duration::from_millis(100);

/// Run a SQL operation and emit a `tracing::warn!` if it takes longer than
/// `SLOW_QUERY_THRESHOLD`. Use this around repository query calls to make
/// hot paths observable in production.
///
/// Example:
/// ```ignore
/// let rows = log_slow_query("list_videos", async {
///     sqlx::query_as::<_, VideoRow>("SELECT ...").fetch_all(&pool).await
/// }).await?;
/// ```
pub async fn log_slow_query<T, E, F, Fut>(label: &str, fut: F) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let start = Instant::now();
    let result = fut().await;
    let elapsed = start.elapsed();
    if elapsed > SLOW_QUERY_THRESHOLD {
        match &result {
            Ok(_) => warn!(query = %label, duration_ms = %elapsed.as_millis(), "slow query"),
            Err(e) => warn!(query = %label, duration_ms = %elapsed.as_millis(), error = %e, "slow query failed"),
        }
    }
    result
}

fn get_migrations_dir() -> PathBuf {
    std::env::var("MIGRATIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations"))
}

fn discover_migrations() -> Vec<(String, String)> {
    let dir = get_migrations_dir();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("Failed to read migrations directory {:?}: {}", dir, e))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "sql"))
        .collect();

    entries.sort_by_key(|entry| entry.file_name());

    entries
        .into_iter()
        .map(|entry| {
            let mut name = entry
                .file_name()
                .into_string()
                .expect("Migration filename is not valid UTF-8");
            name.truncate(name.len() - 4);
            let path = entry.path();
            let sql = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read migration file {:?}: {}", path, e));
            (name, sql)
        })
        .collect()
}

pub async fn init_pool(database_url: &str) -> PgPool {
    let max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let max_retries = 5;
    let mut attempt = 0u32;

    let pool = loop {
        match PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(database_url)
            .await
        {
            Ok(pool) => break pool,
            Err(e) => {
                attempt += 1;
                if attempt >= max_retries {
                    panic!(
                        "Failed to connect to PostgreSQL after {} attempts: {}",
                        max_retries, e
                    );
                }
                let wait_ms = 500u64 * 2u64.pow(attempt - 1);
                tracing::warn!(
                    "DB connection attempt {} failed ({}), retrying in {}ms...",
                    attempt,
                    e,
                    wait_ms
                );
                tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
            }
        }
    };

    info!("Database connection pool established");

    // Run migrations with version tracking
    run_migrations(&pool).await;

    pool
}

async fn run_migrations(pool: &PgPool) {
    // Ensure the migration tracking table exists
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _schema_migrations (
            version VARCHAR(255) PRIMARY KEY,
            applied_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("Failed to create _schema_migrations table: {}", e));

    let migrations = discover_migrations();

    for (name, sql) in migrations {
        let already_applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _schema_migrations WHERE version = $1)",
        )
        .bind(&name)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if already_applied {
            continue;
        }

        // Run each migration in a transaction for safe rollback on failure
        let mut tx = pool.begin().await.unwrap_or_else(|e| {
            panic!("Failed to begin transaction for migration {}: {}", name, e)
        });

        sqlx::raw_sql(&sql)
            .execute(&mut *tx)
            .await
            .unwrap_or_else(|e| panic!("Failed to run migration {}: {}", name, e));

        sqlx::query("INSERT INTO _schema_migrations (version) VALUES ($1)")
            .bind(&name)
            .execute(&mut *tx)
            .await
            .unwrap_or_else(|e| panic!("Failed to record migration {}: {}", name, e));

        tx.commit()
            .await
            .unwrap_or_else(|e| panic!("Failed to commit migration {}: {}", name, e));

        info!("Migration '{}' applied successfully", name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn log_slow_query_passes_through_result() {
        let result: Result<i32, &str> = log_slow_query("test", || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn log_slow_query_passes_through_error() {
        let result: Result<i32, &str> =
            log_slow_query("test", || async { Err("boom") }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn log_slow_query_does_not_log_fast_queries() {
        // A 1ms query should NOT trigger a warning. We can't easily inspect
        // tracing output, but we can at least verify the future completes
        // and the threshold is well above the operation time.
        let start = std::time::Instant::now();
        let _: Result<(), &str> = log_slow_query("fast", || async { Ok(()) }).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
