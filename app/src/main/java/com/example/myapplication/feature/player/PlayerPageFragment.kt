package com.example.myapplication.feature.player

import android.os.Bundle
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.widget.SeekBar
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView
import com.example.myapplication.R
import com.example.myapplication.data.model.VideoItem
import com.example.myapplication.data.network.StreamUrlResolver
import com.example.myapplication.data.repository.VideoRepository
import com.example.myapplication.databinding.ItemPlayerPageBinding
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
    private var longPressRunnable: Runnable? = null
    private var longPressActive = false
    private var userPaused = false
    private var seekJob: Job? = null
    private var saveJob: Job? = null
    private var lastSavedPositionMs: Long = 0L

    val videoId: Long get() = requireArguments().getLong(ARG_ID)
    val video: VideoItem get() {
        val a = requireArguments()
        return VideoItem(
            id = a.getLong(ARG_ID),
            title = a.getString(ARG_TITLE).orEmpty(),
            description = a.getString(ARG_DESC).orEmpty(),
            sourceType = a.getString(ARG_SOURCE) ?: "local_video",
            coverUrl = a.getString(ARG_COVER),
            streamUrl = a.getString(ARG_STREAM).orEmpty(),
            category = a.getString(ARG_CATEGORY) ?: "general"
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
        initializePlayer()
        startSeekUpdater()
    }

    private fun initializePlayer() {
        val item = video
        val p = ExoPlayer.Builder(requireContext()).build()
        val abs = StreamUrlResolver.toAbsoluteStreamUrl(item.streamUrl)
        p.setMediaItem(MediaItem.fromUri(abs))
        p.prepare()
        p.playWhenReady = true
        player = p

        binding.playerView.player = p
        attachTouchHandler(binding.playerView, p)
        binding.btnSkip.setOnClickListener {
            (parentFragment as? PlayerFragment)?.skipToNext()
        }

        p.addListener(object : Player.Listener {
            override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                binding.errorOverlay.isVisible = true
                binding.textError.text = "播放失败: ${error.errorCodeName}"
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                if (playbackState == Player.STATE_ENDED) {
                    savePlaybackPosition()
                    (parentFragment as? PlayerFragment)?.skipToNext()
                }
            }
        })
    }

    private fun attachTouchHandler(view: PlayerView, p: ExoPlayer) {
        val longPressMs = 450L
        val slop = ViewConfiguration.get(view.context).scaledTouchSlop * 2
        var downY = 0f
        var downTime = 0L

        view.setOnTouchListener { v, event ->
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downY = event.y
                    downTime = System.currentTimeMillis()
                    longPressRunnable?.let { v.removeCallbacks(it) }
                    longPressActive = false
                    longPressRunnable = Runnable {
                        longPressActive = true
                        p.playbackParameters = PlaybackParameters(2f)
                        binding.speedIndicator.isVisible = true
                    }
                    longPressRunnable?.let { v.postDelayed(it, longPressMs) }
                    true
                }
                MotionEvent.ACTION_MOVE -> {
                    if (abs(event.y - downY) > slop) {
                        longPressRunnable?.let { v.removeCallbacks(it) }
                        longPressRunnable = null
                        if (longPressActive) {
                            p.playbackParameters = PlaybackParameters(1f)
                            longPressActive = false
                            binding.speedIndicator.isVisible = false
                        }
                        return@setOnTouchListener false
                    }
                    true
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    longPressRunnable?.let { v.removeCallbacks(it) }
                    longPressRunnable = null
                    if (longPressActive) {
                        p.playbackParameters = PlaybackParameters(1f)
                        longPressActive = false
                        binding.speedIndicator.isVisible = false
                    } else if (event.action == MotionEvent.ACTION_UP) {
                        val dt = System.currentTimeMillis() - downTime
                        if (dt < longPressMs) {
                            if (p.isPlaying) {
                                p.pause()
                                userPaused = true
                            } else {
                                userPaused = false
                                if (p.playbackState == Player.STATE_ENDED) p.seekTo(0)
                                p.play()
                            }
                        }
                    }
                    true
                }
                else -> true
            }
        }
    }

    private fun startSeekUpdater() {
        seekJob = lifecycleScope.launch {
            while (isActive) {
                val p = player ?: break
                if (p.isPlaying && p.duration > 0) {
                    val cur = p.currentPosition
                    val dur = p.duration
                    val permille = (cur * 1000L / dur).toInt().coerceIn(0, 1000)
                    binding.playbackSeek.progress = permille
                    binding.textPlaybackTime.text = "${formatMs(cur)} / ${formatMs(dur)}"
                }
                delay(500)
            }
        }

        // Save playback position every 30 seconds
        saveJob = lifecycleScope.launch {
            while (isActive) {
                delay(30_000)
                savePlaybackPosition()
            }
        }
    }

    private fun savePlaybackPosition() {
        val p = player ?: return
        val cur = p.currentPosition
        val dur = p.duration
        if (dur <= 0) return
        // Skip save if position hasn't changed (paused)
        if (cur == lastSavedPositionMs) return
        lastSavedPositionMs = cur
        lifecycleScope.launch {
            withContext(Dispatchers.IO) {
                VideoRepository.getInstance().updatePlaybackHistory(videoId, cur, dur)
            }
        }
    }

    fun getPlayer(): ExoPlayer? = player
    fun getPlaybackSeek(): SeekBar = binding.playbackSeek
    fun getPlaybackTime(): android.widget.TextView = binding.textPlaybackTime

    fun isPausedByUser(): Boolean = userPaused

    fun showOverlay() {
        binding.playbackOverlay.isVisible = true
    }

    fun hideOverlay() {
        binding.playbackOverlay.isVisible = false
    }

    private fun formatMs(ms: Long): String {
        val totalS = (ms + 500) / 1000L
        val s = (totalS % 60L).toInt()
        val m = (totalS / 60L % 60L).toInt()
        val h = (totalS / 3600L).toInt()
        return if (h > 0) String.format(Locale.US, "%d:%02d:%02d", h, m, s)
        else String.format(Locale.US, "%d:%02d", m, s)
    }

    override fun onPause() {
        savePlaybackPosition()
        player?.pause()
        super.onPause()
    }

    override fun onResume() {
        super.onResume()
        if (!userPaused) player?.playWhenReady = true
    }

    override fun onDestroyView() {
        savePlaybackPosition()
        saveJob?.cancel()
        seekJob?.cancel()
        longPressRunnable?.let { binding.playerView.removeCallbacks(it) }
        player?.release()
        player = null
        _binding = null
        super.onDestroyView()
    }

    companion object {
        const val ARG_ID = "id"
        private const val ARG_TITLE = "title"
        private const val ARG_DESC = "description"
        private const val ARG_SOURCE = "source"
        private const val ARG_COVER = "cover"
        private const val ARG_STREAM = "stream"
        private const val ARG_CATEGORY = "category"

        fun newInstance(item: VideoItem): PlayerPageFragment = PlayerPageFragment().apply {
            arguments = Bundle().apply {
                putLong(ARG_ID, item.id)
                putString(ARG_TITLE, item.title)
                putString(ARG_DESC, item.description)
                putString(ARG_SOURCE, item.sourceType)
                if (item.coverUrl != null) putString(ARG_COVER, item.coverUrl)
                putString(ARG_STREAM, item.streamUrl)
                putString(ARG_CATEGORY, item.category)
            }
        }
    }
}
