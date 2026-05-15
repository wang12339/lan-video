package com.example.myapplication.data.model

import kotlinx.serialization.Serializable

@Serializable
data class LoginRequest(
    val username: String,
    val password: String
)

@Serializable
data class RegisterRequest(
    val username: String,
    val password: String
)

@Serializable
data class AuthResponse(
    val ok: Boolean,
    val token: String? = null,
    val error: String? = null
)

@Serializable
data class UserInfoResponse(
    val username: String,
    val isAdmin: Boolean,
    val createdAt: String? = null
)

@Serializable
data class UserProfileResponse(
    val username: String,
    val isAdmin: Boolean,
    val createdAt: String,
    val totalVideosWatched: Int,
    val totalWatchTimeMs: Long,
    val recentHistory: List<RecentWatchItem>
)

@Serializable
data class RecentWatchItem(
    val videoId: Long,
    val title: String,
    val coverUrl: String? = null,
    val streamUrl: String = "",
    val sourceType: String,
    val category: String = "",
    val positionMs: Long,
    val durationMs: Long,
    val updatedAt: String
)
