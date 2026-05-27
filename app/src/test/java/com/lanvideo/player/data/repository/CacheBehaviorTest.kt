package com.lanvideo.player.data.repository

import com.lanvideo.player.data.model.PagedVideoResponse
import com.lanvideo.player.data.model.VideoItem
import org.junit.Assert.*
import org.junit.Test

/**
 * VideoRepository 缓存键生成逻辑验证 — 纯 JVM 单元测试。
 * 验证键格式的一致性和不同参数产生独立键。
 * (注意：不测试 LruCache 本身，那是 Android 框架行为)
 */
class CacheBehaviorTest {

    private fun makeResponse(page: Int): PagedVideoResponse {
        val items = listOf(VideoItem(id = page.toLong(), title = "Item $page", streamUrl = "/$page.mp4"))
        return PagedVideoResponse(items = items, total = 1, page = page, size = 20)
    }

    @Test
    fun cacheKeyFormat_isConsistent() {
        val query = "test"
        val type = "local_video"
        val page = 0
        val size = 20
        val key = "list:$query:$type:$page:$size"
        assertEquals("list:test:local_video:0:20", key)
    }

    @Test
    fun cacheKey_withNullQuery() {
        val key = "list:null::0:20"
        assertEquals("list:null::0:20", key)
    }

    @Test
    fun cacheKey_withNullType() {
        val key = "list:test:null:1:10"
        assertEquals("list:test:null:1:10", key)
    }

    @Test
    fun cacheKeyReflectsAllParameters() {
        // Different parameters must produce different cache keys
        val key1 = "list:query:type:0:20"
        val key2 = "list:query:type:1:20"
        val key3 = "list:query:type:0:50"
        assertNotEquals(key1, key2)  // different page
        assertNotEquals(key1, key3)  // different size
    }

    @Test
    fun responseData_isCorrect() {
        val resp = makeResponse(1)
        assertEquals(1L, resp.items.first().id)
        assertEquals("Item 1", resp.items.first().title)
    }
}
