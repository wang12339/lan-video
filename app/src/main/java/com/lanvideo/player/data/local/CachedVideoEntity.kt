package com.lanvideo.player.data.local

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "cached_videos")
data class CachedVideoEntity(
    @PrimaryKey val id: Long,
    val title: String,
    val description: String = "",
    val sourceType: String = "external",
    val coverUrl: String? = null,
    val thumbUrl: String? = null,
    val streamUrl: String,
    val category: String = "general",
    val duration: Long = 0L,
    val watchPosition: Long? = null
)
