package com.lanvideo.player.ui.player

import android.app.Activity
import android.app.PictureInPictureParams
import android.content.Intent
import android.content.pm.ActivityInfo
import android.os.Build
import android.util.Rational
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import org.koin.androidx.compose.koinViewModel
import coil.compose.AsyncImage
import com.lanvideo.player.data.network.NetworkModule
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import com.lanvideo.player.data.network.AuthDataSourceFactory
import com.lanvideo.player.data.network.StreamUrlResolver
import com.lanvideo.player.ui.theme.BackgroundBlue
import com.lanvideo.player.ui.theme.BackgroundPink
import com.lanvideo.player.ui.theme.CreamYellow
import com.lanvideo.player.ui.theme.Lavender
import com.lanvideo.player.ui.theme.MintGreen
import com.lanvideo.player.ui.theme.SakuraPink
import com.lanvideo.player.ui.theme.SkyBlue
import com.lanvideo.player.ui.theme.TextPrimary
import com.lanvideo.player.ui.theme.TagHotText
import com.lanvideo.player.ui.theme.TextSecondary
import com.lanvideo.player.ui.theme.gradientBackground
import kotlin.math.abs

private fun formatTime(ms: Long): String {
    val totalSeconds = ms / 1000
    val minutes = totalSeconds / 60
    val seconds = totalSeconds % 60
    return "%d:%02d".format(minutes, seconds)
}

// Helper functions first
@Composable
private fun ActionButton(
    icon: String,
    label: String,
    color: Color,
    onClick: () -> Unit = {}
) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.clickable(onClick = onClick)
    ) {
        Box(
            modifier = Modifier
                .size(52.dp)
                .shadow(4.dp, RoundedCornerShape(20.dp))
                .clip(RoundedCornerShape(20.dp))
                .background(color.copy(alpha = 0.2f)),
            contentAlignment = Alignment.Center
        ) {
            Text(icon, fontSize = 24.sp)
        }
        Spacer(modifier = Modifier.height(6.dp))
        Text(label, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = TextSecondary)
    }
}

@Composable
private fun RelatedVideoCard(
    video: PlayerRelatedVideo,
    onClick: () -> Unit = {}
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .shadow(4.dp, RoundedCornerShape(14.dp))
            .clip(RoundedCornerShape(14.dp))
            .background(Color.White.copy(alpha = 0.85f))
            .clickable(onClick = onClick)
    ) {
        Column {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(16f / 9f)
                    .background(
                        Brush.linearGradient(
                            colors = listOf(CreamYellow, Lavender)
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
                        modifier = Modifier.fillMaxSize(),
                        contentScale = ContentScale.Crop
                    )
                } else {
                    Text(video.icon, fontSize = 28.sp)
                }
            }
            Column(modifier = Modifier.padding(8.dp)) {
                Text(
                    text = video.title,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = TextPrimary,
                    maxLines = 2
                )
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = video.timestamp,
                    fontSize = 10.sp,
                    color = TextSecondary
                )
            }
        }
    }
}

