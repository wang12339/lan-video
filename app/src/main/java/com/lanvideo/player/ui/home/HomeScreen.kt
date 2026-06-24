package com.lanvideo.player.ui.home

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
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.koin.androidx.compose.koinViewModel
import com.lanvideo.player.ui.components.VideoCard
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.gradientBackground

@Composable
fun HomeScreen(
    onVideoClick: (String) -> Unit = {},
    onImageClick: (imageUrls: List<String>, startIndex: Int) -> Unit = { _, _ -> },
    viewModel: HomeViewModel = koinViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .gradientBackground()
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp)
                .padding(horizontal = 16.dp)
                .shadow(4.dp, RoundedCornerShape(16.dp))
                .clip(RoundedCornerShape(16.dp))
                .background(Color.White.copy(alpha = 0.7f)),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = "❦ 爱的天堂 ❦",
                fontSize = 22.sp,
                fontWeight = FontWeight.ExtraBold,
                letterSpacing = 4.sp,
                color = SakuraPink
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            uiState.categories.forEach { category ->
                CategoryChip(
                    text = category,
                    isSelected = uiState.selectedCategory == category,
                    onClick = { viewModel.onCategorySelected(category) }
                )
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            items(uiState.videos, key = { it.id }) { video ->
                VideoCard(
                    video = video,
                    onClick = {
                        if (video.sourceType.contains("image")) {
                            val imageUrls = uiState.videos
                                .filter { it.sourceType.contains("image") }
                                .map { it.thumbnailUrl }
                            val idx = imageUrls.indexOf(video.thumbnailUrl).coerceAtLeast(0)
                            onImageClick(imageUrls, idx)
                        } else {
                            onVideoClick(video.id)
                        }
                    }
                )
            }
        }
    }
}

@Composable
private fun CategoryChip(
    text: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(20.dp))
            .background(if (isSelected) SakuraPink else Color.White.copy(alpha = 0.5f))
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        Text(
            text = text,
            fontSize = 12.sp,
            color = if (isSelected) Color.White else TextPrimary
        )
    }
}
