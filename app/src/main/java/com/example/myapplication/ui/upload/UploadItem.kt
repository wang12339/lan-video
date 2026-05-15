package com.example.myapplication.ui.upload

import android.net.Uri
import java.util.UUID

enum class UploadStatus {
    QUEUED,
    CHECKING,
    UPLOADING,
    SUCCESS,
    DUPLICATE,
    FAILED
}

data class UploadItem(
    val id: String = UUID.randomUUID().toString(),
    val uri: Uri,
    val fileName: String,
    val fileSize: Long,
    val status: UploadStatus = UploadStatus.QUEUED,
    val progress: Float = 0f,
    val selected: Boolean = true,
    val errorMessage: String? = null,
    val originalIndex: Int = -1
)
