package com.example.myapplication.data.repository

import android.content.ContentResolver
import android.content.Context
import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.os.SystemClock
import android.provider.OpenableColumns
import com.example.myapplication.data.model.FileCheckItem
import android.util.LruCache
import com.example.myapplication.data.model.PagedVideoResponse
import com.example.myapplication.data.model.PlaybackHistoryRequest
import com.example.myapplication.data.model.PlaybackHistoryResponse
import com.example.myapplication.data.model.VideoItem
import com.example.myapplication.data.model.VideoUpdateRequest
import com.example.myapplication.data.network.LanServerDiscovery
import com.example.myapplication.data.network.NetworkModule
import java.io.ByteArrayOutputStream
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MultipartBody
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okio.BufferedSink

class VideoRepository {
    companion object {
        private const val MAX_CONCURRENT_UPLOADS = 2
        private const val CACHE_MAX_SIZE = 64
        private val videoCache = LruCache<String, PagedVideoResponse>(CACHE_MAX_SIZE)
        @Volatile
        private var instance: VideoRepository? = null

        fun getInstance(): VideoRepository {
            return instance ?: synchronized(this) {
                instance ?: VideoRepository().also { instance = it }
            }
        }
    }
    suspend fun listVideos(query: String? = null, type: String? = null, page: Int = 0, size: Int = 20): Result<PagedVideoResponse> {
        val cacheKey = "list:$query:$type:$page:$size"
        videoCache.get(cacheKey)?.let { return Result.success(it) }
        return runCatching {
            val resp = NetworkModule.createApi().listVideos(query, type, page, size)
            videoCache.put(cacheKey, resp)
            resp
        }
    }

    suspend fun getPlaybackPosition(videoId: Long): Long? {
        return runCatching {
            NetworkModule.createApi().getPlaybackHistory(videoId)
        }.getOrNull()?.positionMs?.takeIf { it > 0L }
    }

    suspend fun getAllPlaybackHistory(): List<PlaybackHistoryResponse> {
        return runCatching {
            NetworkModule.createApi().listPlaybackHistory()
        }.getOrElse { emptyList() }
    }

    private fun is403(e: Throwable): Boolean =
        e is retrofit2.HttpException && e.code() == 403

    private fun mapAdminError(e: Throwable): IOException {
        val msg = if (is403(e)) "管理员权限不足，请在「设置」中配置与服务器一致的 Admin Token"
            else e.message.orEmpty()
        return IOException(msg, e)
    }

    private suspend fun adminOp(block: suspend () -> Result<Boolean>): Result<Boolean> {
        val result = block()
        if (result.isFailure && is403(result.exceptionOrNull()!!)) {
            return Result.failure(IOException("管理员权限不足，请在「设置」中配置与服务器一致的 Admin Token（位于服务器 backend/data/.admin_token 文件中）"))
        }
        return result.fold(
            onSuccess = { Result.success(it) },
            onFailure = { Result.failure(mapAdminError(it)) }
        )
    }

    suspend fun updateVideo(id: Long, title: String?, description: String?, category: String?): Result<Boolean> {
        val api = NetworkModule.createApi()
        return adminOp {
            runCatching { api.updateVideo(id, VideoUpdateRequest(title, description, category)).ok }
        }
    }

    suspend fun deleteVideo(id: Long): Result<Boolean> {
        val api = NetworkModule.createApi()
        return adminOp { runCatching { api.deleteVideo(id).ok } }
    }

    suspend fun deleteVideos(ids: List<Long>): Result<Boolean> {
        val api = NetworkModule.createApi()
        return adminOp { runCatching { api.deleteVideos(ids).ok } }
    }

    suspend fun uploadCover(videoId: Long, imageBytes: ByteArray, mimeType: String = "image/jpeg"): Result<Unit> =
        withContext(Dispatchers.IO) {
            runCatching {
                val base = NetworkModule.getBaseUrl().trimEnd('/')
                val body = okhttp3.MultipartBody.Builder()
                    .setType(okhttp3.MultipartBody.FORM)
                    .addFormDataPart("file", "cover.jpg",
                        imageBytes.toRequestBody(mimeType.toMediaType()))
                    .build()
                val req = Request.Builder()
                    .url("$base/admin/videos/$videoId/cover")
                    .post(body)
                    .build()
                NetworkModule.httpClient().newCall(req).execute().use { resp ->
                    if (!resp.isSuccessful) error("HTTP ${resp.code}")
                }
            }
        }

