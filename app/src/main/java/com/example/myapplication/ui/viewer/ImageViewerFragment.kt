package com.example.myapplication.ui.viewer

import android.animation.Animator
import android.animation.ObjectAnimator
import android.os.Bundle
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.PagerSnapHelper
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import coil.load
import com.example.myapplication.R
import com.example.myapplication.data.network.StreamUrlResolver
import com.example.myapplication.data.repository.VideoRepository
import com.example.myapplication.databinding.FragmentImageViewerBinding
import com.example.myapplication.databinding.ItemImageViewerPageBinding
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class ImageViewerFragment : Fragment() {
    private var _binding: FragmentImageViewerBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository.getInstance()

    private var images: List<String> = emptyList()
    private var titles: List<String> = emptyList()
    private var currentPos = 0
    private var isInfoVisible = false
    private var isFilmstripVisible = false

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentImageViewerBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        val videoId = requireArguments().getLong("videoId", -1L)

        binding.btnBack.setOnClickListener {
            parentFragmentManager.popBackStack()
        }

        ViewCompat.setOnApplyWindowInsetsListener(binding.btnBack) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingStart, top, v.paddingEnd, v.paddingBottom)
            insets
        }

        // Tap to toggle info panel
        binding.viewerPager.setOnClickListener {
            toggleInfoPanel()
        }

        lifecycleScope.launch {
            val result = withContext(Dispatchers.IO) {
                repository.listVideos(type = "local_image", page = 0, size = 1000)
            }
            result.onSuccess { resp ->
                images = resp.items.map { StreamUrlResolver.toAbsoluteStreamUrl(it.streamUrl) }
                titles = resp.items.map { it.title }

                currentPos = if (videoId > 0) {
                    resp.items.indexOfFirst { it.id == videoId }.coerceAtLeast(0)
                } else {
                    0
                }

                if (images.isEmpty()) {
                    binding.textCounter.text = "-- / --"
                    return@onSuccess
                }

                binding.textCounter.text = formatCounter(currentPos, images.size)

                binding.viewerPager.adapter = ZoomableImageAdapter(images, requireActivity())
                binding.viewerPager.setCurrentItem(currentPos, false)
                binding.viewerPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
                    override fun onPageSelected(position: Int) {
                        currentPos = position
                        binding.textCounter.text = formatCounter(position, images.size)
                        updateInfoPanel(position)
                        updateFilmstrip(position)
                    }
                })

                // Filmstrip setup
                setupFilmstrip()
            }.onFailure {
                binding.textCounter.text = "> LOAD FAILED"
            }
        }
    }

    private fun setupFilmstrip() {
        binding.filmstripContainer.visibility = View.VISIBLE
        isFilmstripVisible = true
        val adapter = FilmstripAdapter(images) { pos ->
            binding.viewerPager.setCurrentItem(pos, true)
        }
        binding.filmstripRecycler.layoutManager = LinearLayoutManager(requireContext(), LinearLayoutManager.HORIZONTAL, false)
        binding.filmstripRecycler.adapter = adapter
        val snapHelper = PagerSnapHelper()
        snapHelper.attachToRecyclerView(binding.filmstripRecycler)
    }

    private fun updateFilmstrip(position: Int) {
        val adapter = binding.filmstripRecycler.adapter as? FilmstripAdapter ?: return
        adapter.setActivePosition(position)
        binding.filmstripRecycler.smoothScrollToPosition(position)
    }

    private fun toggleInfoPanel() {
        isInfoVisible = !isInfoVisible
        binding.infoPanel.visibility = if (isInfoVisible) View.VISIBLE else View.GONE
        if (isInfoVisible) updateInfoPanel(currentPos)
        binding.filmstripContainer.visibility = if (isInfoVisible) View.GONE else View.VISIBLE
        isFilmstripVisible = !isInfoVisible
    }

    private fun updateInfoPanel(position: Int) {
        if (position < titles.size) {
            binding.infoFilename.text = "> ${titles[position]}"
        }
        binding.infoResolution.text = "> RES: --"
        binding.infoSize.text = "> SIZE: --"
    }

    private fun formatCounter(pos: Int, total: Int): String {
        return "%02d / %02d".format(pos + 1, total)
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}

