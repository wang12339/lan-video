package com.lanvideo.player.ui.auth

import android.app.Application
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanvideo.player.MyApplication
import com.lanvideo.player.data.model.LoginRequest
import com.lanvideo.player.data.model.RegisterRequest
import com.lanvideo.player.data.network.NetworkModule
import com.lanvideo.player.data.user.AuthSessionStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

sealed class LoginState {
    data object Idle : LoginState()
    data object Loading : LoginState()
    data class Success(val token: String, val username: String) : LoginState()
    data class Error(val message: String) : LoginState()
}

class LoginViewModel(
    private val application: Application
) : ViewModel() {

    private val _loginState = MutableStateFlow<LoginState>(LoginState.Idle)
    val loginState: StateFlow<LoginState> = _loginState

    fun login(username: String, password: String) {
        if (_loginState.value is LoginState.Loading) return
        _loginState.value = LoginState.Loading

        viewModelScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    val api = NetworkModule.createApi()
                    val resp = api.login(LoginRequest(username, password))
                    if (resp.ok && resp.token != null) {
                        AuthResult.Ok(resp.token)
                    } else {
                        AuthResult.Error(resp.error ?: "登录失败")
                    }
                } catch (e: Exception) {
                    AuthResult.Error("无法连接服务器 (${NetworkModule.getBaseUrl()})，请检查服务器地址")
                }
            }

            when (result) {
                is AuthResult.Ok -> {
                    AuthSessionStore.saveSession(application, result.token, username)
                    MyApplication.instance.notifyLanServerEvent(NetworkModule.getBaseUrl())
                    _loginState.value = LoginState.Success(result.token, username)
                }
                is AuthResult.Error -> {
                    _loginState.value = LoginState.Error(result.message)
                }
            }
        }
    }

    fun register(username: String, password: String) {
        if (_loginState.value is LoginState.Loading) return
        _loginState.value = LoginState.Loading

        viewModelScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    val api = NetworkModule.createApi()
                    val resp = api.register(RegisterRequest(username, password))
                    if (resp.ok && resp.token != null) {
                        AuthResult.Ok(resp.token)
                    } else {
                        AuthResult.Error(resp.error ?: "注册失败")
                    }
                } catch (e: Exception) {
                    AuthResult.Error("无法连接服务器 (${NetworkModule.getBaseUrl()})，请检查服务器地址")
                }
            }

            when (result) {
                is AuthResult.Ok -> {
                    AuthSessionStore.saveSession(application, result.token, username)
                    MyApplication.instance.notifyLanServerEvent(NetworkModule.getBaseUrl())
                    _loginState.value = LoginState.Success(result.token, username)
                }
                is AuthResult.Error -> {
                    _loginState.value = LoginState.Error(result.message)
                }
            }
        }
    }

    fun resetState() {
        _loginState.value = LoginState.Idle
    }
}

private sealed class AuthResult {
    data class Ok(val token: String) : AuthResult()
    data class Error(val message: String) : AuthResult()
}
