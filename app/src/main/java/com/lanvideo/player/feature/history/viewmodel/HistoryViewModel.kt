package com.lanvideo.player.feature.history.viewmodel

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.model.RecentWatchItem
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.data.user.AuthSessionStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class HistoryUiState(
    val history: List<RecentWatchItem> = emptyList(),
    val isLoading: Boolean = true,
    val isLoadingMore: Boolean = false,
    val hasMore: Boolean = true,
    val error: String? = null,
    val isLoggedIn: Boolean = false
)

class HistoryViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(HistoryUiState())
    val uiState: StateFlow<HistoryUiState> = _uiState.asStateFlow()

    private var currentPage = 0
    private var totalItems = 0

    fun loadHistory(context: Context) {
        viewModelScope.launch {
            _uiState.update { it.copy(isLoading = true, error = null) }

            if (!AuthSessionStore.isLoggedIn(context)) {
                _uiState.update {
                    it.copy(
                        isLoading = false, isLoggedIn = false,
                        error = "请先登录"
                    )
                }
                return@launch
            }
            _uiState.update { it.copy(isLoggedIn = true) }

            currentPage = 0
            totalItems = 0

            val result = withContext(Dispatchers.IO) {
                VideoRepository.getAllPlaybackHistory(page = 0, size = 20)
            }

            result.onSuccess { resp ->
                val items = resp.items
                currentPage = 1
                totalItems = resp.total.toInt()
                _uiState.update {
                    it.copy(
                        history = items,
                        isLoading = false,
                        hasMore = items.size < resp.total
                    )
                }
            }.onFailure { err ->
                _uiState.update {
                    it.copy(
                        isLoading = false,
                        error = err.message?.take(60) ?: "加载失败"
                    )
                }
            }
        }
    }

    fun loadMore() {
        val s = _uiState.value
        if (s.isLoadingMore || !s.hasMore) return

        _uiState.update { it.copy(isLoadingMore = true) }
        viewModelScope.launch {
            val result = withContext(Dispatchers.IO) {
                VideoRepository.getAllPlaybackHistory(page = currentPage, size = 20)
            }
            result.onSuccess { resp ->
                totalItems = resp.total.toInt()
                currentPage++
                _uiState.update {
                    it.copy(
                        history = it.history + resp.items,
                        isLoadingMore = false,
                        hasMore = it.history.size < totalItems
                    )
                }
            }.onFailure {
                _uiState.update { it.copy(isLoadingMore = false) }
            }
        }
    }
}
