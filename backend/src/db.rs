use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

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
                    attempt, e, wait_ms
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
        )"
    )
    .execute(pool)
    .await
    .unwrap_or_else(|e| panic!("Failed to create _schema_migrations table: {}", e));

    let migrations: Vec<(&str, &str)> = vec![
        ("001_create_users", include_str!("../migrations/001_create_users.sql")),
        ("002_create_auth_tokens", include_str!("../migrations/002_create_auth_tokens.sql")),
        ("003_create_videos", include_str!("../migrations/003_create_videos.sql")),
        ("004_create_playback_history", include_str!("../migrations/004_create_playback_history.sql")),
        ("005_add_indexes", include_str!("../migrations/005_add_indexes.sql")),
        ("006_add_foreign_keys", include_str!("../migrations/006_add_foreign_keys.sql")),
        ("007_add_thumb_url", include_str!("../migrations/007_add_thumb_url.sql")),
        ("008_add_video_stats", include_str!("../migrations/008_add_video_stats.sql")),
        ("009_add_token_expiry", include_str!("../migrations/009_add_token_expiry.sql")),
    ];

    for (name, sql) in migrations {
        let already_applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _schema_migrations WHERE version = $1)"
        )
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if already_applied {
            continue;
        }

        // Run each migration in a transaction for safe rollback on failure
        let mut tx = pool.begin().await
            .unwrap_or_else(|e| panic!("Failed to begin transaction for migration {}: {}", name, e));

        sqlx::raw_sql(sql)
            .execute(&mut *tx)
            .await
            .unwrap_or_else(|e| panic!("Failed to run migration {}: {}", name, e));

        sqlx::query("INSERT INTO _schema_migrations (version) VALUES ($1)")
            .bind(name)
            .execute(&mut *tx)
            .await
            .unwrap_or_else(|e| panic!("Failed to record migration {}: {}", name, e));

        tx.commit().await
            .unwrap_or_else(|e| panic!("Failed to commit migration {}: {}", name, e));

        info!("Migration '{}' applied successfully", name);
    }
}
