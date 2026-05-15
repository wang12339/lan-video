package com.example.myapplication.feature.search

import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.example.myapplication.ConnectionState
import com.example.myapplication.MainActivity
import com.example.myapplication.MyApplication
import com.example.myapplication.R
import com.example.myapplication.data.repository.VideoRepository
import com.example.myapplication.feature.common.FeaturedVideoAdapter
import com.example.myapplication.databinding.FragmentSearchBinding
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

class SearchFragment : Fragment() {
    private var _binding: FragmentSearchBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository.getInstance()
    private var searchAdapter: FeaturedVideoAdapter? = null
    private var searchJob: Job? = null
    private var currentQuery: String = ""
    private var currentPage: Int = 0
    private var totalItems: Long = 0
    private var isLoadingMore: Boolean = false
    private val allResults = mutableListOf<com.example.myapplication.data.model.VideoItem>()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentSearchBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        applyInsets()

        binding.btnSearchMenu.setOnClickListener {
            (requireActivity() as? MainActivity)?.openDrawer()
        }

        searchAdapter = FeaturedVideoAdapter(
            onClick = { item ->
                if (item.sourceType.contains("image", ignoreCase = true)) {
                    findNavController().navigate(R.id.nav_image_viewer, Bundle().apply {
                        putLong("videoId", item.id)
                    })
                } else {
                    findNavController().navigate(R.id.nav_player, Bundle().apply {
                        putLong("videoId", item.id)
                        putString("title", item.title)
                        putString("streamUrl", item.streamUrl)
                        putString("category", item.category)
                    })
                }
            }
        )
        val layoutManager = GridLayoutManager(requireContext(), 2)
        binding.recyclerSearchResults.layoutManager = layoutManager
        binding.recyclerSearchResults.adapter = searchAdapter

        binding.recyclerSearchResults.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrolled(recyclerView: RecyclerView, dx: Int, dy: Int) {
                if (dy <= 0 || isLoadingMore) return
                val totalItemCount = layoutManager.itemCount
                val lastVisibleItem = layoutManager.findLastVisibleItemPosition()
                if (lastVisibleItem >= totalItemCount - 4) {
                    loadNextPage()
                }
            }
        })

        val app = requireActivity().application as MyApplication
        app.connectionState.observe(viewLifecycleOwner) { state ->
            updateConnectionStatus(state)
        }
        binding.searchConnectionStatus.setOnClickListener {
            app.setConnectionState(ConnectionState.SCANNING)
            lifecycleScope.launch {
                com.example.myapplication.data.network.LanServerDiscovery.discoverActiveNetwork(
                    requireContext().applicationContext, force = true
                )
            }
        }

        binding.inputSearch.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_SEARCH) {
                val query = binding.inputSearch.text?.toString()?.trim().orEmpty()
                if (query != currentQuery) {
                    currentQuery = query
                    performSearch()
                }
                true
            } else false
        }
        binding.inputSearch.addTextChangedListener(object : TextWatcher {
            override fun afterTextChanged(s: Editable?) {
                val q = s?.toString()?.trim().orEmpty()
                if (q != currentQuery) {
                    currentQuery = q
                    performSearch()
                }
            }
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
        })
    }

    private fun performSearch() {
        searchJob?.cancel()
        allResults.clear()
        currentPage = 0
        totalItems = 0
        isLoadingMore = false
        searchJob = lifecycleScope.launch {
            if (currentQuery.isBlank()) {
                searchAdapter?.submitList(emptyList())
                binding.emptySearch.isVisible = true
                binding.emptySearchText.text = getString(R.string.search_empty_hint)
                binding.searchLoadMore.isVisible = false
                return@launch
            }
            delay(300)
            binding.searchLoadMore.isVisible = false
            val result = repository.listVideos(query = currentQuery, page = 0, size = 20)
            result.onSuccess { resp ->
                allResults.clear()
                allResults.addAll(resp.items)
                totalItems = resp.total
                currentPage = 0
                searchAdapter?.submitList(allResults.toList())
                binding.emptySearch.isVisible = allResults.isEmpty()
                if (allResults.isEmpty()) {
                    binding.emptySearchText.text = getString(R.string.search_no_results, currentQuery)
                    binding.textResultCount.isVisible = false
                } else {
                    binding.textResultCount.isVisible = true
                    binding.textResultCount.text = "FOUND ${resp.total} RESULTS"
                }
                binding.searchLoadMore.isVisible = false
            }.onFailure { err ->
                searchAdapter?.submitList(emptyList())
                binding.emptySearch.isVisible = true
                binding.searchLoadMore.isVisible = false
                binding.emptySearchText.text = getString(R.string.search_error, err.message ?: "未知错误")
                binding.textResultCount.isVisible = false
            }
        }
    }

    private fun loadNextPage() {
        if (isLoadingMore) return
        val loaded = allResults.size.toLong()
        if (loaded >= totalItems) return
        isLoadingMore = true
        binding.searchLoadMore.isVisible = true
        lifecycleScope.launch {
            val nextPage = currentPage + 1
            repository.listVideos(query = currentQuery, page = nextPage, size = 20)
                .onSuccess { resp ->
                    allResults.addAll(resp.items)
                    totalItems = resp.total
                    currentPage = nextPage
                    searchAdapter?.submitList(allResults.toList())
                    binding.emptySearch.isVisible = false
                }
                .onFailure {
                    // silently fail for load more
                }
            binding.searchLoadMore.isVisible = false
            isLoadingMore = false
        }
    }

    private fun updateConnectionStatus(state: ConnectionState) {
        val status = binding.searchConnectionStatus
        val dot = binding.searchStatusDot
        val text = binding.searchStatusText
        when (state) {
            ConnectionState.CONNECTED -> {
                status.isVisible = false
            }
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

    private fun applyInsets() {
        ViewCompat.setOnApplyWindowInsetsListener(binding.btnSearchMenu) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingStart, top, v.paddingEnd, v.paddingBottom)
            insets
        }
        ViewCompat.setOnApplyWindowInsetsListener(binding.layoutSearch) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingStart, top, v.paddingEnd, v.paddingBottom)
            insets
        }
        ViewCompat.setOnApplyWindowInsetsListener(binding.recyclerSearchResults) { v, insets ->
            val navBar = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
            v.setPadding(v.paddingStart, v.paddingTop, v.paddingEnd, navBar.bottom)
            insets
        }
    }

    override fun onDestroyView() {
        searchJob?.cancel()
        searchAdapter = null
        _binding = null
        super.onDestroyView()
    }
}
