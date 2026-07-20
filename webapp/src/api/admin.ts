// 管理员 API

import { BASE, getToken, request } from './client';

export interface AdminUser {
  id: number;
  username: string;
  isAdmin: boolean;
  approved: boolean;
  createdAt: string;
  hasActiveToken: boolean;
}

export interface AdminVideo {
  id: number;
  title: string;
  description: string;
  sourceType: string;
  coverUrl: string | null;
  streamUrl: string;
  thumbUrl: string | null;
  category: string;
  views: number;
  duration: number;
  createdAt: string;
}

export interface VideoListResponse {
  items: AdminVideo[];
  total: number;
  page: number;
  size: number;
}

// ── 用户管理 ──

export async function listUsers(): Promise<AdminUser[]> {
  return request<AdminUser[]>('/admin/users');
}

export async function deleteUser(id: number): Promise<void> {
  await request(`/admin/users/${id}`, { method: 'DELETE' });
}

// ── 视频管理 ──

export async function listAdminVideos(params: {
  query?: string;
  type?: string;
  category?: string;
  page?: number;
  size?: number;
} = {}): Promise<VideoListResponse> {
  const sp = new URLSearchParams();
  if (params.query) sp.set('query', params.query);
  if (params.type) sp.set('type', params.type);
  if (params.category) sp.set('category', params.category);
  sp.set('page', String(params.page ?? 0));
  sp.set('size', String(params.size ?? 50));
  return request<VideoListResponse>(`/videos?${sp}`);
}

export async function updateVideo(
  id: number,
  data: { title?: string; description?: string; category?: string }
): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/videos/${id}`, { method: 'PUT', body: data });
}

export async function deleteVideo(id: number): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/videos/${id}`, { method: 'DELETE' });
}

export async function deleteVideos(ids: number[]): Promise<{ ok: boolean; deleted?: number }> {
  return request('/admin/videos/batch', { method: 'DELETE', body: ids });
}

// ── 外部视频 ──

export async function addExternalVideo(data: {
  title: string;
  description?: string;
  category?: string;
  stream_url: string;
  cover_url?: string;
}): Promise<{ id: number }> {
  return request('/admin/videos/external', { method: 'POST', body: data });
}

// ── 上传封面 ──

export async function uploadCover(id: number, file: File): Promise<void> {
  const token = getToken();
  const form = new FormData();
  form.append('file', file);
  const res = await fetch(`${BASE}/admin/videos/${id}/cover`, {
    method: 'POST',
    headers: token ? { Authorization: 'Bearer ' + token } : {},
    body: form,
  });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error || `HTTP ${res.status}`);
  }
}

// ── 系统操作 ──

export async function scanMedia(category?: string): Promise<{ added: number }> {
  if (category) {
    const form = new FormData();
    form.append('category', category);
    const token = getToken();
    const res = await fetch(`${BASE}/admin/videos/scan`, {
      method: 'POST',
      headers: token ? { Authorization: 'Bearer ' + token } : {},
      body: form,
      signal: AbortSignal.timeout(600000),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  }
  return request<{ added: number }>('/admin/videos/scan', {
    method: 'POST',
    timeout: 600000,
  });
}

export async function backfillThumbnails(): Promise<{ ok: boolean; generated: number; errors: string[] }> {
  return request<{ ok: boolean; generated: number; errors: string[] }>('/admin/videos/backfill-thumbnails', {
    method: 'POST',
    timeout: 600000,
  });
}

// ── 数据统计 ──

export interface AdminStats {
  totalVideos: number;
  videoCount: number;
  imageCount: number;
  userCount: number;
  pendingCount: number;
  totalViews: number;
  totalDurationSecs: number;
  byType: { type: string; count: number }[];
  byCategory: { category: string; count: number }[];
}

export async function getStats(): Promise<AdminStats> {
  return request<AdminStats>('/admin/stats');
}

// ── 批量改分类 ──

export async function batchUpdateCategory(ids: number[], category: string): Promise<{ ok: boolean; deleted?: number }> {
  return request('/admin/videos/batch-category', { method: 'PUT', body: { ids, category } });
}

// ── 用户管理增强 ──

export async function resetUserPassword(id: number, password: string): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/users/${id}/password`, { method: 'PUT', body: { password } });
}

export async function toggleUserAdmin(id: number): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/users/${id}/admin`, { method: 'PUT' });
}

export async function approveUser(id: number, approved: boolean): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/users/${id}/approve`, { method: 'PUT', body: { approved } });
}

export async function kickUser(id: number): Promise<{ ok: boolean; deleted?: number }> {
  return request(`/admin/users/${id}/kick`, { method: 'POST' });
}

// ── 注册开关 ──

export async function getRegistrationEnabled(): Promise<{ enabled: boolean }> {
  return request<{ enabled: boolean }>('/admin/config/registration');
}

export async function setRegistrationEnabled(enabled: boolean): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>('/admin/config/registration', { method: 'PUT', body: { enabled } });
}

// ── 系统监控 ──

export interface SystemInfo {
  mediaSizeBytes: number;
  mediaSizeHuman: string;
  dbConnections: number;
  rustLog: string;
  mediaRoot: string;
}

export async function getSystemInfo(): Promise<SystemInfo> {
  return request<SystemInfo>('/admin/system');
}
