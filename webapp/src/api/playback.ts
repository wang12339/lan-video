import { request } from './client';
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
  historyCache.clear();
}

export async function listPlaybackHistory(limit = 50): Promise<PlaybackHistory[]> {
  const clamped = Math.max(1, Math.min(MAX_HISTORY_LIMIT, limit));
  const key = `history:${clamped}`;
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
