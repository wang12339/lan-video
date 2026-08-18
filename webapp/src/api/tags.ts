// 标签 API

import { request } from './client';

export interface Tag {
  id: number;
  name: string;
  color: string | null;
  usageCount: number;
}

export interface TagListResponse {
  tags: Tag[];
}

export interface TagCreateRequest {
  name: string;
  color?: string;
}

export interface TagUpdateRequest {
  name?: string;
  color?: string;
}

// Public
export async function listTags(): Promise<Tag[]> {
  const res = await request<TagListResponse>('/tags');
  return res.tags;
}

export async function getPopularTags(): Promise<Tag[]> {
  const res = await request<TagListResponse>('/tags/popular');
  return res.tags;
}

// Admin
export async function createTag(data: TagCreateRequest): Promise<Tag> {
  return request<Tag>('/admin/tags', { method: 'POST', body: data });
}

export async function updateTag(id: number, data: TagUpdateRequest): Promise<Tag> {
  return request<Tag>(`/admin/tags/${id}`, { method: 'PUT', body: data });
}

export async function deleteTag(id: number): Promise<void> {
  await request(`/admin/tags/${id}`, { method: 'DELETE' });
}

// Video tags
export async function getVideoTags(videoId: number): Promise<Tag[]> {
  const res = await request<TagListResponse>(`/videos/${videoId}/tags`);
  return res.tags;
}

export async function addVideoTags(videoId: number, tagIds: number[]): Promise<void> {
  await request(`/videos/${videoId}/tags`, { method: 'POST', body: tagIds });
}

export async function removeVideoTag(videoId: number, tagId: number): Promise<void> {
  await request(`/videos/${videoId}/tags/${tagId}`, { method: 'DELETE' });
}
