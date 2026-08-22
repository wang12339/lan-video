use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;

use crate::models::admin::{TranscodeRequest, TranscodeResponse, TranscodeStatusResponse};
use crate::services::media_service::safe_media_path;
use crate::state::AppState;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};

/// POST /admin/videos/{id}/transcode
///
/// Start transcoding a video to multiple resolutions
pub async fn transcode_video(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<i64>,
    SafeJson(req): SafeJson<TranscodeRequest>,
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
        .repos
        .video
        .find_by_id(video_id)
        .await
        .map_err(|e| internal_error_log("find_by_id failed", &e))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "Video not found"))?;

    // Only local (uploaded) videos can be transcoded. External videos are
    // URL-referenced, and their stream_url must never reach ffmpeg.
    if !video.source_type.starts_with("local_video") {
        return Err(error_response(StatusCode::BAD_REQUEST, "仅本地视频可转码"));
    }

    // Resolve the input inside media_root with the canonicalized prefix
    // check (same as thumbnail generation / variant deletion). A bare
    // `Path::join` would treat the absolute `/media/…` stream_url as
    // replacing media_root and escape it into the filesystem root.
    let video_path = match safe_media_path(&video.stream_url, &state.config.media_root) {
        Some(p) => p,
        None => {
            return Err(error_response(
                StatusCode::NOT_FOUND,
                "Video file not found",
            ))
        }
    };

    // Submit to the persistent task queue so the work survives a crash, is
    // retried on transient failure, and is dead-lettered after all retry
    // attempts (see task_queue.rs). The queue dedupes by job id, so repeated
    // submissions never transcode the same resolution twice.
    let job_id = state
        .task_queue
        .add_video_transcode(video_id, video_path, req.resolutions)
        .await
        .map_err(|e| internal_error_log("Failed to enqueue transcode task", &e))?;

    Ok(Json(TranscodeResponse {
        success: true,
        message: "Transcoding started".to_string(),
        job_id,
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
        .repos
        .video
        .find_by_id(video_id)
        .await
        .map_err(|e| internal_error_log("operation failed", &e))?
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
        .map_err(|e| internal_error_log("operation failed", &e))?;

    // Delete variant record from database
    state
        .repos
        .video
        .delete_variant_record(video_id, &resolution)
        .await
        .map_err(|e| {
            tracing::error!("DB delete variant failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "删除变体记录失败")
        })?;

    // Update video has_variants flag if no variants remain
    let remaining = state
        .repos
        .video
        .count_variants(video_id)
        .await
        .map_err(|e| {
            tracing::error!("DB count variants failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询变体数量失败")
        })?;

    if remaining == 0 {
        state
            .repos
            .video
            .clear_has_variants(video_id)
            .await
            .map_err(|e| {
                tracing::error!("DB update has_variants failed: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "更新视频状态失败")
            })?;
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Variant {} deleted", resolution),
    })))
}

/// POST /admin/videos/{id}/transcode/cancel
///
/// Cancel ongoing transcoding for a video
pub async fn cancel_transcode(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Mark any pending/processing jobs as failed
    let affected = state
        .repos
        .video
        .cancel_transcode_jobs(video_id)
        .await
        .map_err(|e| {
            tracing::error!("DB cancel transcode failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "取消转码任务失败")
        })?;

    if affected == 0 {
        return Ok(Json(serde_json::json!({
            "success": true,
            "message": "没有进行中的转码任务",
        })));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("已取消 {} 个转码任务", affected),
    })))
}

/// POST /admin/videos/{id}/hls
///
/// Start HLS transcoding for adaptive streaming
pub async fn transcode_to_hls(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = crate::util::hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;

    let video = state
        .repos
        .video
        .find_by_id(video_id)
        .await
        .map_err(|e| internal_error_log("find video for HLS transcode", &e))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;

    // Only local videos can be transcoded to HLS
    if video.source_type != "local_video" {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "只有本地视频可以转码为 HLS",
        ));
    }

    let video_path = safe_media_path(&video.stream_url, &state.config.media_root)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频路径"))?;

    // Start HLS transcoding in background
    let transcoder = state.transcoder.clone();
    let video_id_clone = video_id;

    tokio::spawn(async move {
        match transcoder
            .transcode_to_hls(video_id_clone, &video_path)
            .await
        {
            Ok(playlist) => {
                tracing::info!(
                    video_id = video_id_clone,
                    "HLS transcoding completed: {}",
                    playlist.master_url
                );
            }
            Err(e) => {
                tracing::error!(video_id = video_id_clone, "HLS transcoding failed: {}", e);
            }
        }
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "HLS 转码已开始",
    })))
}

/// GET /admin/videos/{id}/hls/status
///
/// Get HLS transcoding status
pub async fn hls_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = crate::util::hashid::decode_id_or_numeric(&id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;

    let hls_dir = state
        .config
        .media_root
        .join("hls")
        .join(video_id.to_string());
    let master_playlist = hls_dir.join("master.m3u8");

    if master_playlist.exists() {
        Ok(Json(serde_json::json!({
            "status": "ready",
            "masterUrl": format!("/media/hls/{}/master.m3u8", video_id),
        })))
    } else {
        // Check if transcoding is in progress
        let variants_dir = hls_dir.join("720p");
        if variants_dir.exists() {
            Ok(Json(serde_json::json!({
                "status": "processing",
            })))
        } else {
            Ok(Json(serde_json::json!({
                "status": "not_started",
            })))
        }
    }
}
