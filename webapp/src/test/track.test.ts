import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'

vi.mock('../api/client', () => ({
  request: vi.fn(),
  getToken: vi.fn(() => null),
}))

import { request, getToken } from '../api/client'
import { track, trackClick, trackPage, trackVideo, initTrackRouter } from '../utils/track'

const mockedRequest = vi.mocked(request)
const mockedGetToken = vi.mocked(getToken)

describe('track', () => {
  beforeEach(() => {
    mockedRequest.mockReset()
    mockedRequest.mockResolvedValue(undefined)
    mockedGetToken.mockReset()
    mockedGetToken.mockReturnValue(null)
  })

  it('skips request when not logged in', async () => {
    await track({ action: '点击', target: 'x' })
    expect(mockedRequest).not.toHaveBeenCalled()
  })

  it('sends request with auth and silent when logged in', async () => {
    mockedGetToken.mockReturnValue('tok123')
    await track({ action: '点击', target: 'x' })
    expect(mockedRequest).toHaveBeenCalledWith('/admin/track', {
      method: 'POST',
      body: { action: '点击', target: 'x', page: undefined },
      auth: true,
      silent: true,
    })
  })

  it('swallows request failures silently', async () => {
    mockedGetToken.mockReturnValue('tok123')
    mockedRequest.mockRejectedValueOnce(new Error('boom'))
    await expect(track({ action: 'a' })).resolves.toBeUndefined()
  })

  it('trackClick adds the current pathname as page', async () => {
    mockedGetToken.mockReturnValue('tok123')
    window.history.pushState({}, '', '/videos/42')
    trackClick('点击视频', '标题')
    await vi.waitFor(() => expect(mockedRequest).toHaveBeenCalledTimes(1))
    expect(mockedRequest.mock.calls[0]?.[1]?.body).toEqual({
      action: '点击视频', target: '标题', page: '/videos/42',
    })
  })

  it('trackPage sends the page visit event', async () => {
    mockedGetToken.mockReturnValue('tok123')
    trackPage('/home')
    await vi.waitFor(() => expect(mockedRequest).toHaveBeenCalledTimes(1))
    expect(mockedRequest.mock.calls[0]?.[1]?.body).toEqual({
      action: '页面访问', page: '/home',
    })
  })

  it('trackVideo prefixes the video id', async () => {
    mockedGetToken.mockReturnValue('tok123')
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
    mockedGetToken.mockReset()
    mockedGetToken.mockReturnValue('tok123')
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
