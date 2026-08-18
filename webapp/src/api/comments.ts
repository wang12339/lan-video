// 评论 API

import { request } from './client';

export interface Comment {
  id: string;
  videoId: string;
  userId: string;
  username: string;
  avatarUrl: string | null;
  content: string;
  parentId: string | null;
  createdAt: string;
}

export interface CommentListResponse {
  comments: Comment[];
  total: number;
}

const MAX_COMMENTS_SIZE = 100; // 后端 /videos/{id}/comments 的 size 上限

export async function listComments(videoId: string, page = 0, size = 20): Promise<CommentListResponse> {
  const params = new URLSearchParams();
  params.set('page', String(Math.max(0, page)));
  params.set('size', String(Math.min(Math.max(1, size), MAX_COMMENTS_SIZE)));
  return request<CommentListResponse>(`/videos/${videoId}/comments?${params}`);
}

export async function listReplies(commentId: string): Promise<Comment[]> {
  return request<Comment[]>(`/comments/${commentId}/replies`);
}

export async function createComment(videoId: string, content: string, parentId?: string): Promise<Comment> {
  const body: { content: string; parent_id?: string } = { content };
  if (parentId !== undefined) body.parent_id = parentId;
  return request<Comment>(`/videos/${videoId}/comments`, {
    method: 'POST',
    body,
  });
}

export async function deleteComment(commentId: string): Promise<void> {
  await request(`/comments/${commentId}`, { method: 'DELETE' });
}
