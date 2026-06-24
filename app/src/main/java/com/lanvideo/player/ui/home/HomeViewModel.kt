package com.lanvideo.player.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.util.VideoFormatters
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class VideoItem(
    val id: String,
    val title: String,
    val thumbnailUrl: String,
    val sourceType: String,
    val category: String,
    val views: Int = 0,
    val timestamp: String,
    val icon: String
)

data class HomeUiState(
    val isLoading: Boolean = true,
    val isLoadingMore: Boolean = false,
    val videos: List<VideoItem> = emptyList(),
    val allVideos: List<VideoItem> = emptyList(),
    val categories: List<String> = listOf("视频", "图片"),
    val selectedCategory: String = "视频",
    val error: String? = null,
    val hasMore: Boolean = true
)

class HomeViewModel(
    private val videoRepository: VideoRepository
) : ViewModel() {
    private val _uiState = MutableStateFlow(HomeUiState())
    val uiState: StateFlow<HomeUiState> = _uiState
    private var currentPage = 0
    private val pageSize = 20

    init {
        loadVideos()
    }

    fun loadVideos(forceRefresh: Boolean = false) {
        currentPage = 0
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)
            val type = when (_uiState.value.selectedCategory) {
                "视频" -> "!local_image"
                "图片" -> "local_image"
                else -> null
            }
            val result = videoRepository.listVideos(type = type, page = 0, size = pageSize, forceRefresh = forceRefresh)
            result.fold(
                onSuccess = { response ->
                    val videos = withContext(Dispatchers.Default) {
                        response.items.map { apiVideo ->
                            mapVideoItem(apiVideo)
                        }
                    }
                    _uiState.value = _uiState.value.copy(
                        isLoading = false,
                        allVideos = videos,
                        videos = videos,
                        hasMore = videos.size >= pageSize
                    )
                    currentPage = 1
                },
                onFailure = { e ->
                    _uiState.value = _uiState.value.copy(
                        isLoading = false,
                        error = e.message ?: "加载失败"
                    )
                }
            )
        }
    }

    fun loadMore() {
        val state = _uiState.value
        if (state.isLoading || state.isLoadingMore || !state.hasMore) return
        _uiState.value = state.copy(isLoadingMore = true)

        viewModelScope.launch {
            val type = when (_uiState.value.selectedCategory) {
                "视频" -> "!local_image"
                "图片" -> "local_image"
                else -> null
            }
            val result = videoRepository.listVideos(type = type, page = currentPage, size = pageSize)
            result.fold(
                onSuccess = { response ->
                    val newVideos = withContext(Dispatchers.Default) {
                        response.items.map { apiVideo ->
                            mapVideoItem(apiVideo)
                        }
                    }
                    val currentVideos = _uiState.value.allVideos + newVideos
                    _uiState.value = _uiState.value.copy(
                        isLoadingMore = false,
                        allVideos = currentVideos,
                        videos = currentVideos,
                        hasMore = newVideos.size >= pageSize
                    )
                    currentPage++
                },
                onFailure = { e ->
                    _uiState.value = _uiState.value.copy(
                        isLoadingMore = false,
                        error = e.message ?: "加载更多失败"
                    )
                }
            )
        }
    }

    private fun mapVideoItem(apiVideo: com.lanvideo.player.data.model.VideoItem): VideoItem {
        val thumbnailUrl = when (apiVideo.sourceType) {
            "local_image" -> apiVideo.streamUrl
            "local_video" -> apiVideo.coverUrl ?: apiVideo.thumbUrl ?: ""
            "external" -> apiVideo.coverUrl ?: apiVideo.thumbUrl ?: ""
            else -> apiVideo.coverUrl ?: ""
        }
        return VideoItem(
            id = apiVideo.id.toString(),
            title = apiVideo.title,
            thumbnailUrl = thumbnailUrl,
            sourceType = apiVideo.sourceType,
            category = apiVideo.category,
            timestamp = VideoFormatters.formatDuration(apiVideo.duration),
            icon = VideoFormatters.getCategoryIcon(apiVideo.category)
        )
    }

    fun onCategorySelected(category: String) {
        _uiState.value = _uiState.value.copy(selectedCategory = category)
        loadVideos(forceRefresh = true)
    }
}
