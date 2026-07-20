// 播放列表 API

import { request } from './client';

export interface Playlist {
  id: number;
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

export async function getPlaylist(id: number): Promise<Playlist> {
  return request<Playlist>(`/playlists/${id}`);
}

export async function createPlaylist(data: { name: string; description?: string; isPublic?: boolean }): Promise<Playlist> {
  return request<Playlist>('/playlists', { method: 'POST', body: { name: data.name, description: data.description, is_public: data.isPublic } });
}

export async function updatePlaylist(id: number, data: { name?: string; description?: string; isPublic?: boolean }): Promise<void> {
  await request(`/playlists/${id}`, { method: 'PUT', body: { name: data.name, description: data.description, is_public: data.isPublic } });
}

export async function deletePlaylist(id: number): Promise<void> {
  await request(`/playlists/${id}`, { method: 'DELETE' });
}

export async function addVideoToPlaylist(playlistId: number, videoId: number): Promise<void> {
  await request(`/playlists/${playlistId}/videos`, { method: 'POST', body: { video_id: videoId } });
}

export async function removeVideoFromPlaylist(playlistId: number, videoId: number): Promise<void> {
  await request(`/playlists/${playlistId}/videos/${videoId}`, { method: 'DELETE' });
}
