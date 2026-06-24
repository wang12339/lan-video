package com.lanvideo.player.ui.upload

import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.data.repository.VideoRepository
import org.koin.compose.koinInject
import com.lanvideo.player.data.util.queryDisplayName
import com.lanvideo.player.data.util.queryFileSize
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.ErrorRed
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary
import com.lanvideo.player.ui.theme.UploadGray
import com.lanvideo.player.ui.theme.UploadGreen
import com.lanvideo.player.ui.theme.UploadMagenta
import com.lanvideo.player.ui.theme.UploadPink
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private data class UploadFile(
    val uri: Uri,
    val name: String,
    val size: Long,
    var status: String = "待上传",
    var progress: Float = 0f
)

@Composable
fun UploadScreen(
    onBackClick: () -> Unit = {},
    videoRepository: VideoRepository = koinInject()
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val files = remember { mutableStateListOf<UploadFile>() }
    var isUploading by remember { mutableStateOf(false) }

    val pickLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.GetMultipleContents()
    ) { uris ->
        if (uris.isNotEmpty()) {
            val cr = context.contentResolver
            uris.forEach { uri ->
                val name = queryDisplayName(cr, uri) ?: "未知文件"
                val size = queryFileSize(cr, uri)
                files.add(UploadFile(uri, name, size))
            }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(colors = listOf(BackgroundPink, BackgroundBlue))
            )
    ) {
        Spacer(modifier = Modifier.height(48.dp))

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "\u2190",
                fontSize = 24.sp,
                color = SakuraPink,
                modifier = Modifier.clickable(onClick = onBackClick)
            )
            Spacer(modifier = Modifier.width(12.dp))
            Text(
                text = "上传视频",
                fontSize = 20.sp,
                fontWeight = FontWeight.Bold,
                color = TextPrimary
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Button(
                onClick = { pickLauncher.launch("video/*") },
                modifier = Modifier.weight(1f),
                colors = ButtonDefaults.buttonColors(containerColor = SakuraPink),
                shape = RoundedCornerShape(12.dp),
                enabled = !isUploading
            ) {
                Text("选择视频", color = Color.White)
            }
            Button(
                onClick = {
                    if (files.isEmpty()) {
                        Toast.makeText(context, "请先选择视频", Toast.LENGTH_SHORT).show()
                        return@Button
                    }
                    isUploading = true
                    scope.launch {
                        val uris = files.map { it.uri }
                        val result = withContext(Dispatchers.IO) {
                            videoRepository.uploadVideosSequential(
                                context = context.applicationContext,
                                uris = uris,
                                onEachProgress = { _, _, _, _, _ -> },
                                onItemStatus = { index, status, progress, error ->
                                    if (index < files.size) {
                                        files[index] = files[index].copy(
                                            status = when (status) {
                                                "checking" -> "检查中"
                                                "uploading" -> "上传中 ${(progress * 100).toInt()}%"
                                                "success" -> "完成"
                                                "duplicate" -> "重复跳过"
                                                "failed" -> "失败: ${error ?: "未知"}"
                                                else -> status
                                            },
                                            progress = progress
                                        )
                                    }
                                }
                            )
                        }
                        isUploading = false
                        val msg = buildString {
                            if (result.successCount > 0) append("成功${result.successCount}个 ")
                            if (result.duplicateCount > 0) append("跳过${result.duplicateCount}个 ")
                            if (result.failCount > 0) append("失败${result.failCount}个")
                        }.ifEmpty { "上传完成" }
                        Toast.makeText(context, msg, Toast.LENGTH_LONG).show()
                        files.removeAll { it.status == "完成" || it.status == "重复跳过" }
                    }
                },
                modifier = Modifier.weight(1f),
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (isUploading) Color.Gray else SakuraPink
                ),
                shape = RoundedCornerShape(12.dp),
                enabled = !isUploading && files.isNotEmpty()
            ) {
                if (isUploading) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(18.dp),
                        color = Color.White,
                        strokeWidth = 2.dp
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                }
                Text("开始上传", color = Color.White)
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        if (files.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = "点击「选择视频」添加文件",
                    fontSize = 14.sp,
                    color = TextSecondary
                )
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                itemsIndexed(files) { index, file ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .shadow(4.dp, RoundedCornerShape(12.dp))
                            .clip(RoundedCornerShape(12.dp))
                            .background(Color.White.copy(alpha = 0.85f))
                            .padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = file.name,
                                fontSize = 14.sp,
                                fontWeight = FontWeight.Medium,
                                color = TextPrimary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                            Spacer(modifier = Modifier.height(4.dp))
                            Text(
                                text = formatSize(file.size),
                                fontSize = 12.sp,
                                color = TextSecondary
                            )
                            if (file.status != "待上传") {
                                Spacer(modifier = Modifier.height(4.dp))
                                LinearProgressIndicator(
                                    progress = { file.progress },
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(4.dp)
                                        .clip(RoundedCornerShape(2.dp)),
                                    color = when {
                                        file.status.startsWith("完成") -> MintGreen
                                        file.status.startsWith("失败") -> UploadPink
                                        else -> SakuraPink
                                    },
                                    trackColor = UploadGray
                                )
                                Spacer(modifier = Modifier.height(2.dp))
                                Text(
                                    text = file.status,
                                    fontSize = 11.sp,
                                    color = when {
                                        file.status.startsWith("完成") -> UploadGreen
                                        file.status.startsWith("失败") -> UploadMagenta
                                        else -> TextSecondary
                                    }
                                )
                            }
                        }
                        if (!isUploading && file.status == "待上传") {
                            Text(
                                text = "\u2715",
                                fontSize = 18.sp,
                                color = ErrorRed,
                                modifier = Modifier
                                    .padding(start = 8.dp)
                                    .clickable { files.removeAt(index) }
                            )
                        }
                    }
                }
            }
        }

        Spacer(modifier = Modifier.height(16.dp))
    }
}

private fun formatSize(bytes: Long): String {
    if (bytes < 1024L) return "$bytes B"
    val k = 1024.0
    val kb = bytes / k
    if (kb < 1024.0) return String.format("%.1f KB", kb)
    val mb = kb / 1024.0
    if (mb < 1024.0) return String.format("%.1f MB", mb)
    return String.format("%.2f GB", mb / 1024.0)
}
