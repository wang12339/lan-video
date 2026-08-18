import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'

vi.mock('../api/client', () => ({
  request: vi.fn(),
  getToken: vi.fn(() => null),
}))

import { request } from '../api/client'
import { track, trackClick, trackPage, trackVideo, initTrackRouter } from '../utils/track'

const mockedRequest = vi.mocked(request)

describe('track', () => {
  beforeEach(() => {
    mockedRequest.mockReset()
    mockedRequest.mockResolvedValue(undefined)
  })

  it('posts the event to /admin/track without auth when logged out', async () => {
    await track({ action: '点击', target: 'x' })
    expect(mockedRequest).toHaveBeenCalledWith('/admin/track', {
      method: 'POST',
      body: { action: '点击', target: 'x', page: undefined },
      auth: false,
    })
  })

  it('uses auth when a token exists', async () => {
    const { getToken } = await import('../api/client')
    vi.mocked(getToken).mockReturnValue('tok123')
    await track({ action: 'a' })
    expect(mockedRequest.mock.calls[0]?.[1]?.auth).toBe(true)
    vi.mocked(getToken).mockReturnValue(null)
  })

  it('swallows request failures silently', async () => {
    mockedRequest.mockRejectedValueOnce(new Error('boom'))
    await expect(track({ action: 'a' })).resolves.toBeUndefined()
  })

  it('trackClick adds the current pathname as page', async () => {
    window.history.pushState({}, '', '/videos/42')
    trackClick('点击视频', '标题')
    await vi.waitFor(() => expect(mockedRequest).toHaveBeenCalledTimes(1))
    expect(mockedRequest.mock.calls[0]?.[1]?.body).toEqual({
      action: '点击视频', target: '标题', page: '/videos/42',
    })
  })

  it('trackPage sends the page visit event', async () => {
    trackPage('/home')
    await vi.waitFor(() => expect(mockedRequest).toHaveBeenCalledTimes(1))
    expect(mockedRequest.mock.calls[0]?.[1]?.body).toEqual({
      action: '页面访问', page: '/home',
    })
  })

  it('trackVideo prefixes the video id', async () => {
    trackVideo('播放', 7)
    await vi.waitFor(() => expect(mockedRequest).toHaveBeenCalledTimes(1))
    expect(mockedRequest.mock.calls[0]?.[1]?.body).toEqual({
      action: '播放', target: '视频#7', page: window.location.pathname,
    })
  })
})

describe('initTrackRouter', () => {
  beforeEach(() => {
    mockedRequest.mockReset()
    mockedRequest.mockResolvedValue(undefined)
    window.history.replaceState({}, '', '/')
    document.body.innerHTML = ''
  })

  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('tracks the initial visit, popstate navigation, and internal link clicks only', async () => {
    initTrackRouter()

    // 1. initial page visit
    expect(mockedRequest).toHaveBeenCalledTimes(1)
    expect(mockedRequest.mock.calls[0]?.[1]?.body).toEqual({
      action: '页面访问', page: '/',
    })

    // 2. popstate with unchanged path is not re-tracked
    window.dispatchEvent(new Event('popstate'))
    expect(mockedRequest).toHaveBeenCalledTimes(1)

    // 3. popstate with a new path is tracked
    window.history.pushState({}, '', '/settings')
    window.dispatchEvent(new Event('popstate'))
    await vi.waitFor(() => expect(mockedRequest).toHaveBeenCalledTimes(2))
    expect(mockedRequest.mock.calls[1]?.[1]?.body).toEqual({
      action: '页面访问', page: '/settings',
    })

    // 4. internal links are tracked; webapp assets / external / href-less links are not
    document.body.innerHTML = [
      '<a href="/videos/1">内部链接</a>',
      '<a href="/webapp/foo">应用资源</a>',
      '<a href="https://example.com">外部链接</a>',
      '<a>无 href</a>',
    ].join('')
    document.querySelectorAll('a').forEach(a =>
      a.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    )
    await vi.waitFor(() => expect(mockedRequest).toHaveBeenCalledTimes(3))
    expect(mockedRequest.mock.calls[2]?.[1]?.body).toEqual({
      action: '导航', target: '/videos/1', page: '/settings',
    })
  })
})