@Composable
private fun VideoPlayerWithControls(
    streamUrl: String,
    onPlaybackProgress: (position: Float, durationMs: Long) -> Unit,
    onPlaybackEnded: () -> Unit,
    isFullscreen: Boolean = false,
    onFullscreenToggle: () -> Unit = {},
    playbackSpeed: Float = 1f,
    onSpeedChange: (Float) -> Unit = {},
    onSeek: (Long) -> Unit = {}
) {
    val context = LocalContext.current
    var isPlaying by remember { mutableStateOf(false) }
    var showControls by remember { mutableStateOf(true) }
    var currentPosition by remember { mutableLongStateOf(0L) }
    var duration by remember { mutableLongStateOf(0L) }
    var isInPipMode by remember { mutableStateOf(false) }
    var isSeeking by remember { mutableStateOf(false) }
    var seekPosition by remember { mutableFloatStateOf(0f) }

    // Gesture state
    var gestureType by remember { mutableStateOf<String?>(null) }
    var gestureValue by remember { mutableFloatStateOf(0f) }
    var gestureStartX by remember { mutableFloatStateOf(0f) }
    var gestureStartY by remember { mutableFloatStateOf(0f) }

    val exoPlayer = remember {
        val dataSourceFactory = AuthDataSourceFactory.create(context)
        ExoPlayer.Builder(context)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSourceFactory))
            .build()
    }

    LaunchedEffect(streamUrl) {
        val absoluteUrl = StreamUrlResolver.toAbsoluteStreamUrl(streamUrl)
        if (absoluteUrl.isNotBlank()) {
            val mediaItem = MediaItem.fromUri(absoluteUrl)
            exoPlayer.setMediaItem(mediaItem)
            exoPlayer.prepare()
            exoPlayer.playWhenReady = true
        }
    }

    LaunchedEffect(playbackSpeed) {
        exoPlayer.setPlaybackSpeed(playbackSpeed)
    }

    DisposableEffect(exoPlayer) {
        val listener = object : Player.Listener {
            override fun onIsPlayingChanged(playing: Boolean) {
                isPlaying = playing
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                when (playbackState) {
                    Player.STATE_READY -> {
                        duration = exoPlayer.duration
                        currentPosition = exoPlayer.currentPosition
                        if (duration > 0) {
                            onPlaybackProgress(currentPosition.toFloat() / duration, duration)
                        }
                    }
                    Player.STATE_ENDED -> {
                        onPlaybackEnded()
                    }
                }
            }
        }
        exoPlayer.addListener(listener)
        onDispose {
            exoPlayer.removeListener(listener)
            exoPlayer.release()
        }
    }

    // Update position periodically
    LaunchedEffect(isPlaying) {
        while (isPlaying) {
            currentPosition = exoPlayer.currentPosition
            if (duration > 0) {
                onPlaybackProgress(currentPosition.toFloat() / duration, duration)
            }
            kotlinx.coroutines.delay(500)
        }
    }

    // Auto-hide controls after 3 seconds
    LaunchedEffect(showControls, isPlaying) {
        if (showControls && isPlaying) {
            kotlinx.coroutines.delay(3000)
            showControls = false
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .pointerInput(Unit) {
                detectDragGestures(
                    onDragStart = { offset ->
                        gestureStartX = offset.x
                        gestureStartY = offset.y
                        gestureType = null
                    },
                    onDrag = { change, dragAmount ->
                        val deltaX = change.position.x - gestureStartX
                        val deltaY = change.position.y - gestureStartY

                        if (gestureType == null) {
                            if (abs(deltaX) > 50 || abs(deltaY) > 50) {
                                gestureType = if (abs(deltaX) > abs(deltaY)) "seek" else {
                                    if (gestureStartX < size.width / 2) "brightness" else "volume"
                                }
                            }
                        }

                        when (gestureType) {
                            "seek" -> gestureValue = deltaX / size.width
                            "brightness" -> gestureValue = -deltaY / size.height
                            "volume" -> gestureValue = -deltaY / size.height
                        }
                        change.consume()
                    },
                    onDragEnd = {
                        when (gestureType) {
                            "seek" -> {
                                val seekDelta = (gestureValue * duration).toLong()
                                exoPlayer.seekTo(currentPosition + seekDelta)
                            }
                            else -> {}
                        }
                        gestureType = null
                        gestureValue = 0f
                    }
                )
            }
            .clickable { showControls = !showControls }
    ) {
        AndroidView(
            factory = { ctx ->
                PlayerView(ctx).apply {
                    player = exoPlayer
                    useController = false
                    layoutParams = FrameLayout.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.MATCH_PARENT
                    )
                }
            },
            modifier = Modifier.fillMaxSize()
        )

        // Gesture indicator overlay
        if (gestureType != null) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.5f)),
                contentAlignment = Alignment.Center
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        text = when (gestureType) {
                            "seek" -> {
                                val seekDelta = (gestureValue * duration / 1000).toInt()
                                if (seekDelta > 0) "+${seekDelta}s" else "${seekDelta}s"
                            }
                            "brightness" -> "亮度"
                            "volume" -> "音量"
                            else -> ""
                        },
                        fontSize = 24.sp,
                        color = Color.White,
                        fontWeight = FontWeight.Bold
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    LinearProgressIndicator(
                        progress = {
                            when (gestureType) {
                                "seek" -> (currentPosition + gestureValue * duration).coerceIn(0f, duration.toFloat()) / duration
                                "brightness", "volume" -> (0.5f + gestureValue).coerceIn(0f, 1f)
                                else -> 0f
                            }
                        },
                        modifier = Modifier
                            .width(120.dp)
                            .height(4.dp)
                            .clip(RoundedCornerShape(2.dp)),
                        color = SakuraPink,
                        trackColor = Color.White.copy(alpha = 0.3f),
                    )
                }
            }
        }

        // Controls overlay
        if (showControls && gestureType == null) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.3f))
                    .clickable {
                        if (isPlaying) exoPlayer.pause() else exoPlayer.play()
                        showControls = false
                    },
                contentAlignment = Alignment.Center
            ) {
                // Play/Pause button
                Box(
                    modifier = Modifier
                        .size(64.dp)
                        .shadow(8.dp, CircleShape)
                        .clip(CircleShape)
                        .background(SakuraPink.copy(alpha = 0.85f)),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = if (isPlaying) "\u23F8" else "\u25B6",
                        fontSize = 28.sp,
                        color = Color.White
                    )
                }

                // Speed selector button - click to cycle
                Box(
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(end = 16.dp, top = 16.dp)
                        .size(40.dp)
                        .shadow(4.dp, CircleShape)
                        .clip(CircleShape)
                        .background(SakuraPink.copy(alpha = 0.85f))
                        .clickable {
                            val speeds = listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f)
                            val nextIdx = (speeds.indexOf(playbackSpeed) + 1) % speeds.size
                            val nextSpeed = speeds[nextIdx]
                            onSpeedChange(nextSpeed)
                            exoPlayer.playbackParameters = PlaybackParameters(nextSpeed)
                        },
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = "${playbackSpeed}x",
                        fontSize = 12.sp,
                        color = Color.White,
                        fontWeight = FontWeight.Bold
                    )
                }

                // PiP button
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    Box(
                        modifier = Modifier
                            .align(Alignment.TopEnd)
                            .padding(end = 16.dp, top = 64.dp)
                            .size(40.dp)
                            .shadow(4.dp, CircleShape)
                            .clip(CircleShape)
                            .background(SakuraPink.copy(alpha = 0.85f))
                            .clickable {
                                val activity = context as? Activity
                                activity?.let {
                                    val params = PictureInPictureParams.Builder()
                                        .setAspectRatio(Rational(16, 9))
                                        .build()
                                    it.enterPictureInPictureMode(params)
                                    isInPipMode = true
                                    showControls = false
                                }
                            },
                        contentAlignment = Alignment.Center
                    ) {
                        Text(text = "\uD83D\uDCFA", fontSize = 18.sp, color = Color.White)
                    }
                }

                // Fullscreen button
                Box(
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(end = 16.dp, top = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) 112.dp else 64.dp)
                        .size(40.dp)
                        .shadow(4.dp, CircleShape)
                        .clip(CircleShape)
                        .background(SakuraPink.copy(alpha = 0.85f))
                        .clickable { onFullscreenToggle() },
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = if (isFullscreen) "\u2B1C" else "\u26F6",
                        fontSize = 18.sp,
                        color = Color.White
                    )
                }

                // Seekable progress bar at bottom
                Column(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .background(Color.Black.copy(alpha = 0.5f))
                        .padding(horizontal = 12.dp, vertical = 8.dp)
                        .pointerInput(Unit) {}
                ) {
                    val progress = if (duration > 0) currentPosition.toFloat() / duration else 0f
                    Box(modifier = Modifier.fillMaxWidth().height(20.dp)) {
                        LinearProgressIndicator(
                            progress = { if (isSeeking) seekPosition else progress },
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .align(Alignment.Center)
                                .clip(RoundedCornerShape(3.dp)),
                            color = Color.Transparent,
                            trackColor = Color.Transparent,
                            strokeCap = StrokeCap.Round,
                        )
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .align(Alignment.Center)
                                .clip(RoundedCornerShape(3.dp))
                                .background(
                                    Brush.horizontalGradient(
                                        colors = listOf(
                                            Color(0xFFFF0000),
                                            Color(0xFFFF8800),
                                            Color(0xFFFFFF00),
                                            Color(0xFF00CC00),
                                            Color(0xFF0088FF),
                                            Color(0xFF6600FF),
                                            Color(0xFFFF0066)
                                        )
                                    )
                                )
                        )
                        Slider(
                            value = if (isSeeking) seekPosition else progress,
                            onValueChange = { value ->
                                isSeeking = true
                                seekPosition = value
                            },
                            onValueChangeFinished = {
                                val seekTo = (seekPosition * duration).toLong()
                                exoPlayer.seekTo(seekTo)
                                isSeeking = false
                            },
                            modifier = Modifier.fillMaxWidth().align(Alignment.Center),
                            colors = SliderDefaults.colors(
                                thumbColor = Color.White,
                                activeTrackColor = Color.Transparent,
                                inactiveTrackColor = Color.Transparent
                            )
                        )
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(
                            text = formatTime(if (isSeeking) (seekPosition * duration).toLong() else currentPosition),
                            fontSize = 11.sp,
                            color = Color.White
                        )
                        Text(
                            text = formatTime(duration),
                            fontSize = 11.sp,
                            color = Color.White
                        )
                    }
                }
            }
        }
    }
}