/** Adapter that wraps images with zoom support via Matrix */
class ZoomableImageAdapter(
    private val urls: List<String>,
    private val activity: android.app.Activity
) : RecyclerView.Adapter<ZoomableImageAdapter.ZoomableViewHolder>() {

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ZoomableViewHolder {
        val binding = ItemImageViewerPageBinding.inflate(
            LayoutInflater.from(parent.context), parent, false
        )
        return ZoomableViewHolder(binding)
    }

    override fun onBindViewHolder(holder: ZoomableViewHolder, position: Int) {
        holder.bind(urls[position])
    }

    override fun getItemCount(): Int = urls.size

    inner class ZoomableViewHolder(private val binding: ItemImageViewerPageBinding) :
        RecyclerView.ViewHolder(binding.root) {

        private var scaleFactor = 1f
        private val maxScale = 5f
        private var matrix = android.graphics.Matrix()

        private val scaleDetector = ScaleGestureDetector(activity, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScale(detector: ScaleGestureDetector): Boolean {
                scaleFactor *= detector.scaleFactor
                scaleFactor = scaleFactor.coerceIn(1f, maxScale)
                matrix.setScale(scaleFactor, scaleFactor, detector.focusX, detector.focusY)
                binding.imageView.imageMatrix = matrix
                binding.imageView.scaleType = android.widget.ImageView.ScaleType.MATRIX
                return true
            }
        })

        private val gestureDetector = android.view.GestureDetector(activity, object : android.view.GestureDetector.SimpleOnGestureListener() {
            override fun onDoubleTap(e: MotionEvent): Boolean {
                scaleFactor = if (scaleFactor > 1.1f) 1f else 2f
                matrix.reset()
                if (scaleFactor > 1f) {
                    matrix.setScale(scaleFactor, scaleFactor, e.x, e.y)
                }
                binding.imageView.imageMatrix = matrix
                binding.imageView.scaleType = android.widget.ImageView.ScaleType.MATRIX
                return true
            }
        })

        fun bind(url: String) {
            binding.imageView.load(url) {
                placeholder(R.drawable.ic_gallery_black_24dp)
                error(R.drawable.ic_slideshow_black_24dp)
            }
            binding.imageView.scaleType = android.widget.ImageView.ScaleType.FIT_CENTER
            binding.imageView.setOnTouchListener { _, event ->
                scaleDetector.onTouchEvent(event)
                gestureDetector.onTouchEvent(event)
                true
            }
        }
    }
}

/** Filmstrip adapter showing small thumbnails */
class FilmstripAdapter(
    private val urls: List<String>,
    private val onClick: (Int) -> Unit
) : RecyclerView.Adapter<FilmstripAdapter.VH>() {

    private var activePos = 0

    fun setActivePosition(pos: Int) {
        activePos = pos
        notifyDataSetChanged()
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): VH {
        val density = parent.resources.displayMetrics.density
        val w = (64 * density).toInt()
        val h = (48 * density).toInt()
        val iv = android.widget.ImageView(parent.context).apply {
            layoutParams = ViewGroup.LayoutParams(w, h)
            scaleType = android.widget.ImageView.ScaleType.CENTER_CROP
            setPadding(2, 2, 2, 2)
        }
        return VH(iv)
    }

    override fun onBindViewHolder(holder: VH, position: Int) {
        holder.imageView.load(urls[position]) {
            placeholder(R.drawable.ic_gallery_black_24dp)
            error(R.drawable.ic_slideshow_black_24dp)
        }
        holder.imageView.alpha = if (position == activePos) 1f else 0.5f
        holder.imageView.setBackgroundResource(if (position == activePos) R.color.neon_cyan_dim else android.R.color.transparent)
        holder.imageView.setOnClickListener { onClick(position) }
    }

    override fun getItemCount(): Int = urls.size

    class VH(val imageView: android.widget.ImageView) : RecyclerView.ViewHolder(imageView)
}
