package com.lanvideo.player.data.util

import android.os.Bundle
import com.lanvideo.player.data.model.RecentWatchItem
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.data.model.PagedVideoResponse
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout

/**
 * 统一导航 Bundle 构建，避免 4 个 Fragment 重复写同样的 bundle 代码
 */

fun VideoItem.toPlayerBundle(): Bundle = Bundle().apply {
    putLong("videoId", id)
    putString("title", title)
    putString("streamUrl", streamUrl)
    putString("category", category)
    watchPosition?.let { putLong("watchPosition", it) }
}

fun VideoItem.toImageViewerBundle(): Bundle = Bundle().apply {
    putLong("videoId", id)
}

fun RecentWatchItem.toPlayerBundle(): Bundle = Bundle().apply {
    putLong("videoId", videoId)
    putString("title", title)
    putString("streamUrl", streamUrl)
    putString("category", category)
    putLong("watchPosition", positionMs)
}

/**
 * 按频道加载视频列表（复用 loadFeed 里的查询逻辑）
 */
suspend fun loadVideosForChannel(
    videoRepository: VideoRepository,
    channel: Int,
    page: Int = 0,
    timeoutMs: Long = 5_000L
): Result<PagedVideoResponse> = withContext(Dispatchers.IO) {
    runCatching {
        withTimeout(timeoutMs) {
            when (channel) {
                2 -> {
                    videoRepository.listVideos(type = "local_image", page = 0, size = 5000)
                        .getOrThrow()
                }
                1 -> {
                    videoRepository.listVideos(type = "!local_image", page = page, size = 200)
                        .getOrThrow()
                }
                else -> {
                    val vidsResult = videoRepository.listVideos(type = "!local_image", page = page, size = 500)
                    val imgsResult = videoRepository.listVideos(type = "local_image", page = 0, size = 5000)

                    val vidsData = vidsResult.getOrNull()
                    val imgsData = imgsResult.getOrNull()

                    if (vidsData != null && imgsData != null) {
                        PagedVideoResponse(
                            items = vidsData.items + imgsData.items,
                            total = vidsData.total + imgsData.total,
                            page = page,
                            size = vidsData.size + imgsData.size
                        )
                    } else if (vidsData != null) {
                        vidsData
                    } else if (imgsData != null) {
                        imgsData
                    } else {
                        throw java.io.IOException(
                            vidsResult.exceptionOrNull()?.message
                                ?: imgsResult.exceptionOrNull()?.message
                                ?: "无法连接服务器"
                        )
                    }
                }
            }
        }
    }
}
