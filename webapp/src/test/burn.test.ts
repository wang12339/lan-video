import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'
import { burnVideo, uploadResumeChunk } from '../api/videos'
import { mapVideo } from '../api/utils'
import { clearToken, saveToken, cacheClear, setOnError, setOnAuthRequired } from '../api/client'
import type { Video } from '../api/types'
import '../i18n'

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: vi.fn().mockResolvedValue(body),
  } as unknown as Response
}

function makeVideo(overrides: Partial<Video> = {}): Video {
  return {
    id: '7',
    title: '阅后即焚视频',
    description: '',
    sourceType: 'local_video',
    coverUrl: null,
    streamUrl: '/media/v1.mp4',
    thumbUrl: null,
    category: '科技',
    views: 0,
    duration: 60,
    createdAt: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

describe('burn after watch', () => {
  beforeEach(() => {
    clearToken()
    setOnError(() => {})
    setOnAuthRequired(() => {})
    cacheClear()
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  describe('mapVideo', () => {
    it('maps burnAfterWatch=true', () => {
      const m = mapVideo(makeVideo({ burnAfterWatch: true }))
      expect(m?.burnAfterWatch).toBe(true)
    })

    it('defaults burnAfterWatch to undefined when absent', () => {
      const m = mapVideo(makeVideo())
      expect(m?.burnAfterWatch).toBeUndefined()
    })
  })

  describe('burnVideo', () => {
    it('POSTs to /videos/{id}/burn with bearer token', async () => {
      const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }))
      vi.stubGlobal('fetch', fetchMock)
      saveToken('tok-123')
      await expect(burnVideo('abc123')).resolves.toBeUndefined()
      const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit]
      expect(url).toBe('/videos/abc123/burn')
      expect(init.method).toBe('POST')
      expect((init.headers as Record<string, string>).Authorization).toBe('Bearer tok-123')
    })
  })

  describe('uploadResumeChunk burn header', () => {
    it('sets x-upload-burn when burnAfterWatch is enabled', async () => {
      const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { received: 100, id: '9' }))
      vi.stubGlobal('fetch', fetchMock)
      saveToken('tok-123')
      await uploadResumeChunk('hash1', 'a.mp4', 100, 'local', new Blob(['x']), true)
      const [, init] = fetchMock.mock.calls[0] as [string, RequestInit]
      expect((init.headers as Record<string, string>)['x-upload-burn']).toBe('1')
    })

    it('omits x-upload-burn by default', async () => {
      const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { received: 100 }))
      vi.stubGlobal('fetch', fetchMock)
      await uploadResumeChunk('hash2', 'b.mp4', 100, 'local', new Blob(['x']))
      const [, init] = fetchMock.mock.calls[0] as [string, RequestInit]
      expect((init.headers as Record<string, string>)['x-upload-burn']).toBeUndefined()
    })
  })
})
