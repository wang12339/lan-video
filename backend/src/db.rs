use sqlx::postgres::PgPoolOptions;
use sqlx::Acquire;
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
            Err(e) => {
                warn!(query = %label, duration_ms = %elapsed.as_millis(), error = %e, "slow query failed")
            }
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
    let max_connections: u32 = match std::env::var("DB_MAX_CONNECTIONS") {
        Ok(v) => match v.parse() {
            Ok(n) if n >= 1 => n,
            _ => {
                warn!("DB_MAX_CONNECTIONS '{}' invalid, defaulting to 100", v);
                100
            }
        },
        Err(_) => 100,
    };

    let max_retries = 5;
    let mut attempt = 0u32;

    let pool = loop {
        match PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .idle_timeout(std::time::Duration::from_secs(300)) // 空闲连接超时5分钟
            .max_lifetime(std::time::Duration::from_secs(1800)) // 连接最大生命周期30分钟
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
    // Use a dedicated connection for the whole migration run:
    // 1. A session-level advisory lock serializes concurrent server instances
    //    so two processes can't apply migrations at the same time (the lock is
    //    released automatically when the connection is dropped).
    // 2. The migration bookkeeping (check + apply + record) stays on one
    //    connection, avoiding read-your-own-writes races with other pool slots.
    let mut conn = pool
        .acquire()
        .await
        .unwrap_or_else(|e| panic!("Failed to acquire connection for migrations: {}", e));

    const MIGRATION_LOCK_KEY: i64 = 0x_4154_4D4F_5320_0001; // "ATMOS " marker
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .unwrap_or_else(|e| panic!("Failed to acquire migration advisory lock: {}", e));

    // Ensure the migration tracking table exists
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _schema_migrations (
            version VARCHAR(255) PRIMARY KEY,
            applied_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&mut *conn)
    .await
    .unwrap_or_else(|e| panic!("Failed to create _schema_migrations table: {}", e));

    let migrations = discover_migrations();

    for (name, sql) in migrations {
        let already_applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _schema_migrations WHERE version = $1)",
        )
        .bind(&name)
        .fetch_one(&mut *conn)
        .await
        .unwrap_or_else(|e| panic!("Failed to check migration status of '{}': {}", name, e));

        if already_applied {
            continue;
        }

        // Run each migration in a transaction so a failure rolls back fully
        // (PostgreSQL DDL is transactional). On failure we abort the server
        // start instead of marking the migration as applied — a half-applied
        // schema is worse than a loud crash, and the transaction guarantees
        // there is no partial state to recover from.
        let mut tx = (&mut *conn).begin().await.unwrap_or_else(|e| {
            panic!(
                "Failed to begin transaction for migration '{}': {}",
                name, e
            )
        });

        if let Err(e) = sqlx::raw_sql(&sql).execute(&mut *tx).await {
            let _ = tx.rollback().await;
            panic!(
                "Migration '{}' FAILED. Rolling back. Fix the migration file or \
                 manually mark it as applied (INSERT INTO _schema_migrations (version) \
                 VALUES ('{}')) if it was already applied out-of-band. Error: {}",
                name, name, e
            );
        }

        sqlx::query("INSERT INTO _schema_migrations (version) VALUES ($1)")
            .bind(&name)
            .execute(&mut *tx)
            .await
            .unwrap_or_else(|e| panic!("Failed to record migration '{}': {}", name, e));

        tx.commit()
            .await
            .unwrap_or_else(|e| panic!("Failed to commit migration '{}': {}", name, e));

        info!("Migration '{}' applied successfully", name);
    }

    // Release the advisory lock (best-effort — the session lock dies with the
    // connection anyway, and the pool returns it on the next acquire).
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .map_err(|e| warn!("Failed to release migration advisory lock: {}", e))
        .ok();
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
        let result: Result<i32, &str> = log_slow_query("test", || async { Err("boom") }).await;
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
