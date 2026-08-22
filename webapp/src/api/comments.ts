/**
 * 评论 API 模块
 * @module comments
 */

import { request } from './client';

/**
 * 评论数据结构
 */
export interface Comment {
  /** 评论唯一标识 */
  id: string;
  /** 所属视频 ID */
  videoId: string;
  /** 评论用户 ID */
  userId: string;
  /** 评论用户名 */
  username: string;
  /** 用户头像 URL，null 表示无头像 */
  avatarUrl: string | null;
  /** 评论内容 */
  content: string;
  /** 父评论 ID，null 表示顶级评论 */
  parentId: string | null;
  /** 评论创建时间（ISO 8601 格式） */
  createdAt: string;
}

/**
 * 评论列表分页响应结构
 */
export interface CommentListResponse {
  /** 评论列表 */
  comments: Comment[];
  /** 评论总数 */
  total: number;
}

/** 后端 /videos/{id}/comments 的 size 上限 */
const MAX_COMMENTS_SIZE = 100;

/**
 * 获取视频的评论列表（分页）
 *
 * @param videoId - 视频 ID
 * @param page - 页码，从 0 开始，默认 0
 * @param size - 每页数量，默认 20，最大 100
 * @returns 评论列表及总数
 */
export async function listComments(videoId: string, page = 0, size = 20): Promise<CommentListResponse> {
  const params = new URLSearchParams();
  params.set('page', String(Math.max(0, page)));
  params.set('size', String(Math.min(Math.max(1, size), MAX_COMMENTS_SIZE)));
  return request<CommentListResponse>(`/videos/${videoId}/comments?${params}`);
}

/**
 * 获取评论的回复列表
 *
 * @param commentId - 父评论 ID
 * @returns 回复评论数组
 */
export async function listReplies(commentId: string): Promise<Comment[]> {
  return request<Comment[]>(`/comments/${commentId}/replies`);
}

/**
 * 创建新评论或回复
 *
 * @param videoId - 视频 ID
 * @param content - 评论内容
 * @param parentId - 可选，父评论 ID；省略则创建顶级评论
 * @returns 创建的评论对象
 */
export async function createComment(videoId: string, content: string, parentId?: string): Promise<Comment> {
  const body: { content: string; parent_id?: string } = { content };
  if (parentId !== undefined) body.parent_id = parentId;
  return request<Comment>(`/videos/${videoId}/comments`, {
    method: 'POST',
    body,
  });
}

/**
 * 删除评论
 *
 * @param commentId - 要删除的评论 ID
 * @returns 删除成功时无返回值
 */
export async function deleteComment(commentId: string): Promise<void> {
  await request(`/comments/${commentId}`, { method: 'DELETE' });
}
