package com.lanvideo.player.feature.search.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.repository.VideoRepository
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SearchUiState(
    val query: String = "",
    val results: List<VideoItem> = allOf(),
    val isLoading: Boolean = false,
    val isLoadingMore: Boolean = false,
    val hasMore: Boolean = true,
    val totalFound: Long = 0,
    val error: String? = null
) {
    companion object {
        fun allOf() = emptyList<VideoItem>()
    }
}

class SearchViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(SearchUiState())
    val uiState: StateFlow<SearchUiState> = _uiState.asStateFlow()

    private var searchJob: Job? = null
    private var currentPage = 0
    private var totalItems: Long = 0

    fun search(query: String) {
        if (query == _uiState.value.query && query.isNotBlank()) return
        _uiState.update { it.copy(query = query) }

        searchJob?.cancel()
        if (query.isBlank()) {
            _uiState.update { it.copy(results = emptyList(), totalFound = 0, isLoading = false) }
            return
        }

        searchJob = viewModelScope.launch {
            delay(300) // 防抖
            _uiState.update { it.copy(isLoading = true, error = null) }

            val result = VideoRepository.listVideos(query = query, page = 0, size = 20)
            result.onSuccess { resp ->
                val items = resp.items
                currentPage = 0
                totalItems = resp.total
                _uiState.update {
                    it.copy(
                        results = items,
                        isLoading = false,
                        totalFound = resp.total,
                        hasMore = items.size < resp.total,
                        error = if (items.isEmpty()) "未找到「$query」相关结果" else null
                    )
                }
            }.onFailure { err ->
                _uiState.update {
                    it.copy(
                        isLoading = false,
                        results = emptyList(),
                        error = err.message?.take(60) ?: "搜索失败"
                    )
                }
            }
        }
    }

    fun loadNextPage() {
        val s = _uiState.value
        if (s.isLoadingMore || s.results.size >= s.totalFound) return

        _uiState.update { it.copy(isLoadingMore = true) }
        viewModelScope.launch {
            val nextPage = currentPage + 1
            VideoRepository.listVideos(query = s.query, page = nextPage, size = 20)
                .onSuccess { resp ->
                    currentPage = nextPage
                    totalItems = resp.total
                    _uiState.update {
                        it.copy(
                            results = it.results + resp.items,
                            isLoadingMore = false,
                            hasMore = it.results.size < resp.total
                        )
                    }
                }
                .onFailure {
                    _uiState.update { it.copy(isLoadingMore = false) }
                }
        }
    }
}
