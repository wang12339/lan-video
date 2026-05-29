package com.lanvideo.player.feature.home.viewmodel

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.data.model.PagedVideoResponse
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.network.LanServerDiscovery
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.data.user.AuthSessionStore
import com.lanvideo.player.data.util.loadVideosForChannel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class HomeUiState(
    val videos: List<VideoItem> = emptyList(),
    val bannerItems: List<VideoItem> = emptyList(),
    val currentChannel: Int = 0,
    val isLoading: Boolean = true,
    val isLoadingMore: Boolean = false,
    val isRefreshing: Boolean = false,
    val hasMore: Boolean = true,
    val error: String? = null,
    val isSelectMode: Boolean = false,
    val selectedCount: Int = 0,
    val isAllSelected: Boolean = false,
    val isLoggedIn: Boolean = false,
    val emptyText: String = ""
)

class HomeViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(HomeUiState())
    val uiState: StateFlow<HomeUiState> = _uiState.asStateFlow()

    private var currentPage = 0
    private var totalItems = 0
    private var didInitialLanDiscover = false

    fun loadFeed(context: Context) {
        viewModelScope.launch {
            _uiState.update { it.copy(isLoading = true, error = null) }

            if (!AuthSessionStore.isLoggedIn(context)) {
                _uiState.update {
                    it.copy(
                        isLoading = false, isRefreshing = false,
                        isLoggedIn = false, videos = emptyList(),
                        emptyText = "请先登录"
                    )
                }
                return@launch
            }
            _uiState.update { it.copy(isLoggedIn = true) }

            // 重置分页
            currentPage = 0
            totalItems = 0
            val chan = _uiState.value.currentChannel

            var pagedResponse = loadVideosForChannel(chan)
            if (pagedResponse.isFailure && !didInitialLanDiscover) {
                didInitialLanDiscover = true
                withContext(Dispatchers.IO) {
                    LanServerDiscovery.discoverActiveNetwork(context.applicationContext, force = true)
                }
                pagedResponse = loadVideosForChannel(chan)
            }

            pagedResponse.onSuccess { resp ->
                val items = resp.items
                currentPage = 1
                totalItems = resp.total.toInt()
                _uiState.update {
                    it.copy(
                        videos = items,
                        bannerItems = items.take(5),
                        isLoading = false,
                        isRefreshing = false,
                        hasMore = items.size < resp.total,
                        emptyText = if (items.isEmpty()) emptyChannelText(chan) else "",
                        error = null
                    )
                }
            }.onFailure { err ->
                _uiState.update {
                    it.copy(
                        isLoading = false, isRefreshing = false,
                        error = err.message?.take(60) ?: "加载失败"
                    )
                }
            }
        }
    }

    /** 后台静默刷新，不重置滚动位置 */
    fun refreshFeed(context: Context) {
        viewModelScope.launch {
            val chan = _uiState.value.currentChannel
            if (!AuthSessionStore.isLoggedIn(context)) return@launch
            VideoRepository.invalidateCache()
            val result = loadVideosForChannel(chan)
            result.onSuccess { resp ->
                val items = resp.items
                currentPage = 1
                totalItems = resp.total.toInt()
                _uiState.update {
                    it.copy(
                        videos = items,
                        bannerItems = items.take(5),
                        hasMore = items.size < resp.total,
                        emptyText = if (items.isEmpty()) emptyChannelText(chan) else ""
                    )
                }
            }
        }
    }

    fun loadMore(context: Context) {
        val s = _uiState.value
        if (s.isLoadingMore || !s.hasMore || s.currentChannel == 2) return
        _uiState.update { it.copy(isLoadingMore = true) }

        viewModelScope.launch {
            val result = kotlinx.coroutines.withTimeoutOrNull(5_000L) {
                VideoRepository.listVideos(
                    type = if (s.currentChannel == 1 || s.currentChannel == 0) "!local_image" else null,
                    page = currentPage, size = 20
                ).getOrNull()
            }

            if (result != null) {
                val allItems = _uiState.value.videos + result.items
                totalItems = result.total.toInt()
                currentPage++
                _uiState.update {
                    it.copy(
                        videos = allItems,
                        isLoadingMore = false,
                        hasMore = allItems.size < totalItems
                    )
                }
            } else {
                _uiState.update { it.copy(isLoadingMore = false) }
            }
        }
    }

    fun switchChannel(context: Context, channel: Int) {
        val current = _uiState.value.currentChannel
        if (current == channel) return
        _uiState.update { it.copy(currentChannel = channel) }
        loadFeed(context)
    }

    fun enterSelectMode(initialPosition: Int = -1) {
        _uiState.update { it.copy(isSelectMode = true, selectedCount = 0, isAllSelected = false) }
    }

    fun exitSelectMode() {
        _uiState.update { it.copy(isSelectMode = false, selectedCount = 0, isAllSelected = false) }
    }

    fun toggleSelection(id: Long, allIds: Set<Long>) {
        // 由 Adapter 管理选中状态，ViewModel 仅同步计数
        val selected = allIds
        _uiState.update {
            it.copy(
                isSelectMode = selected.isNotEmpty(),
                selectedCount = selected.size,
                isAllSelected = selected.size == it.videos.size
            )
        }
    }

    fun clearSelection() {
        _uiState.update {
            it.copy(isSelectMode = false, selectedCount = 0, isAllSelected = false)
        }
    }

    // ── Deletion ──

    fun deleteVideo(context: Context, videoId: Long) {
        viewModelScope.launch {
            val result = withContext(Dispatchers.IO) { VideoRepository.deleteVideo(videoId) }
            if (result.isSuccess) {
                VideoRepository.invalidateCache()
                loadFeed(context)
            }
        }
    }

    fun deleteVideos(context: Context, ids: List<Long>, onDone: (Int) -> Unit) {
        viewModelScope.launch {
            val result = VideoRepository.deleteVideos(ids)
            if (result.isSuccess) {
                VideoRepository.invalidateCache()
                _uiState.update { it.copy(isSelectMode = false, selectedCount = 0, isAllSelected = false) }
                onDone(ids.size)
                loadFeed(context)
            }
        }
    }

    private fun emptyChannelText(channel: Int): String {
        val names = listOf("全部", "视频", "图片")
        return "暂无${names.getOrElse(channel) { "ALL" }}"
    }
}
