package com.lanvideo.player.ui.player

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.network.NetworkModule
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.util.VideoFormatters
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

data class PlayerUiState(
    val videoId: String = "",
    val title: String = "",
    val streamUrl: String = "",
    val coverUrl: String = "",
    val category: String = "",
    val views: Int = 0,
    val timestamp: String = "",
    val isPlaying: Boolean = false,
    val progress: Float = 0f,
    val isLoading: Boolean = true,
    val error: String? = null,
    val relatedVideos: List<PlayerRelatedVideo> = emptyList(),
    val isLiked: Boolean = false,
    val isFavorited: Boolean = false,
    val videoIds: List<String> = emptyList(),
    val playbackSpeed: Float = 1f,
    val isFullscreen: Boolean = false,
    val durationMs: Long = 0L
)

data class PlayerRelatedVideo(
    val id: String,
    val title: String,
    val thumbnailUrl: String,
    val icon: String,
    val views: Int = 0,
    val timestamp: String
)

class PlayerViewModel(
    private val videoRepository: VideoRepository
) : ViewModel() {
    private val _uiState = MutableStateFlow(PlayerUiState())
    val uiState: StateFlow<PlayerUiState> = _uiState.asStateFlow()
    private var historyJob: Job? = null
    private var lastHistoryUpdateMs = 0L

    fun loadVideo(videoId: String) {
        viewModelScope.launch {
            _uiState.update { it.copy(isLoading = true, error = null) }
            try {
                val api = NetworkModule.createApi()
                val videoDeferred = async { api.getVideo(videoId.toLong()) }
                val relatedDeferred = async { videoRepository.listVideos(type = "!local_image", size = 200) }

                val video = videoDeferred.await()
                val likeStatus = async { runCatching { api.getLikeStatus(video.id)["liked"] == true }.getOrDefault(false) }
                val favStatus = async { runCatching { api.getFavoriteStatus(video.id)["favorited"] == true }.getOrDefault(false) }

                val relatedResult = relatedDeferred.await()
                val relatedItems = relatedResult.map { it.items }.getOrElse { emptyList() }
                val related = relatedItems
                    .filter { it.id.toString() != videoId }
                    .shuffled()
                    .take(6)
                    .map { v ->
                        val thumbUrl = when (v.sourceType) {
                            "local_image" -> v.streamUrl
                            else -> v.coverUrl ?: v.thumbUrl ?: ""
                        }
                        PlayerRelatedVideo(
                            id = v.id.toString(),
                            title = v.title,
                            thumbnailUrl = thumbUrl,
                            icon = VideoFormatters.getCategoryIcon(v.category),
                            timestamp = VideoFormatters.formatDuration(v.duration)
                        )
                    }

                _uiState.update {
                    it.copy(
                        videoId = videoId,
                        title = video.title,
                        streamUrl = video.streamUrl,
                        coverUrl = video.coverUrl ?: "",
                        category = video.category,
                        timestamp = VideoFormatters.formatDuration(video.duration),
                        isLoading = false,
                        relatedVideos = related,
                        isLiked = likeStatus.await(),
                        isFavorited = favStatus.await()
                    )
                }
                launch {
                    try {
                        val allResult = videoRepository.listVideos(type = "!local_image", size = 2000)
                        val allIds = allResult.map { it.items }.getOrElse { emptyList() }.map { it.id.toString() }
                        _uiState.update { it.copy(videoIds = allIds) }
                    } catch (_: Exception) {}
                }
            } catch (e: Exception) {
                _uiState.update {
                    it.copy(
                        isLoading = false,
                        error = e.message
                    )
                }
            }
        }
    }

    fun nextVideoId(): String? {
        val ids = _uiState.value.videoIds
        val current = _uiState.value.videoId
        val idx = ids.indexOf(current)
        if (idx < 0 || ids.isEmpty()) return null
        return ids[(idx + 1) % ids.size]
    }

    fun setPlaybackSpeed(speed: Float) {
        _uiState.update { it.copy(playbackSpeed = speed) }
    }

    fun toggleFullscreen() {
        _uiState.update { it.copy(isFullscreen = !it.isFullscreen) }
    }

    fun toggleLike() {
        val id = _uiState.value.videoId.toLongOrNull() ?: return
        viewModelScope.launch {
            try {
                val result = NetworkModule.createApi().toggleLike(id)
                _uiState.update { it.copy(isLiked = result["liked"] == true, error = null) }
            } catch (e: Exception) {
                _uiState.update { it.copy(error = "点赞失败: ${e.message}") }
            }
        }
    }

    fun toggleFavorite() {
        val id = _uiState.value.videoId.toLongOrNull() ?: return
        viewModelScope.launch {
            try {
                val result = NetworkModule.createApi().toggleFavorite(id)
                _uiState.update { it.copy(isFavorited = result["favorited"] == true, error = null) }
            } catch (e: Exception) {
                _uiState.update { it.copy(error = "收藏失败: ${e.message}") }
            }
        }
    }

    fun togglePlayPause() {
        _uiState.update { it.copy(isPlaying = !it.isPlaying) }
    }

    fun onProgressChange(progress: Float, durationMs: Long) {
        _uiState.update { it.copy(progress = progress, durationMs = durationMs) }
        val state = _uiState.value
        val videoId = state.videoId.toLongOrNull() ?: return
        if (durationMs <= 0L) return
        val positionMs = (progress * durationMs).toLong()
        val now = System.currentTimeMillis()
        if (now - lastHistoryUpdateMs > 10_000) {
            lastHistoryUpdateMs = now
            viewModelScope.launch {
                videoRepository.updatePlaybackHistory(videoId, positionMs, durationMs)
            }
        }
    }

    fun onPlaybackEnded() {
        val state = _uiState.value
        _uiState.update { it.copy(progress = 1f) }
        val videoId = state.videoId.toLongOrNull() ?: return
        val durationMs = state.durationMs
        if (durationMs > 0L) {
            viewModelScope.launch {
                videoRepository.updatePlaybackHistory(videoId, durationMs, durationMs)
            }
        }
    }

}
