package com.lanvideo.player.ui.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.ui.home.VideoItem
import com.lanvideo.player.util.VideoFormatters
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SearchUiState(
    val query: String = "",
    val results: List<VideoItem> = emptyList(),
    val searchHistory: List<String> = emptyList(),
    val isSearching: Boolean = false,
    val error: String? = null
)

class SearchViewModel(
    private val videoRepository: VideoRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(SearchUiState())
    val uiState: StateFlow<SearchUiState> = _uiState.asStateFlow()

    fun onQueryChange(query: String) {
        _uiState.update { it.copy(query = query) }
    }

    fun onSearch(query: String) {
        if (query.isBlank()) return
        val updated = _uiState.value.searchHistory.toMutableList()
        updated.remove(query)
        updated.add(0, query)
        val trimmed = updated.take(20)
        _uiState.update { it.copy(searchHistory = trimmed, isSearching = true, error = null) }

        viewModelScope.launch {
            val result = videoRepository.listVideos(query = query, forceRefresh = true)
            result.fold(
                onSuccess = { response ->
                    val videos = response.items.map { apiVideo ->
                        VideoItem(
                            id = apiVideo.id.toString(),
                            title = apiVideo.title,
                            thumbnailUrl = apiVideo.coverUrl ?: "",
                            sourceType = apiVideo.sourceType,
                            category = apiVideo.category,
                            timestamp = VideoFormatters.formatDuration(apiVideo.duration),
                            icon = VideoFormatters.getCategoryIcon(apiVideo.category)
                        )
                    }
                    _uiState.update {
                        it.copy(
                            results = videos,
                            isSearching = false,
                            error = if (videos.isEmpty()) "未找到「$query」相关结果" else null
                        )
                    }
                },
                onFailure = { e ->
                    _uiState.update {
                        it.copy(
                            isSearching = false,
                            error = "搜索失败: ${e.message}"
                        )
                    }
                }
            )
        }
    }

    fun onHistoryClick(query: String) {
        _uiState.update { it.copy(query = query) }
        onSearch(query)
    }

    fun removeHistoryItem(query: String) {
        _uiState.update {
            it.copy(searchHistory = it.searchHistory.filter { item -> item != query })
        }
    }

    fun clearHistory() {
        _uiState.update { it.copy(searchHistory = emptyList()) }
    }
}
