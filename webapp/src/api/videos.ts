// 视频 API

import { BASE, getToken, request } from './client';
import type { VideoListResponse, Video, TranscodeStatusResponse, PlaybackHistory } from './types';

const MAX_PAGE_SIZE = 1000;

interface ListVideosParams {
  query?: string;
  type?: string;
  category?: string;
  page?: number;
  size?: number;
  uploaderId?: number;
}

export async function listVideos({
  query,
  type,
  category,
  page = 0,
  size = 20,
  uploaderId,
}: ListVideosParams = {}): Promise<VideoListResponse> {
  const params = new URLSearchParams();
  if (query) params.set('query', query);
  if (type) params.set('type', type);
  if (category) params.set('category', category);
  if (uploaderId !== undefined) params.set('uploader_id', String(uploaderId));
  params.set('page', String(page));
  params.set('size', String(Math.min(size, MAX_PAGE_SIZE)));
  return request<VideoListResponse>(`/videos?${params}`);
}

export async function getVideo(id: number): Promise<Video> {
  return request<Video>(`/videos/${id}`);
}

export async function incrementViews(id: number): Promise<void> {
  await request(`/videos/${id}/view`, { method: 'POST' });
}

export async function deleteVideo(id: number): Promise<void> {
  await request(`/admin/videos/${id}`, { method: 'DELETE', auth: true });
}

export async function deleteVideos(ids: number[]): Promise<void> {
  await request('/admin/videos/batch', { method: 'DELETE', body: ids, auth: true });
}

export async function listFavorites(): Promise<PlaybackHistory[]> {
  return request<PlaybackHistory[]>('/videos/favorites');
}

// Transcode API

export async function transcodeVideo(
  videoId: number,
  resolutions: string[]
): Promise<{ success: boolean; message: string }> {
  return request(`/admin/videos/${videoId}/transcode`, {
    method: 'POST',
    body: { resolutions },
    auth: true,
  });
}

export async function getTranscodeStatus(
  videoId: number
): Promise<TranscodeStatusResponse> {
  return request(`/admin/videos/${videoId}/transcode/status`, { auth: true });
}

export async function deleteVariant(
  videoId: number,
  resolution: string
): Promise<{ success: boolean; message: string }> {
  return request(`/admin/videos/${videoId}/transcode/${resolution}`, {
    method: 'DELETE',
    auth: true,
  });
}

// Search API

export interface SearchResult {
  id: number;
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
  params.set('page', String(page));
  params.set('size', String(size));
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
  const token = getToken();
  const url = `${BASE}/admin/videos/upload-status?hash=${encodeURIComponent(hash)}`;
  const headers: Record<string, string> = {};
  if (token) headers['Authorization'] = 'Bearer ' + token;
  const res = await fetch(url, { headers });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
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
): Promise<{ received: number; id?: number }> {
  const token = getToken();
  const url = `${BASE}/admin/videos/upload-resume`;
  const headers: Record<string, string> = {
    'x-upload-hash': hash,
    'x-upload-name': sanitizeHeaderValue(fileName),
    'x-upload-size': String(totalSize),
    'x-upload-category': sanitizeHeaderValue(category),
  };
  if (token) headers['Authorization'] = 'Bearer ' + token;
  const res = await fetch(url, { method: 'POST', headers, body: chunk });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error || `HTTP ${res.status}`);
  }
  return res.json();
}
