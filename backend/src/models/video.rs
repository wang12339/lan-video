use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ── Search ──

#[derive(Deserialize, ToSchema)]
pub struct SearchQuery {
    pub q: String,
    pub page: Option<i64>,
    pub size: Option<i64>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct SearchResponse {
    pub items: Vec<SearchResultItem>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(serde::Serialize, ToSchema)]
pub struct SearchResultItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rank: f32,
    pub headline: Option<String>,
}

// ── Video variants ──

#[derive(serde::Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VideoVariantResponse {
    pub resolution: String,
    pub url: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
    pub codec: Option<String>,
}

// ── Video item & list ──

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VideoItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub title: String,
    pub description: String,
    pub source_type: String,
    pub cover_url: Option<String>,
    pub stream_url: String,
    pub thumb_url: Option<String>,
    pub category: String,
    pub views: i64,
    pub duration: i64,
    pub watch_position: Option<i64>,
    #[serde(default)]
    pub has_variants: bool,
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_option_id")]
    pub uploader_id: Option<i64>,
    /// ISO-ish timestamp `%Y-%m-%d %H:%M:%S`（UTC）。旧版响应不含该字段，
    /// 用 `#[serde(default)]` 保持反序列化兼容。
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct PagedVideoResponse {
    pub items: Vec<VideoItem>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VideoQuery {
    pub query: Option<String>,
    #[serde(rename = "type")]
    pub source_type: Option<String>,
    pub category: Option<String>,
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub uploader_id: Option<String>,
    pub sort: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExternalVideoRequest {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub stream_url: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VideoUpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IdResponse {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CheckHashesResponse {
    pub existing: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CheckHashesRequest {
    pub hashes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct FileCheckItem {
    pub name: String,
    pub size: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CheckFilesResponse {
    pub existing_indices: Vec<usize>,
}
