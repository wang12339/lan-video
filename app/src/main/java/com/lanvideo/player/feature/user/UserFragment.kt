package com.lanvideo.player.feature.user

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
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
import androidx.recyclerview.widget.LinearLayoutManager
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.model.RecentWatchItem
import com.lanvideo.player.data.util.ConnectionStatusHelper
import com.lanvideo.player.data.util.toPlayerBundle
import com.lanvideo.player.databinding.FragmentUserBinding
import com.lanvideo.player.feature.user.viewmodel.UserViewModel
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

class UserFragment : Fragment() {
    private var _binding: FragmentUserBinding? = null
    private val binding get() = _binding!!
    private val viewModel: UserViewModel by viewModels()
    private var recentWatchAdapter: RecentWatchAdapter? = null

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentUserBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        ViewCompat.setOnApplyWindowInsetsListener(binding.btnUserMenu) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingStart, top, v.paddingEnd, v.paddingBottom)
            insets
        }

        binding.btnUserMenu.setOnClickListener {
            (requireActivity() as? MainActivity)?.openDrawer()
        }

        val ctx = requireContext()
        val app = requireActivity().application as MyApplication

        app.lanServerEvents.observe(viewLifecycleOwner) { viewModel.loadProfile(ctx) }

        ConnectionStatusHelper(
            statusView = binding.userConnectionStatus,
            statusDot = binding.userStatusDot,
            statusText = binding.userStatusText,
        ).observe(viewLifecycleOwner, app, lifecycleScope)

        binding.recyclerRecent.layoutManager = LinearLayoutManager(requireContext())

        binding.btnLogout.setOnClickListener {
            viewModel.logout(ctx)
            Toast.makeText(ctx, R.string.user_logged_out, Toast.LENGTH_SHORT).show()
            requireActivity().recreate()
        }

        observeViewModel()
        viewModel.loadProfile(ctx)
    }

    override fun onResume() {
        super.onResume()
        viewModel.loadProfile(requireContext())
    }

    private fun observeViewModel() {
        viewLifecycleOwner.lifecycleScope.launch {
            viewLifecycleOwner.repeatOnLifecycle(Lifecycle.State.STARTED) {
                viewModel.uiState.collectLatest { state ->
                    if (!state.isLoggedIn) {
                        binding.userName.text = getString(R.string.user_not_logged_in)
                        binding.userAvatar.text = "?"
                        binding.userRegisteredAt.isVisible = false
                        binding.cardWatched.isVisible = false
                        binding.cardWatchTime.isVisible = false
                        binding.recyclerRecent.isVisible = false
                        binding.userRecentEmpty.isVisible = false
                        binding.btnLogin.isVisible = true
                        binding.btnLogin.setOnClickListener {
                            com.lanvideo.player.ui.auth.LoginDialog().show(
                                parentFragmentManager, "login"
                            )
                        }
                        binding.btnLogout.isVisible = false
                        return@collectLatest
                    }

                    binding.userName.text = state.username ?: ""
                    binding.userAvatar.text = state.avatarLetter
                    binding.btnLogin.isVisible = false
                    binding.btnLogout.isVisible = true

                    binding.userBadge.isVisible = state.isAdmin
                    if (state.isAdmin) {
                        binding.userBadge.text = getString(R.string.user_badge_admin)
                    }

                    binding.userRegisteredAt.isVisible = state.registeredAt.isNotBlank()
                    if (state.registeredAt.isNotBlank()) {
                        binding.userRegisteredAt.text = getString(
                            R.string.user_registered_at,
                            state.registeredAt
                        )
                    }

                    binding.cardWatched.isVisible = true
                    binding.cardWatchTime.isVisible = true
                    binding.userStatWatchedCount.text = state.totalWatched.toString()
                    binding.userStatWatchTime.text = state.watchTimeText

                    if (recentWatchAdapter == null) {
                        recentWatchAdapter = RecentWatchAdapter { item -> openPlayer(item) }
                        binding.recyclerRecent.adapter = recentWatchAdapter
                    }
                    val hasRecent = state.recentHistory.isNotEmpty()
                    binding.recyclerRecent.isVisible = hasRecent
                    binding.userRecentEmpty.isVisible = !hasRecent
                    binding.userRecentEmpty.text = getString(R.string.user_recent_empty)
                    if (hasRecent) {
                        recentWatchAdapter?.submitList(state.recentHistory)
                    }

                    if (state.error != null && recentWatchAdapter?.itemCount == 0) {
                        binding.userRecentEmpty.isVisible = true
                        binding.userRecentEmpty.text = "加载失败: ${state.error}"
                    }
                }
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
            findNavController().navigate(R.id.nav_player, item.toPlayerBundle())
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
