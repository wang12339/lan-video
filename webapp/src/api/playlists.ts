// 播放列表 API

import { request } from './client';

export interface Playlist {
  id: string;
  name: string;
  description: string | null;
  is_public: boolean;
  cover_url: string | null;
  item_count: number;
  created_at: string;
  updated_at: string;
}

export interface PlaylistListResponse {
  playlists: Playlist[];
}

export async function listMyPlaylists(): Promise<Playlist[]> {
  const res = await request<PlaylistListResponse>('/playlists');
  return res.playlists;
}

export async function createPlaylist(data: { name: string; description?: string; isPublic?: boolean }): Promise<Playlist> {
  const body: Record<string, unknown> = { name: data.name };
  if (data.description !== undefined) body.description = data.description;
  if (data.isPublic !== undefined) body.is_public = data.isPublic;
  return request<Playlist>('/playlists', { method: 'POST', body });
}

export async function updatePlaylist(id: string, data: { name?: string; description?: string; isPublic?: boolean }): Promise<void> {
  const body: Record<string, unknown> = {};
  if (data.name !== undefined) body.name = data.name;
  if (data.description !== undefined) body.description = data.description;
  if (data.isPublic !== undefined) body.is_public = data.isPublic;
  await request(`/playlists/${id}`, { method: 'PUT', body });
}

export async function deletePlaylist(id: string): Promise<void> {
  await request(`/playlists/${id}`, { method: 'DELETE' });
}

export async function addVideoToPlaylist(playlistId: string, videoId: string): Promise<void> {
  await request(`/playlists/${playlistId}/videos`, { method: 'POST', body: { video_id: videoId } });
}

export async function removeVideoFromPlaylist(playlistId: string, videoId: string): Promise<void> {
  await request(`/playlists/${playlistId}/videos/${videoId}`, { method: 'DELETE' });
}
