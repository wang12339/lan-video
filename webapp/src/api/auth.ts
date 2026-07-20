// 认证 API

import { request, saveToken, clearToken, setOnAuthRequired, BASE, getToken } from './client';
import type { AuthResponse, UserInfo, UserProfile } from './types';

function requireOk(data: AuthResponse): AuthResponse {
  if (data && data.ok === false) throw new Error(data.error || '操作失败');
  return data;
}

export async function login(username: string, password: string): Promise<AuthResponse> {
  const res = requireOk(await request<AuthResponse>('/auth/login', {
    method: 'POST',
    body: { username, password },
    auth: false,
  }));
  if (res && res.token) saveToken(res.token);
  return res;
}

export async function register(username: string, password: string): Promise<AuthResponse> {
  const res = await request<AuthResponse>('/auth/register', {
    method: 'POST',
    body: { username, password },
    auth: false,
  });
  if (res.ok === false) throw new Error(res.error || '注册失败');
  if (res.token) saveToken(res.token);
  return res;
}

export async function logout(): Promise<void> {
  try { await request('/auth/logout', { method: 'POST' }); } catch { /* ignore */ }
  clearToken();
}

export async function getUserInfo(): Promise<UserInfo> {
  return request<UserInfo>('/auth/user');
}

export async function getUserProfile(): Promise<UserProfile> {
  return request<UserProfile>('/auth/user/profile');
}

let sessionCache: { valid: boolean; expires: number } | null = null;
export async function checkSession(): Promise<boolean> {
  if (sessionCache && Date.now() < sessionCache.expires) return sessionCache.valid;
  try { await request('/auth/user', { auth: true }); sessionCache = { valid: true, expires: Date.now() + 60000 }; return true; }
  catch { sessionCache = { valid: false, expires: Date.now() + 5000 }; return false; }
}

export async function uploadAvatar(file: File): Promise<string> {
  const formData = new FormData();
  formData.append('file', file);
  const token = getToken() || '';
  const res = await fetch(BASE + '/auth/user/avatar', {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}` },
    body: formData,
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: '上传失败' }));
    throw new Error(err.error || '上传失败');
  }
  const data = await res.json();
  return data.avatarUrl;
}

export { setOnAuthRequired };
