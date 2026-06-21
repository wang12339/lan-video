package com.lanvideo.player.ui.user

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

data class UserUiState(
    val username: String = "可爱用户",
    val signature: String = "喜欢可爱的一切~",
    val watchCount: Int = 128,
    val favoriteCount: Int = 36
)

class UserViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(UserUiState())
    val uiState: StateFlow<UserUiState> = _uiState
}
