use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

#[derive(Deserialize)]
pub struct TranscodeRequest {
    pub resolutions: Vec<String>,
}

#[derive(Serialize)]
pub struct TranscodeResponse {
    pub success: bool,
    pub message: String,
    pub job_id: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeStatusResponse {
    pub video_id: i64,
    pub variants: Vec<VariantInfo>,
    pub pending_jobs: Vec<JobInfo>,
}

#[derive(Serialize)]
pub struct VariantInfo {
    pub resolution: String,
    pub file_path: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
}

#[derive(Serialize)]
pub struct JobInfo {
    pub id: i32,
    pub resolution: String,
    pub status: String,
    pub progress: i32,
}

/// POST /admin/videos/{id}/transcode
///
/// Start transcoding a video to multiple resolutions
pub async fn transcode_video(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<i64>,
    Json(req): Json<TranscodeRequest>,
) -> Result<Json<TranscodeResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate resolutions
    let valid_resolutions = ["2160p", "1080p", "720p", "480p", "360p"];
    for res in &req.resolutions {
        if !valid_resolutions.contains(&res.as_str()) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                format!("Invalid resolution: {}", res),
            ));
        }
    }

    // Check if video exists
    let video = state
        .video_repo
        .find_by_id(video_id)
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "Video not found"))?;

    // Get video file path
    let video_path = state.config.media_root.join(&video.stream_url);

    if !video_path.exists() {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "Video file not found",
        ));
    }

    // Start transcoding in background
    let transcoder = state.transcoder.clone();
    let video_id_clone = video_id;
    let resolutions = req.resolutions.clone();

    tokio::spawn(async move {
        match transcoder
            .transcode(video_id_clone, &video_path, resolutions)
            .await
        {
            Ok(variants) => {
                tracing::info!(
                    "Transcoding completed for video {}: {} variants",
                    video_id_clone,
                    variants.len()
                );
                tracing::warn!("transcode save variants to database not yet implemented");
            }
            Err(e) => {
                tracing::error!("Transcoding failed for video {}: {}", video_id_clone, e);
            }
        }
    });

    Ok(Json(TranscodeResponse {
        success: true,
        message: "Transcoding started".to_string(),
        job_id: None,
    }))
}

/// GET /admin/videos/{id}/transcode/status
///
/// Get transcoding status for a video
pub async fn transcode_status(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<i64>,
) -> Result<Json<TranscodeStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if video exists
    let _video = state
        .video_repo
        .find_by_id(video_id)
        .await
        .map_err(|e| { tracing::error!("operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;

    tracing::warn!("transcode get variants from database not yet implemented");
    let variants = Vec::new();

    tracing::warn!("transcode get pending jobs from database not yet implemented");
    let pending_jobs = Vec::new();

    Ok(Json(TranscodeStatusResponse {
        video_id,
        variants,
        pending_jobs,
    }))
}

/// DELETE /admin/videos/{id}/transcode/{resolution}
///
/// Delete a specific variant of a video
pub async fn delete_variant(
    State(state): State<Arc<AppState>>,
    Path((video_id, resolution)): Path<(i64, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Validate resolution
    let valid_resolutions = ["2160p", "1080p", "720p", "480p", "360p"];
    if !valid_resolutions.contains(&resolution.as_str()) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("Invalid resolution: {}", resolution),
        ));
    }

    // Delete variant file
    state
        .transcoder
        .delete_variant(video_id, &resolution)
        .await
        .map_err(|e| { tracing::error!("operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?;

    tracing::warn!("transcode delete variant from database not yet implemented");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Variant {} deleted", resolution),
    })))
}

/// POST /admin/videos/{id}/transcode/cancel
///
/// Cancel ongoing transcoding for a video
pub async fn cancel_transcode(
    State(_state): State<Arc<AppState>>,
    Path(_video_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    tracing::warn!("transcode job cancellation not yet implemented");
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Transcoding cancelled",
    })))
}
