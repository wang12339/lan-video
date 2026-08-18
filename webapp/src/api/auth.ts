// 认证 API

import { request, saveToken, clearToken, setOnAuthRequired, APIError } from './client';
import type { AuthResponse, UserInfo, UserProfile } from './types';

// 登录/注册接口失败时返回 HTTP 200 + { ok: false, error }，
// 统一在此转成 APIError，与 request() 的非 2xx 抛错保持类型一致。
function handleAuthResponse(res: AuthResponse): AuthResponse {
  if (res && res.ok === false) throw new APIError(res.error || '操作失败', 200);
  return res;
}

// 登录/注册成功后持久化 token
function persistToken(res: AuthResponse): AuthResponse {
  if (res.token) saveToken(res.token);
  return res;
}

export async function login(username: string, password: string): Promise<AuthResponse> {
  const res = handleAuthResponse(await request<AuthResponse>('/auth/login', {
    method: 'POST',
    body: { username, password },
    auth: false,
  }));
  return persistToken(res);
}

export async function register(username: string, password: string): Promise<AuthResponse> {
  const res = handleAuthResponse(await request<AuthResponse>('/auth/register', {
    method: 'POST',
    body: { username, password },
    auth: false,
  }));
  return persistToken(res);
}

export async function logout(): Promise<void> {
  try { await request('/auth/logout', { method: 'POST' }); } catch { /* ignore */ }
  // 清除会话缓存，避免登出后 60 秒内 checkSession() 仍返回旧结果
  resetSessionCache();
  clearToken();
}

export async function getUserInfo(): Promise<UserInfo> {
  return request<UserInfo>('/auth/user');
}

export async function getUserProfile(): Promise<UserProfile> {
  return request<UserProfile>('/auth/user/profile');
}

let sessionCache: { valid: boolean; expires: number } | null = null;

function resetSessionCache() {
  sessionCache = null;
}

export async function checkSession(): Promise<boolean> {
  if (sessionCache && Date.now() < sessionCache.expires) return sessionCache.valid;
  try { await request('/auth/user', { auth: true }); sessionCache = { valid: true, expires: Date.now() + 60000 }; return true; }
  catch { sessionCache = { valid: false, expires: Date.now() + 5000 }; return false; }
}

export async function uploadAvatar(file: File): Promise<string> {
  const formData = new FormData();
  formData.append('file', file);
  // 统一走 request()（multipart）：超时、错误本地化、401 处理一致；
  // silent：错误由个人中心页面自行展示，不重复弹全局 Toast
  const data = await request<{ avatarUrl?: string }>('/auth/user/avatar', {
    method: 'POST',
    body: formData,
    silent: true,
  });
  return data.avatarUrl || '';
}

export async function sendVerificationEmail(): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/send-verification-email', {
    method: 'POST',
  });
}

export async function updateEmail(email: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/user/email', {
    method: 'PUT',
    body: { email },
  });
}

export async function forgotPassword(email: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/forgot-password', {
    method: 'POST',
    body: { email },
    auth: false,
  });
}

export async function resetPassword(token: string, password: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/reset-password', {
    method: 'POST',
    body: { token, password },
    auth: false,
  });
}

/** POST /auth/verify-email — 提交邮箱验证令牌（邮件链接走 GET 渲染落地页，API 场景用 POST） */
export async function verifyEmail(token: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/verify-email', {
    method: 'POST',
    body: { token },
    auth: false,
  });
}

export { setOnAuthRequired };
