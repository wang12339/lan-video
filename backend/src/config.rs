use std::path::PathBuf;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_port: u16,
    pub media_root: PathBuf,
    pub webapp_root: PathBuf,
    pub registration_enabled: bool,
    pub cors_origin: String,
    pub cookie_secure: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| {
                tracing::warn!("DATABASE_URL not set, using default (set .env for production)");
                "postgres://kuaile@localhost:5432/lan_video".into()
            });

        let server_port = std::env::var("SERVER_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8082u16);

        let media_root = std::env::var("MEDIA_ROOT")
            .unwrap_or_else(|_| "./media".into());

        let webapp_root = std::env::var("WEBA_ROOT")
            .unwrap_or_else(|_| "./webapp".into());

        let registration_enabled = std::env::var("REGISTRATION_ENABLED")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let cors_origin = std::env::var("CORS_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:8082".into());

        let cookie_secure = std::env::var("COOKIE_SECURE")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        AppConfig {
            database_url,
            server_port,
            media_root: PathBuf::from(media_root),
            webapp_root: PathBuf::from(webapp_root),
            registration_enabled,
            cors_origin,
            cookie_secure,
        }
    }
}
