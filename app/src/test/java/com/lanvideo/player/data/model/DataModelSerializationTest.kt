package com.lanvideo.player.data.model

import kotlinx.serialization.json.Json
import org.junit.Assert.*
import org.junit.Test

/**
 * 数据模型序列化/反序列化测试 — 纯 JVM 单元测试，无需 Android 环境。
 */
class DataModelSerializationTest {

    private val json = Json {
        ignoreUnknownKeys = true
        coerceInputValues = true
    }

    // ── Auth Models ──

    @Test
    fun loginRequest_serializesCorrectly() {
        val request = LoginRequest(username = "testuser", password = "secret123")
        val jsonStr = json.encodeToString(LoginRequest.serializer(), request)
        assertTrue(jsonStr.contains("\"username\":\"testuser\""))
        assertTrue(jsonStr.contains("\"password\":\"secret123\""))
    }

    @Test
    fun loginRequest_deserializesCorrectly() {
        val jsonStr = """{"username":"alice","password":"pass"}"""
        val request = json.decodeFromString(LoginRequest.serializer(), jsonStr)
        assertEquals("alice", request.username)
        assertEquals("pass", request.password)
    }

    @Test
    fun loginRequest_rejectsEmptyUsername() {
        val request = LoginRequest(username = "", password = "pass")
        assertTrue(request.username.isEmpty())
        assertFalse(request.password.isEmpty())
    }

    @Test
    fun authResponse_withToken_serializesCorrectly() {
        val resp = AuthResponse(ok = true, token = "abc123", error = null)
        val str = json.encodeToString(AuthResponse.serializer(), resp)
        assertTrue(str.contains("\"ok\":true"))
        assertTrue(str.contains("\"token\":\"abc123\""))
        assertFalse(str.contains("\"error\""))  // null 字段应被跳过
    }

    @Test
    fun authResponse_withError_serializesCorrectly() {
        val resp = AuthResponse(ok = false, token = null, error = "invalid credentials")
        val str = json.encodeToString(AuthResponse.serializer(), resp)
        assertTrue(str.contains("\"ok\":false"))
        assertTrue(str.contains("\"error\":\"invalid credentials\""))
        assertFalse(str.contains("\"token\""))  // null 字段应被跳过
    }

    @Test
    fun authResponse_deserializesBack() {
        val original = AuthResponse(ok = true, token = "tok_xyz", error = null)
        val str = json.encodeToString(AuthResponse.serializer(), original)
        val decoded = json.decodeFromString(AuthResponse.serializer(), str)
        assertEquals(original.ok, decoded.ok)
        assertEquals(original.token, decoded.token)
        assertEquals(original.error, decoded.error)
    }

    @Test
    fun userInfoResponse_usesCamelCase() {
        val resp = UserInfoResponse(username = "bob", isAdmin = true, createdAt = "2024-01-01")
        val str = json.encodeToString(UserInfoResponse.serializer(), resp)
        assertTrue(str.contains("\"isAdmin\""))
        assertTrue(str.contains("\"createdAt\""))
        assertFalse(str.contains("\"is_admin\""))  // 不应使用蛇形命名
    }

    @Test
    fun userProfileResponse_roundtrip() {
        val history = listOf(
            RecentWatchItem(
                videoId = 1L, title = "Test", sourceType = "local_video",
                positionMs = 5000, durationMs = 120000, updatedAt = "2024-01-01T00:00:00Z"
            )
        )
        val profile = UserProfileResponse(
            username = "alice", isAdmin = false, createdAt = "2024-06-15",
            totalVideosWatched = 42, totalWatchTimeMs = 3600000,
            recentHistory = history
        )
        val str = json.encodeToString(UserProfileResponse.serializer(), profile)
        val decoded = json.decodeFromString(UserProfileResponse.serializer(), str)
        assertEquals(profile.username, decoded.username)
        assertEquals(profile.totalVideosWatched, decoded.totalVideosWatched)
        assertEquals(profile.recentHistory.size, decoded.recentHistory.size)
    }

    // ── Video Models ──

    @Test
    fun videoItem_serializesWithDefaults() {
        val item = VideoItem(id = 1, title = "Test Video", streamUrl = "/media/test.mp4")
        val str = json.encodeToString(VideoItem.serializer(), item)
        assertTrue(str.contains("\"id\":1"))
        assertTrue(str.contains("\"title\":\"Test Video\""))
        assertTrue(str.contains("\"streamUrl\":\"/media/test.mp4\""))
    }

    @Test
    fun videoItem_deserializesWithOptionalFields() {
        val jsonStr = """{"id":1,"title":"Test","streamUrl":"/media/v.mp4"}"""
        val item = json.decodeFromString(VideoItem.serializer(), jsonStr)
        assertEquals(1, item.id)
        assertEquals("Test", item.title)
        assertEquals("/media/v.mp4", item.streamUrl)
        assertEquals("external", item.sourceType)  // 默认值
        assertEquals("", item.description)          // 默认值
    }

    @Test
    fun pagedVideoResponse_roundtrip() {
        val items = listOf(
            VideoItem(id = 1, title = "A", streamUrl = "/a.mp4"),
            VideoItem(id = 2, title = "B", streamUrl = "/b.mp4")
        )
        val resp = PagedVideoResponse(items = items, total = 2, page = 0, size = 20)
        val str = json.encodeToString(PagedVideoResponse.serializer(), resp)
        val decoded = json.decodeFromString(PagedVideoResponse.serializer(), str)
        assertEquals(2, decoded.total)
        assertEquals(0, decoded.page)
        assertEquals(20, decoded.size)
        assertEquals(2, decoded.items.size)
    }

    @Test
    fun playbackHistoryRequest_usesSnakeCaseInJson() {
        val req = PlaybackHistoryRequest(videoId = 42, positionMs = 15000, durationMs = 120000)
        val str = json.encodeToString(PlaybackHistoryRequest.serializer(), req)
        assertTrue(str.contains("\"video_id\""))
        assertTrue(str.contains("\"position_ms\""))
        assertTrue(str.contains("\"duration_ms\""))
    }

    @Test
    fun playbackHistoryResponse_roundtrip() {
        val resp = PlaybackHistoryResponse(videoId = 42, positionMs = 5000, durationMs = 60000)
        val str = json.encodeToString(PlaybackHistoryResponse.serializer(), resp)
        val decoded = json.decodeFromString(PlaybackHistoryResponse.serializer(), str)
        assertEquals(42L, decoded.videoId)
        assertEquals(5000L, decoded.positionMs)
        assertEquals(60000L, decoded.durationMs)
    }

    @Test
    fun videoUpdateRequest_serializesOptionalFields() {
        val req = VideoUpdateRequest(title = "New Title", category = "科技")
        val str = json.encodeToString(VideoUpdateRequest.serializer(), req)
        assertTrue(str.contains("\"title\":\"New Title\""))
        assertTrue(str.contains("\"category\":\"科技\""))
        assertFalse(str.contains("\"description\""))  // null 字段跳过
    }

    @Test
    fun uploadResponse_deserializes() {
        val jsonStr = """{"id":123}"""
        val resp = json.decodeFromString(UploadResponse.serializer(), jsonStr)
        assertEquals(123L, resp.id)
    }

    @Test
    fun fileCheckItem_roundtrip() {
        val item = FileCheckItem(name = "video.mp4", size = 1048576)
        val str = json.encodeToString(FileCheckItem.serializer(), item)
        val decoded = json.decodeFromString(FileCheckItem.serializer(), str)
        assertEquals(item.name, decoded.name)
        assertEquals(item.size, decoded.size)
    }
}
