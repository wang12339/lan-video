package com.lanvideo.player.feature.player

import android.app.PictureInPictureParams
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.util.Rational
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.SeekBar
import android.widget.TextView
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import com.lanvideo.player.R
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.network.AuthDataSourceFactory
import com.lanvideo.player.data.network.StreamUrlResolver
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.databinding.ItemPlayerPageBinding
import java.util.Locale
import kotlin.math.abs
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class PlayerPageFragment : Fragment() {
    private var _binding: ItemPlayerPageBinding? = null
    private val binding get() = _binding!!
    private var player: ExoPlayer? = null
    private var userPaused = false
    private var seekJob: Job? = null
    private var saveJob: Job? = null
    private var lastSavedPositionMs: Long = 0L
    private var isSeekDragging = false
    private var hudVisible = true
    private var isInPip = false
    private var currentSpeed = 1f
    private val speeds = listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f)
    private val speedIds = listOf(
        R.id.speed_05x, R.id.speed_075x, R.id.speed_1x,
        R.id.speed_125x, R.id.speed_15x, R.id.speed_2x
    )

    // Gesture state
    private var gestureMode = GESTURE_NONE
    private var downX = 0f
    private var downY = 0f
    private var downTime = 0L
    private var lastGestureX = 0f
    private var lastGestureY = 0f
    private var audioManager: AudioManager? = null
    private var maxVolume = 0
    private var startBrightness = -1f
    private var startVolume = -1
    private var lastTapTime = 0L
    private var lastTapX = 0f

    private val item: VideoItem get() {
        val a = requireArguments()
        return VideoItem(
            id = a.getLong(ARG_ID),
            title = a.getString(ARG_TITLE).orEmpty(),
            description = a.getString(ARG_DESC).orEmpty(),
            sourceType = a.getString(ARG_SOURCE) ?: "local_video",
            coverUrl = a.getString(ARG_COVER),
            streamUrl = a.getString(ARG_STREAM).orEmpty(),
            category = a.getString(ARG_CATEGORY) ?: "general",
            watchPosition = a.getLong(ARG_WATCH_POS, -1L).let { if (it < 0) null else it }
        )
    }

    override fun onCreateView(
        inflater: LayoutInflater, container: ViewGroup?, savedInstanceState: Bundle?
    ): View {
        _binding = ItemPlayerPageBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        audioManager = requireContext().getSystemService(Context.AUDIO_SERVICE) as AudioManager
        maxVolume = audioManager?.getStreamMaxVolume(AudioManager.STREAM_MUSIC) ?: 15
        initializePlayer()
        setupSpeedSelector()
        startSeekUpdater()
    }

    // ═══════════════════════════════════════════════
    // Player Initialization
    // ═══════════════════════════════════════════════

    private fun initializePlayer() {
        val v = item
        val dataSourceFactory = AuthDataSourceFactory.create(requireContext())
        val p = ExoPlayer.Builder(requireContext())
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSourceFactory))
            .build()
        val abs = StreamUrlResolver.toAbsoluteStreamUrl(v.streamUrl)
        p.setMediaItem(MediaItem.fromUri(abs))
        p.prepare()
        p.playWhenReady = true
        player = p

        binding.playerView.player = p
        attachTouchHandler(binding.playerView, p)

        binding.btnSkip.setOnClickListener {
            (parentFragment as? PlayerFragment)?.skipToNext()
        }

        // Resume playback from saved position
        val resumeMs = v.watchPosition ?: 0L
        if (resumeMs > 2000L) p.seekTo(resumeMs)

        p.addListener(object : Player.Listener {
            override fun onPlayerError(e: androidx.media3.common.PlaybackException) {
                binding.errorOverlay.isVisible = true
                binding.textError.text = "播放失败: ${e.errorCodeName}"
            }
            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED) {
                    savePlaybackPosition()
                    (parentFragment as? PlayerFragment)?.skipToNext()
                }
            }
        })
    }

    // ═══════════════════════════════════════════════
    // Gesture Controls
    // ═══════════════════════════════════════════════

    private fun attachTouchHandler(view: PlayerView, p: ExoPlayer) {
        val density = view.context.resources.displayMetrics.density
        val seekThresholdPx = SEEK_THRESHOLD_DP * density

        view.setOnTouchListener { v, event ->
            if (isInPip) return@setOnTouchListener false

            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downX = event.x; downY = event.y
                    lastGestureX = event.x; lastGestureY = event.y
                    downTime = System.currentTimeMillis()
                    gestureMode = GESTURE_NONE
                    startBrightness = -1f; startVolume = -1
                    true
                }

                MotionEvent.ACTION_MOVE -> {
                    val dx = event.x - downX
                    val dy = event.y - downY
                    val vw = v.width.toFloat()
                    val vh = v.height.toFloat()

                    if (gestureMode == GESTURE_NONE) {
                        gestureMode = when {
                            abs(dy) > seekThresholdPx && abs(dy) > abs(dx) * 1.5f ->
                                if (event.x < vw / 2f) GESTURE_BRIGHTNESS else GESTURE_VOLUME
                            abs(dx) > seekThresholdPx -> GESTURE_SEEK
                            else -> GESTURE_NONE
                        }
                        if (gestureMode == GESTURE_BRIGHTNESS || gestureMode == GESTURE_VOLUME) {
                            initBrightnessVolume(event)
                        }
                        if (gestureMode == GESTURE_SEEK) {
                            binding.skipIndicator.isVisible = true
                        }
                    }

                    when (gestureMode) {
                        GESTURE_BRIGHTNESS -> handleBrightnessGesture(event, vh)
                        GESTURE_VOLUME -> handleVolumeGesture(event, vh)
                        GESTURE_SEEK -> handleSeekGesture(p, event, vw)
                    }
                    true
                }

                MotionEvent.ACTION_UP -> {
                    val dt = System.currentTimeMillis() - downTime
                    val isTap = abs(event.x - downX) < seekThresholdPx &&
                            abs(event.y - downY) < seekThresholdPx && dt < 400L

                    if (gestureMode == GESTURE_SEEK) {
                        binding.skipIndicator.isVisible = false
                    }

                    if (isTap) {
                        val sameSide = (event.x < v.width / 2f) == (lastTapX < v.width / 2f)
                        val isDouble = dt < DOUBLE_TAP_MS && sameSide

                        if (isDouble) {
                            val isLeft = event.x < v.width / 2f
                            val skipMs = (if (isLeft) -SKIP_SEC else SKIP_SEC) * 1000L
                            val newPos = (p.currentPosition + skipMs).coerceIn(0L, p.duration)
                            p.seekTo(newPos)
                            showSkipIndicator("${if (isLeft) "⏪" else "⏩"} ${if (isLeft) "-$SKIP_SEC" else "+$SKIP_SEC"}")
                            lastTapTime = 0L
                        } else {
                            if (System.currentTimeMillis() - lastTapTime > DOUBLE_TAP_MS) {
                                toggleHud()
                            }
                            lastTapTime = System.currentTimeMillis()
                            lastTapX = event.x
                        }
                    }

                    gestureMode = GESTURE_NONE
                    hideAllIndicators()
                    binding.speedPanel.isVisible = false
                    true
                }

                MotionEvent.ACTION_CANCEL -> {
                    gestureMode = GESTURE_NONE
                    hideAllIndicators()
                    true
                }

                else -> true
            }
        }
    }

    private fun initBrightnessVolume(event: MotionEvent) {
        if (gestureMode == GESTURE_BRIGHTNESS) {
            startBrightness = try {
                Settings.System.getInt(requireContext().contentResolver, Settings.System.SCREEN_BRIGHTNESS) / 255f
            } catch (_: Exception) { 0.5f }
        } else {
            startVolume = audioManager?.getStreamVolume(AudioManager.STREAM_MUSIC) ?: 0
        }
    }

    private fun handleBrightnessGesture(event: MotionEvent, vh: Float) {
        val delta = (lastGestureY - event.y) / vh
        val nb = (startBrightness + delta).coerceIn(0.01f, 1.0f)
        try {
            Settings.System.putInt(requireContext().contentResolver, Settings.System.SCREEN_BRIGHTNESS, (nb * 255f).toInt())
        } catch (_: Exception) {
            requireActivity().window.attributes = requireActivity().window.attributes.apply { screenBrightness = nb }
        }
        showGestureIndicator("☀", "${(nb * 100).toInt()}%")
        lastGestureY = event.y
    }

    private fun handleVolumeGesture(event: MotionEvent, vh: Float) {
        val delta = (lastGestureY - event.y) / vh
        val nv = (startVolume + (delta * maxVolume).toInt()).coerceIn(0, maxVolume)
        audioManager?.setStreamVolume(AudioManager.STREAM_MUSIC, nv, 0)
        showGestureIndicator("♪", "${(nv * 100 / maxVolume).toInt()}%")
        lastGestureY = event.y
    }

    private fun handleSeekGesture(p: ExoPlayer, event: MotionEvent, vw: Float) {
        val dx = event.x - lastGestureX
        val delta = (p.duration * (dx / vw)).toLong()
        val np = (p.currentPosition + delta).coerceIn(0L, p.duration)
        p.seekTo(np)
        showSkipIndicator("${if (delta > 0) "⏩" else "⏪"} ${formatMs(np)}")
        lastGestureX = event.x
    }

    private fun showGestureIndicator(icon: String, text: String) {
        binding.gestureIcon.text = icon
        binding.gestureValue.text = text
        binding.gestureIndicator.isVisible = true
    }

    private fun showSkipIndicator(text: String) {
        binding.skipIndicator.text = text
        binding.skipIndicator.isVisible = true
    }

    private fun hideAllIndicators() {
        binding.gestureIndicator.isVisible = false
        binding.skipIndicator.isVisible = false
    }

    private fun toggleHud() {
        hudVisible = !hudVisible
        binding.playbackOverlay.isVisible = hudVisible
        binding.speedIndicator.isVisible = hudVisible
        if (!hudVisible) binding.speedPanel.isVisible = false
    }

    // ═══════════════════════════════════════════════
    // Speed Selector
    // ═══════════════════════════════════════════════

    private fun setupSpeedSelector() {
        binding.speedIndicator.setOnClickListener {
            binding.speedPanel.isVisible = !binding.speedPanel.isVisible
        }
        speedIds.forEachIndexed { i, id ->
            binding.root.findViewById<TextView>(id).setOnClickListener {
                setSpeed(speeds[i])
                binding.speedPanel.isVisible = false
            }
        }
    }

    private fun setSpeed(speed: Float) {
        currentSpeed = speed
        player?.playbackParameters = PlaybackParameters(speed)
        binding.speedIndicator.text = if (speed == 1f) "1x" else "${speed}x"
        speedIds.forEachIndexed { i, id ->
            val tv = binding.root.findViewById<TextView>(id)
            tv.setTextColor(if (speeds[i] == speed) resources.getColor(R.color.neon_cyan, null) else resources.getColor(R.color.text_secondary, null))
            tv.isSelected = speeds[i] == speed
        }
    }

    // ═══════════════════════════════════════════════
    // Seek & Position
    // ═══════════════════════════════════════════════

    private fun startSeekUpdater() {
        seekJob = lifecycleScope.launch {
            while (isActive) {
                val p = player ?: break
                if (p.duration > 0) {
                    val cur = p.currentPosition
                    val dur = p.duration
                    if (!isSeekDragging) {
                        binding.playbackSeek.progress = (cur * 1000L / dur).toInt().coerceIn(0, 1000)
                    }
                    binding.textPlaybackTime.text = "${formatMs(cur)} / ${formatMs(dur)}"
                }
                delay(500)
            }
        }

        binding.playbackSeek.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(sb: SeekBar, p: Int, fromUser: Boolean) {
                if (fromUser) {
                    val pp = player ?: return
                    binding.textPlaybackTime.text = "${formatMs(pp.duration * p / 1000L)} / ${formatMs(pp.duration)}"
                }
            }
            override fun onStartTrackingTouch(sb: SeekBar) { isSeekDragging = true }
            override fun onStopTrackingTouch(sb: SeekBar) {
                val pp = player ?: return
                pp.seekTo(pp.duration * sb.progress / 1000L)
                isSeekDragging = false
            }
        })

        saveJob = lifecycleScope.launch {
            while (isActive) {
                delay(30_000)
                savePlaybackPosition()
            }
        }
    }

    private fun savePlaybackPosition() {
        val p = player ?: return
        val cur = p.currentPosition; val dur = p.duration
        if (dur <= 0 || cur == lastSavedPositionMs) return
        lastSavedPositionMs = cur
        lifecycleScope.launch {
            withContext(Dispatchers.IO) { VideoRepository.updatePlaybackHistory(item.id, cur, dur) }
        }
    }

    // ═══════════════════════════════════════════════
    // Picture-in-Picture
    // ═══════════════════════════════════════════════

    fun requestEnterPip() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.N) return
        if (!requireActivity().packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)) return
        val p = player ?: return
        val vs = p.videoSize
        val ar = if (vs.width > 0 && vs.height > 0) Rational(vs.width, vs.height) else Rational(16, 9)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            try {
                requireActivity().enterPictureInPictureMode(PictureInPictureParams.Builder().setAspectRatio(ar).build())
                isInPip = true
            } catch (_: Exception) { }
        }
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode)
        isInPip = isInPictureInPictureMode
        if (!isInPictureInPictureMode) { hudVisible = true; binding.playbackOverlay.isVisible = true }
        else { binding.playbackOverlay.isVisible = false }
    }

    // ═══════════════════════════════════════════════
    // Lifecycle
    // ═══════════════════════════════════════════════

    private fun formatMs(ms: Long): String {
        val ts = (ms + 500) / 1000L
        val s = (ts % 60L).toInt(); val m = (ts / 60L % 60L).toInt(); val h = (ts / 3600L).toInt()
        return if (h > 0) String.format(Locale.US, "%d:%02d:%02d", h, m, s)
        else String.format(Locale.US, "%d:%02d", m, s)
    }

    override fun onPause() {
        savePlaybackPosition()
        if (!isInPip) player?.pause()
        super.onPause()
    }

    override fun onResume() {
        super.onResume()
        if (isInPip) return
        if (!userPaused) player?.playWhenReady = true
    }

    override fun onDestroyView() {
        savePlaybackPosition()
        saveJob?.cancel(); seekJob?.cancel()
        player?.release(); player = null
        _binding = null
        super.onDestroyView()
    }

    fun getPlayer() = player
    fun isPausedByUser() = userPaused

    companion object {
        const val ARG_ID = "id"
        private const val ARG_TITLE = "title"
        private const val ARG_DESC = "description"
        private const val ARG_SOURCE = "source"
        private const val ARG_COVER = "cover"
        private const val ARG_STREAM = "stream"
        private const val ARG_CATEGORY = "category"
        private const val ARG_WATCH_POS = "watchPos"
        private const val GESTURE_NONE = 0
        private const val GESTURE_BRIGHTNESS = 1
        private const val GESTURE_VOLUME = 2
        private const val GESTURE_SEEK = 3
        private const val SKIP_SEC = 10
        private const val DOUBLE_TAP_MS = 350L
        private const val SEEK_THRESHOLD_DP = 20f

        fun newInstance(item: VideoItem): PlayerPageFragment = PlayerPageFragment().apply {
            arguments = Bundle().apply {
                putLong(ARG_ID, item.id)
                putString(ARG_TITLE, item.title)
                putString(ARG_DESC, item.description)
                putString(ARG_SOURCE, item.sourceType)
                if (item.coverUrl != null) putString(ARG_COVER, item.coverUrl)
                putString(ARG_STREAM, item.streamUrl)
                putString(ARG_CATEGORY, item.category)
                item.watchPosition?.let { putLong(ARG_WATCH_POS, it) }
            }
        }
    }
}
