package com.lanvideo.player.feature.search

import android.content.Context
import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.ArrayAdapter
import android.widget.Toast
import android.widget.AdapterView
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
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.util.ConnectionStatusHelper
import com.lanvideo.player.data.util.toImageViewerBundle
import com.lanvideo.player.data.util.toPlayerBundle
import com.lanvideo.player.databinding.FragmentSearchBinding
import com.lanvideo.player.feature.common.FeaturedVideoAdapter
import com.lanvideo.player.feature.search.viewmodel.SearchViewModel
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

class SearchFragment : Fragment() {
    private var _binding: FragmentSearchBinding? = null
    private val binding get() = _binding!!
    private val viewModel: SearchViewModel by viewModels()
    private var searchAdapter: FeaturedVideoAdapter? = null
    private var searchHistory: SearchHistory? = null
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

        setupSuggestions()
        setupSearchResults()
        setupConnectionObserver()

        // ── Search input listeners ──
        binding.inputSearch.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_SEARCH) {
                val query = binding.inputSearch.text?.toString()?.trim().orEmpty()
                if (query.isNotBlank()) {
                    searchHistory?.addSearch(query)
                    hideSuggestions()
                    viewModel.search(query)
                }
                true
            } else false
        }

        binding.inputSearch.addTextChangedListener(object : TextWatcher {
            override fun afterTextChanged(s: Editable?) {
                val q = s?.toString()?.trim().orEmpty()
                if (q.isBlank()) {
                    showSearchHistory()
                } else {
                    showSuggestions(q)
                }
                viewModel.search(q)
            }
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
        })

        binding.inputSearch.setOnFocusChangeListener { _, hasFocus ->
            if (hasFocus) {
                val q = binding.inputSearch.text?.toString()?.trim().orEmpty()
                if (q.isBlank()) showSearchHistory()
            } else {
                hideSuggestions()
            }
        }

        binding.btnClearSearchHistory.setOnClickListener {
            searchHistory?.clearHistory()
            hideSuggestions()
            Toast.makeText(requireContext(), "搜索历史已清除", Toast.LENGTH_SHORT).show()
        }

        observeViewModel()

        // 自动聚焦
        binding.inputSearch.requestFocus()
        binding.inputSearch.postDelayed({
            val imm = requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
            imm?.showSoftInput(binding.inputSearch, InputMethodManager.SHOW_IMPLICIT)
        }, 200)
    }

    private fun observeViewModel() {
        viewLifecycleOwner.lifecycleScope.launch {
            viewLifecycleOwner.repeatOnLifecycle(Lifecycle.State.STARTED) {
                viewModel.uiState.collectLatest { state ->
                    searchAdapter?.highlightQuery = state.query
                    searchAdapter?.submitList(state.results)
                    binding.emptySearch.isVisible = state.results.isEmpty() && !state.isLoading
                    binding.searchLoadMore.isVisible = state.isLoadingMore

                    if (state.query.isBlank()) {
                        binding.emptySearchText.text = getString(R.string.search_empty_hint)
                        binding.textResultCount.isVisible = false
                    } else if (state.results.isEmpty() && !state.isLoading) {
                        binding.emptySearchText.text = state.error ?: getString(R.string.search_no_results, state.query)
                        binding.textResultCount.isVisible = false
                    } else {
                        binding.textResultCount.isVisible = state.totalFound > 0
                        if (state.totalFound > 0) {
                            binding.textResultCount.text = "找到 ${state.totalFound} 个结果"
                        }
                    }
                }
            }
        }
    }

    private fun setupSuggestions() {
        suggestionsAdapter = object : ArrayAdapter<String>(
            requireContext(),
            R.layout.item_search_suggestion,
            R.id.suggestion_text,
            mutableListOf()
        ) {
            override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
                val view = super.getView(position, convertView, parent)
                val textView = view.findViewById<android.widget.TextView>(R.id.suggestion_text)
                textView.typeface = android.graphics.Typeface.DEFAULT
                val deleteBtn = view.findViewById<android.widget.TextView>(R.id.btn_suggestion_delete)
                deleteBtn.isVisible = true
                deleteBtn.setOnClickListener {
                    val text = getItem(position) ?: return@setOnClickListener
                    searchHistory?.removeSearch(text)
                    val q = binding.inputSearch.text?.toString()?.trim().orEmpty()
                    if (q.isBlank()) showSearchHistory() else showSuggestions(q)
                    Toast.makeText(context, "已删除「${text}」", Toast.LENGTH_SHORT).show()
                }
                return view
            }
        }
        binding.recyclerSearchSuggestions.adapter = suggestionsAdapter
        binding.recyclerSearchSuggestions.onItemClickListener = AdapterView.OnItemClickListener { _, _, position, _ ->
            val text = suggestionsAdapter?.getItem(position) ?: return@OnItemClickListener
            binding.inputSearch.setText(text)
            binding.inputSearch.setSelection(text.length)
            searchHistory?.addSearch(text)
            hideSuggestions()
            viewModel.search(text)
        }
        binding.recyclerSearchSuggestions.onItemLongClickListener = AdapterView.OnItemLongClickListener { _, _, position, _ ->
            val text = suggestionsAdapter?.getItem(position) ?: return@OnItemLongClickListener false
            searchHistory?.removeSearch(text)
            val q = binding.inputSearch.text?.toString()?.trim().orEmpty()
            if (q.isBlank()) showSearchHistory() else showSuggestions(q)
            Toast.makeText(requireContext(), "已删除「${text}」", Toast.LENGTH_SHORT).show()
            true
        }
    }

    private fun setupSearchResults() {
        searchAdapter = FeaturedVideoAdapter(
            onClick = { item ->
                if (item.sourceType.contains("image", ignoreCase = true)) {
                    findNavController().navigate(R.id.nav_image_viewer, item.toImageViewerBundle())
                } else {
                    findNavController().navigate(R.id.nav_player, item.toPlayerBundle())
                }
            }
        )
        val layoutManager = GridLayoutManager(requireContext(), 2)
        binding.recyclerSearchResults.layoutManager = layoutManager
        binding.recyclerSearchResults.adapter = searchAdapter

        binding.recyclerSearchResults.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrolled(recyclerView: RecyclerView, dx: Int, dy: Int) {
                if (dy <= 0) return
                val totalItemCount = layoutManager.itemCount
                val lastVisibleItem = layoutManager.findLastVisibleItemPosition()
                if (lastVisibleItem >= totalItemCount - 4) {
                    viewModel.loadNextPage()
                }
            }
        })
    }

    private fun setupConnectionObserver() {
        val app = requireActivity().application as MyApplication
        ConnectionStatusHelper(
            statusView = binding.searchConnectionStatus,
            statusDot = binding.searchStatusDot,
            statusText = binding.searchStatusText,
        ).observe(viewLifecycleOwner, app, lifecycleScope)
    }

    private fun showSearchHistory() {
        val history = searchHistory?.getHistory().orEmpty()
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
        val history = searchHistory?.getHistory().orEmpty()
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
        searchAdapter = null
        _binding = null
        super.onDestroyView()
    }
}
