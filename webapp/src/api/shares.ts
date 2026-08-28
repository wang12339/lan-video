import { request, ValidationError } from './client'

export interface ShareLink {
  id: string
  videoId: string
  token: string
  shareUrl: string
  expiresAt: string | null
  createdAt: string
}

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

const VALID_ID = /^[a-zA-Z0-9_-]+$/

function assertValidId(id: string, label: string): void {
  if (!id || !VALID_ID.test(id)) {
    throw new ValidationError(`Invalid ${label}`, label)
  }
}

export async function createShareLink(videoId: string, expiresInDays?: number): Promise<ShareLink> {
  assertValidId(videoId, 'videoId')
  if (expiresInDays !== undefined && (!Number.isInteger(expiresInDays) || expiresInDays < 1 || expiresInDays > 365)) {
    throw new ValidationError('expiresInDays must be between 1 and 365', 'expiresInDays')
  }
  const body = expiresInDays !== undefined ? { expiresInDays } : {}
  return request<ShareLink>(`/videos/${videoId}/share`, { method: 'POST', body })
}

export async function getShareVideo(token: string): Promise<ShareVideoInfo> {
  assertValidId(token, 'token')
  return request<ShareVideoInfo>(`/share/${token}`)
}

export async function deleteShareLink(videoId: string, shareId: string): Promise<void> {
  assertValidId(videoId, 'videoId')
  assertValidId(shareId, 'shareId')
  await request(`/videos/${videoId}/share/${shareId}`, { method: 'DELETE' })
}

export async function listMyShares(): Promise<ShareListItem[]> {
  return request<ShareListItem[]>('/auth/user/shares')
}

export async function revokeMyShare(shareId: string): Promise<void> {
  assertValidId(shareId, 'shareId')
  await request(`/auth/user/shares/${shareId}`, { method: 'DELETE' })
}
