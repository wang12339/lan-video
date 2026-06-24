package com.lanvideo.player.ui.history

import android.app.Application
import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.BuildConfig
import com.lanvideo.player.data.network.NetworkModule
import com.lanvideo.player.data.user.AuthSessionStore
import com.lanvideo.player.util.VideoFormatters
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

data class HistoryUiState(
    val historyItems: List<HistoryItem> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null
)

class HistoryViewModel(
    private val application: Application
) : ViewModel() {
    private val _uiState = MutableStateFlow(HistoryUiState())
    val uiState: StateFlow<HistoryUiState> = _uiState

    init {
        loadHistory()
    }

    fun loadHistory() {
        viewModelScope.launch {
            val isLoggedIn = AuthSessionStore.isLoggedIn(application)
            if (BuildConfig.DEBUG) Log.d("HistoryVM", "isLoggedIn=$isLoggedIn")
            if (!isLoggedIn) {
                _uiState.value = HistoryUiState(
                    error = "请先登录"
                )
                return@launch
            }

            _uiState.value = _uiState.value.copy(isLoading = true)
            try {
                val api = NetworkModule.createApi()
                val profile = api.getUserProfile()
                if (BuildConfig.DEBUG) Log.d("HistoryVM", "profile: totalWatched=${profile.totalVideosWatched}, historySize=${profile.recentHistory.size}")
                val items = profile.recentHistory.map { watch ->
                    HistoryItem(
                        id = watch.videoId.toString(),
                        title = watch.title,
                        icon = VideoFormatters.getCategoryIcon(watch.category),
                        timestamp = watch.updatedAt,
                        progress = if (watch.durationMs > 0) {
                            ((watch.positionMs * 100) / watch.durationMs).toInt()
                        } else 0
                    )
                }
                if (BuildConfig.DEBUG) Log.d("HistoryVM", "mapped items: ${items.size}")
                _uiState.value = HistoryUiState(
                    historyItems = items,
                    isLoading = false
                )
            } catch (e: Exception) {
                if (BuildConfig.DEBUG) Log.e("HistoryVM", "loadHistory failed", e)
                _uiState.value = HistoryUiState(
                    isLoading = false,
                    error = e.message
                )
            }
        }
    }
}
