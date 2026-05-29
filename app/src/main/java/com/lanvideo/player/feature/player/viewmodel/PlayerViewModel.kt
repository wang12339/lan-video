package com.lanvideo.player.feature.player.viewmodel

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.repository.VideoRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class PlayerUiState(
    val currentSpeed: Float = 1f,
    val userPaused: Boolean = false,
    val hudVisible: Boolean = true,
    val isInPip: Boolean = false
)

class PlayerViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(PlayerUiState())
    val uiState: StateFlow<PlayerUiState> = _uiState.asStateFlow()

    private var saveJob: Job? = null
    private var lastSavedPositionMs: Long = 0L

    fun setSpeed(speed: Float) {
        _uiState.update { it.copy(currentSpeed = speed) }
    }

    fun togglePause() {
        _uiState.update { it.copy(userPaused = !it.userPaused) }
    }

    fun setUserPaused(paused: Boolean) {
        _uiState.update { it.copy(userPaused = paused) }
    }

    fun toggleHud() {
        _uiState.update { it.copy(hudVisible = !it.hudVisible) }
    }

    fun setHudVisible(visible: Boolean) {
        _uiState.update { it.copy(hudVisible = visible) }
    }

    fun setInPip(pip: Boolean) {
        _uiState.update { it.copy(hudVisible = !pip, isInPip = pip) }
    }

    /** 启动定时保存播放进度 */
    fun startPositionSaver(videoId: Long) {
        saveJob?.cancel()
        saveJob = viewModelScope.launch {
            while (isActive) {
                delay(30_000)
                // 外部通过 savePosition() 触发实际保存
            }
        }
    }

    fun savePosition(videoId: Long, positionMs: Long, durationMs: Long) {
        if (durationMs <= 0 || positionMs == lastSavedPositionMs) return
        lastSavedPositionMs = positionMs
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                VideoRepository.updatePlaybackHistory(videoId, positionMs, durationMs)
            }
        }
    }

    fun cancelPositionSaver() {
        saveJob?.cancel()
    }
}
