import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import React from 'react'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import Home from '../pages/Home/Home'
import type { VideoListResponse, MappedVideo } from '../api/types'

// ── Mocks ──────────────────────────────────────────────────────────────────────

vi.mock('../api/videos', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../api/videos')>()
  return {
    ...mod,
    listVideos: vi.fn(),
  }
})

vi.mock('../api/utils', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../api/utils')>()
  return {
    ...mod,
    mapVideo: vi.fn(),
  }
})

vi.mock('../api/recommendations', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../api/recommendations')>()
  return {
    ...mod,
    getTrendingVideos: vi.fn(),
  }
})

vi.mock('../context/AuthContext', () => ({
  useAuth: vi.fn(),
}))

vi.mock('../utils/track', () => ({
  trackClick: vi.fn(),
}))

vi.mock('../components/VideoCard/VideoCard', () => ({
  __esModule: true,
  default: ({ video }: { video: MappedVideo }) =>
    React.createElement('div', { 'data-testid': 'video-card', 'data-id': video.id }, video.title),
  VideoCardSkeleton: ({ count = 6 }: { count?: number }) =>
    React.createElement('div', { 'data-testid': 'skeleton-grid' }, `${count} skeletons`),
}))

vi.mock('../components/AuthDialog/AuthDialog', () => ({
  __esModule: true,
  default: () => React.createElement('div', { 'data-testid': 'auth-dialog' }),
}))

// ── Helpers ────────────────────────────────────────────────────────────────────

function makeVideoResponse(videos: MappedVideo[], total: number): VideoListResponse {
  return {
    items: videos as unknown as VideoListResponse['items'],
    total,
    page: 0,
    size: 20,
  }
}

function makeMappedVideo(overrides: Partial<MappedVideo> = {}): MappedVideo {
  return {
    id: '1',
    title: '测试视频',
    category: '科技',
    description: '',
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

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0 },
      mutations: { retry: false },
    },
  })
}

function renderHome(route = '/') {
  queryClient = createQueryClient()
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[route]}>
        <Home />
      </MemoryRouter>
    </QueryClientProvider>
  )
}

// ── Setup ──────────────────────────────────────────────────────────────────────

const { useAuth } = await import('../context/AuthContext')
const { listVideos } = await import('../api/videos')
const { getTrendingVideos } = await import('../api/recommendations')
const { mapVideo } = await import('../api/utils')

const mockUseAuth = vi.mocked(useAuth)
const mockListVideos = vi.mocked(listVideos)
const mockGetTrending = vi.mocked(getTrendingVideos)
const mockMapVideo = vi.mocked(mapVideo)

/** 存放被创建的 IntersectionObserver 实例，供测试手动触发回调 */
let ioInstances: Array<{ callback: IntersectionObserverCallback }> = []

