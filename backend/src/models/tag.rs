use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagResponse {
    pub id: i32,
    pub name: String,
    pub color: Option<String>,
    pub usage_count: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagListResponse {
    pub tags: Vec<TagResponse>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(Deserialize)]
pub struct TagQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
}

impl From<crate::services::tag_service::TagResponse> for TagResponse {
    fn from(t: crate::services::tag_service::TagResponse) -> Self {
        TagResponse {
            id: t.id,
            name: t.name,
            color: t.color,
            usage_count: t.usage_count,
        }
    }
}
