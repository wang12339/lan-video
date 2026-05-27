package com.lanvideo.player.feature.search

import android.content.Context
import android.content.SharedPreferences

/**
 * Manages search history using SharedPreferences.
 * Max 20 items, most recent first. Deduplicates on add.
 */
class SearchHistory private constructor(context: Context) {
    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    fun getHistory(): List<String> {
        val raw = prefs.getString(KEY_HISTORY, null) ?: return emptyList()
        return if (raw.isEmpty()) emptyList() else raw.split(DELIMITER)
    }

    fun addSearch(query: String) {
        if (query.isBlank()) return
        val history = getHistory().toMutableList()
        // Deduplicate: remove if already exists
        history.remove(query)
        // Add to front (most recent)
        history.add(0, query)
        // Trim to max
        val trimmed = history.take(MAX_HISTORY)
        saveHistory(trimmed)
    }

    fun removeSearch(query: String) {
        val history = getHistory().toMutableList()
        if (history.remove(query)) {
            saveHistory(history)
        }
    }

    fun clearHistory() {
        prefs.edit().remove(KEY_HISTORY).apply()
    }

    private fun saveHistory(history: List<String>) {
        prefs.edit().putString(KEY_HISTORY, history.joinToString(DELIMITER)).apply()
    }

    companion object {
        private const val PREFS_NAME = "search_history"
        private const val KEY_HISTORY = "history"
        private const val MAX_HISTORY = 20
        private const val DELIMITER = "\n"

        @Volatile
        private var instance: SearchHistory? = null

        fun getInstance(context: Context): SearchHistory {
            return instance ?: synchronized(this) {
                instance ?: SearchHistory(context.applicationContext).also { instance = it }
            }
        }
    }
}
