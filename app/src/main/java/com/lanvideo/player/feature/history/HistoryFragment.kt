package com.lanvideo.player.feature.history

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.fragment.app.viewModels
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.util.ConnectionStatusHelper
import com.lanvideo.player.data.util.toPlayerBundle
import com.lanvideo.player.databinding.FragmentHistoryBinding
import com.lanvideo.player.feature.history.viewmodel.HistoryViewModel
import com.lanvideo.player.feature.user.RecentWatchAdapter
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

class HistoryFragment : Fragment() {

    private var _binding: FragmentHistoryBinding? = null
    private val binding get() = _binding!!
    private val viewModel: HistoryViewModel by viewModels()
    private var historyAdapter: RecentWatchAdapter? = null

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
        observeViewModel()

        viewModel.loadHistory(requireContext())
    }

    override fun onResume() {
        super.onResume()
        if (viewModel.uiState.value.history.isEmpty()) {
            viewModel.loadHistory(requireContext())
        }
    }

    private fun observeViewModel() {
        viewLifecycleOwner.lifecycleScope.launch {
            viewLifecycleOwner.repeatOnLifecycle(Lifecycle.State.STARTED) {
                viewModel.uiState.collectLatest { state ->
                    historyAdapter?.submitList(state.history)
                    binding.recyclerHistory.isVisible = state.history.isNotEmpty()
                    binding.emptyHistory.isVisible = state.history.isEmpty()
                    binding.historyLoadMore.isVisible = state.isLoadingMore

                    if (state.history.isEmpty()) {
                        binding.emptyHistoryText.text = state.error ?: getString(R.string.history_empty)
                    }
                }
            }
        }
    }

    private fun setupRecyclerView() {
        historyAdapter = RecentWatchAdapter { item -> openPlayer(item) }
        binding.recyclerHistory.layoutManager = LinearLayoutManager(requireContext())
        binding.recyclerHistory.adapter = historyAdapter

        binding.recyclerHistory.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrolled(recyclerView: RecyclerView, dx: Int, dy: Int) {
                super.onScrolled(recyclerView, dx, dy)
                val lm = recyclerView.layoutManager as? LinearLayoutManager ?: return
                val visibleItemCount = lm.childCount
                val totalItemCount = lm.itemCount
                val firstVisibleItemPosition = lm.findFirstVisibleItemPosition()
                val state = viewModel.uiState.value
                if (!state.isLoadingMore && state.hasMore
                    && (visibleItemCount + firstVisibleItemPosition) >= totalItemCount
                    && firstVisibleItemPosition >= 0) {
                    viewModel.loadMore()
                }
            }
        })
    }

    private fun observeConnection() {
        val app = requireActivity().application as MyApplication
        ConnectionStatusHelper(
            statusView = binding.historyConnectionStatus,
            statusDot = binding.historyStatusDot,
            statusText = binding.historyStatusText,
        ).observe(viewLifecycleOwner, app, lifecycleScope)
    }

    private fun openPlayer(item: com.lanvideo.player.data.model.RecentWatchItem) {
        if (item.sourceType.contains("image", ignoreCase = true)) {
            findNavController().navigate(
                R.id.nav_image_viewer, Bundle().apply {
                    putLong("videoId", item.videoId)
                }
            )
        } else {
            findNavController().navigate(R.id.nav_player, item.toPlayerBundle())
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
