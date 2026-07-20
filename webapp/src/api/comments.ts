import { request } from './client'

export interface Comment {
  id: number
  videoId: number
  userId: number
  username: string
  avatarUrl: string | null
  content: string
  parentId: number | null
  createdAt: string
}

export interface CommentListResponse {
  comments: Comment[]
  total: number
}

export async function listComments(videoId: number, page = 0, size = 20): Promise<CommentListResponse> {
  return request<CommentListResponse>(`/videos/${videoId}/comments?page=${page}&size=${size}`)
}

export async function listReplies(commentId: number): Promise<Comment[]> {
  return request<Comment[]>(`/comments/${commentId}/replies`)
}

export async function createComment(videoId: number, content: string, parentId?: number): Promise<Comment> {
  return request<Comment>(`/videos/${videoId}/comments`, {
    method: 'POST',
    body: { content, parent_id: parentId },
  })
}

export async function deleteComment(commentId: number): Promise<void> {
  await request(`/comments/${commentId}`, { method: 'DELETE' })
}
