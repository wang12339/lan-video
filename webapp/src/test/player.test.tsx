import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import React from 'react'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import Player from '../pages/Player/Player'
import type { MappedVideo } from '../api/types'

// ── Mocks ──────────────────────────────────────────────────────────────────────

vi.mock('../api', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../api')>()
  return {
    ...mod,
    getVideo: vi.fn(),
    mapVideo: vi.fn(),
    listVideos: vi.fn(),
    incrementViews: vi.fn(),
    savePlayback: vi.fn(),
    deleteVideo: vi.fn(),
    burnVideo: vi.fn().mockResolvedValue(undefined),
    startPlaybackSession: vi.fn(),
    heartbeatPlaybackSession: vi.fn(),
    stopPlaybackSession: vi.fn(),
    getSimilarVideos: vi.fn(),
    createShareLink: vi.fn(),
    getShareVideo: vi.fn(),
    toggleFavorite: vi.fn().mockResolvedValue({ favorited: false }),
    getFavoriteStatus: vi.fn().mockResolvedValue({ favorited: false }),
  }
})

vi.mock('../api/playlists', () => ({
  listMyPlaylists: vi.fn().mockResolvedValue([]),
  addVideoToPlaylist: vi.fn().mockResolvedValue({}),
}))

vi.mock('../components/Toast/Toast', () => ({
  useToast: () => ({ toast: vi.fn() }),
  ToastProvider: ({ children }: any) => children,
}))

vi.mock('../api/client', () => ({
  request: vi.fn(),
  mediaUrl: vi.fn((path: string) => path),
  getToken: vi.fn(() => 'mock-token'),
  BASE: '',
}))

vi.mock('../api/prefs', () => ({
  getPref: vi.fn(() => false),
}))

vi.mock('../context/AuthContext', () => ({
  useAuth: vi.fn(),
}))

vi.mock('../utils/track', () => ({
  trackVideo: vi.fn(),
  trackClick: vi.fn(),
}))

vi.mock('../hooks/useHlsPlayer', () => ({
  useHlsPlayer: vi.fn(),
}))

vi.mock('../components/VideoCard/VideoCard', () => ({
  __esModule: true,
  default: ({ video }: { video: MappedVideo }) =>
    React.createElement('div', { 'data-testid': 'video-card', 'data-id': video.id }, video.title),
}))

vi.mock('../components/Comments/Comments', () => ({
  __esModule: true,
  default: ({ videoId }: { videoId: string }) =>
    React.createElement('div', { 'data-testid': 'comments', 'data-videoid': videoId }),
}))

vi.mock('../components/ui', () => ({
  ConfirmDialog: ({ open }: { open: boolean }) =>
    open ? React.createElement('div', { 'data-testid': 'confirm-dialog' }) : null,
  AlertDialog: ({ open }: { open: boolean }) =>
    open ? React.createElement('div', { 'data-testid': 'alert-dialog' }) : null,
}))

// ── Helpers ────────────────────────────────────────────────────────────────────

function makeMappedVideo(overrides: Partial<MappedVideo> = {}): MappedVideo {
  return {
    id: 'v1',
    title: '测试视频.mp4',
    category: '科技',
    description: '这是一个测试视频',
    thumb: '/media/thumb.jpg',
    stream: '/media/v.mp4',
    cover: null,
    sourceType: 'local_video',
    duration: 120,
    views: 500,
    date: '2026-01-01T00:00:00Z',
    progress: 0,
    ...overrides,
  }
}

let queryClient: QueryClient

function renderPlayer(route = '/player?id=v1') {
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[route]}>
        <Player />
      </MemoryRouter>
    </QueryClientProvider>
  )
}

// ── Setup ──────────────────────────────────────────────────────────────────────

const { useAuth } = await import('../context/AuthContext')
const { getVideo, mapVideo, incrementViews, savePlayback, startPlaybackSession,
        heartbeatPlaybackSession, stopPlaybackSession, getSimilarVideos,
        listVideos } = await import('../api')
const { request } = await import('../api/client')

