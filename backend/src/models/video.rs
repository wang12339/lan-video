use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoItem {
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
    pub uploader_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PagedVideoResponse {
    pub items: Vec<VideoItem>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(Debug, Deserialize)]
pub struct VideoQuery {
    pub query: Option<String>,
    #[serde(rename = "type")]
    pub source_type: Option<String>,
    pub category: Option<String>,
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub uploader_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ExternalVideoRequest {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub stream_url: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VideoUpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IdResponse {
    pub id: i64,
}

#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CheckHashesResponse {
    pub existing: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckHashesRequest {
    pub hashes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileCheckItem {
    pub name: String,
    pub size: i64,
}

#[derive(Debug, Serialize)]
pub struct CheckFilesResponse {
    pub existing_indices: Vec<usize>,
}
