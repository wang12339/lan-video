use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PlaybackHistoryRequest {
    #[serde(deserialize_with = "crate::util::hashid_serde::deserialize_id")]
    pub video_id: i64,
    pub position_ms: i64,
    pub duration_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct PlaybackHistoryResponse {
    #[serde(
        rename = "videoId",
        serialize_with = "crate::util::hashid_serde::serialize_id"
    )]
    pub video_id: i64,
    #[serde(rename = "positionMs")]
    pub position_ms: i64,
    #[serde(rename = "durationMs")]
    pub duration_ms: i64,
}

#[derive(Debug, Serialize)]
pub struct RecentWatchItem {
    #[serde(
        rename = "videoId",
        serialize_with = "crate::util::hashid_serde::serialize_id"
    )]
    pub video_id: i64,
    pub title: String,
    #[serde(rename = "coverUrl")]
    pub cover_url: Option<String>,
    #[serde(rename = "streamUrl")]
    pub stream_url: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    pub category: String,
    #[serde(rename = "positionMs")]
    pub position_ms: i64,
    #[serde(rename = "durationMs")]
    pub duration_ms: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}
