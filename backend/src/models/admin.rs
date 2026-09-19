use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ── admin_logs ──

/// Log entry parsed from JSON log file
#[derive(Serialize, ToSchema)]
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
    pub client_ip: Option<String>,
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

#[derive(Deserialize, ToSchema)]
pub struct LogQuery {
    pub level: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

// ── admin_system ──

#[derive(Deserialize, ToSchema)]
pub struct TrackRequest {
    pub action: String,
    pub target: Option<String>,
    pub page: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct RegistrationToggleRequest {
    pub enabled: bool,
}

// ── admin_transcode ──

#[derive(Deserialize, ToSchema)]
pub struct TranscodeRequest {
    pub resolutions: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TranscodeResponse {
    pub success: bool,
    pub message: String,
    pub job_id: Option<i64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeStatusResponse {
    pub video_id: i64,
    pub variants: Vec<VariantInfo>,
    pub pending_jobs: Vec<JobInfo>,
}

#[derive(Serialize, ToSchema)]
pub struct VariantInfo {
    pub resolution: String,
    pub file_path: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct JobInfo {
    pub id: i32,
    pub resolution: String,
    pub status: String,
    pub progress: i32,
}

// ── admin_user ──

/// Admin reset user password request
#[derive(Deserialize, ToSchema)]
pub struct AdminResetPasswordRequest {
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ApproveRequest {
    pub approved: bool,
}

// ── admin_users ──

/// GET /admin/users 查询参数：服务端搜索/筛选/分页
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct AdminUsersQuery {
    /// 用户名或邮箱模糊搜索
    pub search: Option<String>,
    /// 审批状态：active（已通过，默认）| pending（待审批）| all（全部）
    pub status: Option<String>,
    /// 角色：all（默认）| admin | user
    pub role: Option<String>,
    /// 页码（0 基）
    pub page: Option<i64>,
    /// 每页条数（默认 20，最大 200）
    pub size: Option<i64>,
}
