package com.example.myapplication.data.user

import android.content.Context
import android.content.SharedPreferences

object AuthSessionStore {
    private const val PREFS = "auth_session"
    private const val KEY_TOKEN = "token"
    private const val KEY_USERNAME = "username"

    private fun prefs(context: Context): SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun saveSession(context: Context, token: String, username: String) {
        prefs(context).edit()
            .putString(KEY_TOKEN, token)
            .putString(KEY_USERNAME, username)
            .apply()
    }

    fun getToken(context: Context): String? =
        prefs(context).getString(KEY_TOKEN, null)

    fun getUsername(context: Context): String? =
        prefs(context).getString(KEY_USERNAME, null)

    fun isLoggedIn(context: Context): Boolean =
        getToken(context) != null

    fun clear(context: Context) {
        prefs(context).edit().clear().apply()
    }
}
