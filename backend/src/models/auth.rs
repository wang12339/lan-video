use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserInfoResponse {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub username: String,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "emailVerified")]
    pub email_verified: bool,
    /// 头像 URL（/media/avatars/{id}.{ext}）；未设置时不下发
    #[serde(rename = "avatarUrl", skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    /// 访客模式：当前会话是否为匿名访客影子账号
    #[serde(rename = "isGuest")]
    pub is_guest: bool,
}

#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub username: String,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "totalVideosWatched")]
    pub total_videos_watched: i64,
    #[serde(rename = "totalWatchTimeMs")]
    pub total_watch_time_ms: i64,
    #[serde(rename = "recentHistory")]
    pub recent_history: Vec<super::playback::RecentWatchItem>,
}

// ── Password reset / forgot password ──

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct ForgotPasswordResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordToken {
    pub token: String,
}

// ── Email management ──

#[derive(Debug, Deserialize)]
pub struct UpdateEmailRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct SendVerificationEmailResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}
