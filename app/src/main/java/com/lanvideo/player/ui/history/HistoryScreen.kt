package com.lanvideo.player.ui.history

import androidx.compose.foundation.background
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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

data class HistoryItem(
    val id: String,
    val title: String,
    val icon: String,
    val timestamp: String,
    val progress: Int
)

private val mockHistoryItems = listOf(
    HistoryItem("1", "猫咪的日常", "🐱", "2分钟前", 85),
    HistoryItem("2", "可爱小狗", "🐶", "1小时前", 42),
    HistoryItem("3", "樱花盛开", "🌸", "昨天", 100),
    HistoryItem("4", "小熊探险", "🧸", "2天前", 67),
    HistoryItem("5", "星星夜空", "⭐", "3天前", 23),
    HistoryItem("6", "彩虹糖果", "🌈", "上周", 55),
    HistoryItem("7", "甜蜜蛋糕", "🍰", "上周", 90),
    HistoryItem("8", "魔法森林", "🌲", "2周前", 38)
)

@Composable
fun HistoryScreen(
    onBackClick: () -> Unit = {}
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(48.dp)
                .padding(horizontal = 16.dp)
                .shadow(4.dp, RoundedCornerShape(16.dp))
                .clip(RoundedCornerShape(16.dp))
                .background(Color.White.copy(alpha = 0.7f))
                .padding(horizontal = 20.dp),
            contentAlignment = Alignment.CenterStart
        ) {
            Text(
                text = "观看历史",
                fontSize = 18.sp,
                fontWeight = FontWeight.Bold,
                color = SakuraPink
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            items(mockHistoryItems) { item ->
                HistoryCard(item = item)
            }
        }
    }
}

@Composable
private fun HistoryCard(
    item: HistoryItem
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .shadow(6.dp, RoundedCornerShape(20.dp))
            .clip(RoundedCornerShape(20.dp))
            .background(Color.White.copy(alpha = 0.85f))
            .padding(12.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(64.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(
                        Brush.linearGradient(
                            colors = listOf(SakuraPink, Lavender)
                        )
                    ),
                contentAlignment = Alignment.Center
            ) {
                Text(item.icon, fontSize = 32.sp)
            }

            Spacer(modifier = Modifier.width(12.dp))

            Column(
                modifier = Modifier.weight(1f)
            ) {
                Text(
                    text = item.title,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = TextPrimary
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = item.timestamp,
                    fontSize = 12.sp,
                    color = TextSecondary
                )
                Spacer(modifier = Modifier.height(8.dp))
                Row(
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(6.dp)
                            .clip(RoundedCornerShape(3.dp))
                            .background(TextSecondary.copy(alpha = 0.15f))
                    ) {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth(fraction = item.progress / 100f)
                                .height(6.dp)
                                .clip(RoundedCornerShape(3.dp))
                                .background(
                                    Brush.horizontalGradient(
                                        colors = listOf(SakuraPink, Lavender)
                                    )
                                )
                        )
                    }
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "${item.progress}%",
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = SakuraPink
                    )
                }
            }
        }
    }
}
