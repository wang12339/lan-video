import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'
import { burnVideo } from '../api/videos'
import { clearToken, saveToken, setOnError, setOnAuthRequired } from '../api/client'
import '../i18n'

describe('burn after watch (platform-wide)', () => {
  beforeEach(() => {
    clearToken()
    setOnError(() => {})
    setOnAuthRequired(() => {})
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

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
