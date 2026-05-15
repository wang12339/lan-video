package com.example.myapplication.feature.home

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
import com.example.myapplication.ConnectionState
import com.example.myapplication.MyApplication
import com.example.myapplication.R
import com.example.myapplication.data.network.LanServerDiscovery
import com.example.myapplication.data.model.VideoItem
import com.example.myapplication.data.model.PagedVideoResponse
import com.example.myapplication.data.repository.VideoRepository
import com.example.myapplication.databinding.FragmentHomeBinding
import com.example.myapplication.feature.common.DataStreamAdapter
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout

class HomeFragment : Fragment() {

    private var _binding: FragmentHomeBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository.getInstance()
    private var streamAdapter: DataStreamAdapter? = null
    private var allVideos: List<VideoItem> = emptyList()
    private var currentChannel = 0 // 0=all, 1=video, 2=image
    private val channels = listOf("ALL", "VIDEO", "IMAGE")
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

        binding.btnDeleteSelected.setOnClickListener {
            val ids = streamAdapter?.selectedIds?.toList() ?: return@setOnClickListener
            if (ids.isEmpty()) return@setOnClickListener
            lifecycleScope.launch {
                val result = repository.deleteVideos(ids)
                if (result.isSuccess) {
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
        app.connectionState.observe(viewLifecycleOwner) { state -> updateConnectionStatus(state) }
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

            allVideos = emptyList()
            streamAdapter?.submitList(emptyList())
            binding.recyclerStream.isVisible = false
            binding.loadingContainer.isVisible = true

            val typeFilter = when (currentChannel) {
                1 -> "!local_image"
                2 -> "local_image"
                else -> "" // all
            }
            val pageSize = if (currentChannel == 2) 1000 else 200
            val app = requireActivity().application as MyApplication

            val pagedResponse = try {
                withTimeout(5_000L) {
                    if (typeFilter.isNotEmpty()) {
                        repository.listVideos(type = typeFilter, page = 0, size = pageSize)
                    } else {
                        val vids = repository.listVideos(type = "!local_image", page = 0, size = 200)
                        val imgs = repository.listVideos(type = "local_image", page = 0, size = 100)
                        val allItems = (vids.getOrNull()?.items ?: emptyList()) +
                            (imgs.getOrNull()?.items ?: emptyList())
                        Result.success(com.example.myapplication.data.model.PagedVideoResponse(allItems, allItems.size.toLong(), 0, allItems.size))
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
                    withTimeout(5_000L) { repository.listVideos(type = typeFilter, page = 0, size = pageSize) }
                }.getOrElse { Result.failure(java.io.IOException("连接超时")) }
            }
            if (!isAdded || _binding == null) return@launch

            binding.loadingContainer.isVisible = false
            finalResult.onSuccess { resp ->
                allVideos = resp.items
                applyStream()
            }.onFailure { err ->
                app.setConnectionState(ConnectionState.DISCONNECTED)
                binding.emptyFeedText.text = "> LOAD FAILED: ${err.message?.take(60) ?: "未知错误"}"
                binding.emptyFeed.isVisible = true
            }
        }
    }

    private fun applyStream() {
        val empty = allVideos.isEmpty()
        binding.emptyFeed.isVisible = empty
        val channelName = channels.getOrElse(currentChannel) { "ALL" }
        binding.emptyFeedText.text = if (empty) "> NO ${channelName} MEDIA FOUND" else ""
        if (!empty) {
            streamAdapter?.submitList(allVideos)
            binding.recyclerStream.isVisible = true
        }
    }

    private fun updateConnectionStatus(state: ConnectionState) {
        when (state) {
            ConnectionState.CONNECTED -> binding.connectionStatus.isVisible = false
            ConnectionState.SCANNING -> {
                binding.connectionStatus.isVisible = true
                binding.statusDot.setBackgroundResource(R.drawable.bg_status_pulse)
                (binding.statusDot.background as? android.graphics.drawable.AnimationDrawable)?.start()
                binding.statusText.setText(R.string.connection_scanning)
            }
            ConnectionState.DISCONNECTED -> {
                binding.connectionStatus.isVisible = true
                binding.statusDot.setBackgroundResource(R.drawable.status_dot_red)
                binding.statusText.setText(R.string.connection_disconnected)
            }
        }
    }

    private fun updateSelectionBar(count: Int) {
        binding.selectionBar.isVisible = count > 0
        binding.selectionCount.text = if (count > 0) "> SELECTED: $count" else ""
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
