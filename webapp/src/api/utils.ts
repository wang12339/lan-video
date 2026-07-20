// 数据映射工具

import { mediaUrl } from './client';
import type { Video, PlaybackHistory, MappedPlaylist, MappedVideo, MappedImage, MappedHistory } from './types'
import type { Playlist } from './playlists'

// 占位图缓存
const placeholderCache = new Map<string, string>();
const PLACEHOLDER_CACHE_MAX = 100;

function placeholderDataURL(id: number, type: string): string {
  const key = `${id}:${type}`;
  const cached = placeholderCache.get(key);
  if (cached) return cached;
  const label = type === 'local_image' ? '📷' : '🎬';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180" viewBox="0 0 320 180">
    <rect width="320" height="180" fill="#1a1a2e"/>
    <text x="160" y="80" text-anchor="middle" fill="#4a4a6a" font-size="48">${label}</text>
    <text x="160" y="120" text-anchor="middle" fill="#3a3a5a" font-size="12">${id}</text>
  </svg>`;
  const url = 'data:image/svg+xml,' + encodeURIComponent(svg);
  if (placeholderCache.size >= PLACEHOLDER_CACHE_MAX) {
    const firstKey = placeholderCache.keys().next().value;
    if (firstKey !== undefined) placeholderCache.delete(firstKey);
  }
  placeholderCache.set(key, url);
  return url;
}

// 分类颜色
const CAT_COLORS: Record<string, string> = {
  '科技': '#3b82f6',
  '设计': '#ec4899',
  '音乐': '#8b5cf6',
  '教程': '#10b981',
  '娱乐': '#f59e0b',
  '运动': '#ef4444',
  '记录': '#06b6d4',
};

export function getCatColor(cat: string): string {
  return CAT_COLORS[cat] || '#ffffff';
}

// 格式化工具
export function formatDuration(totalSeconds: number, zeroFallback?: string): string {
  if (totalSeconds === undefined || totalSeconds === null || isNaN(totalSeconds) || totalSeconds < 0) return zeroFallback ?? '';
  if (totalSeconds === 0) return zeroFallback ?? '00:00';
  const s = Math.floor(totalSeconds)
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = s % 60
  const pad = (n: number) => String(n).padStart(2, '0')
  return h > 0
    ? `${h}:${pad(m)}:${pad(sec)}`
    : `${pad(m)}:${pad(sec)}`
}

export function formatViews(n: number | null | undefined): string {
  if (!n && n !== 0) return '';
  if (n >= 100000000) return (n / 100000000).toFixed(1) + '亿';
  if (n >= 10000) return (n / 10000).toFixed(1) + '万';
  if (n >= 1000) return (n / 1000).toFixed(1) + 'k';
  return String(n);
}

export const formatCount = formatViews

// 数据映射
export function mapVideo(v: Video | null): MappedVideo | null {
  if (!v) return null;
  return {
    id: v.id,
    title: v.title || '未命名',
    category: v.category || 'general',
    description: v.description || '',
    thumb: mediaUrl(v.thumbUrl) || mediaUrl(v.coverUrl) || placeholderDataURL(v.id, 'local_video'),
    stream: mediaUrl(v.streamUrl),
    cover: mediaUrl(v.coverUrl),
    sourceType: v.sourceType || 'local_video',
    duration: v.duration || 0,
    views: v.views || 0,
    date: v.createdAt || '',
    progress: v.watchPosition || 0,
    uploaderId: v.uploaderId,
  };
}

export function mapImage(v: Video | null): MappedImage | null {
  if (!v) return null;
  return {
    id: v.id,
    title: v.title || '未命名',
    category: v.category || 'general',
    thumb: mediaUrl(v.thumbUrl) || mediaUrl(v.streamUrl) || placeholderDataURL(v.id, 'local_image'),
    sourceType: v.sourceType || 'local_image',
  };
}

export function mapHistory(h: PlaybackHistory | null): MappedHistory | null {
  if (!h) return null;
  const prog = h.durationMs > 0 ? Math.round((h.positionMs / h.durationMs) * 100) : 0;
  return {
    id: h.videoId,
    title: h.title || '未命名',
    category: h.category || 'general',
    thumb: mediaUrl(h.coverUrl) || (h.streamUrl && h.sourceType === 'local_image' ? mediaUrl(h.streamUrl) : null) || placeholderDataURL(h.videoId || 0, 'local_video'),
    stream: mediaUrl(h.streamUrl),
    sourceType: h.sourceType || 'local_video',
    positionMs: h.positionMs || 0,
    durationMs: h.durationMs || 0,
    updatedAt: h.updatedAt || '',
    progress: prog,
  };
}

export function mapPlaylist(p: Playlist | null): MappedPlaylist | null {
  if (!p) return null;
  return {
    id: p.id,
    name: p.name,
    description: p.description,
    isPublic: p.is_public,
    coverUrl: p.cover_url ? mediaUrl(p.cover_url) : null,
    itemCount: p.item_count,
    createdAt: p.created_at,
    updatedAt: p.updated_at,
  };
}
