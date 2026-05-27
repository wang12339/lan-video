package com.lanvideo.player.feature.home

import android.app.AlertDialog
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import android.widget.Toast
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.lanvideo.player.ConnectionState
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.network.LanServerDiscovery
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.model.PagedVideoResponse
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.data.user.AuthSessionStore
import com.lanvideo.player.data.util.ConnectionStatusHelper
import com.lanvideo.player.databinding.FragmentHomeBinding
import com.lanvideo.player.feature.common.DataStreamAdapter
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout

class HomeFragment : Fragment() {

    private var _binding: FragmentHomeBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository
    private var streamAdapter: DataStreamAdapter? = null
    private var allVideos = mutableListOf<VideoItem>()
    private var currentPage = 0
    private var totalItems = 0
    private var isLoadingMore = false
    private var hasMore = true
    private var currentChannel = 0 // 0=all, 1=video, 2=image
    private val channels = listOf("全部", "视频", "图片")
    private var didInitialLanDiscover = false

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
            observeEvents()
            loadFeed()
        } catch (e: Exception) {
            android.util.Log.e("HomeFragment", "onViewCreated error", e)
            Toast.makeText(requireContext(), "初始化失败: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    override fun onResume() {
        super.onResume()
        loadFeed()
    }

    private fun setupChannelSelector() {
        binding.channelContainer.removeAllViews()
        channels.forEachIndexed { index, name ->
            val chip = TextView(requireContext()).apply {
                text = name
                setPadding(16, 0, 16, 0)
                textSize = 12f
                setTextColor(resources.getColorStateList(R.color.nav_color_selector, null))
                isClickable = true
                isFocusable = true
                setOnClickListener {
                    if (currentChannel != index) {
                        currentChannel = index
                        updateChannelSelection()
                        loadFeed()
                    }
                }
            }
            binding.channelContainer.addView(chip)
        }
        updateChannelSelection()
    }

    private fun updateChannelSelection() {
        for (i in 0 until binding.channelContainer.childCount) {
            val chip = binding.channelContainer.getChildAt(i) as? TextView ?: continue
            chip.isSelected = i == currentChannel
            chip.alpha = if (i == currentChannel) 1f else 0.5f
        }
    }

    private fun setupStream() {
        streamAdapter = DataStreamAdapter(
            onClick = { item ->
                val adapter = streamAdapter ?: return@DataStreamAdapter
                if (adapter.isSelectMode) {
                    adapter.toggleSelection(item.id)
                    updateSelectionBar(adapter.selectedCount)
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
                        lifecycleScope.launch {
                            val result = withContext(Dispatchers.IO) { repository.deleteVideo(item.id) }
                            if (result.isSuccess) {
                                repository.invalidateCache()
                                Toast.makeText(requireContext(), "已删除", Toast.LENGTH_SHORT).show()
                                loadFeed()
                            } else {
                                Toast.makeText(requireContext(), "删除失败: ${result.exceptionOrNull()?.message}", Toast.LENGTH_SHORT).show()
                            }
                        }
                    }
                    .setNegativeButton("取消", null)
                    .show()
            }
        )
        binding.recyclerStream.layoutManager = LinearLayoutManager(requireContext())
        binding.recyclerStream.adapter = streamAdapter

        // 分页滚动监听
        binding.recyclerStream.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrolled(recyclerView: RecyclerView, dx: Int, dy: Int) {
                super.onScrolled(recyclerView, dx, dy)
                val layoutManager = recyclerView.layoutManager as? LinearLayoutManager ?: return
                val visibleItemCount = layoutManager.childCount
                val totalItemCount = layoutManager.itemCount
                val firstVisibleItemPosition = layoutManager.findFirstVisibleItemPosition()
                if (!isLoadingMore && hasMore
                    && (visibleItemCount + firstVisibleItemPosition) >= totalItemCount
                    && firstVisibleItemPosition >= 0) {
                    loadMore()
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
            updateSelectionBar(adapter.selectedCount)
        }
        binding.btnDeleteSelected.setOnClickListener {
            val ids = streamAdapter?.selectedIds?.toList() ?: return@setOnClickListener
            if (ids.isEmpty()) return@setOnClickListener
            lifecycleScope.launch {
                val result = repository.deleteVideos(ids)
                if (result.isSuccess) {
                    repository.invalidateCache()
                    streamAdapter?.clearSelection()
                    updateSelectionBar(0)
                    loadFeed()
                    Toast.makeText(requireContext(), "已删除 ${ids.size} 个", Toast.LENGTH_SHORT).show()
                } else {
                    Toast.makeText(requireContext(),
                        "删除失败: ${result.exceptionOrNull()?.message?.take(60)}",
                        Toast.LENGTH_LONG).show()
                }
            }
        }
        binding.btnCancelSelect.setOnClickListener {
            streamAdapter?.clearSelection()
            updateSelectionBar(0)
        }
    }

    private fun observeEvents() {
        val app = requireActivity().application as MyApplication
        app.lanServerEvents.observe(viewLifecycleOwner) { loadFeed() }
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
                loadFeed()
            }
        }
    }

    private fun loadFeed() {
        lifecycleScope.launch {
            if (!isAdded || _binding == null) return@launch

            // 未登录时不请求，登录后由 LoginDialog 触发重载
            if (!AuthSessionStore.isLoggedIn(requireContext())) {
                allVideos.clear()
                streamAdapter?.submitList(emptyList())
                binding.recyclerStream.isVisible = false
                binding.loadingContainer.isVisible = false
                binding.emptyFeed.isVisible = true
                binding.emptyFeedText.text = "> 请先登录"
                return@launch
            }

            // 重置分页状态
            allVideos.clear()
            currentPage = 0
            hasMore = true
            totalItems = 0
            streamAdapter?.submitList(emptyList())
            binding.recyclerStream.isVisible = false
            binding.loadingContainer.isVisible = true

            val typeFilter = when (currentChannel) {
                1 -> "!local_image"
                2 -> "local_image"
                else -> "" // all
            }
            val app = requireActivity().application as MyApplication

            val pagedResponse = try {
                withTimeout(5_000L) {
                    when (currentChannel) {
                        2 -> {
                            // 图片频道：一次性加载
                            repository.listVideos(type = "local_image", page = 0, size = 1000)
                        }
                        1 -> {
                            // 视频频道：分页加载
                            repository.listVideos(type = "!local_image", page = 0, size = 20)
                        }
                        else -> {
                            // 全部频道：分页加载视频 + 一次性加载图片
                            val vids = repository.listVideos(type = "!local_image", page = 0, size = 20)
                            val imgs = repository.listVideos(type = "local_image", page = 0, size = 1000)
                            if (vids.isFailure && imgs.isFailure) {
                                throw java.io.IOException(
                                    vids.exceptionOrNull()?.message
                                        ?: imgs.exceptionOrNull()?.message
                                        ?: "无法连接服务器"
                                )
                            }
                            val vidsItems = vids.getOrNull()?.items ?: emptyList()
                            val imgsItems = imgs.getOrNull()?.items ?: emptyList()
                            val vidsTotal = vids.getOrNull()?.total ?: 0L
                            Result.success(PagedVideoResponse(
                                vidsItems + imgsItems,
                                vidsTotal + imgsItems.size,
                                0,
                                vidsItems.size + imgsItems.size
                            ))
                        }
                    }
                }
            } catch (e: Exception) {
                Result.failure<PagedVideoResponse>(e)
            }

            var finalResult = pagedResponse
            if (finalResult.isFailure && !didInitialLanDiscover) {
                didInitialLanDiscover = true
                withContext(Dispatchers.IO) {
                    LanServerDiscovery.discoverActiveNetwork(requireContext().applicationContext, force = true)
                }
                if (!isAdded || _binding == null) return@launch
                finalResult = runCatching {
                    withTimeout(5_000L) {
                        when (currentChannel) {
                            2 -> repository.listVideos(type = "local_image", page = 0, size = 1000)
                            1 -> repository.listVideos(type = "!local_image", page = 0, size = 20)
                            else -> {
                                val vids = repository.listVideos(type = "!local_image", page = 0, size = 20)
                                val imgs = repository.listVideos(type = "local_image", page = 0, size = 1000)
                                val allItems = (vids.getOrNull()?.items ?: emptyList()) +
                                    (imgs.getOrNull()?.items ?: emptyList())
                                Result.success(PagedVideoResponse(allItems, allItems.size.toLong(), 0, allItems.size))
                            }
                        }
                    }
                }.getOrElse { Result.failure(java.io.IOException("连接超时")) }
            }
            if (!isAdded || _binding == null) return@launch

            binding.loadingContainer.isVisible = false
            finalResult.onSuccess { resp ->
                allVideos.addAll(resp.items)
                totalItems = resp.total.toInt()
                hasMore = allVideos.size < totalItems
                currentPage = 1 // 下一页页码
                applyStream()
            }.onFailure { err ->
                app.setConnectionState(ConnectionState.DISCONNECTED)
                binding.emptyFeedText.text = "> 加载失败: ${err.message?.take(60) ?: "未知错误"}"
                binding.emptyFeed.isVisible = true
            }
        }
    }

    private fun loadMore() {
        if (currentChannel == 2) return // 图片频道已一次性加载完
        isLoadingMore = true
        binding.loadingMore.isVisible = true
        lifecycleScope.launch {
            if (!isAdded || _binding == null) return@launch
            val app = requireActivity().application as MyApplication

            val result = try {
                withTimeout(5_000L) {
                    if (currentChannel == 0) {
                        // 全部频道：只加载更多视频
                        val vids = repository.listVideos(type = "!local_image", page = currentPage, size = 20)
                        if (vids.isFailure) {
                            throw vids.exceptionOrNull() ?: java.io.IOException("加载更多失败")
                        }
                        vids.getOrThrow()
                    } else {
                        repository.listVideos(type = "!local_image", page = currentPage, size = 20).getOrThrow()
                    }
                }
            } catch (e: Exception) {
                null
            }

            if (!isAdded || _binding == null) return@launch
            binding.loadingMore.isVisible = false
            isLoadingMore = false

            if (result != null) {
                allVideos.addAll(result.items)
                totalItems = result.total.toInt()
                hasMore = allVideos.size < totalItems
                currentPage++
                streamAdapter?.submitList(allVideos.toList())
                binding.recyclerStream.isVisible = true
            } else {
                Toast.makeText(requireContext(), "加载更多失败", Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun applyStream() {
        val empty = allVideos.isEmpty()
        binding.emptyFeed.isVisible = empty
        val channelName = channels.getOrElse(currentChannel) { "ALL" }
        binding.emptyFeedText.text = if (empty) "> 暂无${channelName}" else ""
        if (!empty) {
            streamAdapter?.submitList(allVideos.toList())
            binding.recyclerStream.isVisible = true
        }
    }

    private fun updateSelectionBar(count: Int) {
        binding.selectionBar.isVisible = count > 0
        binding.selectionCount.text = if (count > 0) "> 已选: $count" else ""
        val adapter = streamAdapter
        if (adapter != null && adapter.isAllSelected()) {
            binding.btnSelectAll.text = "> 取消全选"
        } else {
            binding.btnSelectAll.text = "> 全选"
        }
    }

    private fun enterBatchDeleteMode() {
        if (!isAdded) return
        lifecycleScope.launch {
            binding.loadingContainer.isVisible = true
            val typeFilter = if (currentChannel == 2) "local_image" else "!local_image"
            val result = withContext(Dispatchers.IO) {
                repository.listVideos(type = typeFilter, page = 0, size = 1000)
            }
            binding.loadingContainer.isVisible = false
            result.onSuccess { resp ->
                streamAdapter?.submitList(resp.items)
                streamAdapter?.enterSelectMode(-1)
                updateSelectionBar(0)
                binding.recyclerStream.isVisible = true
            }.onFailure {
                Toast.makeText(requireContext(), "加载失败: ${it.message}", Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun openPlayer(item: VideoItem) {
        findNavController().navigate(R.id.nav_player, Bundle().apply {
            putLong("videoId", item.id)
            putString("title", item.title)
            putString("streamUrl", item.streamUrl)
            putString("category", item.category)
            putLong("watchPosition", item.watchPosition ?: 0L)
        })
    }

    private fun openImageViewer(item: VideoItem) {
        findNavController().navigate(R.id.nav_image_viewer, Bundle().apply {
            putLong("videoId", item.id)
        })
    }

    fun onBackPressed(): Boolean {
        if (streamAdapter?.isSelectMode == true) {
            streamAdapter?.clearSelection()
            updateSelectionBar(0)
            return true
        }
        return false
    }

    override fun onDestroyView() {
        super.onDestroyView()
        streamAdapter = null
        _binding = null
    }
}
