package com.example.myapplication.data.network

import com.example.myapplication.data.model.AuthResponse
import com.example.myapplication.data.model.LoginRequest
import com.example.myapplication.data.model.PagedVideoResponse
import com.example.myapplication.data.model.PlaybackHistoryRequest
import com.example.myapplication.data.model.PlaybackHistoryResponse
import com.example.myapplication.data.model.FileCheckItem
import com.example.myapplication.data.model.RegisterRequest
import com.example.myapplication.data.model.UserInfoResponse
import com.example.myapplication.data.model.UserProfileResponse
import com.example.myapplication.data.model.VideoItem
import com.example.myapplication.data.model.VideoUpdateRequest
import com.example.myapplication.data.model.VideoUpdateResponse
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.HTTP
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path
import retrofit2.http.Query

interface VideoApiService {
    @GET("videos")
    suspend fun listVideos(
        @Query("query") query: String? = null,
        @Query("type") type: String? = null,
        @Query("page") page: Int? = null,
        @Query("size") size: Int? = null
    ): PagedVideoResponse

    @GET("videos/{id}")
    suspend fun getVideo(@Path("id") id: Long): VideoItem

    @GET("playback/history/{videoId}")
    suspend fun getPlaybackHistory(@Path("videoId") videoId: Long): PlaybackHistoryResponse

    @GET("playback/history")
    suspend fun listPlaybackHistory(): List<PlaybackHistoryResponse>

    @PUT("admin/videos/{id}")
    suspend fun updateVideo(@Path("id") id: Long, @Body request: VideoUpdateRequest): VideoUpdateResponse

    @DELETE("admin/videos/{id}")
    suspend fun deleteVideo(@Path("id") id: Long): VideoUpdateResponse

    @HTTP(method = "DELETE", path = "admin/videos/batch", hasBody = true)
    suspend fun deleteVideos(@Body ids: List<Long>): VideoUpdateResponse

    @POST("playback/history")
    suspend fun updatePlaybackHistory(@Body request: PlaybackHistoryRequest)

    @POST("admin/videos/check-hashes")
    suspend fun checkHashes(@Body request: Map<String, List<String>>): Map<String, Set<String>>

    @POST("admin/videos/check-files")
    suspend fun checkFiles(@Body files: List<FileCheckItem>): Map<String, Set<Int>>

    @POST("auth/register")
    suspend fun register(@Body request: RegisterRequest): AuthResponse

    @POST("auth/login")
    suspend fun login(@Body request: LoginRequest): AuthResponse

    @POST("auth/logout")
    suspend fun logout(): AuthResponse

    @GET("auth/user")
    suspend fun getUserInfo(): UserInfoResponse

    @GET("auth/user/profile")
    suspend fun getUserProfile(): UserProfileResponse
}
