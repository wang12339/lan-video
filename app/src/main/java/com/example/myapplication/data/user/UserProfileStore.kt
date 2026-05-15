package com.example.myapplication.data.user

import android.content.Context
import android.content.SharedPreferences
import java.security.SecureRandom

/**
 * 仅用于匿名用户标识（X-Username 头），登录用户使用 [com.example.myapplication.data.network.AuthSessionStore]。
 */
object UserProfileStore {
    private const val PREFS = "user_profile"
    private const val KEY_USER_NUMBER = "user_number"

    private fun prefs(context: Context): SharedPreferences {
        return context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    }

    fun getOrCreateUserId(context: Context): String {
        val p = prefs(context)
        val existing = p.getString(KEY_USER_NUMBER, null)
        if (existing != null) return existing
        val n = SecureRandom().nextInt(900_000) + 100_000
        val id = n.toString()
        p.edit().putString(KEY_USER_NUMBER, id).apply()
        return id
    }

    fun getHandle(context: Context): String {
        return "@${getOrCreateUserId(context)}"
    }
}
