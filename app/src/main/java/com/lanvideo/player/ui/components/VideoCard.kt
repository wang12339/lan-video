package com.lanvideo.player.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.ui.theme.CreamYellow
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

private val CategoryEmoji = mapOf(
    "general" to "\uD83C\uDFAC",
    "pet" to "\uD83D\uDC3E",
    "funny" to "\uD83E\uDD23",
    "healing" to "\uD83C\uDF38",
    "hot" to "\uD83D\uDD25",
    "food" to "\uD83C\uDF5E",
    "travel" to "\u2708\uFE0F",
    "music" to "\uD83C\uDFB5"
)

private val CategoryColor = mapOf(
    "general" to Pair(SkyBlue, Color(0xFF4A90D9)),
    "pet" to Pair(MintGreen, Color(0xFF228B22)),
    "funny" to Pair(Lavender, Color(0xFF6B3FA0)),
    "healing" to Pair(SakuraPink, Color(0xFFC71585)),
    "hot" to Pair(CreamYellow, Color(0xFF8B6914)),
    "food" to Pair(CreamYellow, Color(0xFF8B6914)),
    "travel" to Pair(SkyBlue, Color(0xFF4A90D9)),
    "music" to Pair(Lavender, Color(0xFF6B3FA0))
)

private val ImageGradientBrush = Brush.linearGradient(
    colors = listOf(SakuraPink, SkyBlue, MintGreen)
)

@Composable
fun VideoCard(
    video: VideoItem,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val catColor = CategoryColor[video.category] ?: Pair(SkyBlue, TextPrimary)
    val catEmoji = CategoryEmoji[video.category] ?: "\uD83C\uDFAC"

    Row(
        modifier = modifier
            .fillMaxWidth()
            .shadow(6.dp, RoundedCornerShape(20.dp))
            .clip(RoundedCornerShape(20.dp))
            .background(Color.White.copy(alpha = 0.85f))
            .clickable(onClick = onClick)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .size(120.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(ImageGradientBrush),
            contentAlignment = Alignment.Center
        ) {
            Text(catEmoji, fontSize = 56.sp)
        }

        Spacer(modifier = Modifier.width(12.dp))

        Column(
            modifier = Modifier.weight(1f)
        ) {
            Text(
                text = video.title,
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold,
                color = TextPrimary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis
            )

            Spacer(modifier = Modifier.height(6.dp))

            Text(
                text = video.description.ifEmpty { "No description" },
                fontSize = 13.sp,
                color = TextSecondary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )

            Spacer(modifier = Modifier.height(8.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(8.dp))
                        .background(catColor.first)
                        .padding(horizontal = 8.dp, vertical = 2.dp)
                ) {
                    Text(
                        text = "$catEmoji ${video.category}",
                        fontSize = 11.sp,
                        color = catColor.second
                    )
                }
            }
        }
    }
}
