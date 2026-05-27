package com.lanvideo.player.feature.search

import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.widget.ArrayAdapter
import android.widget.ListView
import android.widget.TextView
import android.widget.Toast
import android.widget.AdapterView
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.lanvideo.player.ConnectionState
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.feature.common.FeaturedVideoAdapter
import com.lanvideo.player.data.util.ConnectionStatusHelper
import com.lanvideo.player.databinding.FragmentSearchBinding
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

class SearchFragment : Fragment() {
    private var _binding: FragmentSearchBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository
    private var searchAdapter: FeaturedVideoAdapter? = null
    private var searchJob: Job? = null
    private var currentQuery: String = ""
    private var currentPage: Int = 0
    private var totalItems: Long = 0
    private var isLoadingMore: Boolean = false
    private val allResults = mutableListOf<com.lanvideo.player.data.model.VideoItem>()

    // Search history & suggestions
    private lateinit var searchHistory: SearchHistory
    private var suggestionsAdapter: ArrayAdapter<String>? = null
    private var isShowingHistory: Boolean = false

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

        searchHistory = SearchHistory.getInstance(requireContext())

        binding.btnSearchMenu.setOnClickListener {
            (requireActivity() as? MainActivity)?.openDrawer()
        }

        // Setup suggestions ListView
        suggestionsAdapter = object : ArrayAdapter<String>(
            requireContext(),
            android.R.layout.simple_list_item_1,
            mutableListOf()
        ) {
            override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
                val view = super.getView(position, convertView, parent)
                if (view is TextView) {
                    view.setTextColor(ContextCompat.getColor(context, R.color.text_primary))
                    view.setBackgroundResource(R.drawable.bg_suggestion_item)
                    view.setPadding(32, 16, 32, 16)
                    view.textSize = 14f
                    view.typeface = android.graphics.Typeface.MONOSPACE
                }
                return view
            }
        }
        binding.recyclerSearchSuggestions.adapter = suggestionsAdapter
        binding.recyclerSearchSuggestions.onItemClickListener = AdapterView.OnItemClickListener { _, _, position, _ ->
            val text = suggestionsAdapter?.getItem(position) ?: return@OnItemClickListener
            binding.inputSearch.setText(text)
            binding.inputSearch.setSelection(text.length)
            currentQuery = text
            searchHistory.addSearch(text)
            hideSuggestions()
            performSearch()
        }
        binding.recyclerSearchSuggestions.onItemLongClickListener = AdapterView.OnItemLongClickListener { _, _, position, _ ->
            val text = suggestionsAdapter?.getItem(position) ?: return@OnItemLongClickListener false
            searchHistory.removeSearch(text)
            val q = binding.inputSearch.text?.toString()?.trim().orEmpty()
            if (q.isBlank()) {
                showSearchHistory()
            } else {
                showSuggestions(q)
            }
            Toast.makeText(requireContext(), "已删除「${text}」", Toast.LENGTH_SHORT).show()
            true
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
        ConnectionStatusHelper(
            statusView = binding.searchConnectionStatus,
            statusDot = binding.searchStatusDot,
            statusText = binding.searchStatusText,
        ).observe(viewLifecycleOwner, app, lifecycleScope)

        // --- Search input listeners ---

        binding.inputSearch.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_SEARCH) {
                val query = binding.inputSearch.text?.toString()?.trim().orEmpty()
                if (query.isNotBlank()) {
                    currentQuery = query
                    searchHistory.addSearch(query)
                    hideSuggestions()
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
                    if (q.isBlank()) {
                        // Show history when input is empty
                        showSearchHistory()
                    } else {
                        // Show suggestions (fuzzy match from history)
                        showSuggestions(q)
                    }
                    performSearch()
                }
            }
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
        })

        // Show search history when input field gains focus and is empty
        binding.inputSearch.setOnFocusChangeListener { _, hasFocus ->
            if (hasFocus) {
                val q = binding.inputSearch.text?.toString()?.trim().orEmpty()
                if (q.isBlank()) {
                    showSearchHistory()
                }
            } else {
                hideSuggestions()
            }
        }

        // Clear history button
        binding.btnClearSearchHistory.setOnClickListener {
            searchHistory.clearHistory()
            hideSuggestions()
            Toast.makeText(requireContext(), "搜索历史已清除", Toast.LENGTH_SHORT).show()
        }

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
                searchAdapter?.highlightQuery = null
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
                searchAdapter?.highlightQuery = currentQuery
                searchAdapter?.submitList(allResults.toList())
                binding.emptySearch.isVisible = allResults.isEmpty()
                if (allResults.isEmpty()) {
                    binding.emptySearchText.text = getString(R.string.search_no_results, currentQuery)
                    binding.textResultCount.isVisible = false
                } else {
                    binding.textResultCount.isVisible = true
                    binding.textResultCount.text = "找到 ${resp.total} 个结果"
                }
                binding.searchLoadMore.isVisible = false
            }.onFailure { err ->
                searchAdapter?.submitList(emptyList())
                searchAdapter?.highlightQuery = currentQuery
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

    private fun showSearchHistory() {
        val history = searchHistory.getHistory()
        if (history.isEmpty()) {
            hideSuggestions()
            return
        }
        isShowingHistory = true
        suggestionsAdapter?.clear()
        suggestionsAdapter?.addAll(history)
        suggestionsAdapter?.notifyDataSetChanged()
        binding.recyclerSearchSuggestions.isVisible = true
        binding.btnClearSearchHistory.isVisible = true
    }

    private fun showSuggestions(query: String) {
        val history = searchHistory.getHistory()
        val matches = history.filter { it.contains(query, ignoreCase = true) }
        if (matches.isEmpty()) {
            hideSuggestions()
            return
        }
        isShowingHistory = false
        suggestionsAdapter?.clear()
        suggestionsAdapter?.addAll(matches)
        suggestionsAdapter?.notifyDataSetChanged()
        binding.recyclerSearchSuggestions.isVisible = true
        binding.btnClearSearchHistory.isVisible = false
    }

    private fun hideSuggestions() {
        binding.recyclerSearchSuggestions.isVisible = false
        binding.btnClearSearchHistory.isVisible = false
        suggestionsAdapter?.clear()
        suggestionsAdapter?.notifyDataSetChanged()
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
