use std::path::PathBuf;
use std::fs;
use rand::Rng;
use base64::Engine;
use tracing::info;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_port: u16,
    pub media_root: PathBuf,
    pub admin_token: String,
    pub enforce_admin_token: bool,
    pub access_password: String,
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

        let enforce_admin_token = std::env::var("ENFORCE_ADMIN_TOKEN")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        // Ensure data/ directory and load/generate admin token
        let data_dir = PathBuf::from("./data");
        fs::create_dir_all(&data_dir).expect("Failed to create data directory");

        let admin_token = load_or_generate_admin_token(&data_dir);

        // Ensure media root exists
        fs::create_dir_all(&media_root).expect("Failed to create media root directory");

        if !enforce_admin_token {
            info!("Admin token enforcement is DISABLED (Android client compatible mode)");
        }

        // Load access password from data/.access_password
        let access_password = load_or_generate_access_password(&data_dir);

        AppConfig {
            database_url,
            server_port,
            media_root: PathBuf::from(media_root),
            admin_token,
            enforce_admin_token,
            access_password,
        }
    }
}

fn load_or_generate_admin_token(data_dir: &PathBuf) -> String {
    let token_file = data_dir.join(".admin_token");

    if token_file.exists() {
        let token = fs::read_to_string(&token_file)
            .expect("Failed to read admin token file")
            .trim()
            .to_string();
        if !token.is_empty() {
            return token;
        }
    }

    // Generate 24-byte base64url token
    let mut buf = [0u8; 24];
    rand::rng().fill(&mut buf);
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf);

    fs::write(&token_file, &token).expect("Failed to write admin token file");
    info!("Generated new admin token and saved to {:?}", token_file);

    token
}

fn load_or_generate_access_password(data_dir: &PathBuf) -> String {
    let pw_file = data_dir.join(".access_password");

    if pw_file.exists() {
        let pw = fs::read_to_string(&pw_file)
            .expect("Failed to read access password file")
            .trim()
            .to_string();
        if !pw.is_empty() {
            info!("Access password: {} (set)", "*".repeat(pw.len().min(8)));
            return pw;
        }
    }

    // Generate a random 6-digit numeric password
    let pw: String = (0..6).map(|_| rand::rng().random_range(0..10).to_string()).collect();
    fs::write(&pw_file, &pw).expect("Failed to write access password file");
    info!("Generated new access password");

    pw
}