use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    #[serde(
        default,
        deserialize_with = "crate::util::hashid_serde::deserialize_option_id"
    )]
    pub parent_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct CommentQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentResponse {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub video_id: i64,
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub user_id: i64,
    pub username: String,
    pub avatar_url: Option<String>,
    pub content: String,
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_option_id")]
    pub parent_id: Option<i64>,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentListResponse {
    pub comments: Vec<CommentResponse>,
    pub total: i64,
}
