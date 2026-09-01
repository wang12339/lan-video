use std::fmt;
use std::net::IpAddr;
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
    pub admin_ip_whitelist: Vec<IpAddr>,
    pub upload_quota_bytes: i64,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub migrations_dir: Option<PathBuf>,
    pub sentry_dsn: String,
    pub sentry_environment: String,
    pub app_env: String,
    pub allow_first_user_admin: bool,
    pub trusted_proxy: bool,
    pub hashid_salt: String,
    pub transcode_timeout_secs: u64,
    pub ffprobe_timeout_secs: u64,
    pub transcode_concurrency: usize,
    pub transcode_max_duration_secs: u64,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
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
            .field("admin_ip_whitelist", &self.admin_ip_whitelist)
            .field("upload_quota_bytes", &self.upload_quota_bytes)
            .field("app_env", &self.app_env)
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

    /// 构造转码器配置（由 `Transcoder::new` 消费）。
    pub fn transcode_settings(&self) -> crate::services::transcoder::TranscodeSettings {
        crate::services::transcoder::TranscodeSettings {
            transcode_timeout: std::time::Duration::from_secs(self.transcode_timeout_secs),
            ffprobe_timeout: std::time::Duration::from_secs(self.ffprobe_timeout_secs),
            concurrency: self.transcode_concurrency,
            max_duration_secs: self.transcode_max_duration_secs,
            ffmpeg_path: self.ffmpeg_path.clone(),
            ffprobe_path: self.ffprobe_path.clone(),
        }
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

        // ADMIN_IP_WHITELIST: 逗号分隔的 IP 列表（opt-in）。为空则不对
        // /admin/* 做来源限制；非空时仅白名单内的 IP 可访问管理接口。
        let admin_ip_whitelist = std::env::var("ADMIN_IP_WHITELIST")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<IpAddr>().ok())
                    .collect()
            })
            .unwrap_or_default();

        let upload_quota_bytes = std::env::var("UPLOAD_QUOTA_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50 * 1024 * 1024 * 1024);

        let db_max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(100);

        let db_min_connections: u32 = std::env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n >= 1 && n <= db_max_connections)
            .unwrap_or(2);

        let migrations_dir = std::env::var("MIGRATIONS_DIR")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        let sentry_dsn = std::env::var("SENTRY_DSN").unwrap_or_default();
        let sentry_environment =
            std::env::var("SENTRY_ENVIRONMENT").unwrap_or_else(|_| "production".into());
        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "production".into());
        let allow_first_user_admin = parse_bool_env("ALLOW_FIRST_USER_ADMIN", false);
        let trusted_proxy = parse_bool_env("TRUSTED_PROXY", false);
        let hashid_salt = std::env::var("HASHID_SALT").unwrap_or_default();
        let transcode_timeout_secs = std::env::var("TRANSCODE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);
        let ffprobe_timeout_secs = std::env::var("FFPROBE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let transcode_concurrency = std::env::var("TRANSCODE_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1)
            .max(1);
        let transcode_max_duration_secs = std::env::var("TRANSCODE_MAX_DURATION_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7200);
        let ffmpeg_path = std::env::var("FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".into());
        let ffprobe_path = std::env::var("FFPROBE_PATH").unwrap_or_else(|_| "ffprobe".into());

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
            admin_ip_whitelist,
            upload_quota_bytes,
            db_max_connections,
            db_min_connections,
            migrations_dir,
            sentry_dsn,
            sentry_environment,
            app_env,
            allow_first_user_admin,
            trusted_proxy,
            hashid_salt,
            transcode_timeout_secs,
            ffprobe_timeout_secs,
            transcode_concurrency,
            transcode_max_duration_secs,
            ffmpeg_path,
            ffprobe_path,
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

    /// Serializes tests that mutate the shared `PARSE_BOOL_TEST` env var so
    /// they cannot race when the test binary runs with `--test-threads` parallel.
    fn lock_parse_bool_env() -> std::sync::MutexGuard<'static, ()> {
        use std::sync::Mutex;
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn parse_bool_true_variants() {
        let _env_guard = lock_parse_bool_env();
        assert!(!parse_bool_env("NONEXISTENT_KEY_FOR_TEST", false));
        for val in ["true", "TRUE", "True", "1", " true ", "1 "] {
            std::env::set_var("PARSE_BOOL_TEST", val);
            assert!(parse_bool_env("PARSE_BOOL_TEST", false), "val={val}");
        }
        std::env::remove_var("PARSE_BOOL_TEST");
    }

    #[test]
    fn parse_bool_false_variants() {
        let _env_guard = lock_parse_bool_env();
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
            admin_ip_whitelist: Vec::new(),
            upload_quota_bytes: 0,
            db_max_connections: 100,
            db_min_connections: 2,
            migrations_dir: None,
            sentry_dsn: String::new(),
            sentry_environment: "production".into(),
            app_env: "test".into(),
            allow_first_user_admin: false,
            trusted_proxy: false,
            hashid_salt: String::new(),
            transcode_timeout_secs: 3600,
            ffprobe_timeout_secs: 30,
            transcode_concurrency: 1,
            transcode_max_duration_secs: 7200,
            ffmpeg_path: "ffmpeg".into(),
            ffprobe_path: "ffprobe".into(),
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
            admin_ip_whitelist: Vec::new(),
            upload_quota_bytes: 0,
            db_max_connections: 100,
            db_min_connections: 2,
            migrations_dir: None,
            sentry_dsn: String::new(),
            sentry_environment: "production".into(),
            app_env: "test".into(),
            allow_first_user_admin: false,
            trusted_proxy: false,
            hashid_salt: String::new(),
            transcode_timeout_secs: 3600,
            ffprobe_timeout_secs: 30,
            transcode_concurrency: 1,
            transcode_max_duration_secs: 7200,
            ffmpeg_path: "ffmpeg".into(),
            ffprobe_path: "ffprobe".into(),
        };
        assert!(!config.registration_enabled());
        config.set_registration_enabled(true);
        assert!(config.registration_enabled());
        config.set_registration_enabled(false);
        assert!(!config.registration_enabled());
    }
}
