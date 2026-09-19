// 管理员 API

import { request, cacheClear, APIError } from './client';
import type { VideoListResponse as PaginatedVideoListResponse } from './types';

/** 待审批用户数变化事件：管理页审批操作后派发，导航徽标立即刷新 */
export const PENDING_USERS_CHANGED_EVENT = 'atmos:pending-users-changed'

/** 待审批注册用户数（管理导航徽标轮询用，仅返回计数） */
export async function getPendingUserCount(): Promise<number> {
  const res = await request<{ count: number }>('/admin/users/pending/count', {
    auth: true,
    skipCache: true,
    silent: true,
  })
  return res.count
}

/** 判断是否为权限类错误（403），供页面统一给出中文提示 */
export function isForbidden(err: unknown): boolean {
  return err instanceof APIError && err.status === 403;
}

export interface AdminUser {
  id: string;
  username: string;
  isAdmin: boolean;
  approved: boolean;
  createdAt: string;
  hasActiveToken: boolean;
  /** 访客影子账号（访客模式），待真实账号认证后自动合并删除 */
  isGuest?: boolean;
}

export interface AdminVideo {
  id: string;
  title: string;
  description: string;
  sourceType: string;
  coverUrl: string | null;
  streamUrl: string;
  thumbUrl: string | null;
  category: string;
  views: number;
  duration: number;
  /** 后端可能不返回（旧版契约），排序代码需容忍 undefined */
  createdAt?: string;
}

/** 复用 types.ts 的分页信封定义，仅将 items 收窄为管理员视频结构 */
export type VideoListResponse = Omit<PaginatedVideoListResponse, 'items'> & {
  items: AdminVideo[];
};

// ── 用户管理 ──

/** GET /admin/users 查询参数（服务端搜索/筛选/分页） */
export interface AdminUsersQuery {
  /** 用户名或邮箱模糊搜索 */
  search?: string;
  /** 审批状态：active（已通过，默认）| pending | all */
  status?: 'active' | 'pending' | 'all';
  /** 角色筛选 */
  role?: 'all' | 'admin' | 'user';
  /** 页码（0 基） */
  page?: number;
  /** 每页条数（默认 20，最大 200） */
  size?: number;
}

export interface AdminUsersPage {
  items: AdminUser[];
  total: number;
  page: number;
  size: number;
}

export async function listUsers(params: AdminUsersQuery = {}): Promise<AdminUsersPage> {
  const qs = new URLSearchParams();
  if (params.search) qs.set('search', params.search);
  if (params.status) qs.set('status', params.status);
  if (params.role && params.role !== 'all') qs.set('role', params.role);
  qs.set('page', String(Math.max(0, params.page ?? 0)));
  qs.set('size', String(params.size ?? 20));
  return request<AdminUsersPage>(`/admin/users?${qs}`);
}

export async function deleteUser(id: string): Promise<void> {
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
  id: string,
  data: { title?: string; description?: string; category?: string }
): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/videos/${id}`, { method: 'PUT', body: data });
}

// 与 videos.ts 完全重复的删除实现在此移除，直接转出以保持唯一实现；
// 调用方（Admin/VideosTab）不依赖返回值，返回类型收窄为 void 无行为影响。
export { deleteVideo, deleteVideos } from './videos';

// ── 外部视频 ──

export async function addExternalVideo(data: {
  title: string;
  description?: string;
  category?: string;
  stream_url: string;
  cover_url?: string;
}): Promise<{ id: string }> {
  return request('/admin/videos/external', { method: 'POST', body: data });
}

// ── 上传封面 ──

export async function uploadCover(id: string, file: File): Promise<void> {
  const form = new FormData();
  form.append('file', file);
  // multipart 统一走 request()（自带超时/错误本地化/401/CSRF 头）；
  // silent：错误由后台媒体页自行展示
  await request(`/admin/videos/${id}/cover`, {
    method: 'POST',
    body: form,
    silent: true,
  });
}

// ── 系统操作 ──

export async function scanMedia(category?: string): Promise<{ added: number }> {
  const form = new FormData();
  if (category) form.append('category', category);
  // 后端接受空 body 或 multipart；长超时（10 分钟扫描）；silent：错误由系统页展示
  const data = await request<{ added: number }>('/admin/videos/scan', {
    method: 'POST',
    body: form,
    timeout: 600000,
    silent: true,
  });
  // 扫描会新增媒体文件，清掉前端响应缓存避免短时间 TTL 内看到旧列表
  cacheClear();
  return data;
}

export async function backfillThumbnails(): Promise<{ ok: boolean; generated: number; errors: string[] }> {
  return request<{ ok: boolean; generated: number; errors: string[] }>('/admin/videos/backfill-thumbnails', {
    method: 'POST',
    timeout: 600000,
  });
}

/** 回填图片 EXIF（后台逐张解析原图并写入 exif_* 列） */
export async function backfillExif(): Promise<{ ok: boolean; processed: number; errors: string[] }> {
  return request<{ ok: boolean; processed: number; errors: string[] }>('/admin/videos/backfill-exif', {
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

export async function batchUpdateCategory(ids: string[], category: string): Promise<{ ok: boolean; deleted?: number }> {
  return request('/admin/videos/batch-category', { method: 'PUT', body: { ids, category } });
}

// ── 用户管理增强 ──

export async function resetUserPassword(id: string, password: string): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/users/${id}/password`, { method: 'PUT', body: { password } });
}

export async function toggleUserAdmin(id: string): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/users/${id}/admin`, { method: 'PUT' });
}

export async function approveUser(id: string, approved: boolean): Promise<{ ok: boolean; error?: string }> {
  return request(`/admin/users/${id}/approve`, { method: 'PUT', body: { approved } });
}

export async function kickUser(id: string): Promise<{ ok: boolean; deleted?: number }> {
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
