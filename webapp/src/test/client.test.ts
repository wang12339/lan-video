import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'
import {
  request, health, getToken, saveToken, clearToken, setOnError, setOnAuthRequired,
  cacheClear, APIError,
} from '../api/client'
import '../i18n'

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: vi.fn().mockResolvedValue(body),
  } as unknown as Response
}

function invalidJsonResponse(status: number): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: vi.fn().mockRejectedValue(new SyntaxError('Unexpected token in JSON')),
  } as unknown as Response
}

describe('client request', () => {
  beforeEach(() => {
    clearToken()
    setOnError(() => {})
    setOnAuthRequired(() => {})
    cacheClear()
    vi.clearAllMocks()
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  it('returns parsed JSON for successful GET requests', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(200, { data: 1 })))
    await expect(request<{ data: number }>('/videos')).resolves.toEqual({ data: 1 })
  })

  it('sends JSON body, headers, and credentials', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await request('/videos/1', { method: 'PATCH', body: { title: '新标题' } })
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/videos/1')
    expect(init.method).toBe('PATCH')
    expect(init.body).toBe(JSON.stringify({ title: '新标题' }))
    expect((init.headers as Record<string, string>)['Content-Type']).toBe('application/json')
    expect((init.headers as Record<string, string>)['X-Requested-With']).toBe('XMLHttpRequest')
    expect(init.credentials).toBe('same-origin')
    expect((init.headers as Record<string, string>).Authorization).toBeUndefined()
  })

  it('adds the Bearer token when auth is enabled and a token exists', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}))
    vi.stubGlobal('fetch', fetchMock)
    saveToken('secret-token')
    await request('/videos')
    const init = fetchMock.mock.calls[0]?.[1] as RequestInit
    expect((init.headers as Record<string, string>).Authorization).toBe('Bearer secret-token')
  })

  it('skips the token when auth is disabled', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}))
    vi.stubGlobal('fetch', fetchMock)
    saveToken('secret-token')
    await request('/videos', { auth: false })
    const init = fetchMock.mock.calls[0]?.[1] as RequestInit
    expect((init.headers as Record<string, string>).Authorization).toBeUndefined()
  })

  it('caches GET responses and serves them without refetching', async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(jsonResponse(200, { data: 1 }))
      .mockResolvedValueOnce(jsonResponse(200, { data: 2 }))
    vi.stubGlobal('fetch', fetchMock)
    const first = await request('/videos')
    const second = await request('/videos')
    expect(first).toEqual({ data: 1 })
    expect(second).toEqual({ data: 1 })
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })

  it('invalidates GET cache after a mutation on the same path', async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(jsonResponse(200, { data: 1 }))
      .mockResolvedValueOnce(jsonResponse(204, null))
      .mockResolvedValueOnce(jsonResponse(200, { data: 2 }))
    vi.stubGlobal('fetch', fetchMock)
    await request('/videos')
    await expect(request('/videos', { method: 'POST', body: {} })).resolves.toBeNull()
    const after = await request('/videos')
    expect(after).toEqual({ data: 2 })
    expect(fetchMock).toHaveBeenCalledTimes(3)
  })

  it('skips the cache when skipCache is set', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { data: 1 }))
    vi.stubGlobal('fetch', fetchMock)
    await request('/videos', { skipCache: true })
    await request('/videos', { skipCache: true })
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })

  it('keeps Chinese backend messages and localizes English ones', async () => {
    vi.stubGlobal('fetch', vi.fn()
      .mockResolvedValueOnce(jsonResponse(404, { error: '视频不存在' }))
      .mockResolvedValueOnce(jsonResponse(404, { error: 'not found' })))
    await expect(request('/videos/1')).rejects.toThrow('视频不存在')
    await expect(request('/videos/2')).rejects.toThrow('资源不存在')
  })

  it('localizes server errors by status code', async () => {
    vi.stubGlobal('fetch', vi.fn()
      .mockResolvedValueOnce(jsonResponse(500, { error: 'internal server error' }))
      .mockResolvedValueOnce(jsonResponse(429, { error: 'too many requests' }))
      .mockResolvedValueOnce(jsonResponse(400, {})))
    await expect(request('/videos')).rejects.toThrow('服务器内部错误')
    await expect(request('/videos')).rejects.toThrow('请求过于频繁，请稍后再试')
    await expect(request('/videos')).rejects.toThrow('发生未知错误')
  })

  it('throws localized errors for non-JSON error responses', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(invalidJsonResponse(500)))
    await expect(request('/videos')).rejects.toThrow('服务器内部错误')
  })

  it('notifies the global error callback with an APIError', async () => {
    const onError = vi.fn()
    setOnError(onError)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(404, { error: 'not found' })))
    await expect(request('/videos')).rejects.toThrow('资源不存在')
    expect(onError).toHaveBeenCalledTimes(1)
    const err = onError.mock.calls[0]?.[0] as APIError
    expect(err).toBeInstanceOf(APIError)
    expect(err.status).toBe(404)
  })

  it('maps a 401 to AuthError, clears the token, and notifies the auth callback', async () => {
    const onAuth = vi.fn()
    setOnAuthRequired(onAuth)
    saveToken('stale-token')
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(401, { error: 'authentication failed' })))
    await expect(request('/me')).rejects.toThrow('请登录后继续')
    expect(getToken()).toBeNull()
    expect(onAuth).toHaveBeenCalledWith('请登录后继续')
  })

  it('keeps Chinese messages on 401 and passes them to the auth callback', async () => {
    const onAuth = vi.fn()
    setOnAuthRequired(onAuth)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(401, { error: '登录已过期' })))
    await expect(request('/me')).rejects.toThrow('登录已过期')
    expect(onAuth).toHaveBeenCalledWith('登录已过期')
  })

  it('handles 401 with a non-JSON body', async () => {
    const onAuth = vi.fn()
    setOnAuthRequired(onAuth)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(invalidJsonResponse(401)))
    await expect(request('/me')).rejects.toThrow('请登录后继续')
    expect(onAuth).toHaveBeenCalled()
  })

  it('throws a localized APIError on network failure', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')))
    const err = await request('/videos').catch((e: unknown) => e)
    expect(err).toBeInstanceOf(APIError)
    expect((err as APIError).message).toBe('网络连接失败')
    expect((err as APIError).status).toBe(0)
  })

  it('maps an internal timeout to a localized APIError', async () => {
    vi.useFakeTimers()
    vi.stubGlobal('fetch', vi.fn((_url: string, init: RequestInit) => new Promise((_resolve, reject) => {
      init.signal?.addEventListener('abort', () =>
        reject(new DOMException('The operation was aborted.', 'AbortError'))
      )
    })))
    const pending = request('/videos').then(
      () => { throw new Error('should have rejected') },
      (e: unknown) => e
    )
    vi.advanceTimersByTime(15000)
    const err = await pending
    expect(err).toBeInstanceOf(APIError)
    expect((err as APIError).message).toBe('请求超时')
    expect((err as APIError).status).toBe(0)
  })

  it('rethrows caller-abort errors without an APIError', async () => {
    const controller = new AbortController()
    vi.stubGlobal('fetch', vi.fn((_url: string, init: RequestInit) => new Promise((_resolve, reject) => {
      init.signal?.addEventListener('abort', () =>
        reject(new DOMException('The operation was aborted.', 'AbortError'))
      )
    })))
    const pending = request('/videos', { signal: controller.signal }).then(
      () => { throw new Error('should have rejected') },
      (e: unknown) => e
    )
    controller.abort()
    const err = await pending
    expect((err as Error).name).toBe('AbortError')
  })

  it('does not notify the global error callback when silent is set', async () => {
    const onError = vi.fn()
    setOnError(onError)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(500, { error: '服务器开小差了' })))
    const err = await request('/videos', { silent: true }).catch((e: unknown) => e)
    expect(err).toBeInstanceOf(APIError)
    expect((err as APIError).message).toBe('服务器开小差了')
    expect(onError).not.toHaveBeenCalled()
  })

  it('sends Blob bodies directly without a JSON Content-Type', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { received: 5 }))
    vi.stubGlobal('fetch', fetchMock)
    const blob = new Blob(['chunk-data'])
    await request('/admin/videos/upload-resume', { method: 'POST', body: blob })
    const init = fetchMock.mock.calls[0]?.[1] as RequestInit
    expect(init.body).toBe(blob)
    const headers = init.headers as Record<string, string>
    expect(headers['Content-Type']).toBeUndefined()
    expect(headers['X-Requested-With']).toBe('XMLHttpRequest')
  })

  it('returns null for 204 responses', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(204, null)))
    await expect(request('/videos', { method: 'DELETE' })).resolves.toBeNull()
  })
})

describe('client health', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('returns true when the health endpoint responds ok', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true } as Response))
    await expect(health()).resolves.toBe(true)
  })

  it('returns false on failure', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')))
    await expect(health()).resolves.toBe(false)
  })

  it('returns false when the endpoint is not ok', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false } as Response))
    await expect(health()).resolves.toBe(false)
  })
})