    suspend fun updatePlaybackHistory(videoId: Long, positionMs: Long, durationMs: Long): Result<Unit> {
        return runCatching {
            NetworkModule.createApi().updatePlaybackHistory(
                PlaybackHistoryRequest(videoId, positionMs, durationMs)
            )
        }
    }

    /**
     * 按顺序上传多个视频；[onEachProgress] 在 IO 线程回调，主线程请用 [android.app.Activity.runOnUiThread] 等处理。
     * @param onEachProgress (当前第几个从 1 开始, 总数, 本文件已传字节, 本文件总字节 -1 表示未知, 本文件名)
     */
    private suspend fun extractVideoFrame(context: Context, uri: Uri): ByteArray? = withContext(Dispatchers.IO) {
        try {
            val retriever = MediaMetadataRetriever()
            retriever.setDataSource(context, uri)
            val bitmap: Bitmap? = retriever.getFrameAtTime(
                1_000_000L,
                MediaMetadataRetriever.OPTION_CLOSEST_SYNC
            )
            if (bitmap == null) {
                retriever.release()
                return@withContext null
            }
            val scaled = Bitmap.createScaledBitmap(bitmap, 320, 180, true)
            retriever.release()
            if (scaled !== bitmap) bitmap.recycle()
            val out = ByteArrayOutputStream()
            scaled.compress(Bitmap.CompressFormat.JPEG, 80, out)
            scaled.recycle()
            out.toByteArray()
        } catch (_: Exception) {
            null
        }
    }

    suspend fun uploadVideosSequential(
        context: Context,
        uris: List<Uri>,
        category: String = "local",
        onEachProgress: (fileIndex1: Int, total: Int, bytesRead: Long, contentLength: Long, fileName: String) -> Unit,
        onItemStatus: ((index: Int, status: String, progress: Float, error: String?) -> Unit)? = null
    ): UploadBatchResult = withContext(Dispatchers.IO) {
        if (uris.isEmpty()) {
            return@withContext UploadBatchResult(0, 0, emptyList())
        }
        val app = context.applicationContext
        val cr = app.contentResolver

        // Step 1: 预检查 — 按 (原始文件名, 文件大小) 去重，无需读文件内容
        onEachProgress(-1, uris.size, 0L, -1L, "正在检查文件...")
        val fileMetas = uris.map { uri ->
            val displayName = queryDisplayName(cr, uri) ?: "unknown"
            val fileSize = openableSizeOrMinusOne(cr, uri)
            FileCheckItem(displayName, fileSize)
        }
        val skipIndices = mutableSetOf<Int>()
        if (fileMetas.isNotEmpty() && fileMetas.any { it.size > 0 }) {
            // 分批检查，每批最大 500，避免请求体过大
            val BATCH_SIZE = 500
            val existingIndices = mutableSetOf<Int>()
            fileMetas.chunked(BATCH_SIZE).forEachIndexed { batchIdx, batch ->
                val offset = batchIdx * BATCH_SIZE
                val batchResult = runCatching {
                    NetworkModule.createApi().checkFiles(batch)
                        .getOrElse("existing_indices") { emptySet<Int>() }
                }.getOrNull() ?: emptySet()
                for (i in batchResult) {
                    existingIndices.add(offset + i)
                }
            }
            for (i in existingIndices) {
                skipIndices.add(i)
                onItemStatus?.invoke(i, "duplicate", 1f, null)
            }
        }
        // 注意：对于 fileSize = -1（无法获取大小的文件），跳过预检查，直接上传让服务端判重

        // Step 2: 并行上传（最大 MAX_CONCURRENT_UPLOADS 个并发）
        val doneNames = mutableListOf<String>()
        var fail = 0
        var duplicate = skipIndices.size
        var firstError: String? = null

        val uploadSemaphore = Semaphore(MAX_CONCURRENT_UPLOADS)
        val uploadDeferreds: List<kotlinx.coroutines.Deferred<UploadFileResult>> = uris.withIndex()
            .filter { (i, _) -> i !in skipIndices }
            .map { (i, uri) ->
                val name = safeUploadFileName(
                    queryDisplayName(cr, uri) ?: "upload-${System.currentTimeMillis()}.mp4"
                )
                async(Dispatchers.IO) {
                    uploadSemaphore.acquire()
                    try {
                        val r = uploadVideo(
                            context = app,
                            uri = uri,
                            category = category,
                            fileHash = null,
                            onProgress = { read, totalBytes ->
                                onEachProgress(i + 1, uris.size, read, totalBytes, name)
                                val progress = if (totalBytes > 0) read.toFloat() / totalBytes else 0f
                                onItemStatus?.invoke(i, "uploading", progress, null)
                            }
                        )
                        val videoId = r.getOrNull()
                        val errorMsg = r.exceptionOrNull()?.message
                        UploadFileResult(i, name, videoId, errorMsg)
                    } finally {
                        uploadSemaphore.release()
                    }
                }
            }
        for (deferred in uploadDeferreds) {
            val result = deferred.await()
            val i = result.index
            val name = result.fileName
            if (result.videoId != null) {
                onItemStatus?.invoke(i, "success", 1f, null)
                doneNames.add(name)
                val videoId = result.videoId
                val mime = cr.getType(uris[i]) ?: ""
                if (mime.startsWith("video/")) {
                    // 利用其他文件仍在并行上传的间隙执行封面上传
                    try {
                        val frame = extractVideoFrame(app, uris[i])
                        if (frame != null) {
                            uploadCover(videoId, frame)
                        }
                    } catch (_: Exception) { }
                }
            } else {
                val errMsg = result.errorMessage ?: "未知错误"
                if (errMsg.contains("无需重复上传")) {
                    onItemStatus?.invoke(i, "duplicate", 1f, null)
                    duplicate++
                } else {
                    onItemStatus?.invoke(i, "failed", 0f, errMsg)
                    if (firstError == null) {
                        firstError = errMsg
                    }
                    fail++
                }
            }
            }

        UploadBatchResult(
            successCount = doneNames.size,
            failCount = fail,
            successFileNames = doneNames,
            duplicateCount = duplicate,
            firstErrorMessage = firstError
        )
    }

