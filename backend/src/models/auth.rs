use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct AccessRequest {
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
    pub username: String,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
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
