// 图片库列表响应缓存（模块级）
//
// 从 Gallery 页面抽出为独立模块：AuthContext.logout() 需要在登出时主动
// 清空该缓存，页面文件不应被 context 反向依赖。
// 键包含登录态标识，避免登出/切换账号后读到他人私有数据（与 api/client.ts
// LRU 的 anon/token 隔离一致）。
import { getToken } from './client';
import type { MappedImage } from './types';

export interface GalleryCacheEntry {
  items: MappedImage[];
  total: number;
  timestamp: number;
}

const CACHE_TTL_MS = 5 * 60 * 1000; // 5 分钟
const MAX_CACHE_ENTRIES = 200;

const apiCache = new Map<string, GalleryCacheEntry>();

export function getGalleryCacheKey(type: string, query: string, page: number, size: number): string {
  const tokenPart = getToken() ?? 'anon';
  return `${tokenPart}:${type}:${query}:${page}:${size}`;
}

export function getGalleryCachedData(key: string): GalleryCacheEntry | null {
  const entry = apiCache.get(key);
  if (!entry) return null;
  if (Date.now() - entry.timestamp > CACHE_TTL_MS) {
    apiCache.delete(key);
    return null;
  }
  return entry;
}

export function setGalleryCacheData(key: string, items: MappedImage[], total: number): void {
  // 限制缓存大小，防止内存泄漏
  if (apiCache.size > MAX_CACHE_ENTRIES) {
    const oldestKey = apiCache.keys().next().value;
    if (oldestKey !== undefined) apiCache.delete(oldestKey);
  }
  apiCache.set(key, { items, total, timestamp: Date.now() });
}

/** 清空图片库缓存（登出时由 AuthContext 调用；焚毁后也用于防止读到旧列表） */
export function clearGalleryCache(): void {
  apiCache.clear();
}
