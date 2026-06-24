package com.lanvideo.player.data.network

import android.content.Context
import com.lanvideo.player.BuildConfig
import com.lanvideo.player.MyApplication
import com.lanvideo.player.data.user.AuthSessionStore
import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.logging.HttpLoggingInterceptor
import retrofit2.Retrofit
import com.jakewharton.retrofit2.converter.kotlinx.serialization.asConverterFactory
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Cache

object NetworkModule {
    @Volatile
    private var currentBaseUrl: String = ""

    private val json = Json {
        ignoreUnknownKeys = true
        coerceInputValues = true
    }

    @Volatile
    private var cachedApi: VideoApiService? = null

    private val client by lazy {
        val app = MyApplication.instance
        val builder = OkHttpClient.Builder()
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .writeTimeout(30, TimeUnit.SECONDS)
            .addInterceptor { chain ->
                val original = chain.request()
                val reqBuilder = original.newBuilder()
                val token = AuthSessionStore.getToken(app)
                if (token != null) {
                    reqBuilder.header("Authorization", "Bearer $token")
                }
                val response = chain.proceed(reqBuilder.build())
                // Token expired/invalid — clear session
                if (response.code == 401 && token != null) {
                    AuthSessionStore.clear(app)
                    MyApplication.instance.notifyUnauthorized()
                }
                response
            }
        if (BuildConfig.DEBUG) {
            builder.addInterceptor(HttpLoggingInterceptor().apply {
                level = HttpLoggingInterceptor.Level.BASIC
            })
        }
        builder.build()
    }

    val uploadClient by lazy {
        client.newBuilder()
            .readTimeout(4, TimeUnit.HOURS)
            .writeTimeout(4, TimeUnit.HOURS)
            .build()
    }

    private const val DEFAULT_BASE_URL = "https://atmos.whanghui.top"

    fun init(context: Context) {
        val saved = ServerConfigStore.loadBaseUrl(context)?.trim()
        if (!saved.isNullOrBlank()) {
            currentBaseUrl = if (saved.endsWith("/")) saved else "$saved/"
        } else {
            currentBaseUrl = DEFAULT_BASE_URL
            ServerConfigStore.saveBaseUrl(context, DEFAULT_BASE_URL)
        }
    }

    fun updateBaseUrl(newBaseUrl: String, context: Context? = null, notify: Boolean = true) {
        val normalized = if (newBaseUrl.endsWith("/")) newBaseUrl else "$newBaseUrl/"
        val previous = currentBaseUrl
        currentBaseUrl = normalized
        val ctx = context?.applicationContext ?: MyApplication.instance
        ServerConfigStore.saveBaseUrl(ctx, normalized)
        if (notify && (normalized != previous)) {
            cachedApi = null // invalidate on URL change
            runCatching { MyApplication.instance.notifyLanServerEvent(normalized) }
        }
    }

    fun getBaseUrl(): String = currentBaseUrl

    fun httpClient(): OkHttpClient = client

    fun createApi(): VideoApiService {
        val cached = cachedApi
        if (cached != null) return cached
        return synchronized(this) {
            cachedApi?.let { return@synchronized it }
            val api = Retrofit.Builder()
                .baseUrl(currentBaseUrl)
                .client(client)
                .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
                .build()
                .create(VideoApiService::class.java)
            cachedApi = api
            api
        }
    }
}
