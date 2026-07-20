import { request } from './client'

export interface ShareLink {
  id: number
  videoId: number
  token: string
  shareUrl: string
  expiresAt: string | null
  createdAt: string
}

/** Response from GET /auth/user/shares — no raw token is ever returned. */
export interface ShareListItem {
  id: number
  expiresAt: string | null
  createdAt: string
  active: boolean
}

export interface ShareVideoInfo {
  id: number
  title: string
  description: string | null
  category: string
  thumbUrl: string | null
  sourceType: string
  streamUrl: string
  share: {
    id: number
    expiresAt: string | null
  }
}

export async function createShareLink(videoId: number, expiresInDays?: number): Promise<ShareLink> {
  return request<ShareLink>(`/videos/${videoId}/share`, {
    method: 'POST',
    body: { expiresInDays },
  })
}

export async function getShareVideo(token: string): Promise<ShareVideoInfo> {
  return request<ShareVideoInfo>(`/share/${token}`)
}

export async function deleteShareLink(videoId: number, shareId: number): Promise<void> {
  await request(`/videos/${videoId}/share/${shareId}`, { method: 'DELETE' })
}

/** GET /auth/user/shares — list share links owned by the current user. */
export async function listMyShares(): Promise<ShareListItem[]> {
  return request<ShareListItem[]>('/auth/user/shares')
}

/** DELETE /auth/user/shares/{shareId} — revoke a share by its id. */
export async function revokeMyShare(shareId: number): Promise<void> {
  await request(`/auth/user/shares/${shareId}`, { method: 'DELETE' })
}