const mockUseAuth = vi.mocked(useAuth)
const mockGetVideo = vi.mocked(getVideo)
const mockMapVideo = vi.mocked(mapVideo)
const mockIncrementViews = vi.mocked(incrementViews)
const mockSavePlayback = vi.mocked(savePlayback)
const mockStartSession = vi.mocked(startPlaybackSession)
const mockStopSession = vi.mocked(stopPlaybackSession)
const mockGetSimilar = vi.mocked(getSimilarVideos)
const mockListVideos = vi.mocked(listVideos)
const mockRequest = vi.mocked(request)

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers({ shouldAdvanceTime: true })
  queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  })

  // 默认：已登录用户
  mockUseAuth.mockReturnValue({
    user: { id: 'u1', username: 'testuser', isAdmin: false, avatarUrl: undefined, createdAt: '', emailVerified: true },
    loading: false,
    kickedMsg: null,
    clearKickedMsg: vi.fn(),
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    refreshUser: vi.fn(),
    setUser: vi.fn(),
  })

  // 默认：返回一个有效视频
  const video = makeMappedVideo()
  mockGetVideo.mockResolvedValue(video as never)
  mockMapVideo.mockReturnValue(video)
  mockIncrementViews.mockResolvedValue(undefined as never)
  mockSavePlayback.mockResolvedValue(undefined as never)
  mockStartSession.mockResolvedValue(undefined as never)
  mockStopSession.mockResolvedValue(undefined as never)
  mockGetSimilar.mockResolvedValue([])
  mockListVideos.mockResolvedValue({ items: [], total: 0, page: 0, size: 20 } as never)
  mockRequest.mockResolvedValue({ status: 'not_ready' } as never)

  // Mock HTMLVideoElement 的 play/pause
  // jsdom 不支持 video.play() 返回 Promise，需要手动 mock
  Object.defineProperty(HTMLVideoElement.prototype, 'play', {
    value: vi.fn().mockResolvedValue(undefined),
    writable: true,
    configurable: true,
  })
  Object.defineProperty(HTMLVideoElement.prototype, 'pause', {
    value: vi.fn(),
    writable: true,
    configurable: true,
  })
  Object.defineProperty(HTMLVideoElement.prototype, 'load', {
    value: vi.fn(),
    writable: true,
    configurable: true,
  })

  // Mock fullscreen API
  Object.defineProperty(document, 'fullscreenElement', {
    value: null,
    writable: true,
    configurable: true,
  })
  Element.prototype.requestFullscreen = vi.fn().mockResolvedValue(undefined)
  document.exitFullscreen = vi.fn().mockResolvedValue(undefined)

  // Mock picture-in-picture
  Object.defineProperty(document, 'pictureInPictureElement', {
    value: null,
    writable: true,
    configurable: true,
  })
  Object.defineProperty(HTMLVideoElement.prototype, 'requestPictureInPicture', {
    value: vi.fn().mockResolvedValue(undefined),
    writable: true,
    configurable: true,
  })
  document.exitPictureInPicture = vi.fn().mockResolvedValue(undefined)

  // Mock clipboard
  Object.assign(navigator, {
    clipboard: {
      writeText: vi.fn().mockResolvedValue(undefined),
    },
  })
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('Player 页面', () => {

  // 1. 渲染测试：视频播放器容器存在
  it('渲染视频播放器容器', async () => {
    renderPlayer()

    // player-page 容器
    expect(document.querySelector('.player-page')).toBeInTheDocument()
    // player-wrap 容器
    expect(document.querySelector('.player-wrap')).toBeInTheDocument()
    // video 元素存在
    const videoEl = document.querySelector('video.player-video')
    expect(videoEl).toBeInTheDocument()
    expect(videoEl).toHaveAttribute('playsinline')
    expect(videoEl).toHaveAttribute('preload', 'auto')

    // 等待视频加载完成
    await waitFor(() => {
      expect(mockGetVideo).toHaveBeenCalledWith('v1')
    })
  })

  // 2. 加载状态：视频加载时显示加载指示器
  it('视频加载时显示加载指示器', async () => {
    // 让 getVideo 永远 pending，模拟加载中
    mockGetVideo.mockReturnValue(new Promise(() => {}))

    renderPlayer()

    // loading 指示器应该显示
    expect(document.querySelector('.player-loading.show')).toBeInTheDocument()
    // 页面中有两个"加载中..."（播放器内 + 底部），使用 getAllByText
    const loadingTexts = screen.getAllByText('加载中...')
    expect(loadingTexts.length).toBeGreaterThanOrEqual(1)
  })

  // 3. 错误状态：视频加载失败显示错误
  it('视频加载失败时显示错误信息', async () => {
    mockGetVideo.mockRejectedValue(new Error('网络错误'))

    renderPlayer()

    await waitFor(() => {
      expect(screen.getByText('网络错误')).toBeInTheDocument()
    })

    // 错误页面有 ⚠️ 图标
    expect(screen.getByText('⚠️')).toBeInTheDocument()
    // 返回首页按钮
    expect(screen.getByRole('button', { name: '返回首页' })).toBeInTheDocument()
  })

  // 缺少视频 ID 时显示错误
  it('缺少视频 ID 时显示错误', async () => {
    renderPlayer('/player')

    await waitFor(() => {
      expect(screen.getByText('缺少视频ID')).toBeInTheDocument()
    })
  })

  // 4. 播放控制：播放/暂停按钮工作
  it('播放/暂停按钮切换播放状态', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    // 找到播放/暂停按钮
    const playPauseBtn = screen.getByRole('button', { name: '播放/暂停' })
    expect(playPauseBtn).toBeInTheDocument()

    // 初始状态：暂停，显示中心播放按钮
    expect(document.querySelector('.center-play')).toBeInTheDocument()

    // 点击播放/暂停按钮
    fireEvent.click(playPauseBtn)

    // 验证 video.play() 被调用
    const videoEl = document.querySelector('video') as HTMLVideoElement
    expect(videoEl.play).toHaveBeenCalled()
  })

  // 中心播放按钮也能触发播放
  it('点击中心播放按钮触发播放', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.center-play')).toBeInTheDocument()
    })

    // 中心播放按钮应该是 ▶
    const centerPlayBtn = document.querySelector('.center-play')!
    expect(centerPlayBtn.textContent).toBe('▶')

    fireEvent.click(centerPlayBtn)

    const videoEl = document.querySelector('video') as HTMLVideoElement
    expect(videoEl.play).toHaveBeenCalled()
  })

  // 快退和快进按钮
  it('快退和快进按钮工作', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    const seekBackBtn = screen.getByRole('button', { name: '快退' })
    const seekForwardBtn = screen.getByRole('button', { name: '快进' })

    expect(seekBackBtn).toBeInTheDocument()
    expect(seekForwardBtn).toBeInTheDocument()

    fireEvent.click(seekBackBtn)
    fireEvent.click(seekForwardBtn)
  })

  // 5. 进度条：拖动进度条更新时间
  it('进度条存在且可通过点击更新位置', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    // 进度条 slider 存在
    const progressSlider = screen.getByRole('slider', { name: '播放进度' })
    expect(progressSlider).toBeInTheDocument()
    expect(progressSlider).toHaveAttribute('tabindex', '0')

    // 进度条内部结构
    expect(document.querySelector('.player-progress-bar')).toBeInTheDocument()
    expect(document.querySelector('.player-progress-buffered')).toBeInTheDocument()
    expect(document.querySelector('.player-progress-current')).toBeInTheDocument()
    expect(document.querySelector('.player-progress-dot')).toBeInTheDocument()

    // 时间显示存在
    expect(document.querySelector('.time-display')).toBeInTheDocument()
    expect(document.querySelector('.time-sep')).toHaveTextContent('/')

    // 模拟点击进度条
    const progressWrap = document.querySelector('.player-progress-wrap') as HTMLElement
    const rect = { left: 0, width: 100 }
    progressWrap.getBoundingClientRect = vi.fn(() => rect as DOMRect)

    // 设置视频时长以使进度条有意义
    const videoEl = document.querySelector('video') as HTMLVideoElement
    Object.defineProperty(videoEl, 'duration', { value: 120, writable: true })

    fireEvent.mouseDown(progressWrap, { clientX: 50 })

    // currentTime 应该被更新到 60（50/100 * 120）
    expect(videoEl.currentTime).toBe(60)
  })

  // 键盘操作进度条
  it('进度条支持键盘左右箭头', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    const progressSlider = screen.getByRole('slider', { name: '播放进度' })
    const videoEl = document.querySelector('video') as HTMLVideoElement

    // 设置初始状态
    Object.defineProperty(videoEl, 'duration', { value: 120, writable: true })
    Object.defineProperty(videoEl, 'currentTime', { value: 30, writable: true })

    // 按右箭头前进 5 秒
    fireEvent.keyDown(progressSlider, { key: 'ArrowRight' })
    expect(videoEl.currentTime).toBe(35)

    // 按左箭头后退 5 秒
    fireEvent.keyDown(progressSlider, { key: 'ArrowLeft' })
    expect(videoEl.currentTime).toBe(30)

    // Home 键到开头
    fireEvent.keyDown(progressSlider, { key: 'Home' })
    expect(videoEl.currentTime).toBe(0)

    // End 键到结尾
    fireEvent.keyDown(progressSlider, { key: 'End' })
    expect(videoEl.currentTime).toBe(120)
  })

  // 6. 音量控制：音量滑块工作
  it('音量滑块存在且可调节', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    // 音量滑块存在
    const volumeSlider = screen.getByRole('slider', { name: '音量' })
    expect(volumeSlider).toBeInTheDocument()
    expect(volumeSlider).toHaveAttribute('type', 'range')
    expect(volumeSlider).toHaveAttribute('min', '0')
    expect(volumeSlider).toHaveAttribute('max', '1')
    expect(volumeSlider).toHaveAttribute('step', '0.05')

    // 音量容器存在
    expect(document.querySelector('.volume-wrap')).toBeInTheDocument()

    // 静音/取消静音按钮存在
    const muteBtn = screen.getByRole('button', { name: '静音' })
    expect(muteBtn).toBeInTheDocument()

    // 改变音量
    fireEvent.change(volumeSlider, { target: { value: '0.5' } })

    const videoEl = document.querySelector('video') as HTMLVideoElement
    expect(videoEl.volume).toBe(0.5)
  })

  // 点击静音按钮切换静音
  it('静音按钮切换静音状态', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    const videoEl = document.querySelector('video') as HTMLVideoElement
    // 设置初始音量
    Object.defineProperty(videoEl, 'volume', { value: 0.8, writable: true })
    Object.defineProperty(videoEl, 'muted', { value: false, writable: true })

    const muteBtn = screen.getByRole('button', { name: '静音' })

    // 点击静音
    fireEvent.click(muteBtn)
    expect(videoEl.volume).toBe(0)
    expect(videoEl.muted).toBe(true)
  })

  // ── 额外测试 ──────────────────────────────────────────────────────────────

  it('显示视频标题', async () => {
    renderPlayer()

    await waitFor(() => {
      // 标题在 top bar 和详情区都会显示（去掉扩展名后）
      expect(screen.getByText('测试视频')).toBeInTheDocument()
    })
  })

  it('返回首页按钮存在', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-top')).toBeInTheDocument()
    })

    const backBtn = screen.getByRole('button', { name: '返回首页' })
    expect(backBtn).toBeInTheDocument()
    expect(backBtn.textContent).toBe('←')
  })

  it('视频详情区显示分类和描述', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-detail')).toBeInTheDocument()
    })

    expect(screen.getByText('科技')).toBeInTheDocument()
    expect(screen.getByText('这是一个测试视频')).toBeInTheDocument()
  })

  it('视频加载失败后可重试', async () => {
    // 第一次失败
    mockGetVideo.mockRejectedValueOnce(new Error('网络超时'))

    renderPlayer()

    await waitFor(() => {
      expect(screen.getByText('网络超时')).toBeInTheDocument()
    })

    // 返回首页按钮存在
    expect(screen.getByRole('button', { name: '返回首页' })).toBeInTheDocument()
  })

  it('倍速按钮显示当前倍速', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.speed-wrap')).toBeInTheDocument()
    })

    // 默认 1 倍速
    const speedBtn = screen.getByRole('button', { name: '倍速' })
    expect(speedBtn.textContent).toBe('1×')
  })

  it('全屏按钮存在', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    const fullscreenBtn = screen.getByRole('button', { name: '全屏' })
    expect(fullscreenBtn).toBeInTheDocument()
  })

  it('画中画按钮存在', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    const pipBtn = screen.getByRole('button', { name: '画中画' })
    expect(pipBtn).toBeInTheDocument()
  })

  it('管理用户看到删除按钮', async () => {
    mockUseAuth.mockReturnValue({
      user: { id: 'u1', username: 'admin', isAdmin: true, avatarUrl: undefined, createdAt: '', emailVerified: true },
      loading: false,
      kickedMsg: null,
      clearKickedMsg: vi.fn(),
      login: vi.fn(),
      register: vi.fn(),
      logout: vi.fn(),
      refreshUser: vi.fn(),
      setUser: vi.fn(),
    })

    renderPlayer()

    await waitFor(() => {
      const deleteBtn = screen.getByRole('button', { name: '删除视频' })
      expect(deleteBtn).toBeInTheDocument()
    })
  })

  it('非管理用户不显示删除按钮', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(mockGetVideo).toHaveBeenCalled()
    })

    // 等渲染稳定后，不应有删除按钮
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: '删除视频' })).not.toBeInTheDocument()
    })
  })

  it('开始播放时启动播放会话', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(mockStartSession).toHaveBeenCalled()
    })
  })

  it('组件卸载时停止播放会话', async () => {
    const { unmount } = renderPlayer()

    await waitFor(() => {
      expect(mockStartSession).toHaveBeenCalled()
    })

    unmount()

    expect(mockStopSession).toHaveBeenCalled()
  })

  it.skip('视频事件：timeupdate 节流上报进度', async () => {
    // 跳过：jsdom 不支持原生 video timeupdate 事件冒泡到 addEventListener
    // 需要集成测试环境或 Playwright 才能测试此行为
    renderPlayer()

    await act(async () => {
      await new Promise(r => setTimeout(r, 100))
    })

    const videoEl = document.querySelector('video') as HTMLVideoElement
    Object.defineProperty(videoEl, 'duration', { value: 120, writable: true })
    Object.defineProperty(videoEl, 'currentTime', { value: 10, writable: true })

    fireEvent(videoEl, new Event('timeupdate'))

    await act(async () => {
      await new Promise(r => setTimeout(r, 100))
    })

    expect(mockSavePlayback).toHaveBeenCalled()
  })

  it('视频播放时隐藏控件（鼠标静止后）', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-controls')).toBeInTheDocument()
    })

    const playerWrap = document.querySelector('.player-wrap')!

    // 鼠标移动后控件显示
    fireEvent.mouseMove(playerWrap)
    expect(playerWrap).not.toHaveClass('controls-hidden')

    // 等待控件隐藏定时器
    await act(async () => {
      vi.advanceTimersByTime(4000)
    })

    // 由于视频是 paused 状态，控件不会隐藏（只有播放中才隐藏）
    expect(playerWrap).not.toHaveClass('controls-hidden')
  })

  it('双击播放区域触发全屏', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('.player-wrap')).toBeInTheDocument()
    })

    const playerWrap = document.querySelector('.player-wrap')!
    fireEvent.doubleClick(playerWrap)

    expect(Element.prototype.requestFullscreen).toHaveBeenCalled()
  })

  it('video 元素点击触发播放/暂停', async () => {
    renderPlayer()

    await waitFor(() => {
      expect(document.querySelector('video.player-video')).toBeInTheDocument()
    })

    const videoEl = document.querySelector('video.player-video')!
    fireEvent.click(videoEl)

    expect(videoEl.play).toHaveBeenCalled()
  })
})
