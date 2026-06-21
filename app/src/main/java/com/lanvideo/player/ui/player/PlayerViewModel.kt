package com.lanvideo.player.ui.player

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

data class PlayerUiState(
    val videoId: String = "",
    val title: String = "",
    val timestamp: String = "",
    val views: Int = 0,
    val isPlaying: Boolean = false,
    val progress: Float = 0f,
    val isLoading: Boolean = true,
    val relatedVideos: List<PlayerRelatedVideo> = emptyList()
)

data class PlayerRelatedVideo(
    val id: String,
    val title: String,
    val icon: String,
    val views: Int,
    val timestamp: String
)

class PlayerViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(PlayerUiState())
    val uiState: StateFlow<PlayerUiState> = _uiState.asStateFlow()

    fun loadVideo(videoId: String) {
        _uiState.update {
            it.copy(
                videoId = videoId,
                title = "可爱动画合集",
                timestamp = "2分钟前",
                views = 1200,
                isLoading = false,
                relatedVideos = listOf(
                    PlayerRelatedVideo("2", "萌宠日常", "\uD83D\uDC36", 856, "5分钟前"),
                    PlayerRelatedVideo("3", "搞笑片段", "\uD83D\uDC31", 2100, "10分钟前"),
                    PlayerRelatedVideo("4", "治愈系视频", "\uD83E\uDD8A", 678, "15分钟前"),
                    PlayerRelatedVideo("5", "可爱合集", "\uD83D\uDC3B", 999, "20分钟前"),
                    PlayerRelatedVideo("6", "宠物趣事", "\uD83D\uDC25", 1500, "25分钟前"),
                )
            )
        }
    }

    fun togglePlayPause() {
        _uiState.update { it.copy(isPlaying = !it.isPlaying) }
    }

    fun onProgressChange(progress: Float) {
        _uiState.update { it.copy(progress = progress) }
    }
}
