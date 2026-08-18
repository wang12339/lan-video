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
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from: String,
    pub redis_url: String,
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SECURITY: redact credentials from database_url in debug output
        let redacted_url = redact_url(&self.database_url);
        let redacted_redis = redact_url(&self.redis_url);
        f.debug_struct("AppConfig")
            .field("database_url", &redacted_url)
            .field("server_port", &self.server_port)
            .field("public_url", &self.public_url)
            .field("media_root", &self.media_root)
            .field("webapp_root", &self.webapp_root)
            .field("registration_enabled", &self.registration_enabled())
            .field("cors_origin", &self.cors_origin)
            .field("cookie_secure", &self.cookie_secure)
            .field("log_dir", &self.log_dir)
            .field("redis_url", &redacted_redis)
            .finish()
    }
}

/// Replace `scheme://user:pass@host` with `scheme://***:***@host`.
fn redact_url(url: &str) -> String {
    match url.find("://") {
        Some(pos) => {
            let scheme_and_rest = &url[pos + 3..];
            match scheme_and_rest.find('@') {
                Some(at) => format!("{}://***:***@{}", &url[..pos], &scheme_and_rest[at + 1..]),
                None => url.to_string(),
            }
        }
        None => url.to_string(),
    }
}

/// Parse a boolean env var, accepting case-insensitive `true`/`1` and
/// falling back to `default` when unset or unparseable.
fn parse_bool_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "true" | "1")
        })
        .unwrap_or(default)
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
            "postgres://kuaile@localhost:5432/atmos_video".into()
        });

        let server_port = std::env::var("SERVER_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or_else(|| {
                tracing::warn!("SERVER_PORT invalid or unset, defaulting to 8082");
                8082u16
            });

        let media_root = std::env::var("MEDIA_ROOT").unwrap_or_else(|_| "./media".into());

        let webapp_root = std::env::var("WEBAPP_ROOT").unwrap_or_else(|_| "./webapp/dist".into());

        let registration_enabled = parse_bool_env("REGISTRATION_ENABLED", false);

        let registration_enabled = std::sync::Arc::new(AtomicBool::new(registration_enabled));

        let public_url = std::env::var("PUBLIC_URL").unwrap_or_else(|_| {
            panic!("PUBLIC_URL must be set in production. This is the external-accessible base URL for share links, hotlink protection, and HTTPS redirects.");
        });

        let cors_origin = std::env::var("CORS_ORIGIN").unwrap_or_default();

        // Default to true — only disable explicitly for local development over HTTP
        let cookie_secure = parse_bool_env("COOKIE_SECURE", true);

        let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "./logs".into());

        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());

        let smtp_host = std::env::var("SMTP_HOST").unwrap_or_default();
        let smtp_port = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(587u16);
        let smtp_username = std::env::var("SMTP_USERNAME").unwrap_or_default();
        let smtp_password = std::env::var("SMTP_PASSWORD").unwrap_or_default();
        let smtp_from = std::env::var("SMTP_FROM").unwrap_or_default();

        let redis_url = std::env::var("REDIS_URL").unwrap_or_default();

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
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            smtp_from,
            redis_url,
        }
    }
}
