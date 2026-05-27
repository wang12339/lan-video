package com.lanvideo.player.data.network

import android.content.Context
import android.content.SharedPreferences

object ServerConfigStore {
    private const val PREFS = "lan_video_prefs"
    private const val KEY_BASE_URL = "server_base_url"

    private fun prefs(context: Context): SharedPreferences {
        return context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    }

    fun loadBaseUrl(context: Context): String? {
        return prefs(context).getString(KEY_BASE_URL, null)
    }

    fun saveBaseUrl(context: Context, baseUrl: String) {
        prefs(context).edit().putString(KEY_BASE_URL, baseUrl).apply()
    }
}
