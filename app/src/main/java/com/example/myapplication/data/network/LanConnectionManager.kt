package com.example.myapplication.data.network

import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import androidx.appcompat.app.AppCompatActivity
import com.example.myapplication.MyApplication
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

object LanConnectionManager {
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    @Volatile
    private var started = false
    private var reconnectJob: Job? = null

    fun start(activity: AppCompatActivity) {
        if (started) return
        val cm = activity.getSystemService(ConnectivityManager::class.java) ?: return
        val app = activity.application as MyApplication
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .addTransportType(NetworkCapabilities.TRANSPORT_ETHERNET)
            .build()
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                app.applicationScope.launch {
                    LanServerDiscovery.discover(
                        activity.applicationContext,
                        cm,
                        network,
                        force = true
                    )
                }
            }

            override fun onLost(network: Network) {
                app.setConnectionState(com.example.myapplication.ConnectionState.DISCONNECTED)
            }
        }
        cm.registerNetworkCallback(request, callback)
        networkCallback = callback
        started = true

        app.applicationScope.launch {
            LanServerDiscovery.discoverActiveNetwork(activity.applicationContext, force = true)
        }

        startReconnectPoll(activity)
    }

    fun stop(activity: AppCompatActivity) {
        if (!started) return
        val cm = activity.getSystemService(ConnectivityManager::class.java) ?: return
        networkCallback?.let { cm.unregisterNetworkCallback(it) }
        networkCallback = null
        reconnectJob?.cancel()
        reconnectJob = null
        started = false
    }

    private fun startReconnectPoll(activity: AppCompatActivity) {
        reconnectJob?.cancel()
        val app = activity.application as MyApplication
        reconnectJob = app.applicationScope.launch {
            while (isActive) {
                delay(5_000L)
                if (app.connectionState.value == com.example.myapplication.ConnectionState.DISCONNECTED) {
                    app.setConnectionState(com.example.myapplication.ConnectionState.SCANNING)
                    LanServerDiscovery.discoverActiveNetwork(
                        activity.applicationContext,
                        force = true
                    )
                }
            }
        }
    }
}
