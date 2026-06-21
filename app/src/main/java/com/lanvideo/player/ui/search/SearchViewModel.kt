package com.lanvideo.player.ui.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.ui.home.VideoItem
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

class SearchViewModel : ViewModel() {

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
            val mockResults = listOf(
                VideoItem("s1", "可爱动画合集", "", "动画", 1200, "2分钟前", "\uD83D\uDC30"),
                VideoItem("s2", "萌宠日常", "", "萌宠", 856, "5分钟前", "\uD83D\uDC36"),
                VideoItem("s3", "搞笑片段", "", "搞笑", 2100, "10分钟前", "\uD83D\uDC31"),
                VideoItem("s4", "治愈系视频", "", "治愈", 678, "15分钟前", "\uD83E\uDD8A"),
                VideoItem("s5", "可爱合集", "", "动画", 999, "20分钟前", "\uD83D\uDC3B"),
                VideoItem("s6", "宠物趣事", "", "萌宠", 1500, "25分钟前", "\uD83D\uDC25"),
            )
            val filtered = mockResults.filter {
                it.title.contains(query, ignoreCase = true) || it.category.contains(query, ignoreCase = true)
            }
            _uiState.update {
                it.copy(
                    results = filtered.ifEmpty { mockResults },
                    isSearching = false,
                    error = if (filtered.isEmpty() && query.isNotBlank()) "未找到「$query」相关结果" else null
                )
            }
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
