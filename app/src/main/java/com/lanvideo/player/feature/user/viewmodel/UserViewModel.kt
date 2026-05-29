package com.lanvideo.player.feature.user.viewmodel

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.model.RecentWatchItem
import com.lanvideo.player.data.model.UserProfileResponse
import com.lanvideo.player.data.network.NetworkModule
import com.lanvideo.player.data.user.AuthSessionStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class UserUiState(
    val isLoggedIn: Boolean = false,
    val username: String? = null,
    val avatarLetter: String = "?",
    val isAdmin: Boolean = false,
    val registeredAt: String = "",
    val totalWatched: Int = 0,
    val watchTimeText: String = "",
    val recentHistory: List<RecentWatchItem> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null
)

class UserViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(UserUiState())
    val uiState: StateFlow<UserUiState> = _uiState.asStateFlow()

    fun loadProfile(context: Context) {
        viewModelScope.launch {
            val username = AuthSessionStore.getUsername(context)

            if (username == null) {
                _uiState.update { it.copy(isLoggedIn = false) }
                return@launch
            }

            _uiState.update {
                it.copy(
                    isLoggedIn = true,
                    username = username,
                    avatarLetter = username.firstOrNull()?.uppercase() ?: "U",
                    isLoading = true
                )
            }

            val result = withContext(Dispatchers.IO) {
                runCatching {
                    NetworkModule.createApi().getUserProfile()
                }
            }

            result.onSuccess { profile ->
                val hours = profile.totalWatchTimeMs / 3600000f
                val watchTimeText = if (hours >= 1f) {
                    String.format("%.1f 小时", hours)
                } else {
                    "${profile.totalWatchTimeMs / 60000}分钟"
                }

                _uiState.update {
                    it.copy(
                        isLoading = false,
                        isAdmin = profile.isAdmin,
                        registeredAt = profile.createdAt.take(10),
                        totalWatched = profile.totalVideosWatched,
                        watchTimeText = watchTimeText,
                        recentHistory = profile.recentHistory
                    )
                }
            }.onFailure { e ->
                _uiState.update {
                    it.copy(
                        isLoading = false,
                        error = e.message?.take(40) ?: "加载失败"
                    )
                }
            }
        }
    }

    fun logout(context: Context) {
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    val api = NetworkModule.createApi()
                    runCatching { api.logout() }
                }
            } catch (_: Exception) { }
            AuthSessionStore.clear(context)
        }
    }
}
