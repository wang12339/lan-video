package com.lanvideo.player.data.user

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * Encrypted token/session storage.
 * Uses AES-256 GCM via Android Keystore — token is encrypted at rest
 * and only decryptable within this app.
 */
object AuthSessionStore {
    private const val PREFS = "auth_session_encrypted"
    private const val KEY_TOKEN = "token"
    private const val KEY_USERNAME = "username"

    private fun prefs(context: Context): SharedPreferences {
        val masterKey = MasterKey.Builder(context.applicationContext)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()

        return EncryptedSharedPreferences.create(
            context.applicationContext,
            PREFS,
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
    }

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
