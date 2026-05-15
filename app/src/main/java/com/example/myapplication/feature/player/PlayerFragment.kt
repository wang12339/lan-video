package com.example.myapplication.feature.player

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.viewpager2.adapter.FragmentStateAdapter
import androidx.viewpager2.widget.ViewPager2
import com.example.myapplication.R
import com.example.myapplication.data.model.VideoItem
import com.example.myapplication.data.repository.VideoRepository
import com.example.myapplication.databinding.FragmentPlayerBinding
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class PlayerFragment : Fragment() {
    private var _binding: FragmentPlayerBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository.getInstance()
    private var videos: List<VideoItem> = emptyList()
    private var currentPage = 0
    private var hudVisible = true
    private val hudHandler = Handler(Looper.getMainLooper())
    private val hudTimeoutMs = 3000L

    private val hudHideRunnable = Runnable { hideHud() }

    override fun onCreateView(
        inflater: LayoutInflater, container: ViewGroup?, savedInstanceState: Bundle?
    ): View {
        _binding = FragmentPlayerBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        val videoId = requireArguments().getLong("videoId")
        val category = requireArguments().getString("category") ?: ""
        binding.textTitle.text = "> ${requireArguments().getString("title").orEmpty()}"
        binding.btnBack.setOnClickListener { parentFragmentManager.popBackStack() }

        // Tap on video area to toggle HUD + left/right zones for prev/next
        binding.viewPagerPlayer.setOnTouchListener { _, event ->
            if (event.action == MotionEvent.ACTION_UP) {
                val x = event.x
                val width = binding.viewPagerPlayer.width
                when {
                    x < width / 3f -> {
                        // Left 1/3: previous
                        val prev = currentPage - 1
                        if (prev >= 0) {
                            binding.viewPagerPlayer.setCurrentItem(prev, true)
                        }
                    }
                    x > 2 * width / 3f -> {
                        // Right 1/3: next
                        val next = currentPage + 1
                        if (next < videos.size) {
                            binding.viewPagerPlayer.setCurrentItem(next, true)
                        }
                    }
                    else -> {
                        // Middle: toggle HUD
                        toggleHud()
                    }
                }
            }
            true
        }

        lifecycleScope.launch {
            val result = withContext(Dispatchers.IO) {
                repository.listVideos(type = "!local_image", size = 200)
            }
            result.onSuccess { resp ->
                val sameCategory = resp.items.filter { it.category == category }
                val others = resp.items.filter { it.category != category }
                videos = sameCategory + others
                if (videos.isEmpty()) {
                    binding.textTitle.text = "> NO VIDEOS"
                    return@onSuccess
                }
                val startIdx = videos.indexOfFirst { it.id == videoId }.coerceAtLeast(0)
                currentPage = startIdx
                binding.textTitle.text = "> ${videos[startIdx].title}"

                binding.viewPagerPlayer.adapter = PlayerPagerAdapter(this@PlayerFragment, videos)
                binding.viewPagerPlayer.setCurrentItem(startIdx, false)
                setupPageDots()
                updatePageCounter()

                binding.viewPagerPlayer.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
                    override fun onPageSelected(position: Int) {
                        currentPage = position
                        binding.textTitle.text = "> ${videos.getOrNull(position)?.title ?: ""}"
                        updatePageDots(position)
                        updatePageCounter()
                    }
                })

                // Start auto-hide timer
                resetHudTimer()
            }
        }
    }

    private fun setupPageDots() {
        binding.dotContainer.removeAllViews()
        for (i in videos.indices) {
            val dot = ImageView(requireContext()).apply {
                setImageResource(if (i == currentPage) R.drawable.shape_dot_active else R.drawable.shape_dot_inactive)
                val size = if (i == currentPage) 8 else 6
                val p = if (i == currentPage) 0 else 1
                layoutParams = ViewGroup.MarginLayoutParams(size + 4, size + 4)
                setPadding(p, p, p, p)
            }
            binding.dotContainer.addView(dot)
        }
        binding.dotContainer.visibility = View.VISIBLE
    }

    private fun updatePageDots(position: Int) {
        for (i in 0 until binding.dotContainer.childCount) {
            val dot = binding.dotContainer.getChildAt(i) as? ImageView ?: continue
            dot.setImageResource(if (i == position) R.drawable.shape_dot_active else R.drawable.shape_dot_inactive)
        }
    }

    private fun updatePageCounter() {
        binding.textPageCounter.text = "${currentPage + 1}/${videos.size}"
    }

    private fun toggleHud() {
        if (hudVisible) hideHud() else showHud()
    }

    private fun showHud() {
        hudVisible = true
        binding.hudTop.visibility = View.VISIBLE
        binding.dotContainer.visibility = View.VISIBLE
        binding.textPageCounter.visibility = View.VISIBLE
        resetHudTimer()
    }

    private fun hideHud() {
        hudVisible = false
        binding.hudTop.visibility = View.GONE
        binding.dotContainer.visibility = View.GONE
        binding.textPageCounter.visibility = View.GONE
        hudHandler.removeCallbacks(hudHideRunnable)
    }

    private fun resetHudTimer() {
        hudHandler.removeCallbacks(hudHideRunnable)
        hudHandler.postDelayed(hudHideRunnable, hudTimeoutMs)
    }

    fun skipToNext() {
        val next = currentPage + 1
        if (next < videos.size) {
            binding.viewPagerPlayer.setCurrentItem(next, true)
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        hudHandler.removeCallbacks(hudHideRunnable)
        _binding = null
    }
}

class PlayerPagerAdapter(
    private val fragment: PlayerFragment,
    private val items: List<VideoItem>
) : FragmentStateAdapter(fragment) {
    override fun getItemCount(): Int = items.size
    override fun getItemId(position: Int): Long = items[position].id
    override fun containsItem(itemId: Long): Boolean = items.any { it.id == itemId }
    override fun createFragment(position: Int): Fragment = PlayerPageFragment.newInstance(items[position])
}
