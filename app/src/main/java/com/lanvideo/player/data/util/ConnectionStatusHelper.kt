package com.lanvideo.player.data.util

import android.graphics.drawable.AnimationDrawable
import android.view.View
import android.widget.TextView
import androidx.core.view.isVisible
import androidx.lifecycle.LifecycleOwner
import com.lanvideo.player.ConnectionState
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.network.LanServerDiscovery
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * 连接状态栏的封装工具 — 统一 4 个 Fragment 中重复的连接状态处理。
 *
 * 用法：
 * ```
 * val connHelper = ConnectionStatusHelper(
 *     statusView = binding.connectionStatus,
 *     statusDot = binding.statusDot,
 *     statusText = binding.statusText,
 * )
 * connHelper.observe(viewLifecycleOwner, app, lifecycleScope)
 * ```
 */
class ConnectionStatusHelper(
    private val statusView: View,
    private val statusDot: View,
    private val statusText: TextView,
) {
    /**
     * 绑定 LiveData 观察 + 点击重扫事件。
     */
    fun observe(lifecycleOwner: LifecycleOwner, app: MyApplication, scope: CoroutineScope) {
        app.connectionState.observe(lifecycleOwner) { state -> update(state) }
        statusView.setOnClickListener {
            app.setConnectionState(ConnectionState.SCANNING)
            scope.launch {
                LanServerDiscovery.discoverActiveNetwork(
                    statusView.context.applicationContext, force = true
                )
            }
        }
    }

    private fun update(state: ConnectionState) {
        when (state) {
            ConnectionState.CONNECTED -> {
                statusView.isVisible = false
            }
            ConnectionState.SCANNING -> {
                statusView.isVisible = true
                statusDot.setBackgroundResource(R.drawable.bg_status_pulse)
                (statusDot.background as? AnimationDrawable)?.start()
                statusText.setText(R.string.connection_scanning)
            }
            ConnectionState.DISCONNECTED -> {
                statusView.isVisible = true
                statusDot.setBackgroundResource(R.drawable.status_dot_red)
                statusText.setText(R.string.connection_disconnected)
            }
        }
    }
}
