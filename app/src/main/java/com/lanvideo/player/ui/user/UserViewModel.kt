package com.lanvideo.player.ui.user

import android.app.Application
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.LanServerRefresh
import com.lanvideo.player.MyApplication
import com.lanvideo.player.data.network.NetworkModule
import com.lanvideo.player.data.user.AuthSessionStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

data class UserUiState(
    val username: String = "游客",
    val signature: String = "点击登录享受更多功能",
    val watchCount: Int = 0,
    val favoriteCount: Int = 0,
    val isLoggedIn: Boolean = false,
    val isLoading: Boolean = false,
    val error: String? = null
)

class UserViewModel(
    private val application: Application
) : ViewModel() {
    private val _uiState = MutableStateFlow(UserUiState())
    val uiState: StateFlow<UserUiState> = _uiState

    private val lanServerObserver = Observer<LanServerRefresh> {
        loadUserInfo()
    }

    init {
        loadUserInfo()
        // Listen for login events
        MyApplication.instance.lanServerEvents.observeForever(lanServerObserver)
    }

    override fun onCleared() {
        super.onCleared()
        MyApplication.instance.lanServerEvents.removeObserver(lanServerObserver)
    }

    fun loadUserInfo() {
        viewModelScope.launch {
            val isLoggedIn = AuthSessionStore.isLoggedIn(application)
            if (!isLoggedIn) {
                _uiState.value = UserUiState(
                    username = "游客",
                    signature = "点击登录享受更多功能",
                    isLoggedIn = false
                )
                return@launch
            }

            _uiState.value = _uiState.value.copy(isLoading = true)
            try {
                val api = NetworkModule.createApi()
                val profile = api.getUserProfile()
                _uiState.value = UserUiState(
                    username = profile.username,
                    signature = if (profile.isAdmin) "管理员" else "可爱用户",
                    watchCount = profile.totalVideosWatched,
                    favoriteCount = 0,
                    isLoggedIn = true,
                    isLoading = false
                )
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    isLoading = false,
                    error = e.message
                )
            }
        }
    }

    fun logout() {
        viewModelScope.launch {
            try {
                val api = NetworkModule.createApi()
                api.logout()
            } catch (_: Exception) {}
            AuthSessionStore.clear(application)
            _uiState.value = UserUiState(
                username = "游客",
                signature = "点击登录享受更多功能",
                isLoggedIn = false
            )
        }
    }
}
