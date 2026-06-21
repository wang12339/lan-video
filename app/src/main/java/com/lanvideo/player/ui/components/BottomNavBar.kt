package com.lanvideo.player.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanvideo.player.ui.navigation.Screen
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary

@Composable
fun BottomNavBar(
    currentRoute: String?,
    onNavigate: (Screen) -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(64.dp)
                .shadow(8.dp, RoundedCornerShape(24.dp))
                .clip(RoundedCornerShape(24.dp))
                .background(Color(0xFFE8F4FD).copy(alpha = 0.9f))
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceAround
        ) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.clickable { /* TODO */ }
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .shadow(4.dp, CircleShape)
                        .clip(CircleShape)
                        .background(
                            Brush.linearGradient(
                                colors = listOf(SakuraPink, Lavender)
                            )
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Text("\uD83D\uDCAC", fontSize = 20.sp)
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text("消息", fontSize = 11.sp, color = TextPrimary)
            }

            Box(
                modifier = Modifier
                    .offset(y = (-12).dp)
                    .size(60.dp)
                    .shadow(8.dp, CircleShape)
                    .clip(CircleShape)
                    .background(
                        Brush.linearGradient(
                            colors = listOf(SakuraPink, SkyBlue)
                        )
                    ),
                contentAlignment = Alignment.Center
            ) {
                Box(
                    modifier = Modifier
                        .size(56.dp)
                        .clip(CircleShape)
                        .background(Color.White),
                    contentAlignment = Alignment.Center
                ) {
                    Text("\uD83D\uDC30", fontSize = 32.sp)
                }
            }

            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                modifier = Modifier.clickable { onNavigate(Screen.User) }
            ) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .shadow(4.dp, CircleShape)
                        .clip(CircleShape)
                        .background(
                            Brush.linearGradient(
                                colors = listOf(SakuraPink, MintGreen)
                            )
                        ),
                    contentAlignment = Alignment.Center
                ) {
                    Text("\uD83C\uDF5E", fontSize = 20.sp)
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text("我的", fontSize = 11.sp, color = TextPrimary)
            }
        }
    }
}
