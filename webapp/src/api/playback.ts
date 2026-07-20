// 播放进度 API

import { request } from './client';
import type { PlaybackHistory } from './types';

export async function getPlayback(videoId: number): Promise<{ positionMs: number; durationMs: number }> {
  return request(`/playback/history/${videoId}`);
}

export async function savePlayback(
  videoId: number,
  positionMs: number,
  durationMs: number
): Promise<void> {
  await request('/playback/history', {
    method: 'POST',
    body: { video_id: videoId, position_ms: positionMs, duration_ms: durationMs },
  });
}

export async function listPlaybackHistory(): Promise<PlaybackHistory[]> {
  return request<PlaybackHistory[]>('/playback/history');
}

// --- 播放会话跟踪 ---

export async function startPlaybackSession(videoId: number): Promise<void> {
  await request('/playback/session/start', {
    method: 'POST',
    body: { video_id: videoId },
  });
}

export async function heartbeatPlaybackSession(videoId: number): Promise<void> {
  await request('/playback/session/heartbeat', {
    method: 'POST',
    body: { video_id: videoId },
  });
}

export async function stopPlaybackSession(videoId: number): Promise<void> {
  await request('/playback/session/stop', {
    method: 'POST',
    body: { video_id: videoId },
  });
}
