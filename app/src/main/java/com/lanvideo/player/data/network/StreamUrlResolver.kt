package com.lanvideo.player.data.network

/**
 * 后端 [stream_url] 多为站点相对路径（如 /media/xxx.mp4）或已带完整 http(s) 地址。
 * ExoPlayer/Coil 需绝对 URL 才能从局域网服务拉流。
 *
 * 认证通过 [AuthDataSourceFactory] 提供的自定义 DataSource 在请求头中附加 token，
 * 不再将 token 放在 URL 查询参数中。
 */
object StreamUrlResolver {
    fun toAbsoluteStreamUrl(
        streamUrl: String,
        baseUrl: String = NetworkModule.getBaseUrl()
    ): String {
        val t = streamUrl.trim()
        if (t.isEmpty()) return t
        return if (t.startsWith("http://", ignoreCase = true) || t.startsWith("https://", ignoreCase = true)) {
            t
        } else {
            val b = baseUrl.trim().removeSuffix("/")
            if (t.startsWith("/")) b + t else "$b/$t"
        }
    }
}
