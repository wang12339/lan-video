use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_port: u16,
    pub public_url: String,
    pub media_root: PathBuf,
    pub webapp_root: PathBuf,
    pub log_dir: PathBuf,
    pub data_dir: PathBuf,
    pub registration_enabled: std::sync::Arc<AtomicBool>,
    pub cors_origin: String,
    pub cookie_secure: bool,
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppConfig")
            .field("database_url", &self.database_url)
            .field("server_port", &self.server_port)
            .field("public_url", &self.public_url)
            .field("media_root", &self.media_root)
            .field("webapp_root", &self.webapp_root)
            .field("registration_enabled", &self.registration_enabled())
            .field("cors_origin", &self.cors_origin)
            .field("cookie_secure", &self.cookie_secure)
            .field("log_dir", &self.log_dir)
            .finish()
    }
}

impl AppConfig {
    pub fn registration_enabled(&self) -> bool {
        self.registration_enabled.load(Ordering::Relaxed)
    }

    pub fn set_registration_enabled(&self, val: bool) {
        self.registration_enabled.store(val, Ordering::Relaxed);
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            tracing::warn!("DATABASE_URL not set, using default (set .env for production)");
            "postgres://kuaile@localhost:5432/lan_video".into()
        });

        let server_port = std::env::var("SERVER_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8082u16);

        let media_root = std::env::var("MEDIA_ROOT").unwrap_or_else(|_| "./media".into());

        let webapp_root = std::env::var("WEBAPP_ROOT").unwrap_or_else(|_| "./webapp/dist".into());

        let registration_enabled = std::env::var("REGISTRATION_ENABLED")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let registration_enabled = std::sync::Arc::new(AtomicBool::new(registration_enabled));

        let public_url = std::env::var("PUBLIC_URL").unwrap_or_default();

        let cors_origin = std::env::var("CORS_ORIGIN").unwrap_or_else(|_| {
            // SECURITY: only include the dev origin when APP_ENV is explicitly
            // set to "development". Production deployments MUST set
            // CORS_ORIGIN explicitly. Empty default in production prevents the
            // CORS allowlist from inadvertently including localhost.
            match std::env::var("APP_ENV")
                .ok()
                .as_deref()
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("development") | Some("dev") | Some("local") => "http://localhost:8082".into(),
                _ => String::new(),
            }
        });

        let cookie_secure = std::env::var("COOKIE_SECURE")
            .ok()
            .map(|v| v == "true" || v == "1")
            // Default to true — only disable explicitly for local development over HTTP
            .unwrap_or(true);

        let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "./logs".into());

        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());

        AppConfig {
            database_url,
            server_port,
            public_url,
            media_root: PathBuf::from(media_root),
            webapp_root: PathBuf::from(webapp_root),
            log_dir: PathBuf::from(log_dir),
            data_dir: PathBuf::from(data_dir),
            registration_enabled,
            cors_origin,
            cookie_secure,
        }
    }
}
