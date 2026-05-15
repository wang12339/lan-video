package com.example.myapplication.ui.settings

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
import com.example.myapplication.ConnectionState
import com.example.myapplication.MainActivity
import com.example.myapplication.MyApplication
import com.example.myapplication.R
import com.example.myapplication.data.network.LanServerDiscovery
import com.example.myapplication.data.network.NetworkModule
import com.example.myapplication.data.network.ServerConfigStore
import com.example.myapplication.databinding.FragmentSettingsBinding
import kotlinx.coroutines.launch

class SettingsFragment : Fragment() {
    private var _binding: FragmentSettingsBinding? = null
    private val binding get() = _binding!!

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentSettingsBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        applyInsets()
        binding.btnSettingsMenu.setOnClickListener {
            (requireActivity() as? MainActivity)?.openDrawer()
        }

        val ctx = requireContext()
        val app = requireActivity().application as MyApplication
        app.connectionState.observe(viewLifecycleOwner) { state ->
            val status = binding.settingsConnectionStatus
            val dot = binding.settingsStatusDot
            val text = binding.settingsStatusText
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
        binding.settingsConnectionStatus.setOnClickListener {
            app.setConnectionState(ConnectionState.SCANNING)
            lifecycleScope.launch {
                LanServerDiscovery.discoverActiveNetwork(
                    ctx.applicationContext, force = true
                )
            }
        }

        val rawUrl = ServerConfigStore.loadBaseUrl(ctx) ?: NetworkModule.getBaseUrl()
        binding.inputServerUrl.setText(rawUrl)
        binding.inputAdminToken.setText(ServerConfigStore.loadAdminToken(ctx) ?: "")

        binding.btnSaveServer.setOnClickListener {
            val url = binding.inputServerUrl.text?.toString()?.trim().orEmpty()
            if (url.isBlank()) {
                Toast.makeText(ctx, "请输入服务器地址", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            if (!url.startsWith("http://") && !url.startsWith("https://")) {
                Toast.makeText(ctx, "地址必须以 http:// 或 https:// 开头", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val hostPart = url.removePrefix("http://").removePrefix("https://").split("/").first()
            if (hostPart.isBlank() || (!hostPart.contains(".") && hostPart != "localhost")) {
                Toast.makeText(ctx, "地址格式不正确，例如 http://192.168.1.100:8082", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val normalized = if (url.endsWith("/")) url else "$url/"
            NetworkModule.updateBaseUrl(normalized, ctx, notify = true)
            Toast.makeText(ctx, "已保存，正在重新连接", Toast.LENGTH_SHORT).show()
        }

        binding.btnSaveToken.setOnClickListener {
            val token = binding.inputAdminToken.text?.toString()?.trim().orEmpty()
            ServerConfigStore.saveAdminToken(ctx, token)
            Toast.makeText(ctx, "Admin Token 已保存", Toast.LENGTH_SHORT).show()
        }
    }

    private fun applyInsets() {
        ViewCompat.setOnApplyWindowInsetsListener(binding.btnSettingsMenu) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingStart, top, v.paddingEnd, v.paddingBottom)
            insets
        }
        ViewCompat.setOnApplyWindowInsetsListener(binding.scrollSettings) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.statusBars() or WindowInsetsCompat.Type.navigationBars())
            v.setPadding(v.paddingStart, bars.top, v.paddingEnd, bars.bottom)
            insets
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
