package com.lanvideo.player.data.network

import org.junit.Assert.*
import org.junit.Test

/**
 * StreamUrlResolver 纯逻辑测试 — 不依赖 Android 环境。
 */
class StreamUrlResolverTest {

    @Test
    fun absoluteUrl_returnsAsIs() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "http://server:8082/media/video.mp4",
            baseUrl = "http://localhost:8082"
        )
        assertEquals("http://server:8082/media/video.mp4", result)
    }

    @Test
    fun httpsUrl_returnsAsIs() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "https://cdn.example.com/video.mp4",
            baseUrl = "http://localhost:8082"
        )
        assertEquals("https://cdn.example.com/video.mp4", result)
    }

    @Test
    fun relativeUrl_prependsBaseUrl() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "/media/video.mp4",
            baseUrl = "http://192.168.1.100:8082"
        )
        assertEquals("http://192.168.1.100:8082/media/video.mp4", result)
    }

    @Test
    fun relativeUrl_withoutLeadingSlash() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "media/video.mp4",
            baseUrl = "http://192.168.1.100:8082"
        )
        assertEquals("http://192.168.1.100:8082/media/video.mp4", result)
    }

    @Test
    fun baseUrl_withTrailingSlash() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "/media/video.mp4",
            baseUrl = "http://192.168.1.100:8082/"
        )
        assertEquals("http://192.168.1.100:8082/media/video.mp4", result)
    }

    @Test
    fun emptyStreamUrl_returnsEmpty() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "",
            baseUrl = "http://localhost:8082"
        )
        assertEquals("", result)
    }

    @Test
    fun whitespaceStreamUrl_returnsEmpty() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "   ",
            baseUrl = "http://localhost:8082"
        )
        assertEquals("", result)
    }

    @Test
    fun baseUrlTrimmed() {
        val result = StreamUrlResolver.toAbsoluteStreamUrl(
            streamUrl = "/media/v.mp4",
            baseUrl = "  http://localhost:8082  "
        )
        assertEquals("http://localhost:8082/media/v.mp4", result)
    }
}
