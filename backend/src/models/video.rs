use chrono::{DateTime, Utc};
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

/// 图片/视频的 EXIF 元数据，字段与 `exif_service::ParsedExif` 一一对应。
///
/// 全部字段可空：解析不到 EXIF 的记录不会返回该对象（见 `VideoItem::exif`）。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageExif {
    /// 拍摄时间（EXIF DateTimeOriginal，UTC）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>)]
    pub taken_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aperture: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shutter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iso: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focal_length: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<i32>,
}

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
    /// EXIF 元数据：所有 exif 字段均为空时省略该字段（旧版响应兼容）。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exif: Option<ImageExif>,
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
    /// 按 EXIF 拍摄日期筛选（含当天），格式 `YYYY-MM-DD`
    pub taken_after: Option<String>,
    /// 按 EXIF 拍摄日期筛选（含当天），格式 `YYYY-MM-DD`
    pub taken_before: Option<String>,
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
