package com.lanvideo.player.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

data class VideoItem(
    val id: String,
    val title: String,
    val thumbnailUrl: String,
    val category: String,
    val views: Int,
    val timestamp: String,
    val icon: String
)

data class HomeUiState(
    val isLoading: Boolean = true,
    val videos: List<VideoItem> = emptyList(),
    val categories: List<String> = listOf("全部", "视频", "图片"),
    val selectedCategory: String = "全部"
)

class HomeViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(HomeUiState())
    val uiState: StateFlow<HomeUiState> = _uiState

    init {
        loadVideos()
    }

    private fun loadVideos() {
        viewModelScope.launch {
            val mockVideos = listOf(
                VideoItem("1", "可爱动画合集", "", "动画", 1200, "2分钟前", "\uD83D\uDC30"),
                VideoItem("2", "萌宠日常", "", "萌宠", 856, "5分钟前", "\uD83D\uDC36"),
                VideoItem("3", "搞笑片段", "", "搞笑", 2100, "10分钟前", "\uD83D\uDC31"),
                VideoItem("4", "治愈系视频", "", "治愈", 678, "15分钟前", "\uD83E\uDD8A"),
                VideoItem("5", "可爱合集", "", "动画", 999, "20分钟前", "\uD83D\uDC3B"),
                VideoItem("6", "宠物趣事", "", "萌宠", 1500, "25分钟前", "\uD83D\uDC25"),
            )
            _uiState.value = HomeUiState(
                isLoading = false,
                videos = mockVideos
            )
        }
    }

    fun onCategorySelected(category: String) {
        _uiState.value = _uiState.value.copy(selectedCategory = category)
    }
}