// Main PlayerScreen function
@Composable
fun PlayerScreen(
    videoId: String = "",
    onBackClick: () -> Unit = {},
    onVideoClick: (String) -> Unit = {},
    viewModel: PlayerViewModel = koinViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val context = LocalContext.current

    LaunchedEffect(videoId) {
        viewModel.loadVideo(videoId)
    }

    // Handle fullscreen mode
    LaunchedEffect(uiState.isFullscreen) {
        val activity = context as? Activity ?: return@LaunchedEffect
        if (uiState.isFullscreen) {
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            WindowCompat.setDecorFitsSystemWindows(activity.window, false)
            WindowInsetsControllerCompat(activity.window, activity.window.decorView).apply {
                hide(WindowInsetsCompat.Type.systemBars())
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        } else {
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            WindowCompat.setDecorFitsSystemWindows(activity.window, true)
            WindowInsetsControllerCompat(activity.window, activity.window.decorView).show(WindowInsetsCompat.Type.systemBars())
        }
    }

    DisposableEffect(Unit) {
        onDispose {
            (context as? Activity)?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        }
    }

    val isFullscreen = uiState.isFullscreen
    val bgModifier = if (isFullscreen) {
        Modifier.fillMaxSize().background(Color.Black)
    } else {
        Modifier.fillMaxSize().gradientBackground()
    }

    Column(modifier = bgModifier) {
        Box(
            modifier = if (isFullscreen) Modifier.fillMaxSize()
            else Modifier.fillMaxWidth().height(240.dp)
                .background(Color.Black)
        ) {
            if (uiState.isLoading) {
                CircularProgressIndicator(color = SakuraPink, modifier = Modifier.align(Alignment.Center))
            } else if (uiState.streamUrl.isNotBlank()) {
                VideoPlayerWithControls(
                    streamUrl = uiState.streamUrl,
                    onPlaybackProgress = { pos, dur -> viewModel.onProgressChange(pos, dur) },
                    onPlaybackEnded = {
                        viewModel.onPlaybackEnded()
                        viewModel.nextVideoId()?.let { onVideoClick(it) }
                    },
                    isFullscreen = isFullscreen,
                    onFullscreenToggle = { viewModel.toggleFullscreen() },
                    playbackSpeed = uiState.playbackSpeed,
                    onSpeedChange = { viewModel.setPlaybackSpeed(it) }
                )
            } else {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("\uD83C\uDFAC", fontSize = 64.sp)
                }
            }
        }

        if (!isFullscreen) {
            Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
                Spacer(modifier = Modifier.height(12.dp))
                Text(text = uiState.title, fontSize = 18.sp, fontWeight = FontWeight.Bold, color = TextPrimary)
                Spacer(modifier = Modifier.height(6.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(CreamYellow).padding(horizontal = 8.dp, vertical = 4.dp)) {
                        Text(uiState.category, fontSize = 11.sp, color = TagHotText)
                    }
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(text = uiState.timestamp, fontSize = 12.sp, color = TextSecondary)
                }
                Spacer(modifier = Modifier.height(16.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    ActionButton(
                        if (uiState.isLiked) "\uD83D\uDE0D" else "\uD83E\uDD70",
                        if (uiState.isLiked) "已喜欢" else "喜欢",
                        SakuraPink,
                        onClick = { viewModel.toggleLike() }
                    )
                    ActionButton(
                        if (uiState.isFavorited) "\u2B50" else "\uD83D\uDC9C",
                        if (uiState.isFavorited) "已收藏" else "收藏",
                        SkyBlue,
                        onClick = { viewModel.toggleFavorite() }
                    )
                    ActionButton("\uD83D\uDCE3", "分享", MintGreen, onClick = {
                        val shareIntent = Intent(Intent.ACTION_SEND).apply {
                            type = "text/plain"
                            putExtra(Intent.EXTRA_TEXT, "${uiState.title} ${NetworkModule.getBaseUrl().trimEnd('/')}/webapp/player.html?id=${uiState.videoId}")
                        }
                        context.startActivity(Intent.createChooser(shareIntent, "分享视频"))
                    })
                }
                Spacer(modifier = Modifier.height(20.dp))
                Text(text = "相关视频", fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = TextPrimary)
                Spacer(modifier = Modifier.height(12.dp))
            }

            LazyVerticalGrid(
                columns = GridCells.Fixed(3),
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .padding(horizontal = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                items(uiState.relatedVideos, key = { it.id }) { video ->
                    RelatedVideoCard(video = video, onClick = { onVideoClick(video.id) })
                }
            }
        }
    }
}
