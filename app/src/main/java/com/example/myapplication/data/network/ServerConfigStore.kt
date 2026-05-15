package com.example.myapplication.data.network

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

    /** 仅用于迁走错误默认的网关地址，让自动扫网重新选主机 */
    fun removeBaseUrl(context: Context) {
        prefs(context).edit().remove(KEY_BASE_URL).apply()
    }

    fun loadAdminToken(context: Context): String? {
        return prefs(context).getString(KEY_ADMIN_TOKEN, null)
    }

    fun saveAdminToken(context: Context, token: String) {
        prefs(context).edit().putString(KEY_ADMIN_TOKEN, token).apply()
    }

    private const val KEY_ADMIN_TOKEN = "admin_token"
}
