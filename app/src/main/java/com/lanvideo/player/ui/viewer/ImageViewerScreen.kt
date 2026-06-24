package com.lanvideo.player.ui.viewer

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.pager.VerticalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import com.lanvideo.player.data.network.NetworkModule

@Composable
fun ImageViewerScreen(
    imageUrls: List<String>,
    startIndex: Int = 0,
    onBackClick: () -> Unit = {}
) {
    val urls = remember(imageUrls) { imageUrls }
    val initialPage = remember(startIndex, urls.size) {
        startIndex.coerceIn(0, (urls.size - 1).coerceAtLeast(0))
    }

    val pagerState = rememberPagerState(
        initialPage = initialPage,
        pageCount = { urls.size }
    )

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black),
        contentAlignment = Alignment.Center
    ) {
        VerticalPager(
            state = pagerState,
            modifier = Modifier.fillMaxSize()
        ) { page ->
            val url = urls[page]
            val fullUrl = remember(url) {
                if (url.startsWith("http")) url
                else "${NetworkModule.getBaseUrl().trimEnd('/')}$url"
            }

            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center
            ) {
                AsyncImage(
                    model = fullUrl,
                    contentDescription = null,
                    modifier = Modifier.fillMaxSize(),
                    contentScale = ContentScale.Fit
                )
            }
        }

        // Page indicator
        if (urls.size > 1) {
            Text(
                text = "${pagerState.currentPage + 1} / ${urls.size}",
                fontSize = 14.sp,
                color = Color.White,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 24.dp)
                    .background(Color.Black.copy(alpha = 0.5f), CircleShape)
                    .padding(horizontal = 12.dp, vertical = 6.dp)
            )
        }

        // Back button
        Box(
            modifier = Modifier
                .align(Alignment.TopStart)
                .padding(16.dp)
                .size(40.dp)
                .shadow(4.dp, CircleShape)
                .clip(CircleShape)
                .background(Color.Black.copy(alpha = 0.5f))
                .clickable { onBackClick() },
            contentAlignment = Alignment.Center
        ) {
            Text("←", fontSize = 20.sp, color = Color.White)
        }
    }
}
