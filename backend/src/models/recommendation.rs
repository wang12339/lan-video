use serde::Serialize;

#[derive(Serialize)]
pub struct RecommendationResponse {
    pub items: Vec<RecommendationItem>,
    pub total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub title: String,
    pub category: Option<String>,
    pub thumb_url: Option<String>,
    pub score: f64,
    pub reason: String,
}
