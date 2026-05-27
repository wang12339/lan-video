package com.lanvideo.player.feature.history

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.LinearLayoutManager
import com.lanvideo.player.ConnectionState
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.model.RecentWatchItem
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.data.user.AuthSessionStore
import com.lanvideo.player.databinding.FragmentHistoryBinding
import com.lanvideo.player.feature.user.RecentWatchAdapter
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class HistoryFragment : Fragment() {

    private var _binding: FragmentHistoryBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository
    private var historyAdapter: RecentWatchAdapter? = null
    private var allHistory: List<RecentWatchItem> = emptyList()
    private val pageSize = 20

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentHistoryBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        ViewCompat.setOnApplyWindowInsetsListener(binding.btnHistoryMenu) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingStart, top, v.paddingEnd, v.paddingBottom)
            insets
        }

        binding.btnHistoryMenu.setOnClickListener {
            (requireActivity() as? MainActivity)?.openDrawer()
        }

        setupRecyclerView()
        observeConnection()
        loadHistory()
    }

    override fun onResume() {
        super.onResume()
        loadHistory()
    }

    private fun setupRecyclerView() {
        historyAdapter = RecentWatchAdapter { item -> openPlayer(item) }
        binding.recyclerHistory.layoutManager = LinearLayoutManager(requireContext())
        binding.recyclerHistory.adapter = historyAdapter
    }

    private fun observeConnection() {
        val app = requireActivity().application as MyApplication
        app.connectionState.observe(viewLifecycleOwner) { state ->
            val status = binding.historyConnectionStatus
            val dot = binding.historyStatusDot
            val text = binding.historyStatusText
            when (state) {
                ConnectionState.CONNECTED -> status.isVisible = false
                ConnectionState.SCANNING -> {
                    status.isVisible = true
                    dot.setBackgroundResource(R.drawable.bg_status_pulse)
                    (dot.background as? android.graphics.drawable.AnimationDrawable)?.start()
                    text.setText(R.string.connection_scanning)
                }
                ConnectionState.DISCONNECTED -> {
                    status.isVisible = true
                    dot.setBackgroundResource(R.drawable.status_dot_red)
                    text.setText(R.string.connection_disconnected)
                }
            }
        }
        binding.historyConnectionStatus.setOnClickListener {
            app.setConnectionState(ConnectionState.SCANNING)
            lifecycleScope.launch {
                com.lanvideo.player.data.network.LanServerDiscovery.discoverActiveNetwork(
                    requireContext().applicationContext, force = true
                )
                loadHistory()
            }
        }
    }

    private fun loadHistory() {
        lifecycleScope.launch {
            if (!isAdded || _binding == null) return@launch

            val ctx = requireContext()

            // 未登录状态
            if (!AuthSessionStore.isLoggedIn(ctx)) {
                allHistory = emptyList()
                historyAdapter?.submitList(emptyList())
                binding.recyclerHistory.isVisible = false
                binding.emptyHistory.isVisible = true
                binding.emptyHistoryText.text = "> 请先登录"
                binding.historyLoadMore.isVisible = false
                return@launch
            }

            binding.historyLoadMore.isVisible = false
            binding.emptyHistory.isVisible = false

            val result = withContext(Dispatchers.IO) {
                repository.getAllPlaybackHistory()
            }

            if (!isAdded || _binding == null) return@launch

            allHistory = result
            val empty = allHistory.isEmpty()
            binding.recyclerHistory.isVisible = !empty
            binding.emptyHistory.isVisible = empty
            binding.emptyHistoryText.text = getString(R.string.history_empty)

            if (!empty) {
                // 前端分页显示
                historyAdapter?.submitList(allHistory.take(pageSize))
            }
        }
    }

    private fun openPlayer(item: RecentWatchItem) {
        if (item.sourceType.contains("image", ignoreCase = true)) {
            findNavController().navigate(
                R.id.nav_image_viewer, Bundle().apply {
                    putLong("videoId", item.videoId)
                }
            )
        } else {
            findNavController().navigate(
                R.id.nav_player, Bundle().apply {
                    putLong("videoId", item.videoId)
                    putString("title", item.title)
                    putString("category", item.category)
                    putString("streamUrl", item.streamUrl)
                }
            )
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