beforeEach(() => {
  vi.clearAllMocks()
  ioInstances = []

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

  // 默认：无视频返回
  mockListVideos.mockResolvedValue(makeVideoResponse([], 0))
  mockGetTrending.mockResolvedValue([])

  // mapVideo 透传
  mockMapVideo.mockImplementation((v: unknown) => v as MappedVideo)

  // IntersectionObserver mock — 用 class 确保 `new` 正常工作
  class MockIntersectionObserver {
    callback: IntersectionObserverCallback
    root: Element | null = null
    rootMargin = ''
    thresholds: ReadonlyArray<number> = []
    constructor(cb: IntersectionObserverCallback) {
      this.callback = cb
      ioInstances.push(this)
    }
    observe(_target: Element) {}
    unobserve(_target: Element) {}
    disconnect() {}
    takeRecords(): IntersectionObserverEntry[] { return [] }
  }
  vi.stubGlobal('IntersectionObserver', MockIntersectionObserver)
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('Home 页面', () => {
  // 1. 渲染测试：验证标题、视频列表容器存在
  it('渲染时显示分类栏和视频列表容器', () => {
    renderHome()
    // 分类栏包含所有分类标签
    expect(screen.getByRole('button', { name: '全部' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '科技' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '音乐' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '外部' })).toBeInTheDocument()
    // 哨兵元素存在
    expect(document.querySelector('.load-sentinel')).toBeInTheDocument()
    // home 容器存在
    expect(document.querySelector('.home')).toBeInTheDocument()
  })

  // 2. 加载状态：验证加载时显示骨架屏
  it('加载中时显示骨架屏', () => {
    // 让 listVideos 永远 pending
    mockListVideos.mockReturnValue(new Promise(() => {}))
    renderHome()
    // 骨架屏通过 VideoCardSkeleton 渲染
    expect(screen.getByTestId('skeleton-grid')).toBeInTheDocument()
  })

  // 3. 空状态：无视频时显示空状态提示
  it('无视频时显示空状态提示', async () => {
    mockListVideos.mockResolvedValue(makeVideoResponse([], 0))
    renderHome()
    await waitFor(() => {
      expect(screen.getByText('暂无视频')).toBeInTheDocument()
    })
    // 空状态图标
    expect(screen.getByText('🎬')).toBeInTheDocument()
  })

  // 4. 错误状态：API 错误时显示错误提示
  it('API 错误时显示错误提示和重试按钮', async () => {
    mockListVideos.mockRejectedValue(new Error('网络错误'))
    renderHome()
    await waitFor(() => {
      expect(screen.getByText('网络连接失败')).toBeInTheDocument()
    })
    const retryBtn = screen.getByRole('button', { name: '重试' })
    expect(retryBtn).toBeInTheDocument()

    // 点击重试按钮可以再次调用 listVideos
    const callCountBefore = mockListVideos.mock.calls.length
    fireEvent.click(retryBtn)
    await waitFor(() => {
      expect(mockListVideos.mock.calls.length).toBeGreaterThan(callCountBefore)
    })
  })

  // 5. 搜索功能：输入搜索词后调用 API
  it('URL 带搜索参数时传入 query 调用 listVideos', async () => {
    mockListVideos.mockResolvedValue(makeVideoResponse([], 0))
    renderHome('/?q=测试搜索')
    await waitFor(() => {
      expect(mockListVideos).toHaveBeenCalled()
    })
    const lastCall = mockListVideos.mock.calls[mockListVideos.mock.calls.length - 1]
    expect(lastCall?.[0]).toHaveProperty('query', '测试搜索')
  })

  // 6. 分页：滚动到底部加载更多
  it('分页：有更多页时 IntersectionObserver 触发加载下一页', async () => {
    const video1 = makeMappedVideo({ id: 'v1', title: '视频一' })
    const video2 = makeMappedVideo({ id: 'v2', title: '视频二' })

    // 第一页返回 2 个视频，total=4（还有更多）
    mockListVideos.mockResolvedValueOnce(makeVideoResponse([video1, video2], 4))

    renderHome()

    // 等待第一页加载完成
    await waitFor(() => {
      expect(screen.getByText('视频一')).toBeInTheDocument()
      expect(screen.getByText('视频二')).toBeInTheDocument()
    })

    // 模拟 IntersectionObserver 触发（哨兵进入视口）
    expect(ioInstances.length).toBeGreaterThan(0)
    const lastIO = ioInstances[ioInstances.length - 1]!

    // 第二页返回更多视频
    const video3 = makeMappedVideo({ id: 'v3', title: '视频三' })
    mockListVideos.mockResolvedValueOnce(makeVideoResponse([video3], 4))

    await act(async () => {
      lastIO.callback(
        [{ isIntersecting: true } as IntersectionObserverEntry],
        {} as IntersectionObserver
      )
    })

    // 应调用第二次 listVideos（page=1）
    await waitFor(() => {
      expect(mockListVideos.mock.calls.length).toBeGreaterThanOrEqual(2)
    })

    const secondCall = mockListVideos.mock.calls[1]
    expect(secondCall?.[0]).toHaveProperty('page', 1)
  })

  // 额外：显示视频卡片
  it('有视频数据时渲染视频卡片', async () => {
    const video = makeMappedVideo({ id: 'v1', title: '我的视频' })
    mockListVideos.mockResolvedValue(makeVideoResponse([video], 1))

    renderHome()

    await waitFor(() => {
      expect(screen.getByText('我的视频')).toBeInTheDocument()
    })
    expect(screen.getByTestId('video-card')).toHaveAttribute('data-id', 'v1')
  })

  // 额外：未登录时显示极简登录页，不调用 listVideos / trending
  it('未登录时显示极简登录页，不调用任何视频 API', () => {
    mockUseAuth.mockReturnValue({
      user: null,
      loading: false,
      kickedMsg: null,
      clearKickedMsg: vi.fn(),
      login: vi.fn(),
      register: vi.fn(),
      logout: vi.fn(),
      refreshUser: vi.fn(),
      setUser: vi.fn(),
    })

    renderHome()

    expect(screen.getByText('Atmos Video')).toBeInTheDocument()
    // 登录按钮（不显示任何视频卡片）
    expect(screen.getByRole('button', { name: '登录 / 注册' })).toBeInTheDocument()
    expect(screen.queryByTestId('video-card')).not.toBeInTheDocument()
    expect(screen.queryByTestId('auth-dialog')).not.toBeInTheDocument()
    // 未登录不调用视频列表 / 热门推荐 API
    expect(mockListVideos).not.toHaveBeenCalled()
    expect(mockGetTrending).not.toHaveBeenCalled()
  })

  // 额外：搜索空结果提示
  it('搜索无结果时显示搜索空状态提示', async () => {
    mockListVideos.mockResolvedValue(makeVideoResponse([], 0))

    renderHome('/?q=不存在的内容')

    await waitFor(() => {
      expect(screen.getByText(/未找到与「不存在的内容」相关的视频/)).toBeInTheDocument()
    })
  })

  // 额外：分类切换
  it('点击分类标签切换分类', async () => {
    mockListVideos.mockResolvedValue(makeVideoResponse([], 0))
    renderHome()

    fireEvent.click(screen.getByRole('button', { name: '科技' }))

    await waitFor(() => {
      const lastCall = mockListVideos.mock.calls[mockListVideos.mock.calls.length - 1]
      expect(lastCall?.[0]).toHaveProperty('category', '科技')
    })
  })

  // 额外：热门推荐显示
  it('有热门推荐且无搜索关键词时显示热门推荐区块', async () => {
    const trending = [
      makeMappedVideo({ id: 't1', title: '热门视频一' }),
      makeMappedVideo({ id: 't2', title: '热门视频二' }),
    ]
    mockGetTrending.mockResolvedValue(trending)
    mockListVideos.mockResolvedValue(makeVideoResponse([], 0))

    renderHome()

    await waitFor(() => {
      expect(screen.getByText('热门推荐')).toBeInTheDocument()
      expect(screen.getByText('热门视频一')).toBeInTheDocument()
      expect(screen.getByText('热门视频二')).toBeInTheDocument()
    })
  })
})
