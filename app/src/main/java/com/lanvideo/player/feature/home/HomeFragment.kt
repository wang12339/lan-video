package com.lanvideo.player.feature.home

import android.app.AlertDialog
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.TextView
import android.widget.Toast
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.viewModels
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.RecyclerView
import androidx.viewpager2.widget.ViewPager2
import coil.load
import com.lanvideo.player.ConnectionState
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.network.LanServerDiscovery
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.data.network.StreamUrlResolver
import com.lanvideo.player.data.util.ConnectionStatusHelper
import com.lanvideo.player.data.util.toImageViewerBundle
import com.lanvideo.player.data.util.toPlayerBundle
import com.lanvideo.player.databinding.FragmentHomeBinding
import com.lanvideo.player.databinding.ItemDataSlabBinding
import com.lanvideo.player.feature.common.DataStreamAdapter
import com.lanvideo.player.feature.home.viewmodel.HomeViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class HomeFragment : Fragment() {

    private var _binding: FragmentHomeBinding? = null
    private val binding get() = _binding!!
    private val viewModel: HomeViewModel by viewModels()
    private var streamAdapter: DataStreamAdapter? = null
    private val channels = listOf("全部", "视频", "图片")
    private var didInitialLanDiscover = false
    private var bannerAdapter: BannerPagerAdapter? = null
    private val bannerHandler = Handler(Looper.getMainLooper())
    private var bannerRunning = false
    private val bannerScrollRunnable = object : Runnable {
        override fun run() {
            val pager = binding.bannerPager
            val next = pager.currentItem + 1
            if (next < (bannerAdapter?.itemCount ?: 0)) {
                pager.setCurrentItem(next, true)
            } else {
                pager.setCurrentItem(0, true)
            }
            if (bannerRunning) bannerHandler.postDelayed(this, 5000L)
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater, container: ViewGroup?, savedInstanceState: Bundle?
    ): View {
        _binding = FragmentHomeBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        try {
            binding.btnHomeMenu.setOnClickListener {
                (requireActivity() as? com.lanvideo.player.MainActivity)?.openDrawer()
            }
            ViewCompat.setOnApplyWindowInsetsListener(binding.recyclerStream) { v, insets ->
                val navBar = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
                v.setPadding(v.paddingStart, v.paddingTop, v.paddingEnd, navBar.bottom)
                insets
            }
            setupChannelSelector()
            setupStream()
            setupSwipeRefresh()
            setupBanner()
            observeViewModel()
            observeEvents()
            viewModel.loadFeed(requireContext())
        } catch (e: Exception) {
            android.util.Log.e("HomeFragment", "onViewCreated error", e)
            Toast.makeText(requireContext(), "初始化失败: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    override fun onResume() {
        super.onResume()
        val s = viewModel.uiState.value
        if (s.videos.isNotEmpty()) {
            viewModel.refreshFeed(requireContext())
        } else {
            viewModel.loadFeed(requireContext())
        }
    }

    private fun observeViewModel() {
        viewLifecycleOwner.lifecycleScope.launch {
            viewLifecycleOwner.repeatOnLifecycle(Lifecycle.State.STARTED) {
                viewModel.uiState.collectLatest { state ->
                    streamAdapter?.submitList(state.videos)
                    updateBanner(state.bannerItems)
                    updateSelectionBar(state)
                    binding.recyclerStream.isVisible = state.videos.isNotEmpty() && !state.isLoading
                    binding.loadingContainer.isVisible = state.isLoading && state.videos.isEmpty()
                    binding.loadingMore.isVisible = state.isLoadingMore
                    binding.swipeRefresh.isRefreshing = state.isRefreshing
                    if (state.videos.isEmpty() && !state.isLoading) {
                        binding.emptyFeed.isVisible = true
                        binding.emptyFeedText.text = state.error ?: state.emptyText
                    } else {
                        binding.emptyFeed.isVisible = false
                    }
                }
            }
        }
    }

    private fun setupChannelSelector() {
        binding.channelContainer.removeAllViews()
        channels.forEachIndexed { index, name ->
            val chip = TextView(requireContext()).apply {
                text = name
                setPadding(20, 6, 20, 6)
                textSize = 14f
                setTextColor(resources.getColorStateList(R.color.nav_color_selector, null))
                isClickable = true
                isFocusable = true
                setOnClickListener {
                    viewModel.switchChannel(requireContext(), index)
                    updateChannelSelection(index)
                }
            }
            binding.channelContainer.addView(chip)
        }
        updateChannelSelection(0)
    }

    private fun updateChannelSelection(selectedIndex: Int) {
        for (i in 0 until binding.channelContainer.childCount) {
            val chip = binding.channelContainer.getChildAt(i) as? TextView ?: continue
            chip.isSelected = i == selectedIndex
            chip.alpha = if (i == selectedIndex) 1f else 0.6f
            chip.typeface = if (i == selectedIndex) android.graphics.Typeface.DEFAULT_BOLD else android.graphics.Typeface.DEFAULT
        }
    }

    private fun setupStream() {
        streamAdapter = DataStreamAdapter(
            onClick = { item ->
                val adapter = streamAdapter ?: return@DataStreamAdapter
                if (adapter.isSelectMode) {
                    adapter.toggleSelection(item.id)
                    val allIds = adapter.selectedIds
                    viewModel.toggleSelection(item.id, allIds)
                    updateSelectionBar(viewModel.uiState.value)
                } else if (item.sourceType.contains("image", ignoreCase = true)) {
                    openImageViewer(item)
                } else {
                    openPlayer(item)
                }
            },
            onLongClick = { item ->
                AlertDialog.Builder(requireContext())
                    .setTitle("删除视频")
                    .setMessage("确定要删除「${item.title}」吗？")
                    .setPositiveButton("删除") { _, _ ->
                        viewModel.deleteVideo(requireContext(), item.id)
                        Toast.makeText(requireContext(), "已删除", Toast.LENGTH_SHORT).show()
                    }
                    .setNegativeButton("取消", null)
                    .show()
            }
        )
        binding.recyclerStream.layoutManager = GridLayoutManager(requireContext(), 2)
        binding.recyclerStream.adapter = streamAdapter

        binding.recyclerStream.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrolled(recyclerView: RecyclerView, dx: Int, dy: Int) {
                super.onScrolled(recyclerView, dx, dy)
                val layoutManager = recyclerView.layoutManager as? GridLayoutManager ?: return
                val visibleItemCount = layoutManager.childCount
                val totalItemCount = layoutManager.itemCount
                val firstVisibleItemPosition = layoutManager.findFirstVisibleItemPosition()
                if (!viewModel.uiState.value.isLoadingMore && viewModel.uiState.value.hasMore
                    && (visibleItemCount + firstVisibleItemPosition) >= totalItemCount
                    && firstVisibleItemPosition >= 0) {
                    viewModel.loadMore(requireContext())
                }
            }
        })

        binding.btnSelectAll.setOnClickListener {
            val adapter = streamAdapter ?: return@setOnClickListener
            if (adapter.isAllSelected()) {
                adapter.clearSelection()
            } else {
                adapter.selectAll()
            }
            viewModel.toggleSelection(-1, adapter.selectedIds)
        }
        binding.btnDeleteSelected.setOnClickListener {
            val ids = streamAdapter?.selectedIds?.toList() ?: return@setOnClickListener
            if (ids.isEmpty()) return@setOnClickListener
            viewModel.deleteVideos(requireContext(), ids) { count ->
                streamAdapter?.clearSelection()
                Toast.makeText(requireContext(), "已删除 $count 个", Toast.LENGTH_SHORT).show()
            }
        }
        binding.btnCancelSelect.setOnClickListener {
            streamAdapter?.clearSelection()
            viewModel.clearSelection()
        }
    }

    private fun setupSwipeRefresh() {
        binding.swipeRefresh.setOnRefreshListener {
            viewModel.loadFeed(requireContext())
        }
        binding.swipeRefresh.setColorSchemeResources(R.color.brand, R.color.neon_cyan)
    }

    private fun setupBanner() {
        bannerAdapter = BannerPagerAdapter(
            onClick = { item ->
                if (item.sourceType.contains("image", ignoreCase = true)) {
                    findNavController().navigate(R.id.nav_image_viewer, item.toImageViewerBundle())
                } else {
                    findNavController().navigate(R.id.nav_player, item.toPlayerBundle())
                }
            }
        )
        binding.bannerPager.adapter = bannerAdapter
        binding.bannerPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                updateBannerDots(position)
            }
        })
    }

    private fun updateBanner(items: List<VideoItem>) {
        val bannerItems = items.take(5)
        bannerAdapter?.submitList(bannerItems)
        binding.bannerPager.isVisible = bannerItems.isNotEmpty()
        setupBannerDots(bannerItems.size)
        startBannerAutoScroll(bannerItems.size > 1)
    }

    private fun setupBannerDots(count: Int) {
        binding.bannerDots.removeAllViews()
        for (i in 0 until count) {
            val dot = ImageView(requireContext()).apply {
                setImageResource(if (i == 0) R.drawable.shape_dot_active else R.drawable.shape_dot_inactive)
                val size = 8
                val params = ViewGroup.MarginLayoutParams(size + 4, size + 4)
                params.marginStart = 3; params.marginEnd = 3
                layoutParams = params
            }
            binding.bannerDots.addView(dot)
        }
        binding.bannerDots.isVisible = count > 0
    }

    private fun updateBannerDots(position: Int) {
        for (i in 0 until binding.bannerDots.childCount) {
            val dot = binding.bannerDots.getChildAt(i) as? ImageView ?: continue
            dot.setImageResource(if (i == position) R.drawable.shape_dot_active else R.drawable.shape_dot_inactive)
        }
    }

    private fun startBannerAutoScroll(enable: Boolean) {
        bannerRunning = enable
        bannerHandler.removeCallbacks(bannerScrollRunnable)
        if (enable) bannerHandler.postDelayed(bannerScrollRunnable, 5000L)
    }

    private class BannerPagerAdapter(
        private val onClick: (VideoItem) -> Unit
    ) : RecyclerView.Adapter<BannerPagerAdapter.BannerVH>() {
        private var items: List<VideoItem> = emptyList()

        fun submitList(list: List<VideoItem>) { items = list; notifyDataSetChanged() }

        override fun getItemCount() = items.size
        override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): BannerVH {
            val binding = ItemDataSlabBinding.inflate(LayoutInflater.from(parent.context), parent, false)
            return BannerVH(binding)
        }
        override fun onBindViewHolder(holder: BannerVH, position: Int) {
            holder.bind(items[position])
        }

        inner class BannerVH(private val binding: ItemDataSlabBinding) :
            RecyclerView.ViewHolder(binding.root) {
            fun bind(item: VideoItem) {
                val ctx = binding.root.context
                val imgUrl = item.coverUrl?.takeIf { it.isNotBlank() } ?: item.streamUrl
                binding.imageCover.load(StreamUrlResolver.toAbsoluteStreamUrl(imgUrl)) {
                    placeholder(R.drawable.ic_gallery_black_24dp)
                    error(R.drawable.ic_slideshow_black_24dp)
                    crossfade(true)
                }
                binding.textTitle.text = item.title
                binding.textMetadata.isVisible = false
                binding.badgeDuration.isVisible = false
                binding.root.setOnClickListener { onClick(item) }
            }
        }
    }

    private fun observeEvents() {
        val app = requireActivity().application as MyApplication
        app.lanServerEvents.observe(viewLifecycleOwner) { viewModel.loadFeed(requireContext()) }
        ConnectionStatusHelper(
            statusView = binding.connectionStatus,
            statusDot = binding.statusDot,
            statusText = binding.statusText,
        ).observe(viewLifecycleOwner, app, lifecycleScope)
        app.batchDeleteRequested.observe(viewLifecycleOwner) { requested ->
            if (requested) {
                enterBatchDeleteMode()
                app.setBatchDeleteRequested(false)
            }
        }
        binding.connectionStatus.setOnClickListener {
            app.setConnectionState(ConnectionState.SCANNING)
            lifecycleScope.launch {
                LanServerDiscovery.discoverActiveNetwork(requireContext().applicationContext, force = true)
                viewModel.loadFeed(requireContext())
            }
        }
    }

    private fun enterBatchDeleteMode() {
        lifecycleScope.launch {
            binding.loadingContainer.isVisible = true
            val typeFilter = if (viewModel.uiState.value.currentChannel == 2) "local_image" else "!local_image"
            val result = withContext(Dispatchers.IO) {
                VideoRepository.listVideos(type = typeFilter, page = 0, size = 1000)
            }
            binding.loadingContainer.isVisible = false
            result.onSuccess { resp ->
                streamAdapter?.submitList(resp.items)
                streamAdapter?.enterSelectMode(-1)
                viewModel.enterSelectMode(-1)
                binding.recyclerStream.isVisible = true
            }.onFailure {
                Toast.makeText(requireContext(), "加载失败: ${it.message}", Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun updateSelectionBar(state: com.lanvideo.player.feature.home.viewmodel.HomeUiState) {
        binding.selectionBar.isVisible = state.isSelectMode
        binding.selectionCount.text = if (state.selectedCount > 0) "已选: ${state.selectedCount}" else ""
        binding.btnSelectAll.text = if (state.isAllSelected) "取消全选" else "全选"
    }

    private fun openPlayer(item: VideoItem) {
        findNavController().navigate(R.id.nav_player, item.toPlayerBundle())
    }

    private fun openImageViewer(item: VideoItem) {
        findNavController().navigate(R.id.nav_image_viewer, item.toImageViewerBundle())
    }

    fun onBackPressed(): Boolean {
        val s = viewModel.uiState.value
        if (s.isSelectMode) {
            streamAdapter?.clearSelection()
            viewModel.clearSelection()
            return true
        }
        return false
    }

    override fun onDestroyView() {
        super.onDestroyView()
        bannerRunning = false
        bannerHandler.removeCallbacks(bannerScrollRunnable)
        streamAdapter = null
        _binding = null
    }
}
