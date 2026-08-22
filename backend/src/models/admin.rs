use serde::{Deserialize, Serialize};

// ── admin_logs ──

/// Log entry parsed from JSON log file
#[derive(Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

#[derive(Deserialize)]
pub struct LogQuery {
    pub level: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

// ── admin_system ──

#[derive(Deserialize)]
pub struct TrackRequest {
    pub action: String,
    pub target: Option<String>,
    pub page: Option<String>,
}

#[derive(Deserialize)]
pub struct RegistrationToggleRequest {
    pub enabled: bool,
}

// ── admin_transcode ──

#[derive(Deserialize)]
pub struct TranscodeRequest {
    pub resolutions: Vec<String>,
}

#[derive(Serialize)]
pub struct TranscodeResponse {
    pub success: bool,
    pub message: String,
    pub job_id: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeStatusResponse {
    pub video_id: i64,
    pub variants: Vec<VariantInfo>,
    pub pending_jobs: Vec<JobInfo>,
}

#[derive(Serialize)]
pub struct VariantInfo {
    pub resolution: String,
    pub file_path: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
}

#[derive(Serialize)]
pub struct JobInfo {
    pub id: i32,
    pub resolution: String,
    pub status: String,
    pub progress: i32,
}

// ── admin_user ──

/// Admin reset user password request
#[derive(Deserialize)]
pub struct AdminResetPasswordRequest {
    pub password: String,
}

#[derive(Deserialize)]
pub struct ApproveRequest {
    pub approved: bool,
}
