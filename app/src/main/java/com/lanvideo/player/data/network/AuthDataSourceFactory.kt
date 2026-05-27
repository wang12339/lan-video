package com.lanvideo.player.data.network

import android.content.Context
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultHttpDataSource
import com.lanvideo.player.data.user.AuthSessionStore

/**
 * 为 ExoPlayer 提供带 Authorization header 的 DataSource.Factory。
 * 避免将 token 放在 URL 查询参数中。
 */
object AuthDataSourceFactory {
    fun create(context: Context): DataSource.Factory {
        val token = AuthSessionStore.getToken(context)
        val props = if (token != null) {
            mapOf("Authorization" to "Bearer $token")
        } else {
            emptyMap()
        }
        return DefaultHttpDataSource.Factory()
            .setDefaultRequestProperties(props)
            .setConnectTimeoutMs(30_000)
            .setReadTimeoutMs(4 * 60 * 60 * 1000)
    }
}
