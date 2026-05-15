package com.example.myapplication

import android.app.Application
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import com.example.myapplication.data.network.NetworkModule
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

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

    override fun onCreate() {
        super.onCreate()
        instance = this
        NetworkModule.init(this)

        // Custom Coil ImageLoader with memory cache + disk cache
        val imageLoader = coil.ImageLoader.Builder(this)
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

    companion object {
        lateinit var instance: MyApplication
            private set
    }
}
