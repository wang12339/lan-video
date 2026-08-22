use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateShareRequest {
    pub expires_in_days: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateShareResponse {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub video_id: i64,
    /// Raw share token — shown ONCE on creation. Never returned by any other endpoint.
    pub token: String,
    pub share_url: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareListItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub active: bool,
}
