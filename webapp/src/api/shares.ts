import { request } from './client'

export interface ShareLink {
  id: string
  videoId: string
  token: string
  shareUrl: string
  expiresAt: string | null
  createdAt: string
}

/** Response from GET /auth/user/shares — no raw token is ever returned. */
export interface ShareListItem {
  id: string
  expiresAt: string | null
  createdAt: string
  active: boolean
}

export interface ShareVideoInfo {
  id: string
  title: string
  description: string | null
  category: string
  thumbUrl: string | null
  sourceType: string
  streamUrl: string
  share: {
    id: string
    expiresAt: string | null
  }
}

export async function createShareLink(videoId: string, expiresInDays?: number): Promise<ShareLink> {
  // 未指定有效期时不传字段，交给后端默认策略（不依赖 JSON.stringify 丢弃 undefined 的行为）
  const body = expiresInDays !== undefined ? { expiresInDays } : {}
  return request<ShareLink>(`/videos/${videoId}/share`, {
    method: 'POST',
    body,
  })
}

export async function getShareVideo(token: string): Promise<ShareVideoInfo> {
  return request<ShareVideoInfo>(`/share/${token}`)
}

export async function deleteShareLink(videoId: string, shareId: string): Promise<void> {
  await request(`/videos/${videoId}/share/${shareId}`, { method: 'DELETE' })
}

/** GET /auth/user/shares — list share links owned by the current user. */
export async function listMyShares(): Promise<ShareListItem[]> {
  return request<ShareListItem[]>('/auth/user/shares')
}

/** DELETE /auth/user/shares/{shareId} — revoke a share by its id. */
export async function revokeMyShare(shareId: string): Promise<void> {
  await request(`/auth/user/shares/${shareId}`, { method: 'DELETE' })
}
