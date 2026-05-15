package com.example.myapplication.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class VideoItem(
    val id: Long,
    val title: String,
    val description: String = "",
    val sourceType: String = "external",
    val coverUrl: String? = null,
    val streamUrl: String,
    val category: String = "general",
    val duration: Long = 0L,
    val watchPosition: Long = 0L
)

@Serializable
data class VideoListResponse(
    val items: List<VideoItem>
)

@Serializable
data class PlaybackHistoryRequest(
    @SerialName("video_id") val videoId: Long,
    @SerialName("position_ms") val positionMs: Long,
    @SerialName("duration_ms") val durationMs: Long
)

@Serializable
data class PlaybackHistoryResponse(
    @SerialName("video_id") val videoId: Long,
    @SerialName("position_ms") val positionMs: Long,
    @SerialName("duration_ms") val durationMs: Long = 0L
)

@Serializable
data class VideoUpdateRequest(
    val title: String? = null,
    val description: String? = null,
    val category: String? = null
)

@Serializable
data class VideoUpdateResponse(
    val ok: Boolean,
    val error: String? = null
)

@Serializable
data class UploadResponse(
    val id: Long
)

@Serializable
data class PagedVideoResponse(
    val items: List<VideoItem>,
    val total: Long,
    val page: Int,
    val size: Int
)

@Serializable
data class FileCheckItem(val name: String, val size: Long)