    data class UploadBatchResult(
        val successCount: Int,
        val failCount: Int,
        val successFileNames: List<String>,
        val duplicateCount: Int = 0,
        val firstErrorMessage: String? = null
    )

    /** 为上传总进度条预取各文件大小（-1 表示未知） */
    suspend fun openableSizesForUris(context: Context, uris: List<Uri>): List<Long> = withContext(Dispatchers.IO) {
        val cr = context.contentResolver
        uris.map { openableSizeOrMinusOne(cr, it) }
    }

    private data class UploadFileResult(
        val index: Int,
        val fileName: String,
        val videoId: Long?,
        val errorMessage: String? = null
    )
    suspend fun uploadVideo(
        context: Context,
        uri: Uri,
        category: String = "local",
        fileHash: String? = null,
        onProgress: ((bytesRead: Long, contentLength: Long) -> Unit)? = null
    ): Result<Long> = withContext(Dispatchers.IO) {
        val app = context.applicationContext
        fun baseUrl() = NetworkModule.getBaseUrl().trimEnd('/')

        suspend fun rediscover() {
            LanServerDiscovery.discoverActiveNetwork(app, force = true)
            delay(600)
        }

        fun postOnce(): Result<Long> {
            val base = baseUrl()
            return runCatching {
                val cr = context.contentResolver
                val fileName = safeUploadFileName(
                    queryDisplayName(cr, uri) ?: "upload-${System.currentTimeMillis()}.mp4"
                )
                val mime = cr.getType(uri) ?: "application/octet-stream"
                val url = "$base/admin/videos/upload"
                val mediaType = runCatching { mime.toMediaType() }
                    .getOrElse { "application/octet-stream".toMediaType() }
                val contentLength = openableSizeOrMinusOne(cr, uri)
                var lastNotifyMs = 0L
                val filePart = object : RequestBody() {
                    override fun contentType() = mediaType
                    override fun contentLength() = if (contentLength > 0L) contentLength else -1L
                    override fun writeTo(sink: BufferedSink) {
                        val input = cr.openInputStream(uri) ?: error("无法读取所选文件（可重试用相册/文件选择器）")
                        onProgress?.invoke(0L, contentLength)
                        input.use { ins ->
                            val buf = ByteArray(256 * 1024)
                            var n: Int
                            var total = 0L
                            while (ins.read(buf).also { n = it } != -1) {
                                sink.write(buf, 0, n)
                                total += n
                                onProgress?.let { o ->
                                    val now = SystemClock.elapsedRealtime()
                                    if (now - lastNotifyMs >= 100L || (contentLength > 0L && total >= contentLength)) {
                                        lastNotifyMs = now
                                        o(total, contentLength)
                                    }
                                }
                            }
                            onProgress?.invoke(total, contentLength)
                        }
                    }
                }
                val body = MultipartBody.Builder()
                    .setType(MultipartBody.FORM)
                    .addFormDataPart("category", category)
                    .apply {
                        if (!fileHash.isNullOrBlank()) {
                            addFormDataPart("fileHash", fileHash)
                        }
                    }
                    .addFormDataPart("file", fileName, filePart)
                    .build()
                val request = Request.Builder()
                    .url(url)
                    .post(body)
                    .build()
                NetworkModule.httpClient().newCall(request).execute().use { response ->
                    if (!response.isSuccessful) {
                        val errBody = response.body?.string().orEmpty()
                            .replace("\n", " ")
                            .take(400)
                        val detail = if (errBody.isNotBlank()) errBody else "无响应体"
                        error("HTTP ${response.code}: $detail")
                    }
                    val respBody = response.body?.string().orEmpty()
                    // parse {"id": 123}
                    val idMatch = Regex("\"id\"\\s*:\\s*(\\d+)").find(respBody)
                    idMatch?.groupValues?.get(1)?.toLong()
                        ?: error("无法获取视频 ID")
                }
            }.fold(
                onSuccess = { Result.success(it) },
                onFailure = { e ->
                    val errMsg = e.message.orEmpty()
                    val mapped = if (errMsg.contains("文件已存在")) {
                        IOException("该文件已上传过，无需重复上传")
                    } else if (errMsg.contains("HTTP 403")) {
                        IOException("管理员权限不足，请检查访问密码")
                    } else {
                        mapUploadNetworkError(e, base)
                    }
                    Result.failure(mapped)
                }
            )
        }

        var result = postOnce()
        if (result.isFailure) {
            val errMsg = result.exceptionOrNull()?.message.orEmpty()
            // 服务器明确拒绝的错误（重复、权限不足）不重试
            val isServerRejection = errMsg.contains("无需重复上传") || errMsg.contains("权限不足")
            if (!isServerRejection) {
                for (attempt in 0 until 3) {
                    delay(800L * (attempt + 1))
                    rediscover()
                    result = postOnce()
                    if (result.isSuccess) break
                }
            }
        }
        result
    }

