package com.example.myapplication.data.network

import android.content.Context
import com.example.myapplication.MyApplication
import com.example.myapplication.data.user.AuthSessionStore
import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.logging.HttpLoggingInterceptor
import retrofit2.Retrofit
import com.jakewharton.retrofit2.converter.kotlinx.serialization.asConverterFactory
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import java.io.File
import okhttp3.Cache

object NetworkModule {
    @Volatile
    private var currentBaseUrl: String = "http://192.168.66.186:8082/"

    private var appContext: Context? = null

    private val json = Json {
        ignoreUnknownKeys = true
    }

    /**
     * 不能用 HEADERS+BODY 记录完整 body：上传视频时 [HttpLoggingInterceptor] 会试图缓冲整个流导致失败或 OOM。
     * 大文件需足够长的 [readTimeout]/[writeTimeout]（默认 10s 会中断上传）。
     */
    private val client by lazy {
        val ctx = appContext
        OkHttpClient.Builder()
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(10, TimeUnit.MINUTES)
            .writeTimeout(10, TimeUnit.MINUTES)
            .addInterceptor { chain ->
                val original = chain.request()
                val builder = original.newBuilder()
                if (ctx != null) {
                    val username = AuthSessionStore.getUsername(ctx)
                    if (username != null) {
                        builder.header("X-Username", username)
                    }
                    val token = AuthSessionStore.getToken(ctx)
                    if (token != null) {
                        builder.header("Authorization", "Bearer $token")
                    }
                }
                chain.proceed(builder.build())
            }
            .addInterceptor(HttpLoggingInterceptor().apply {
                level = HttpLoggingInterceptor.Level.HEADERS
            })
            .cache(Cache(ctx?.cacheDir?.resolve("okhttp_cache") ?: File("."), 50 * 1024 * 1024))
            .build()
    }

    fun init(context: Context) {
        appContext = context.applicationContext
        val ctx = context.applicationContext
        val raw = ServerConfigStore.loadBaseUrl(ctx)?.trim()
        // Clear old default IPs (192.168.1.x) so the new default takes effect
        if (raw != null && isOldDefault(raw)) {
            ServerConfigStore.removeBaseUrl(ctx)
            currentBaseUrl = "http://192.168.66.186:8082/"
        } else if (!raw.isNullOrBlank()) {
            currentBaseUrl = if (raw.endsWith("/")) raw else "$raw/"
        } else {
            currentBaseUrl = "http://192.168.66.186:8082/"
        }
    }

    private fun isOldDefault(url: String): Boolean {
        val u = url.trim().removeSuffix("/").lowercase()
        return u == "http://192.168.1.100:8082" || u == "http://192.168.1.1:8082"
    }

    fun updateBaseUrl(newBaseUrl: String, context: Context? = null, notify: Boolean = true) {
        val normalized = if (newBaseUrl.endsWith("/")) newBaseUrl else "$newBaseUrl/"
        val previous = currentBaseUrl
        currentBaseUrl = normalized
        val ctx = context?.applicationContext ?: appContext
        if (ctx != null) {
            ServerConfigStore.saveBaseUrl(ctx, normalized)
        }
        if (notify && (normalized != previous)) {
            runCatching { MyApplication.instance.notifyLanServerEvent(normalized) }
        }
    }

    fun getBaseUrl(): String = currentBaseUrl

    /** 与 [createApi] 共用的 OkHttp 实例，供 multipart 上传等使用 */
    fun httpClient(): OkHttpClient = client

    fun createApi(): VideoApiService {
        return Retrofit.Builder()
            .baseUrl(currentBaseUrl)
            .client(client)
            .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
            .build()
            .create(VideoApiService::class.java)
    }

}
