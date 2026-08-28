import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import Gallery, { clearGalleryCache } from '../pages/Gallery/Gallery'
import type { MappedImage, VideoListResponse } from '../api/types'

// ── IntersectionObserver Mock ──────────────────────────────────────────────────
// jsdom 不支持 IntersectionObserver，需要手动 mock
type IntersectionObserverCallback = (entries: IntersectionObserverEntry[], observer: IntersectionObserver) => void

let ioCallback: IntersectionObserverCallback | null = null

class MockIntersectionObserver implements IntersectionObserver {
  readonly root: Element | Document | null = null
  readonly rootMargin: string = ''
  readonly thresholds: ReadonlyArray<number> = []

  constructor(callback: IntersectionObserverCallback, _options?: IntersectionObserverInit) {
    ioCallback = callback
  }

  observe(_target: Element): void {}
  unobserve(_target: Element): void {}
  disconnect(): void {
    ioCallback = null
  }
  takeRecords(): IntersectionObserverEntry[] {
    return []
  }
}

// 安装 mock
Object.defineProperty(globalThis, 'IntersectionObserver', {
  writable: true,
  configurable: true,
  value: MockIntersectionObserver,
})

/** 模拟 IntersectionObserver 触发：哨兵元素进入视口 */
function triggerIntersection(isIntersecting: boolean) {
  if (!ioCallback) return
  const entry = {
    isIntersecting,
    boundingClientRect: {} as DOMRectReadOnly,
    intersectionRatio: isIntersecting ? 1 : 0,
    intersectionRect: {} as DOMRectReadOnly,
    rootBounds: null,
    target: document.createElement('div'),
    time: Date.now(),
  } as IntersectionObserverEntry
  act(() => {
    ioCallback!([entry], {} as IntersectionObserver)
  })
}

// ── Mocks ──────────────────────────────────────────────────────────────────────

vi.mock('../api', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../api')>()
  return {
    ...mod,
    listVideos: vi.fn(),
    mapImage: vi.fn(),
  }
})

vi.mock('../api/client', () => ({
  request: vi.fn(),
  mediaUrl: vi.fn((path: string) => path),
  getToken: vi.fn(() => 'test-token'),
}))

vi.mock('../context/AuthContext', () => ({
  useAuth: vi.fn(),
}))

// ── Helpers ────────────────────────────────────────────────────────────────────

function makeMappedImage(overrides: Partial<MappedImage> = {}): MappedImage {
  return {
    id: 'img1',
    title: '测试图片',
    category: 'general',
    thumb: '/media/thumb.jpg',
    original: '/media/original.jpg',
    sourceType: 'local_image',
    ...overrides,
  }
}

function makeImageResponse(images: MappedImage[], total: number): VideoListResponse {
  return {
    items: images as unknown as VideoListResponse['items'],
    total,
    page: 0,
    size: 40,
  }
}

function renderGallery(route = '/gallery') {
  return render(
    <MemoryRouter initialEntries={[route]}>
      <Gallery />
    </MemoryRouter>
  )
}

/** 模拟滚动到底部，触发无限滚动加载（通过 IntersectionObserver 哨兵） */
function scrollToBottom() {
  triggerIntersection(true)
}

// ── Setup ──────────────────────────────────────────────────────────────────────

const { listVideos, mapImage } = await import('../api')
const { useAuth } = await import('../context/AuthContext')

const mockListVideos = vi.mocked(listVideos)
const mockMapImage = vi.mocked(mapImage)
const mockUseAuth = vi.mocked(useAuth)

function makeUser() {
  return {
    id: 1,
    username: 'testuser',
    email: 'test@example.com',
    isAdmin: false,
    role: 1 as const,
    avatarUrl: null,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  ioCallback = null
  clearGalleryCache()

  // 默认：已登录用户
  mockUseAuth.mockReturnValue({
    user: makeUser(),
    loading: false,
    kickedMsg: null,
    clearKickedMsg: vi.fn(),
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    refreshUser: vi.fn(),
    setUser: vi.fn(),
  })

  // 默认：空结果
  mockListVideos.mockResolvedValue(makeImageResponse([], 0))

  // mapImage 透传
  mockMapImage.mockImplementation((v: unknown) => v as MappedImage)

  // 模拟 scrollHeight > innerHeight 以禁用自动补页
  Object.defineProperty(document.documentElement, 'scrollHeight', { value: 2000, configurable: true })
  Object.defineProperty(window, 'innerHeight', { value: 800, configurable: true })
})

