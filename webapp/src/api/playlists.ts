/**
 * 播放列表 API 模块
 * @module playlists
 */

import { request } from './client';

/**
 * 播放列表对象
 */
export interface Playlist {
  /** 播放列表唯一标识符 */
  id: string;
  /** 播放列表名称 */
  name: string;
  /** 播放列表描述，可为空 */
  description: string | null;
  /** 是否公开可见 */
  is_public: boolean;
  /** 封面图片 URL，可为空 */
  cover_url: string | null;
  /** 播放列表中的视频数量 */
  item_count: number;
  /** 创建时间（ISO 8601 格式） */
  created_at: string;
  /** 最后更新时间（ISO 8601 格式） */
  updated_at: string;
}

/**
 * 播放列表列表响应
 */
export interface PlaylistListResponse {
  /** 播放列表数组 */
  playlists: Playlist[];
}

/**
 * 获取当前用户的播放列表
 * @returns {Promise<Playlist[]>} 播放列表数组
 * @example
 * const playlists = await listMyPlaylists();
 */
export async function listMyPlaylists(): Promise<Playlist[]> {
  const res = await request<PlaylistListResponse>('/playlists');
  return res.playlists;
}

/**
 * 创建新播放列表
 * @param {Object} data - 播放列表数据
 * @param {string} data.name - 播放列表名称（必填）
 * @param {string} [data.description] - 播放列表描述（可选）
 * @param {boolean} [data.isPublic] - 是否公开（可选，默认为私有）
 * @returns {Promise<Playlist>} 创建成功的播放列表对象
 * @example
 * const playlist = await createPlaylist({ name: '我的收藏', description: '喜欢的视频', isPublic: true });
 */
export async function createPlaylist(data: { name: string; description?: string; isPublic?: boolean }): Promise<Playlist> {
  const body: Record<string, unknown> = { name: data.name };
  if (data.description !== undefined) body.description = data.description;
  if (data.isPublic !== undefined) body.is_public = data.isPublic;
  return request<Playlist>('/playlists', { method: 'POST', body });
}

/**
 * 更新播放列表信息
 * @param {string} id - 播放列表 ID
 * @param {Object} data - 更新数据（所有字段可选）
 * @param {string} [data.name] - 新名称
 * @param {string} [data.description] - 新描述
 * @param {boolean} [data.isPublic] - 是否公开
 * @returns {Promise<void>}
 * @example
 * await updatePlaylist('playlist-123', { name: '新名称', isPublic: false });
 */
export async function updatePlaylist(id: string, data: { name?: string; description?: string; isPublic?: boolean }): Promise<void> {
  const body: Record<string, unknown> = {};
  if (data.name !== undefined) body.name = data.name;
  if (data.description !== undefined) body.description = data.description;
  if (data.isPublic !== undefined) body.is_public = data.isPublic;
  await request(`/playlists/${id}`, { method: 'PUT', body });
}

/**
 * 删除播放列表
 * @param {string} id - 播放列表 ID
 * @returns {Promise<void>}
 * @example
 * await deletePlaylist('playlist-123');
 */
export async function deletePlaylist(id: string): Promise<void> {
  await request(`/playlists/${id}`, { method: 'DELETE' });
}

/**
 * 向播放列表添加视频
 * @param {string} playlistId - 播放列表 ID
 * @param {string} videoId - 视频 ID
 * @returns {Promise<void>}
 * @example
 * await addVideoToPlaylist('playlist-123', 'video-456');
 */
export async function addVideoToPlaylist(playlistId: string, videoId: string): Promise<void> {
  await request(`/playlists/${playlistId}/videos`, { method: 'POST', body: { video_id: videoId } });
}

/**
 * 从播放列表移除视频
 * @param {string} playlistId - 播放列表 ID
 * @param {string} videoId - 视频 ID
 * @returns {Promise<void>}
 * @example
 * await removeVideoFromPlaylist('playlist-123', 'video-456');
 */
export async function removeVideoFromPlaylist(playlistId: string, videoId: string): Promise<void> {
  await request(`/playlists/${playlistId}/videos/${videoId}`, { method: 'DELETE' });
}
