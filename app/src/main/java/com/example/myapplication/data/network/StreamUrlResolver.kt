package com.example.myapplication.data.network

/**
 * 后端 [stream_url] 多为站点相对路径（如 /media/xxx.mp4）或已带完整 http(s) 地址。
 * ExoPlayer/Coil 需绝对 URL 才能从局域网服务拉流。
 */
object StreamUrlResolver {
    fun toAbsoluteStreamUrl(
        streamUrl: String,
        baseUrl: String = NetworkModule.getBaseUrl()
    ): String {
        val t = streamUrl.trim()
        if (t.isEmpty()) return t
        if (t.startsWith("http://", ignoreCase = true) || t.startsWith("https://", ignoreCase = true)) {
            return t
        }
        val b = baseUrl.trim().removeSuffix("/")
        return if (t.startsWith("/")) {
            b + t
        } else {
            "$b/$t"
        }
    }
}
