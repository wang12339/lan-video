package com.lanvideo.player.data.local

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query

@Dao
interface VideoDao {

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(videos: List<CachedVideoEntity>)

    @Query("SELECT * FROM cached_videos ORDER BY id DESC")
    suspend fun getAll(): List<CachedVideoEntity>

    @Query("SELECT * FROM cached_videos WHERE title LIKE '%' || :query || '%' OR description LIKE '%' || :query || '%' ORDER BY id DESC")
    suspend fun search(query: String): List<CachedVideoEntity>

    @Query("DELETE FROM cached_videos")
    suspend fun deleteAll()

    @Query("SELECT COUNT(*) FROM cached_videos")
    suspend fun getCount(): Int
}
