use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct CreatePlaylistRequest {
    pub name: String,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdatePlaylistRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddVideoRequest {
    #[serde(deserialize_with = "crate::util::hashid_serde::deserialize_id")]
    pub video_id: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct ReorderRequest {
    #[serde(deserialize_with = "crate::util::hashid_serde::deserialize_vec_ids")]
    pub video_ids: Vec<i64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistResponse {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub cover_url: Option<String>,
    pub item_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistListResponse {
    pub playlists: Vec<PlaylistResponse>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideoItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub title: String,
    pub description: String,
    pub source_type: String,
    pub cover_url: Option<String>,
    pub stream_url: String,
    pub category: String,
    pub views: i64,
    pub duration: i64,
}

impl From<crate::repositories::playlist_repo::PlaylistVideoRow> for PlaylistVideoItem {
    fn from(v: crate::repositories::playlist_repo::PlaylistVideoRow) -> Self {
        Self {
            id: v.id,
            title: v.title,
            description: v.description,
            source_type: v.source_type,
            cover_url: v.cover_url,
            stream_url: v.stream_url,
            category: v.category,
            views: v.views,
            duration: v.duration,
        }
    }
}
