package com.lanvideo.player

import android.app.Application
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import com.lanvideo.player.data.network.NetworkModule
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import io.sentry.Sentry
import io.sentry.android.core.SentryAndroid
import com.lanvideo.player.BuildConfig

data class LanServerRefresh(
    val baseUrl: String,
    val id: Long = System.nanoTime()
)

enum class ConnectionState { CONNECTED, SCANNING, DISCONNECTED }

class MyApplication : Application() {
    private val applicationJob = SupervisorJob()
    val applicationScope = CoroutineScope(applicationJob + Dispatchers.Main.immediate)

    private val _lanServerEvents = MutableLiveData<LanServerRefresh>()
    val lanServerEvents: LiveData<LanServerRefresh> = _lanServerEvents

    private val _connectionState = MutableLiveData(ConnectionState.SCANNING)
    val connectionState: LiveData<ConnectionState> = _connectionState

    private val _batchDeleteRequested = MutableLiveData(false)
    val batchDeleteRequested: LiveData<Boolean> = _batchDeleteRequested

    private val _unauthorizedEvent = MutableLiveData<Boolean?>(null)
    val unauthorizedEvent: LiveData<Boolean?> = _unauthorizedEvent

    override fun onCreate() {
        super.onCreate()
        instance = this
        NetworkModule.init(this)

        // ── Sentry crash reporting ──
        initSentry()

        // Custom Coil ImageLoader with memory cache + disk cache + auth
        val imageLoader = coil.ImageLoader.Builder(this)
            .okHttpClient { NetworkModule.httpClient() }
            .memoryCache {
                coil.memory.MemoryCache.Builder(this)
                    .maxSizePercent(0.25)
                    .build()
            }
            .diskCache {
                coil.disk.DiskCache.Builder()
                    .directory(cacheDir.resolve("image_cache"))
                    .maxSizeBytes(100L * 1024 * 1024)
                    .build()
            }
            .build()
        coil.Coil.setImageLoader(imageLoader)
    }

    private fun initSentry() {
        val dsn = BuildConfig.SENTRY_DSN
        if (dsn.isNotBlank()) {
            SentryAndroid.init(this) { options ->
                options.dsn = dsn
                options.tracesSampleRate = if (BuildConfig.BUILD_VARIANT == "release") 0.2 else 1.0
                options.environment = BuildConfig.BUILD_VARIANT
                options.isSendDefaultPii = true
                options.isAttachScreenshot = true
                options.isAttachViewHierarchy = true
                options.profilesSampleRate = 0.2
            }
            Sentry.setTag("app_version", BuildConfig.VERSION_NAME)
            Sentry.setTag("app_build", BuildConfig.VERSION_CODE.toString())
        }
    }

    override fun onTerminate() {
        applicationJob.cancel()
        super.onTerminate()
    }

    fun notifyLanServerEvent(baseUrl: String) {
        _lanServerEvents.postValue(LanServerRefresh(baseUrl))
    }

    fun setConnectionState(state: ConnectionState) {
        _connectionState.postValue(state)
    }

    fun setBatchDeleteRequested(value: Boolean) {
        _batchDeleteRequested.postValue(value)
    }

    fun notifyUnauthorized() {
        _unauthorizedEvent.postValue(true)
    }

    fun clearUnauthorizedEvent() {
        _unauthorizedEvent.postValue(false)
    }

    companion object {
        lateinit var instance: MyApplication
            private set
    }
}
