package com.lanvideo.player.util

object VideoFormatters {

    fun formatDuration(durationMs: Long): String {
        val seconds = durationMs / 1000
        val minutes = seconds / 60
        val hours = minutes / 60
        return when {
            hours > 0 -> "${hours}小时前"
            minutes > 0 -> "${minutes}分钟前"
            else -> "刚刚"
        }
    }

    fun getCategoryIcon(category: String): String {
        return when (category.lowercase()) {
            "动画", "animation" -> "🐰"
            "萌宠", "pet" -> "🐶"
            "搞笑", "funny" -> "🐱"
            "治愈", "healing" -> "🦊"
            "音乐", "music" -> "🎵"
            "游戏", "game" -> "🎮"
            "科技" -> "🔬"
            "设计" -> "🎨"
            "教程" -> "📚"
            "娱乐" -> "🎬"
            "运动" -> "⚽"
            "记录" -> "📷"
            else -> "🎬"
        }
    }
}
