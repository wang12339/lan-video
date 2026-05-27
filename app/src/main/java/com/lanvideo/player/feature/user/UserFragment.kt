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
import androidx.lifecycle.lifecycleScope
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.LinearLayoutManager
import com.lanvideo.player.ConnectionState
import com.lanvideo.player.MainActivity
import com.lanvideo.player.MyApplication
import com.lanvideo.player.R
import com.lanvideo.player.data.model.RecentWatchItem
import com.lanvideo.player.data.network.NetworkModule
import com.lanvideo.player.data.user.AuthSessionStore
import com.lanvideo.player.data.util.ConnectionStatusHelper
import com.lanvideo.player.databinding.FragmentUserBinding
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class UserFragment : Fragment() {
    private var _binding: FragmentUserBinding? = null
    private val binding get() = _binding!!
    private var recentWatchAdapter: RecentWatchAdapter? = null
    private var dataLoaded = false

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

        // 登录成功或服务器切换时刷新
        app.lanServerEvents.observe(viewLifecycleOwner) { loadUserProfile() }

        ConnectionStatusHelper(
            statusView = binding.userConnectionStatus,
            statusDot = binding.userStatusDot,
            statusText = binding.userStatusText,
        ).observe(viewLifecycleOwner, app, lifecycleScope)

        binding.recyclerRecent.layoutManager = LinearLayoutManager(requireContext())

        binding.btnLogout.setOnClickListener {
            lifecycleScope.launch {
                try {
                    withContext(Dispatchers.IO) {
                        val api = NetworkModule.createApi()
                        runCatching { api.logout() }
                    }
                } catch (_: Exception) { }
                AuthSessionStore.clear(ctx)
                Toast.makeText(ctx, R.string.user_logged_out, Toast.LENGTH_SHORT).show()
                requireActivity().recreate()
            }
        }

        loadUserProfile()
    }

    override fun onResume() {
        super.onResume()
        loadUserProfile()
    }

    private fun loadUserProfile() {
        lifecycleScope.launch {
            val ctx = requireContext()
            val username = AuthSessionStore.getUsername(ctx)

            if (username == null) {
                // 未登录状态
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
                return@launch
            }

            // 已登录：先显示用户名
            binding.userName.text = username
            binding.userAvatar.text = username.firstOrNull()?.uppercase() ?: "U"
            binding.btnLogin.isVisible = false
            binding.btnLogout.isVisible = true

            // 从服务器加载完整信息
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    val api = NetworkModule.createApi()
                    api.getUserProfile()
                }
            }

            dataLoaded = true

            result.onSuccess { profile ->
                binding.userBadge.isVisible = profile.isAdmin
                if (profile.isAdmin) {
                    binding.userBadge.text = getString(R.string.user_badge_admin)
                }

                binding.userRegisteredAt.isVisible = profile.createdAt.isNotBlank()
                if (profile.createdAt.isNotBlank()) {
                    binding.userRegisteredAt.text = getString(
                        R.string.user_registered_at,
                        profile.createdAt.take(10)
                    )
                }

                // 统计卡片
                binding.cardWatched.isVisible = true
                binding.cardWatchTime.isVisible = true
                binding.userStatWatchedCount.text = profile.totalVideosWatched.toString()

                val hours = profile.totalWatchTimeMs / 3600000f
                binding.userStatWatchTime.text = if (hours >= 1f) {
                    String.format("%.1f 小时", hours)
                } else {
                    val minutes = profile.totalWatchTimeMs / 60000
                    "${minutes}分钟"
                }

                // 最近播放
                if (recentWatchAdapter == null) {
                    recentWatchAdapter = RecentWatchAdapter { item -> openPlayer(item) }
                    binding.recyclerRecent.adapter = recentWatchAdapter
                }
                binding.recyclerRecent.isVisible = profile.recentHistory.isNotEmpty()
                binding.userRecentEmpty.isVisible = profile.recentHistory.isEmpty()
                binding.userRecentEmpty.text = getString(R.string.user_recent_empty)
                recentWatchAdapter?.submitList(profile.recentHistory)
            }.onFailure { e ->
                // API 失败时不清空已有的数据，只追加提示
                if (recentWatchAdapter == null || recentWatchAdapter?.itemCount == 0) {
                    binding.userRecentEmpty.isVisible = true
                    binding.userRecentEmpty.text = "加载失败: ${e.message?.take(40) ?: "未知错误"}"
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
            findNavController().navigate(
                R.id.nav_player, Bundle().apply {
                    putLong("videoId", item.videoId)
                    putString("title", item.title)
                    putString("category", item.category)
                    putString("streamUrl", item.streamUrl)
                    putLong("watchPosition", item.positionMs)
                }
            )
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
