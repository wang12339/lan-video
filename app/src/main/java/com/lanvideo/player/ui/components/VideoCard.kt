package com.lanvideo.player.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import com.lanvideo.player.data.network.NetworkModule
import com.lanvideo.player.ui.home.VideoItem
import com.lanvideo.player.ui.theme.CreamYellow
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TagHotText
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@Composable
fun VideoCard(
    video: VideoItem,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .shadow(6.dp, RoundedCornerShape(20.dp))
            .clip(RoundedCornerShape(20.dp))
            .background(Color.White.copy(alpha = 0.85f))
            .clickable(onClick = onClick)
    ) {
        Column {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(topStart = 20.dp, topEnd = 20.dp))
                    .background(
                        Brush.linearGradient(
                            colors = listOf(SakuraPink, Lavender)
                        )
                    ),
                contentAlignment = Alignment.Center
            ) {
                if (video.thumbnailUrl.isNotBlank()) {
                    val fullUrl = remember(video.thumbnailUrl) {
                        if (video.thumbnailUrl.startsWith("http")) {
                            video.thumbnailUrl
                        } else {
                            "${NetworkModule.getBaseUrl().trimEnd('/')}${video.thumbnailUrl}"
                        }
                    }
                    AsyncImage(
                        model = fullUrl,
                        contentDescription = video.title,
                        modifier = Modifier.fillMaxWidth(),
                        contentScale = ContentScale.FillWidth
                    )
                } else {
                    Text(video.icon, fontSize = 56.sp)
                }
            }

            Column(modifier = Modifier.padding(12.dp)) {
                Text(
                    text = video.title,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = TextPrimary,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = "${video.timestamp} · ${video.views} 观看",
                    fontSize = 11.sp,
                    color = TextSecondary
                )
                Spacer(modifier = Modifier.height(8.dp))
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(12.dp))
                        .background(CreamYellow)
                        .padding(horizontal = 8.dp, vertical = 4.dp)
                ) {
                    Text(video.category, fontSize = 10.sp, color = TagHotText)
                }
            }
        }
    }
}
