// 播放进度 API

import { request } from './client';
import type { PlaybackHistory } from './types';

const MAX_HISTORY_LIMIT = 200; // 后端 /playback/history 的 limit 上限

export async function savePlayback(
  videoId: string,
  positionMs: number,
  durationMs: number
): Promise<void> {
  // silent：播放页每 10s 后台上报，失败静默记日志，不弹全局 Toast
  await request('/playback/history', {
    method: 'POST',
    silent: true,
    body: {
      video_id: videoId,
      position_ms: Math.max(0, Math.floor(positionMs)),
      duration_ms: Math.max(0, Math.floor(durationMs)),
    },
  });
}

export async function listPlaybackHistory(limit = 50): Promise<PlaybackHistory[]> {
  const clamped = Math.max(1, Math.min(MAX_HISTORY_LIMIT, limit));
  const res = await request<{ items: PlaybackHistory[]; total: number }>(`/playback/history?limit=${clamped}`);
  return res.items ?? [];
}

// --- 播放会话跟踪 ---
// 以下均为播放页 fire-and-forget 的后台调用（页面已 .catch 吞错），
// 统一 silent：失败静默记日志，避免心跳/会话续期失败弹全局 Toast 刷屏

export async function startPlaybackSession(videoId: string): Promise<void> {
  await request('/playback/session/start', {
    method: 'POST',
    silent: true,
    body: { video_id: videoId },
  });
}

export async function heartbeatPlaybackSession(videoId: string): Promise<void> {
  await request('/playback/session/heartbeat', {
    method: 'POST',
    silent: true,
    body: { video_id: videoId },
  });
}

export async function stopPlaybackSession(videoId: string): Promise<void> {
  await request('/playback/session/stop', {
    method: 'POST',
    silent: true,
    body: { video_id: videoId },
  });
}
