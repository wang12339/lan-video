import { request, getToken } from './client';
// 命名空间导入仅用于能力检测：测试环境可能把 client 整体 mock 成不含
// registerWriteListener 的对象，直接命名导入会在访问时抛错（属性 get 陷阱）
import * as client from './client';
import type { PlaybackHistory } from './types';

const MAX_HISTORY_LIMIT = 200;

const inflight = new Map<string, Promise<unknown>>();

function dedupe<T>(key: string, fn: () => Promise<T>): Promise<T> {
  const existing = inflight.get(key);
  if (existing) return existing as Promise<T>;
  const p = fn().finally(() => inflight.delete(key));
  inflight.set(key, p);
  return p;
}

const historyCache = new Map<string, { data: PlaybackHistory[]; ts: number }>();
const HISTORY_TTL = 30_000;

/** 清空播放历史缓存（登出时调用，避免切换账号后串读上一个用户的数据） */
export function clearPlaybackHistoryCache(): void {
  historyCache.clear();
}

// 视频增删改/进度上报/登出都会让首页"最近观看"与个人历史变得陈旧。
// client 的规则表感知不到本模块私有的 historyCache，反向 import 又会成环，
// 故通过写事件注册表单向订阅：仅命中相关前缀时清缓存，其余写路径不动。
const HISTORY_INVALIDATION_PREFIXES = ['/videos', '/admin/videos', '/playback/history', '/auth/logout'];

let writeListenerRegistered = false;

function ensureHistoryWriteListener(): void {
  if (writeListenerRegistered) return;
  writeListenerRegistered = true;
  if (!('registerWriteListener' in client)) return;
  client.registerWriteListener((path) => {
    if (HISTORY_INVALIDATION_PREFIXES.some(prefix => path.startsWith(prefix))) {
      clearPlaybackHistoryCache();
    }
  });
}

// 模块加载时注册一次（幂等标志防止重复注册）
ensureHistoryWriteListener();

export async function savePlayback(
  videoId: string,
  positionMs: number,
  durationMs: number
): Promise<void> {
  await dedupe(`save:${videoId}`, () =>
    request('/playback/history', {
      method: 'POST',
      silent: true,
      body: {
        video_id: videoId,
        position_ms: Math.max(0, Math.floor(positionMs)),
        duration_ms: Math.max(0, Math.floor(durationMs)),
      },
    })
  );
  clearPlaybackHistoryCache();
}

export async function listPlaybackHistory(limit = 50): Promise<PlaybackHistory[]> {
  const clamped = Math.max(1, Math.min(MAX_HISTORY_LIMIT, limit));
  // 键含登录态标识：避免登出/切换账号后命中上一个用户的历史缓存
  const key = `${getToken() ?? 'anon'}:history:${clamped}`;
  const cached = historyCache.get(key);
  if (cached && Date.now() - cached.ts < HISTORY_TTL) return cached.data;

  const res = await request<{ items: PlaybackHistory[]; total: number }>(`/playback/history?limit=${clamped}`);
  const items = res.items ?? [];
  historyCache.set(key, { data: items, ts: Date.now() });
  return items;
}

export async function startPlaybackSession(videoId: string): Promise<void> {
  await dedupe(`session:start:${videoId}`, () =>
    request('/playback/session/start', {
      method: 'POST',
      silent: true,
      body: { video_id: videoId },
    })
  );
}

export async function heartbeatPlaybackSession(videoId: string): Promise<void> {
  await dedupe(`session:hb:${videoId}`, () =>
    request('/playback/session/heartbeat', {
      method: 'POST',
      silent: true,
      body: { video_id: videoId },
    })
  );
}

export async function stopPlaybackSession(videoId: string): Promise<void> {
  inflight.delete(`session:hb:${videoId}`);
  await dedupe(`session:stop:${videoId}`, () =>
    request('/playback/session/stop', {
      method: 'POST',
      silent: true,
      body: { video_id: videoId },
    })
  );
}
