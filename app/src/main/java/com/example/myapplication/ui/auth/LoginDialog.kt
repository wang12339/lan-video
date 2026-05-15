package com.example.myapplication.ui.auth

import android.app.Dialog
import android.graphics.Color
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.Toast
import androidx.core.view.isVisible
import androidx.fragment.app.DialogFragment
import androidx.lifecycle.lifecycleScope
import com.example.myapplication.data.model.LoginRequest
import com.example.myapplication.data.model.RegisterRequest
import com.example.myapplication.data.network.NetworkModule
import com.example.myapplication.data.user.AuthSessionStore
import com.example.myapplication.databinding.DialogLoginBinding
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class LoginDialog : DialogFragment() {
    private var _binding: DialogLoginBinding? = null
    private val binding get() = _binding!!

    private var isRegisterMode = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setStyle(STYLE_NO_TITLE, android.R.style.Theme_Black_NoTitleBar_Fullscreen)
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DialogLoginBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onStart() {
        super.onStart()
        val dialog = dialog ?: return
        dialog.setCancelable(false)
        dialog.setCanceledOnTouchOutside(false)
        dialog.window?.apply {
            setBackgroundDrawable(android.graphics.drawable.ColorDrawable(Color.TRANSPARENT))
            setDimAmount(0.85f)
            setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE)
            setLayout(
                WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.WRAP_CONTENT
            )
        }
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        view.post {
            binding.inputUsername.requestFocus()
        }

        binding.inputPassword.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == android.view.inputmethod.EditorInfo.IME_ACTION_DONE) {
                submit()
                true
            } else false
        }

        binding.inputConfirm.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == android.view.inputmethod.EditorInfo.IME_ACTION_DONE) {
                submit()
                true
            } else false
        }

        binding.btnSubmit.setOnClickListener { submit() }
        binding.btnToggleMode.setOnClickListener { toggleMode() }

        binding.btnServerConfig.setOnClickListener {
            val ctx = context ?: return@setOnClickListener
            val input = android.widget.EditText(ctx).apply {
                setText(NetworkModule.getBaseUrl())
                setSingleLine()
                setSelection(text.length)
            }
            android.app.AlertDialog.Builder(ctx)
                .setTitle("服务器地址")
                .setMessage("输入视频服务器的完整地址\n例如: http://192.168.1.100:8082")
                .setView(input)
                .setPositiveButton("保存") { _, _ ->
                    val url = input.text.toString().trim()
                    if (url.isNotBlank()) {
                        NetworkModule.updateBaseUrl(url, ctx)
                    }
                }
                .setNegativeButton("取消", null)
                .show()
        }
    }

    private fun toggleMode() {
        isRegisterMode = !isRegisterMode
        if (isRegisterMode) {
            binding.loginTitle.text = getString(com.example.myapplication.R.string.login_btn_register)
            binding.loginSubtitle.setText(com.example.myapplication.R.string.login_subtitle)
            binding.inputConfirmLayout.isVisible = true
            binding.btnSubmit.text = getString(com.example.myapplication.R.string.login_btn_register)
            binding.btnToggleMode.text = getString(com.example.myapplication.R.string.login_toggle_login)
        } else {
            binding.loginTitle.text = getString(com.example.myapplication.R.string.login_title)
            binding.loginSubtitle.setText(com.example.myapplication.R.string.login_subtitle)
            binding.inputConfirmLayout.isVisible = false
            binding.btnSubmit.text = getString(com.example.myapplication.R.string.login_btn_login)
            binding.btnToggleMode.text = getString(com.example.myapplication.R.string.login_toggle_register)
        }
        binding.textError.isVisible = false
    }

    private fun submit() {
        val username = binding.inputUsername.text?.toString().orEmpty().trim()
        val password = binding.inputPassword.text?.toString().orEmpty()

        if (username.isBlank()) {
            showError("请输入用户名")
            return
        }
        if (password.isBlank()) {
            showError("请输入密码")
            return
        }
        if (password.length < 4) {
            showError("密码至少需要 4 个字符")
            return
        }

        if (isRegisterMode) {
            val confirm = binding.inputConfirm.text?.toString().orEmpty()
            if (confirm != password) {
                showError("两次输入的密码不一致")
                return
            }
        }

        binding.btnSubmit.isEnabled = false
        binding.textError.isVisible = false

        lifecycleScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    val api = NetworkModule.createApi()
                    if (isRegisterMode) {
                        val resp = api.register(RegisterRequest(username, password))
                        if (resp.ok && resp.token != null) {
                            AuthResult.Ok(resp.token)
                        } else {
                            AuthResult.Error(resp.error ?: "注册失败")
                        }
                    } else {
                        val resp = api.login(LoginRequest(username, password))
                        if (resp.ok && resp.token != null) {
                            AuthResult.Ok(resp.token)
                        } else {
                            AuthResult.Error(resp.error ?: "登录失败")
                        }
                    }
                } catch (e: Exception) {
                    AuthResult.Error("无法连接服务器 (${NetworkModule.getBaseUrl()})，请检查服务器地址")
                }
            }

            val b = _binding ?: return@launch
            b.btnSubmit.isEnabled = true
            when (result) {
                is AuthResult.Ok -> {
                    val ctx = requireContext()
                    AuthSessionStore.saveSession(ctx, result.token, username)
                    Toast.makeText(ctx, if (isRegisterMode) "注册成功" else "登录成功", Toast.LENGTH_SHORT).show()
                    dismissAllowingStateLoss()
                }
                is AuthResult.Error -> showError(result.message)
            }
        }
    }

    private sealed class AuthResult {
        data class Ok(val token: String) : AuthResult()
        data class Error(val message: String) : AuthResult()
    }

    private fun showError(msg: String) {
        val b = _binding ?: return
        b.textError.isVisible = true
        b.textError.text = msg
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
