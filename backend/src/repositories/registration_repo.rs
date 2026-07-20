use sqlx::PgPool;

#[derive(Clone)]
pub struct RegistrationRepository {
    pool: PgPool,
}

impl RegistrationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_enabled(&self) -> Result<bool, sqlx::Error> {
        let result: Option<(String,)> =
            sqlx::query_as("SELECT value FROM server_config WHERE key = 'registration_enabled'")
                .fetch_optional(&self.pool)
                .await?;
        Ok(result.is_some_and(|r| r.0 == "true"))
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<(), sqlx::Error> {
        let val = if enabled { "true" } else { "false" };
        sqlx::query(
            "INSERT INTO server_config (key, value) VALUES ('registration_enabled', $1) ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = CURRENT_TIMESTAMP"
        )
        .bind(val)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