    private fun openableSizeOrMinusOne(cr: ContentResolver, uri: Uri): Long {
        if (uri.scheme == ContentResolver.SCHEME_CONTENT) {
            cr.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val i = c.getColumnIndex(OpenableColumns.SIZE)
                    if (i >= 0 && !c.isNull(i)) {
                        return c.getLong(i).coerceAtLeast(0L)
                    }
                }
            }
        }
        return -1L
    }

    private fun mapUploadNetworkError(e: Throwable, base: String): Throwable {
        val msg = e.message.orEmpty()
        return if (e is IOException && (
                e is java.net.ConnectException ||
                    e is java.net.SocketTimeoutException ||
                    e is java.net.UnknownHostException ||
                    msg.contains("Failed to connect", ignoreCase = true)
                )
        ) {
            IOException(
                "无法连接 $base。请与电脑同一 Wi-Fi，电脑已运行视频服务；" +
                    "若刚进入应用，请稍等自动发现或首页能播视频后再上传。详情：$msg",
                e
            )
        } else {
            e
        }
    }

    private fun safeUploadFileName(raw: String): String {
        val name = raw.substringAfterLast('/').substringAfterLast('\\')
        if (name.isBlank()) return "upload-${System.currentTimeMillis()}.mp4"
        return name
            .replace(Regex("""[\\/:*?"<>|]"""), "_")
            .take(120)
            .ifBlank { "upload.mp4" }
    }

    private fun queryDisplayName(cr: ContentResolver, uri: Uri): String? {
        if (uri.scheme == ContentResolver.SCHEME_CONTENT) {
            cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val i = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (i >= 0) return c.getString(i)
                }
            }
        }
        return uri.lastPathSegment
    }
}
