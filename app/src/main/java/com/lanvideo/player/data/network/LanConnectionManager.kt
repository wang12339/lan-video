package com.lanvideo.player.data.network

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import com.lanvideo.player.MyApplication
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

object LanConnectionManager {
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    @Volatile
    private var started = false
    private var reconnectJob: Job? = null
    private var appContext: Context? = null
    private var connectivityManager: ConnectivityManager? = null

    fun start(context: Context) {
        if (started) return
        val ctx = context.applicationContext
        val cm = ctx.getSystemService(ConnectivityManager::class.java) ?: return
        val app = ctx.applicationContext as MyApplication
        appContext = ctx
        connectivityManager = cm

        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .addTransportType(NetworkCapabilities.TRANSPORT_ETHERNET)
            .build()
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                app.applicationScope.launch {
                    LanServerDiscovery.discover(
                        ctx,
                        cm,
                        network,
                        force = true
                    )
                }
            }

            override fun onLost(network: Network) {
                app.setConnectionState(com.lanvideo.player.ConnectionState.DISCONNECTED)
            }
        }
        cm.registerNetworkCallback(request, callback)
        networkCallback = callback
        started = true

        app.applicationScope.launch {
            LanServerDiscovery.discoverActiveNetwork(ctx, force = true)
        }

        startReconnectPoll()
    }

    fun stop() {
        if (!started) return
        val cm = connectivityManager ?: return
        networkCallback?.let { cm.unregisterNetworkCallback(it) }
        networkCallback = null
        reconnectJob?.cancel()
        reconnectJob = null
        started = false
        appContext = null
        connectivityManager = null
    }

    private fun startReconnectPoll() {
        reconnectJob?.cancel()
        val ctx = appContext ?: return
        val app = ctx.applicationContext as MyApplication
        reconnectJob = app.applicationScope.launch {
            while (isActive) {
                delay(5_000L)
                if (app.connectionState.value == com.lanvideo.player.ConnectionState.DISCONNECTED) {
                    app.setConnectionState(com.lanvideo.player.ConnectionState.SCANNING)
                    LanServerDiscovery.discoverActiveNetwork(
                        ctx,
                        force = true
                    )
                }
            }
        }
    }
}
