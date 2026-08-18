// 视频 API

import { request } from './client';
import type { VideoListResponse, Video, TranscodeStatusResponse, PlaybackHistory } from './types';

const MAX_PAGE_SIZE = 1000;
const MAX_SEARCH_SIZE = 100; // 后端 /videos/search 的 size 上限

interface ListVideosParams {
  query?: string;
  type?: string;
  category?: string;
  page?: number;
  size?: number;
  uploaderId?: string;
  sort?: string;
}

export async function listVideos({
  query,
  type,
  category,
  page = 0,
  size = 20,
  uploaderId,
  sort,
}: ListVideosParams = {}): Promise<VideoListResponse> {
  const params = new URLSearchParams();
  if (query) params.set('query', query);
  if (type) params.set('type', type);
  if (category) params.set('category', category);
  if (uploaderId !== undefined) params.set('uploader_id', String(uploaderId));
  if (sort) params.set('sort', sort);
  params.set('page', String(Math.max(0, page)));
  params.set('size', String(Math.min(size, MAX_PAGE_SIZE)));
  return request<VideoListResponse>(`/videos?${params}`);
}

export async function getVideo(id: string): Promise<Video> {
  return request<Video>(`/videos/${id}`);
}

export async function incrementViews(id: string): Promise<void> {
  await request(`/videos/${id}/view`, { method: 'POST' });
}

export async function deleteVideo(id: string): Promise<void> {
  await request(`/admin/videos/${id}`, { method: 'DELETE', auth: true });
}

export async function deleteVideos(ids: string[]): Promise<void> {
  await request('/admin/videos/batch', { method: 'DELETE', body: ids, auth: true });
}

export async function listFavorites(): Promise<PlaybackHistory[]> {
  return request<PlaybackHistory[]>('/videos/favorites');
}

// Transcode API

export async function transcodeVideo(
  videoId: string,
  resolutions: string[]
): Promise<{ success: boolean; message: string }> {
  return request(`/admin/videos/${videoId}/transcode`, {
    method: 'POST',
    body: { resolutions },
    auth: true,
  });
}

export async function getTranscodeStatus(
  videoId: string
): Promise<TranscodeStatusResponse> {
  // silent：转码状态查询由页面自行展示错误，避免轮询/刷新失败弹全局 Toast
  return request(`/admin/videos/${videoId}/transcode/status`, { auth: true, silent: true });
}

// Search API

export interface SearchResult {
  id: string;
  title: string;
  description: string | null;
  category: string | null;
  rank: number;
  headline: string | null;
}

export interface SearchResponse {
  items: SearchResult[];
  total: number;
  page: number;
  size: number;
}

export async function searchVideos(
  query: string,
  page = 0,
  size = 20
): Promise<SearchResponse> {
  const params = new URLSearchParams();
  params.set('q', query);
  params.set('page', String(Math.max(0, page)));
  params.set('size', String(Math.min(Math.max(1, size), MAX_SEARCH_SIZE)));
  return request(`/videos/search?${params}`);
}

export async function searchSuggest(
  query: string
): Promise<string[]> {
  const params = new URLSearchParams();
  params.set('q', query);
  return request(`/videos/search/suggest?${params}`);
}

// Upload resume API

export async function getUploadStatus(hash: string): Promise<{ received: number }> {
  // skipCache：上传进度随时在变，不能命中 30s 响应缓存
  // silent：断点查询失败不弹全局 Toast（页面自行回退到从头上传）
  return request<{ received: number }>(
    `/admin/videos/upload-status?hash=${encodeURIComponent(hash)}`,
    { auth: true, skipCache: true, silent: true }
  );
}

function sanitizeHeaderValue(value: string): string {
  return value.replace(/[\r\n]/g, '').trim();
}

export async function uploadResumeChunk(
  hash: string,
  fileName: string,
  totalSize: number,
  category: string,
  chunk: Blob
): Promise<{ received: number; id?: string }> {
   const headers: Record<string, string> = {
    'x-upload-hash': hash,
    'x-upload-name': sanitizeHeaderValue(fileName),
    'x-upload-size': String(totalSize),
    'x-upload-category': sanitizeHeaderValue(category),
  };
  // Blob 直传复用 request()：统一超时、后端中文错误本地化（保留中文原文）、
  // 401 触发全局登出。silent：分片失败由上传页按文件展示 errorMsg，不弹 Toast；
  // noInvalidate：分片请求是高频写，不能每个分片都清空 /videos 缓存。
  return request<{ received: number; id?: string }>('/admin/videos/upload-resume', {
    method: 'POST',
    body: chunk,
    headers,
    silent: true,
    noInvalidate: true,
  });
}