afterEach(() => {
  vi.restoreAllMocks()
})

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('Gallery 页面', () => {
  // ======================================================================
  // 1) 图片网格展示
  // ======================================================================
  describe('图片网格展示', () => {
    it('渲染时显示页面标题和标签', () => {
      renderGallery()
      expect(screen.getByText('GALLERY')).toBeInTheDocument()
      expect(screen.getByText('图片浏览')).toBeInTheDocument()
    })

    it('初始加载时显示骨架屏', () => {
      mockListVideos.mockReturnValue(new Promise(() => {}))
      const { container } = renderGallery()
      expect(container.querySelector('.gallery-skeleton-grid')).toBeInTheDocument()
      expect(container.querySelectorAll('.gallery-skeleton-card').length).toBeGreaterThan(0)
    })

    it('有图片数据时渲染图片卡片', async () => {
      const images = [
        makeMappedImage({ id: 'img1', title: '风景照', thumb: '/media/img1.jpg' }),
        makeMappedImage({ id: 'img2', title: '城市夜景', thumb: '/media/img2.jpg' }),
      ]
      mockListVideos.mockResolvedValue(makeImageResponse(images, 2))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(2)
      })

      const imgs = container.querySelectorAll('.gallery-card img')
      expect(imgs).toHaveLength(2)
      expect(imgs[0]).toHaveAttribute('src', '/media/img1.jpg')
      expect(imgs[0]).toHaveAttribute('alt', '风景照')
      expect(imgs[1]).toHaveAttribute('src', '/media/img2.jpg')
      expect(imgs[1]).toHaveAttribute('alt', '城市夜景')
    })

    it('无图片时显示空状态', async () => {
      mockListVideos.mockResolvedValue(makeImageResponse([], 0))
      renderGallery()

      await waitFor(() => {
        expect(screen.getByText('暂无图片')).toBeInTheDocument()
      })
      expect(screen.getByText('📷')).toBeInTheDocument()
    })

    it('API 错误时显示错误提示和重试按钮', async () => {
      mockListVideos.mockRejectedValue(new Error('网络错误'))
      renderGallery()

      await waitFor(() => {
        expect(screen.getByText('加载失败，请检查网络后重试')).toBeInTheDocument()
      })

      const retryBtn = screen.getByRole('button', { name: '重试' })
      expect(retryBtn).toBeInTheDocument()

      const callCountBefore = mockListVideos.mock.calls.length
      fireEvent.click(retryBtn)
      await waitFor(() => {
        expect(mockListVideos.mock.calls.length).toBeGreaterThan(callCountBefore)
      })
    })

    it('加载中显示加载文字', () => {
      mockListVideos.mockReturnValue(new Promise(() => {}))
      renderGallery()
      expect(screen.getByText('加载中...')).toBeInTheDocument()
    })

    it('显示图片总数', async () => {
      const images = Array.from({ length: 5 }, (_, i) =>
        makeMappedImage({ id: `img${i}`, title: `图片${i}` })
      )
      mockListVideos.mockResolvedValue(makeImageResponse(images, 100))

      renderGallery()

      await waitFor(() => {
        expect(screen.getByText('共 100 张')).toBeInTheDocument()
      })
    })

    it('图片卡片可点击且有正确的 aria 属性', async () => {
      const images = [makeMappedImage({ id: 'img1', title: '风景照' })]
      mockListVideos.mockResolvedValue(makeImageResponse(images, 1))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(1)
      })

      const card = container.querySelector('.gallery-card')!
      expect(card).toHaveAttribute('role', 'button')
      expect(card).toHaveAttribute('tabindex', '0')
      expect(card).toHaveAttribute('aria-label', '查看大图：风景照')
    })

    it('默认使用网格布局（不含 wide 类）', async () => {
      const images = [makeMappedImage({ id: 'img1' })]
      mockListVideos.mockResolvedValue(makeImageResponse(images, 1))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(1)
      })

      const grid = container.querySelector('.gallery-grid')
      expect(grid).toBeInTheDocument()
      expect(grid).not.toHaveClass('wide')
    })

    it('图片加载失败时隐藏该图片', async () => {
      const images = [makeMappedImage({ id: 'img1', title: '坏图', thumb: '/media/broken.jpg' })]
      mockListVideos.mockResolvedValue(makeImageResponse(images, 1))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card img')).toHaveLength(1)
      })

      const img = container.querySelector('.gallery-card img') as HTMLImageElement
      fireEvent.error(img)
      // GalleryCard 在图片加载失败后移除 <img> 元素
      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card img')).toHaveLength(0)
      })
    })
  })

  // ======================================================================
  // 2) 筛选功能
  // ======================================================================
  describe('筛选功能', () => {
    it('URL 带搜索参数时传入 query 调用 listVideos', async () => {
      mockListVideos.mockResolvedValue(makeImageResponse([], 0))
      renderGallery('/gallery?q=风景')

      await waitFor(() => {
        expect(mockListVideos).toHaveBeenCalled()
      })

      const lastCall = mockListVideos.mock.calls[mockListVideos.mock.calls.length - 1]
      expect(lastCall?.[0]).toHaveProperty('type', 'local_image')
      expect(lastCall?.[0]).toHaveProperty('query', '风景')
    })

    it('搜索输入框同步 URL 中的查询参数', async () => {
      mockListVideos.mockResolvedValue(makeImageResponse([], 0))
      renderGallery('/gallery?q=测试词')

      await waitFor(() => {
        const input = screen.getByPlaceholderText('搜索图片...')
        expect(input).toHaveValue('测试词')
      })
    })

    it('输入搜索词后防抖写入 URL（触发重新加载）', async () => {
      mockListVideos.mockResolvedValue(makeImageResponse([], 0))
      renderGallery()

      await waitFor(() => {
        expect(mockListVideos).toHaveBeenCalled()
      })

      const input = screen.getByPlaceholderText('搜索图片...')
      fireEvent.change(input, { target: { value: '新搜索' } })

      // 输入框已更新
      expect(input).toHaveValue('新搜索')

      // 等待防抖后触发重新加载（300ms 防抖 + 异步）
      await waitFor(() => {
        const calls = mockListVideos.mock.calls
        const hasNewQuery = calls.some((c) => c[0]?.query === '新搜索')
        expect(hasNewQuery).toBe(true)
      }, { timeout: 2000 })
    })

    it('清空搜索框后触发重新加载', async () => {
      mockListVideos.mockResolvedValue(makeImageResponse([], 0))
      renderGallery('/gallery?q=旧搜索')

      await waitFor(() => {
        expect(mockListVideos).toHaveBeenCalled()
      })

      const initialCallCount = mockListVideos.mock.calls.length

      const input = screen.getByPlaceholderText('搜索图片...')
      expect(input).toHaveValue('旧搜索')

      fireEvent.change(input, { target: { value: '' } })

      // 等待防抖后触发重新加载
      await waitFor(() => {
        expect(mockListVideos.mock.calls.length).toBeGreaterThan(initialCallCount)
      }, { timeout: 2000 })
    })

    it('切换布局：点击宽幅按钮添加 wide 类', async () => {
      const images = [makeMappedImage({ id: 'img1' })]
      mockListVideos.mockResolvedValue(makeImageResponse(images, 1))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(1)
      })

      const wideBtn = screen.getByRole('button', { name: '宽幅视图' })
      fireEvent.click(wideBtn)

      // MemoryRouter 中 setSearchParams 同步更新
      expect(container.querySelector('.gallery-grid')).toHaveClass('wide')
    })

    it('切换布局：点击网格按钮移除 wide 类', async () => {
      const images = [makeMappedImage({ id: 'img1' })]
      mockListVideos.mockResolvedValue(makeImageResponse(images, 1))

      const { container } = renderGallery('/gallery?view=wide')

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(1)
      })

      expect(container.querySelector('.gallery-grid')).toHaveClass('wide')

      const gridBtn = screen.getByRole('button', { name: '网格视图' })
      fireEvent.click(gridBtn)

      expect(container.querySelector('.gallery-grid')).not.toHaveClass('wide')
    })

    it('布局按钮有正确的 aria-pressed 属性', () => {
      mockListVideos.mockResolvedValue(makeImageResponse([], 0))
      renderGallery()

      const gridBtn = screen.getByRole('button', { name: '网格视图' })
      const wideBtn = screen.getByRole('button', { name: '宽幅视图' })

      expect(gridBtn).toHaveAttribute('aria-pressed', 'true')
      expect(wideBtn).toHaveAttribute('aria-pressed', 'false')
    })

    it('listVideos 传入 type=local_image', async () => {
      mockListVideos.mockResolvedValue(makeImageResponse([], 0))
      renderGallery()

      await waitFor(() => {
        expect(mockListVideos).toHaveBeenCalled()
      })

      expect(mockListVideos.mock.calls[0]?.[0]).toHaveProperty('type', 'local_image')
    })
  })

  // ======================================================================
  // 3) 无限滚动
  // ======================================================================
  describe('无限滚动', () => {
    it('滚动到底部时加载下一页', async () => {
      const PAGE_SIZE = 40
      const page1Images = Array.from({ length: PAGE_SIZE }, (_, i) =>
        makeMappedImage({ id: `img${i}` })
      )
      mockListVideos.mockResolvedValueOnce(makeImageResponse(page1Images, 80))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(PAGE_SIZE)
      })

      const page2Images = Array.from({ length: PAGE_SIZE }, (_, i) =>
        makeMappedImage({ id: `img${PAGE_SIZE + i}` })
      )
      mockListVideos.mockResolvedValueOnce(makeImageResponse(page2Images, 80))

      await act(async () => {
        scrollToBottom()
      })

      await waitFor(() => {
        expect(mockListVideos.mock.calls.length).toBeGreaterThanOrEqual(2)
      })

      const secondCall = mockListVideos.mock.calls[1]
      expect(secondCall?.[0]).toHaveProperty('page', 1)
      expect(secondCall?.[0]).toHaveProperty('type', 'local_image')
    })

    it('不足一页时不再加载更多', async () => {
      const images = Array.from({ length: 5 }, (_, i) =>
        makeMappedImage({ id: `img${i}` })
      )
      mockListVideos.mockResolvedValue(makeImageResponse(images, 5))

      renderGallery()

      await waitFor(() => {
        expect(mockListVideos).toHaveBeenCalledTimes(1)
      })

      await act(async () => {
        scrollToBottom()
      })

      // 不应发起第二次请求
      expect(mockListVideos).toHaveBeenCalledTimes(1)
    })

    it('加载中不重复触发', async () => {
      let resolvePage1: (v: VideoListResponse) => void
      const page1Promise = new Promise<VideoListResponse>((r) => { resolvePage1 = r })
      mockListVideos.mockReturnValueOnce(page1Promise)

      renderGallery()

      await act(async () => {
        scrollToBottom()
        scrollToBottom()
        scrollToBottom()
      })

      expect(mockListVideos).toHaveBeenCalledTimes(1)

      await act(async () => {
        resolvePage1!(makeImageResponse([], 0))
      })
    })

    it('追加页面时去重', async () => {
      const PAGE_SIZE = 40
      const img = makeMappedImage({ id: 'dup', title: '重复图' })

      const page1 = Array.from({ length: PAGE_SIZE }, (_, i) =>
        i === 0 ? img : makeMappedImage({ id: `img${i}` })
      )
      mockListVideos.mockResolvedValueOnce(makeImageResponse(page1, 80))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(PAGE_SIZE)
      })

      const page2 = [img, ...Array.from({ length: PAGE_SIZE - 1 }, (_, i) =>
        makeMappedImage({ id: `img${PAGE_SIZE + i}` })
      )]
      mockListVideos.mockResolvedValueOnce(makeImageResponse(page2, 80))

      await act(async () => {
        scrollToBottom()
      })

      await waitFor(() => {
        expect(mockListVideos.mock.calls.length).toBeGreaterThanOrEqual(2)
      })

      // 去重后卡片数 < PAGE_SIZE * 2
      const cards = container.querySelectorAll('.gallery-card')
      expect(cards.length).toBeLessThan(PAGE_SIZE + PAGE_SIZE)
    })

    it('追加失败时显示错误提示和重试按钮', async () => {
      const PAGE_SIZE = 40
      const page1 = Array.from({ length: PAGE_SIZE }, (_, i) =>
        makeMappedImage({ id: `img${i}` })
      )
      mockListVideos.mockResolvedValueOnce(makeImageResponse(page1, 80))

      const { container } = renderGallery()

      await waitFor(() => {
        expect(container.querySelectorAll('.gallery-card')).toHaveLength(PAGE_SIZE)
      })

      mockListVideos.mockRejectedValueOnce(new Error('网络错误'))

      await act(async () => {
        scrollToBottom()
      })

      await waitFor(() => {
        expect(screen.getByText('加载更多失败，请重试')).toBeInTheDocument()
      })

      const retryBtns = screen.getAllByRole('button', { name: '重试' })
      expect(retryBtns.length).toBeGreaterThan(0)
    })

    it('所有图片加载完毕后显示"没有更多了"', async () => {
      const images = Array.from({ length: 3 }, (_, i) =>
        makeMappedImage({ id: `img${i}` })
      )
      mockListVideos.mockResolvedValue(makeImageResponse(images, 3))

      renderGallery()

      await waitFor(() => {
        expect(screen.getByText('没有更多了')).toBeInTheDocument()
      })
    })
  })

  // ======================================================================
  // 4) 灯箱预览
  // ======================================================================
  describe('灯箱预览', () => {
    const images = [
      makeMappedImage({ id: 'img1', title: '风景照', thumb: '/media/img1.jpg', original: '/media/img1-original.jpg' }),
      makeMappedImage({ id: 'img2', title: '城市夜景', thumb: '/media/img2.jpg', original: '/media/img2-original.jpg' }),
      makeMappedImage({ id: 'img3', title: '日落', thumb: '/media/img3.jpg', original: '/media/img3-original.jpg' }),
    ]

    async function renderWithImages() {
      mockListVideos.mockResolvedValue(makeImageResponse(images, 3))

      const result = renderGallery()

      await waitFor(() => {
        expect(result.container.querySelectorAll('.gallery-card')).toHaveLength(3)
      })

      return result
    }

    it('点击图片卡片打开灯箱', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })

      const lightboxImg = document.querySelector('.lightbox-img') as HTMLImageElement
      expect(lightboxImg).toHaveAttribute('src', '/media/img1-original.jpg')
      expect(lightboxImg).toHaveAttribute('alt', '风景照')
    })

    it('灯箱显示图片标题和计数', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-title')).toBeInTheDocument()
      })

      const title = document.querySelector('.lightbox-title')!
      expect(title).toHaveTextContent('风景照')

      const counter = document.querySelector('.lightbox-counter')!
      expect(counter).toHaveTextContent('1 / 3')
    })

    it('灯箱有正确的 ARIA 属性', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        const lightbox = document.querySelector('.lightbox')
        expect(lightbox).toHaveAttribute('role', 'dialog')
        expect(lightbox).toHaveAttribute('aria-modal', 'true')
        expect(lightbox).toHaveAttribute('aria-label', '图片预览：风景照')
        expect(lightbox).toHaveAttribute('tabindex', '-1')
      })
    })

    it('点击关闭按钮关闭灯箱', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })

      const closeBtn = document.querySelector('.lightbox-close') as HTMLElement
      fireEvent.click(closeBtn)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).not.toBeInTheDocument()
      })
    })

    it('点击灯箱背景关闭灯箱', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })

      const lightbox = document.querySelector('.lightbox')!
      fireEvent.click(lightbox)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).not.toBeInTheDocument()
      })
    })

    it('点击灯箱图片不关闭灯箱', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })

      const lightboxImg = document.querySelector('.lightbox-img')!
      fireEvent.click(lightboxImg)

      expect(document.querySelector('.lightbox')).toBeInTheDocument()
    })

    it('按下 Escape 键关闭灯箱', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })

      fireEvent.keyDown(document, { key: 'Escape' })

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).not.toBeInTheDocument()
      })
    })

    it('按下右箭头键切换到下一张', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('1 / 3')
      })

      fireEvent.keyDown(document, { key: 'ArrowRight' })

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('2 / 3')
      })

      const lightboxImg = document.querySelector('.lightbox-img') as HTMLImageElement
      expect(lightboxImg).toHaveAttribute('src', '/media/img2-original.jpg')
      expect(lightboxImg).toHaveAttribute('alt', '城市夜景')
    })

    it('按下左箭头键切换到上一张', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[1]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('2 / 3')
      })

      fireEvent.keyDown(document, { key: 'ArrowLeft' })

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('1 / 3')
      })

      const lightboxImg = document.querySelector('.lightbox-img') as HTMLImageElement
      expect(lightboxImg).toHaveAttribute('src', '/media/img1-original.jpg')
    })

    it('第一张图片时隐藏上一张按钮', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })

      expect(document.querySelector('.lightbox-prev')).not.toBeInTheDocument()
      expect(document.querySelector('.lightbox-next')).toBeInTheDocument()
    })

    it('最后一张图片时隐藏下一张按钮', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[2]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })

      expect(document.querySelector('.lightbox-prev')).toBeInTheDocument()
      expect(document.querySelector('.lightbox-next')).not.toBeInTheDocument()
    })

    it('点击导航按钮切换图片', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('1 / 3')
      })

      const nextBtn = document.querySelector('.lightbox-next')!
      fireEvent.click(nextBtn)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('2 / 3')
      })

      const prevBtn = document.querySelector('.lightbox-prev')!
      fireEvent.click(prevBtn)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('1 / 3')
      })
    })

    it('键盘 Enter 键打开灯箱', async () => {
      const { container } = await renderWithImages()

      const cards = container.querySelectorAll('.gallery-card')
      fireEvent.keyDown(cards[0]!, { key: 'Enter' })

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })
    })

    it('键盘空格键打开灯箱', async () => {
      const { container } = await renderWithImages()

      const cards = container.querySelectorAll('.gallery-card')
      fireEvent.keyDown(cards[0]!, { key: ' ' })

      await waitFor(() => {
        expect(document.querySelector('.lightbox')).toBeInTheDocument()
      })
    })

    it('灯箱打开时添加 overflow-hidden 类', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.documentElement.classList.contains('overflow-hidden')).toBe(true)
      })
    })

    it('灯箱关闭时移除 overflow-hidden 类', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.documentElement.classList.contains('overflow-hidden')).toBe(true)
      })

      fireEvent.keyDown(document, { key: 'Escape' })

      await waitFor(() => {
        expect(document.documentElement.classList.contains('overflow-hidden')).toBe(false)
      })
    })

    it('灯箱导航不会越界（左边界）', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[0]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('1 / 3')
      })

      fireEvent.keyDown(document, { key: 'ArrowLeft' })
      fireEvent.keyDown(document, { key: 'ArrowLeft' })
      fireEvent.keyDown(document, { key: 'ArrowLeft' })

      expect(document.querySelector('.lightbox-counter')).toHaveTextContent('1 / 3')
    })

    it('灯箱导航不会越界（右边界）', async () => {
      const { container } = await renderWithImages()

      fireEvent.click(container.querySelectorAll('.gallery-card')[2]!)

      await waitFor(() => {
        expect(document.querySelector('.lightbox-counter')).toHaveTextContent('3 / 3')
      })

      fireEvent.keyDown(document, { key: 'ArrowRight' })
      fireEvent.keyDown(document, { key: 'ArrowRight' })

      expect(document.querySelector('.lightbox-counter')).toHaveTextContent('3 / 3')
    })
  })
})
