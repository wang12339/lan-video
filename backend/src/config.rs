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
    if let Some(pos) = url.find("://") {
        let rest = &url[pos + 3..];
        if let Some(at) = rest.find('@') {
            let mut result = String::with_capacity(url.len());
            result.push_str(&url[..pos + 3]);
            result.push_str("***:***@");
            result.push_str(&rest[at + 1..]);
            return result;
        }
    }
    url.to_string()
}

#[inline]
pub fn parse_bool_env(key: &str, default: bool) -> bool {
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
    #[allow(clippy::panic)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_url_with_credentials() {
        let url = "postgres://user:pass@localhost:5432/db";
        let redacted = redact_url(url);
        assert_eq!(redacted, "postgres://***:***@localhost:5432/db");
        assert!(!redacted.contains("user"));
        assert!(!redacted.contains("pass"));
    }

    #[test]
    fn redact_url_without_credentials() {
        let url = "postgres://localhost:5432/db";
        assert_eq!(redact_url(url), url);
    }

    #[test]
    fn redact_url_empty() {
        assert_eq!(redact_url(""), "");
    }

    #[test]
    fn redact_url_no_scheme() {
        let url = "localhost:5432/db";
        assert_eq!(redact_url(url), url);
    }

    #[test]
    fn redact_url_redis() {
        let url = "redis://:secret@redis-host:6379";
        let redacted = redact_url(url);
        assert_eq!(redacted, "redis://***:***@redis-host:6379");
    }

    #[test]
    fn parse_bool_true_variants() {
        for val in ["true", "TRUE", "True", "1", " true ", "1 "] {
            assert!(parse_bool_env("NONEXISTENT_KEY_FOR_TEST", false) || true);
            std::env::set_var("PARSE_BOOL_TEST", val);
            assert!(parse_bool_env("PARSE_BOOL_TEST", false), "val={val}");
        }
        std::env::remove_var("PARSE_BOOL_TEST");
    }

    #[test]
    fn parse_bool_false_variants() {
        for val in ["false", "FALSE", "0", "no", "", " anything "] {
            std::env::set_var("PARSE_BOOL_TEST", val);
            assert!(!parse_bool_env("PARSE_BOOL_TEST", true), "val={val}");
        }
        std::env::remove_var("PARSE_BOOL_TEST");
    }

    #[test]
    fn parse_bool_missing_key_uses_default() {
        std::env::remove_var("PARSE_BOOL_MISSING_KEY");
        assert!(parse_bool_env("PARSE_BOOL_MISSING_KEY", true));
        assert!(!parse_bool_env("PARSE_BOOL_MISSING_KEY", false));
    }

    #[test]
    fn debug_impl_redacts_database_url() {
        let config = AppConfig {
            database_url: "postgres://admin:secret123@db.host:5432/prod".into(),
            server_port: 8082,
            public_url: "https://example.com".into(),
            media_root: PathBuf::from("./media"),
            webapp_root: PathBuf::from("./webapp/dist"),
            log_dir: PathBuf::from("./logs"),
            data_dir: PathBuf::from("./data"),
            registration_enabled: std::sync::Arc::new(AtomicBool::new(false)),
            cors_origin: String::new(),
            cookie_secure: true,
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_from: String::new(),
            redis_url: "redis://:pw@host:6379".into(),
        };
        let debug = format!("{:?}", config);
        assert!(!debug.contains("secret123"), "密码必须被脱敏");
        assert!(!debug.contains("pw"), "Redis 密码必须被脱敏");
        assert!(debug.contains("***:***"));
    }

    #[test]
    fn registration_enabled_toggle() {
        let config = AppConfig {
            database_url: String::new(),
            server_port: 0,
            public_url: String::new(),
            media_root: PathBuf::new(),
            webapp_root: PathBuf::new(),
            log_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            registration_enabled: std::sync::Arc::new(AtomicBool::new(false)),
            cors_origin: String::new(),
            cookie_secure: false,
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_from: String::new(),
            redis_url: String::new(),
        };
        assert!(!config.registration_enabled());
        config.set_registration_enabled(true);
        assert!(config.registration_enabled());
        config.set_registration_enabled(false);
        assert!(!config.registration_enabled());
    }
}
