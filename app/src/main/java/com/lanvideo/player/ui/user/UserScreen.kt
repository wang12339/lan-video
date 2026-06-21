package com.lanvideo.player.ui.user

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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TextSecondary

@Composable
fun UserScreen(
    onHistoryClick: () -> Unit = {},
    onSettingsClick: () -> Unit = {},
    onAboutClick: () -> Unit = {},
    viewModel: UserViewModel = viewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(BackgroundPink, BackgroundBlue)
                )
            )
            .padding(horizontal = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Spacer(modifier = Modifier.height(48.dp))

        Box(
            modifier = Modifier.size(100.dp),
            contentAlignment = Alignment.Center
        ) {
            Box(
                modifier = Modifier
                    .size(100.dp)
                    .clip(CircleShape)
                    .background(
                        Brush.sweepGradient(
                            colors = listOf(SakuraPink, SkyBlue, SakuraPink)
                        )
                    )
            )
            Box(
                modifier = Modifier
                    .size(92.dp)
                    .clip(CircleShape)
                    .background(
                        Brush.verticalGradient(
                            colors = listOf(BackgroundPink, BackgroundBlue)
                        )
                    ),
                contentAlignment = Alignment.Center
            ) {
                Text("\uD83D\uDC38", fontSize = 48.sp)
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = uiState.username,
            fontSize = 22.sp,
            fontWeight = FontWeight.Bold,
            color = TextPrimary
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = uiState.signature,
            fontSize = 14.sp,
            color = TextSecondary
        )

        Spacer(modifier = Modifier.height(24.dp))

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .shadow(6.dp, RoundedCornerShape(20.dp))
                .clip(RoundedCornerShape(20.dp))
                .background(Color.White.copy(alpha = 0.85f))
                .padding(vertical = 20.dp),
            horizontalArrangement = Arrangement.SpaceEvenly
        ) {
            StatItem(count = uiState.watchCount, label = "观看")
            Box(
                modifier = Modifier
                    .width(1.dp)
                    .height(40.dp)
                    .background(TextSecondary.copy(alpha = 0.3f))
            )
            StatItem(count = uiState.favoriteCount, label = "收藏")
        }

        Spacer(modifier = Modifier.height(24.dp))

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .shadow(6.dp, RoundedCornerShape(20.dp))
                .clip(RoundedCornerShape(20.dp))
                .background(Color.White.copy(alpha = 0.85f))
        ) {
            MenuItem(icon = "\uD83D\uDC40", label = "观看历史", onClick = onHistoryClick)
            MenuItem(icon = "\u2699\uFE0F", label = "设置", onClick = onSettingsClick)
            MenuItem(icon = "\u2139\uFE0F", label = "关于", onClick = onAboutClick, showDivider = false)
        }
    }
}

@Composable
private fun StatItem(count: Int, label: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            text = "$count",
            fontSize = 20.sp,
            fontWeight = FontWeight.Bold,
            color = SakuraPink
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = label,
            fontSize = 12.sp,
            color = TextSecondary
        )
    }
}

@Composable
private fun MenuItem(
    icon: String,
    label: String,
    onClick: () -> Unit,
    showDivider: Boolean = true
) {
    Column {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(horizontal = 20.dp, vertical = 16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(icon, fontSize = 22.sp)
            Spacer(modifier = Modifier.width(12.dp))
            Text(
                text = label,
                fontSize = 15.sp,
                color = TextPrimary,
                modifier = Modifier.weight(1f)
            )
            Text("\u276F", fontSize = 14.sp, color = TextSecondary)
        }
        if (showDivider) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(0.5.dp)
                    .padding(horizontal = 20.dp)
                    .background(TextSecondary.copy(alpha = 0.15f))
            )
        }
    }
}
