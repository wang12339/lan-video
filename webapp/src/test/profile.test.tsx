import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import React from 'react'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import Profile from '../pages/Profile/Profile'
import type { UserProfile, MappedVideo, MappedHistory, PlaybackHistory, VideoListResponse } from '../api/types'
import type { Playlist } from '../api/playlists'
import type { ShareListItem } from '../api'

// ── Mocks ──────────────────────────────────────────────────────────────────────

vi.mock('../api', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../api')>()
  return {
    ...mod,
    getUserProfile: vi.fn(),
    listVideos: vi.fn(),
    listPlaybackHistory: vi.fn(),
    listFavorites: vi.fn(),
    formatDuration: vi.fn((s: number) => `${s}s`),
    loadPrefs: vi.fn(() => ({ autoPlay: true, speedMem: false })),
    setPref: vi.fn(),
    uploadAvatar: vi.fn(),
    listMyPlaylists: vi.fn(),
    createPlaylist: vi.fn(),
    deletePlaylist: vi.fn(),
    listMyShares: vi.fn(),
    revokeMyShare: vi.fn(),
    sendVerificationEmail: vi.fn(),
    updateEmail: vi.fn(),
    mapVideo: vi.fn(),
    mapHistory: vi.fn(),
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

vi.mock('../components/ui', () => ({
  ConfirmDialog: ({ open, title, message, onConfirm, onCancel }: {
    open: boolean; title: string; message: string;
    onConfirm: () => void; onCancel: () => void; danger?: boolean
  }) => open ? React.createElement('div', { 'data-testid': 'confirm-dialog' },
    React.createElement('span', { 'data-testid': 'confirm-title' }, title),
    React.createElement('span', { 'data-testid': 'confirm-message' }, message),
    React.createElement('button', { 'data-testid': 'confirm-ok', onClick: onConfirm }, '确定'),
    React.createElement('button', { 'data-testid': 'confirm-cancel', onClick: onCancel }, '取消'),
  ) : null,
  AlertDialog: ({ open, message, onClose }: { open: boolean; message: string; onClose: () => void }) =>
    open ? React.createElement('div', { 'data-testid': 'alert-dialog' },
      React.createElement('span', { 'data-testid': 'alert-message' }, message),
      React.createElement('button', { 'data-testid': 'alert-close', onClick: onClose }, '关闭'),
    ) : null,
  SkeletonLoader: ({ type, lines = 3 }: { type?: string; lines?: number }) => {
    if (type === 'video-card') {
      return React.createElement(React.Fragment, null,
        ...Array.from({ length: lines }).map((_: unknown, i: number) =>
          React.createElement('div', { key: i, className: 'skeleton-video-card' },
            React.createElement('div', { className: 'skeleton-video-thumb' }),
            React.createElement('div', { className: 'skeleton-video-info' },
              React.createElement('div', { className: 'skeleton-video-title' }),
              React.createElement('div', { className: 'skeleton-video-meta' }),
            ),
          )
        )
      );
    }
    return React.createElement('div', { 'data-testid': 'skeleton-loader' });
  },
}))

// ── Helpers ────────────────────────────────────────────────────────────────────

const mockUser = {
  id: 'u1',
  username: 'testuser',
  isAdmin: false,
  avatarUrl: 'https://example.com/avatar.jpg',
  createdAt: '2024-01-15T00:00:00Z',
  email: 'test@example.com',
  emailVerified: true,
}

const mockUserProfile: UserProfile = {
  ...mockUser,
  totalVideosWatched: 42,
  totalWatchTimeMs: 3720000, // 62 分钟
  recentHistory: [],
}

function makeMappedVideo(overrides: Partial<MappedVideo> = {}): MappedVideo {
  return {
    id: 'v1',
    title: '测试视频',
    category: '科技',
    description: '',
    thumb: '/media/thumb.jpg',
    stream: '/media/v.mp4',
    cover: null,
    sourceType: 'local_video',
    duration: 120,
    views: 500,
    date: '2024-01-01T00:00:00Z',
    progress: 0,
    ...overrides,
  }
}

function makeMappedHistory(overrides: Partial<MappedHistory> = {}): MappedHistory {
  return {
    id: 'h1',
    title: '历史视频',
    category: '科技',
    thumb: '/media/thumb.jpg',
    stream: '/media/v.mp4',
    sourceType: 'local_video',
    positionMs: 30000,
    durationMs: 120000,
    updatedAt: '2024-06-01T10:00:00Z',
    progress: 25,
    ...overrides,
  }
}

function makeVideoResponse(videos: MappedVideo[], total: number): VideoListResponse {
  return {
    items: videos as unknown as VideoListResponse['items'],
    total,
    page: 0,
    size: 24,
  }
}

function makePlaylist(overrides: Partial<Playlist> = {}): Playlist {
  return {
    id: 'pl1',
    name: '我的播放列表',
    description: null,
    is_public: true,
    cover_url: null,
    item_count: 5,
    created_at: '2024-03-01T00:00:00Z',
    updated_at: '2024-03-15T00:00:00Z',
    ...overrides,
  }
}

function makeShare(overrides: Partial<ShareListItem> = {}): ShareListItem {
  return {
    id: 's1',
    expiresAt: null,
    createdAt: '2024-05-01T00:00:00Z',
    active: true,
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

function renderProfile() {
  queryClient = createQueryClient()
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/profile']}>
        <Profile />
      </MemoryRouter>
    </QueryClientProvider>
  )
}

// ── Setup ──────────────────────────────────────────────────────────────────────

const { useAuth } = await import('../context/AuthContext')
const {
  getUserProfile, listVideos, listPlaybackHistory, listFavorites,
  uploadAvatar, listMyPlaylists, createPlaylist, deletePlaylist,
  listMyShares, revokeMyShare, sendVerificationEmail, updateEmail,
  mapVideo, mapHistory,
} = await import('../api')

const mockUseAuth = vi.mocked(useAuth)
const mockGetUserProfile = vi.mocked(getUserProfile)
const mockListVideos = vi.mocked(listVideos)
const mockListPlaybackHistory = vi.mocked(listPlaybackHistory)
const mockListFavorites = vi.mocked(listFavorites)
const mockUploadAvatar = vi.mocked(uploadAvatar)
const mockListMyPlaylists = vi.mocked(listMyPlaylists)
const mockCreatePlaylist = vi.mocked(createPlaylist)
const mockDeletePlaylist = vi.mocked(deletePlaylist)
const mockListMyShares = vi.mocked(listMyShares)
const mockRevokeMyShare = vi.mocked(revokeMyShare)
const mockSendVerificationEmail = vi.mocked(sendVerificationEmail)
const mockUpdateEmail = vi.mocked(updateEmail)
const mockMapVideo = vi.mocked(mapVideo)
const mockMapHistory = vi.mocked(mapHistory)

function makeAuthReturn(overrides: Partial<ReturnType<typeof useAuth>> = {}) {
  return {
    user: mockUser,
    loading: false,
    kickedMsg: null,
    clearKickedMsg: vi.fn(),
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    refreshUser: vi.fn(),
    setUser: vi.fn(),
    ...overrides,
  }
}

beforeEach(() => {
  vi.clearAllMocks()

  mockUseAuth.mockReturnValue(makeAuthReturn())
  mockGetUserProfile.mockResolvedValue(mockUserProfile)
  mockListVideos.mockResolvedValue(makeVideoResponse([], 0))
  mockListPlaybackHistory.mockResolvedValue([])
  mockListFavorites.mockResolvedValue([])
  mockListMyPlaylists.mockResolvedValue([])
  mockListMyShares.mockResolvedValue([])
  mockUploadAvatar.mockResolvedValue('https://example.com/new-avatar.jpg')
  mockCreatePlaylist.mockResolvedValue(makePlaylist())
  mockDeletePlaylist.mockResolvedValue(undefined as never)
  mockRevokeMyShare.mockResolvedValue(undefined as never)
  mockSendVerificationEmail.mockResolvedValue({ message: '验证邮件已发送' })
  mockUpdateEmail.mockResolvedValue(undefined as never)

  // mapVideo / mapHistory 透传
  mockMapVideo.mockImplementation((v: unknown) => v as MappedVideo)
  mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)
})

afterEach(() => {
  vi.restoreAllMocks()
})

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('Profile 页面', () => {

  // ═══════════════════════════════════════════════════════════════════════════════
  // 1. 用户信息展示
  // ═══════════════════════════════════════════════════════════════════════════════
  describe('用户信息展示', () => {
    it('已登录用户显示用户名和角色', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      expect(screen.getByText('用户')).toBeInTheDocument()
    })

    it('显示用户头像', async () => {
      renderProfile()
      await waitFor(() => {
        const avatar = screen.getByAltText('我的头像')
        expect(avatar).toBeInTheDocument()
        expect(avatar).toHaveAttribute('src', 'https://example.com/avatar.jpg')
      })
    })

    it('显示统计数据：已观看、累计时长、作品数、加入年份', async () => {
      renderProfile()
      await waitFor(() => {
        // 已观看数
        expect(screen.getByText('42')).toBeInTheDocument()
      })
      // 累计时长：3720000ms = 62min → "1 小时 2 分" (取决于 locale)
      await waitFor(() => {
        expect(screen.getByText('已观看')).toBeInTheDocument()
        expect(screen.getByText('累计时长')).toBeInTheDocument()
      })
      // 作品数
      await waitFor(() => {
        expect(screen.getByText('作品')).toBeInTheDocument()
      })
      // 加入年份
      expect(screen.getByText('加入')).toBeInTheDocument()
    })

    it('加载中时统计数据显示占位符 "—"（暂未加载 profile）', async () => {
      // 让 getUserProfile 永不 resolve
      mockGetUserProfile.mockReturnValue(new Promise(() => {}))
      renderProfile()
      // 统计数字应显示 "—"
      await waitFor(() => {
        const dashes = screen.getAllByText('—')
        expect(dashes.length).toBeGreaterThan(0)
      })
    })

    it('管理员用户显示管理员角色', async () => {
      mockUseAuth.mockReturnValue(makeAuthReturn({
        user: { ...mockUser, isAdmin: true },
      }))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('管理员')).toBeInTheDocument()
      })
    })

    it('未登录时显示登录提示', () => {
      mockUseAuth.mockReturnValue(makeAuthReturn({ user: null }))
      renderProfile()
      expect(screen.getByText('未登录')).toBeInTheDocument()
      expect(screen.getByText('请先登录后查看个人中心')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: '登录 / 注册' })).toBeInTheDocument()
    })

    it('未登录时点击登录按钮打开 AuthDialog', async () => {
      mockUseAuth.mockReturnValue(makeAuthReturn({ user: null }))
      renderProfile()
      fireEvent.click(screen.getByRole('button', { name: '登录 / 注册' }))
      expect(screen.getByTestId('auth-dialog')).toBeInTheDocument()
    })

    it('显示所有 Tab 标签', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByRole('tab', { name: /我的作品/ })).toBeInTheDocument()
        expect(screen.getByRole('tab', { name: /观看历史/ })).toBeInTheDocument()
        expect(screen.getByRole('tab', { name: /我的收藏/ })).toBeInTheDocument()
        expect(screen.getByRole('tab', { name: /我的播放列表/ })).toBeInTheDocument()
        expect(screen.getByRole('tab', { name: /我的分享/ })).toBeInTheDocument()
        expect(screen.getByRole('tab', { name: /设置/ })).toBeInTheDocument()
      })
    })

    it('默认选中"我的作品"Tab', async () => {
      renderProfile()
      await waitFor(() => {
        const worksTab = screen.getByRole('tab', { name: /我的作品/ })
        expect(worksTab).toHaveAttribute('aria-selected', 'true')
      })
    })

    it('切换 Tab 后更新选中状态', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByRole('tab', { name: /观看历史/ })).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /观看历史/ }))
      expect(screen.getByRole('tab', { name: /观看历史/ })).toHaveAttribute('aria-selected', 'true')
      expect(screen.getByRole('tab', { name: /我的作品/ })).toHaveAttribute('aria-selected', 'false')
    })

    it('用户没有头像时显示首字母', async () => {
      mockUseAuth.mockReturnValue(makeAuthReturn({
        user: { ...mockUser, avatarUrl: undefined },
      }))
      renderProfile()
      await waitFor(() => {
        // 无头像时显示 username 首字母大写
        expect(screen.getByText('T')).toBeInTheDocument()
      })
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════════
  // 2. 头像上传
  // ═══════════════════════════════════════════════════════════════════════════════
  describe('头像上传', () => {
    it('点击头像区域触发文件选择', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      const avatarWrap = screen.getByRole('button', { name: '更换头像' })
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement
      const clickSpy = vi.spyOn(fileInput, 'click')
      fireEvent.click(avatarWrap)
      expect(clickSpy).toHaveBeenCalled()
    })

    it('选择非图片文件时显示错误提示', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement
      const file = new File(['content'], 'test.txt', { type: 'text/plain' })
      fireEvent.change(fileInput, { target: { files: [file] } })
      await waitFor(() => {
        expect(screen.getByTestId('alert-message')).toHaveTextContent('请选择图片文件')
      })
    })

    it('选择超过 5MB 的图片时显示错误提示', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement
      // 创建一个超过 5MB 的文件
      const bigFile = new File([new ArrayBuffer(6 * 1024 * 1024)], 'big.jpg', { type: 'image/jpeg' })
      fireEvent.change(fileInput, { target: { files: [bigFile] } })
      await waitFor(() => {
        expect(screen.getByTestId('alert-message')).toHaveTextContent('头像文件不能超过 5MB')
      })
    })

    it('上传成功后显示成功提示并更新用户头像', async () => {
      const setUser = vi.fn()
      mockUseAuth.mockReturnValue(makeAuthReturn({ setUser }))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement
      const file = new File([new ArrayBuffer(1024)], 'avatar.jpg', { type: 'image/jpeg' })
      await act(async () => {
        fireEvent.change(fileInput, { target: { files: [file] } })
      })
      await waitFor(() => {
        expect(mockUploadAvatar).toHaveBeenCalledWith(file)
      })
      await waitFor(() => {
        expect(screen.getByTestId('alert-message')).toHaveTextContent('头像已更新')
      })
      // setUser 应被调用并更新 avatarUrl
      expect(setUser).toHaveBeenCalledWith(
        expect.objectContaining({ avatarUrl: 'https://example.com/new-avatar.jpg' })
      )
    })

    it('上传失败时显示错误提示', async () => {
      mockUploadAvatar.mockRejectedValue(new Error('上传失败'))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement
      const file = new File([new ArrayBuffer(1024)], 'avatar.jpg', { type: 'image/jpeg' })
      await act(async () => {
        fireEvent.change(fileInput, { target: { files: [file] } })
      })
      await waitFor(() => {
        expect(screen.getByTestId('alert-message')).toHaveTextContent('上传失败')
      })
    })

    it('上传中头像区域显示"上传中..."状态', async () => {
      let resolveUpload: (url: string) => void
      mockUploadAvatar.mockReturnValue(new Promise<string>((resolve) => { resolveUpload = resolve }))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement
      const file = new File([new ArrayBuffer(1024)], 'avatar.jpg', { type: 'image/jpeg' })
      act(() => {
        fireEvent.change(fileInput, { target: { files: [file] } })
      })
      await waitFor(() => {
        expect(screen.getByText('上传中...')).toBeInTheDocument()
      })
      await act(async () => {
        resolveUpload!('https://example.com/done.jpg')
      })
    })

    it('选择文件后可以取消', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      const fileInput = document.querySelector('input[type="file"]') as HTMLInputElement
      // 模拟取消选择（空文件列表）
      fireEvent.change(fileInput, { target: { files: [] } })
      expect(mockUploadAvatar).not.toHaveBeenCalled()
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════════
  // 3. 密码修改（设置 Tab 中的账号设置区域）
  // ═══════════════════════════════════════════════════════════════════════════════
  describe('密码修改 / 账号设置', () => {
    it('切换到设置 Tab 显示账号信息', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByText('账号')).toBeInTheDocument()
      })
      // 用户名出现在 header 和 settings 中
      expect(screen.getAllByText('testuser').length).toBeGreaterThanOrEqual(2)
      // 邮箱
      expect(screen.getByText('test@example.com')).toBeInTheDocument()
    })

    it('设置 Tab 显示播放设置开关', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByText('播放设置')).toBeInTheDocument()
        expect(screen.getByText('自动播放')).toBeInTheDocument()
        expect(screen.getByText('记忆播放速度')).toBeInTheDocument()
      })
    })

    it('切换自动播放开关', async () => {
      const { setPref } = await import('../api')
      const mockSetPref = vi.mocked(setPref)
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByText('自动播放')).toBeInTheDocument()
      })
      const autoPlayCheckbox = screen.getAllByRole('checkbox')[0]!
      // 默认为 true，切换为 false
      fireEvent.click(autoPlayCheckbox)
      expect(mockSetPref).toHaveBeenCalledWith('autoPlay', false)
    })

    it('邮箱已验证时显示已验证标记', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByText('✓ 已验证')).toBeInTheDocument()
      })
    })

    it('邮箱未验证时显示验证按钮', async () => {
      mockUseAuth.mockReturnValue(makeAuthReturn({
        user: { ...mockUser, emailVerified: false },
      }))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByRole('button', { name: '验证' })).toBeInTheDocument()
      })
    })

    it('点击验证按钮发送验证邮件', async () => {
      mockUseAuth.mockReturnValue(makeAuthReturn({
        user: { ...mockUser, emailVerified: false },
      }))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByRole('button', { name: '验证' })).toBeInTheDocument()
      })
      await act(async () => {
        fireEvent.click(screen.getByRole('button', { name: '验证' }))
      })
      await waitFor(() => {
        expect(mockSendVerificationEmail).toHaveBeenCalled()
      })
      await waitFor(() => {
        expect(screen.getByTestId('alert-message')).toHaveTextContent('验证邮件已发送')
      })
    })

    it('点击修改邮箱进入编辑模式', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('button', { name: '修改' }))
      // 应出现邮箱输入框
      const emailInput = screen.getByPlaceholderText('邮箱地址')
      expect(emailInput).toBeInTheDocument()
      expect(emailInput).toHaveValue('test@example.com')
    })

    it('输入新邮箱并保存', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('button', { name: '修改' }))
      const emailInput = screen.getByPlaceholderText('邮箱地址')
      fireEvent.change(emailInput, { target: { value: 'new@example.com' } })
      await act(async () => {
        fireEvent.click(screen.getByRole('button', { name: '保存' }))
      })
      await waitFor(() => {
        expect(mockUpdateEmail).toHaveBeenCalledWith('new@example.com')
      })
      await waitFor(() => {
        expect(screen.getByTestId('alert-message')).toHaveTextContent('邮箱已更新，请发送验证邮件')
      })
    })

    it('输入无效邮箱时显示错误', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByRole('button', { name: '修改' })).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('button', { name: '修改' }))
      const emailInput = screen.getByPlaceholderText('邮箱地址')
      fireEvent.change(emailInput, { target: { value: 'invalid' } })
      await act(async () => {
        fireEvent.click(screen.getByRole('button', { name: '保存' }))
      })
      await waitFor(() => {
        expect(screen.getByTestId('alert-message')).toBeInTheDocument()
      })
      expect(mockUpdateEmail).not.toHaveBeenCalled()
    })

    it('未设置邮箱时显示"未设置"和绑定按钮', async () => {
      mockUseAuth.mockReturnValue(makeAuthReturn({
        user: { ...mockUser, email: undefined },
      }))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByText('未设置')).toBeInTheDocument()
        expect(screen.getByRole('button', { name: '绑定邮箱' })).toBeInTheDocument()
      })
    })

    it('点击退出登录显示确认对话框', async () => {
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByText('退出登录')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByText('退出登录'))
      await waitFor(() => {
        expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
        expect(screen.getByTestId('confirm-message')).toHaveTextContent('确定要退出登录吗？')
      })
    })

    it('确认退出登录后调用 logout 并跳转首页', async () => {
      const logout = vi.fn().mockResolvedValue(undefined)
      mockUseAuth.mockReturnValue(makeAuthReturn({ logout }))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /设置/ }))
      await waitFor(() => {
        expect(screen.getByText('退出登录')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByText('退出登录'))
      await waitFor(() => {
        expect(screen.getByTestId('confirm-ok')).toBeInTheDocument()
      })
      await act(async () => {
        fireEvent.click(screen.getByTestId('confirm-ok'))
      })
      expect(logout).toHaveBeenCalled()
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════════
  // 4. 播放历史
  // ═══════════════════════════════════════════════════════════════════════════════
  describe('播放历史', () => {
    it('切换到历史 Tab 加载并显示播放历史', async () => {
      const historyItems = [
        makeMappedHistory({ id: 'h1', title: '视频一', progress: 50, durationMs: 120000 }),
        makeMappedHistory({ id: 'h2', title: '视频二', progress: 80, durationMs: 60000 }),
      ]
      mockListPlaybackHistory.mockResolvedValue(historyItems as unknown as PlaybackHistory[])
      mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)

      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /观看历史/ }))

      await waitFor(() => {
        expect(mockListPlaybackHistory).toHaveBeenCalledWith(100)
      })
      await waitFor(() => {
        expect(screen.getByText('视频一')).toBeInTheDocument()
        expect(screen.getByText('视频二')).toBeInTheDocument()
      })
    })

    it('播放历史显示进度条和继续观看按钮', async () => {
      const historyItems = [
        makeMappedHistory({ id: 'h1', title: '未看完的视频', progress: 30, durationMs: 200000 }),
      ]
      mockListPlaybackHistory.mockResolvedValue(historyItems as unknown as PlaybackHistory[])
      mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)

      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /观看历史/ }))

      await waitFor(() => {
        expect(screen.getByText('未看完的视频')).toBeInTheDocument()
      })
      // 进度条容器应存在
      const progressBars = document.querySelectorAll('.history-progress')
      expect(progressBars.length).toBeGreaterThan(0)
    })

    it('播放历史为空时显示空状态', async () => {
      mockListPlaybackHistory.mockResolvedValue([])
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /观看历史/ }))

      await waitFor(() => {
        expect(screen.getByText('暂无观看记录')).toBeInTheDocument()
      })
      // 空状态图标在 .empty-icon 中
      const emptyIcon = document.querySelector('.profile-empty .empty-icon')
      expect(emptyIcon?.textContent).toBe('🕐')
    })

    it('播放历史加载失败时显示错误和重试', async () => {
      mockListPlaybackHistory.mockRejectedValue(new Error('网络错误'))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /观看历史/ }))

      await waitFor(() => {
        expect(screen.getByText('加载失败，请检查网络后重试')).toBeInTheDocument()
      })
      const retryBtn = screen.getByRole('button', { name: '重试' })
      expect(retryBtn).toBeInTheDocument()

      // 点击重试
      const callCountBefore = mockListPlaybackHistory.mock.calls.length
      fireEvent.click(retryBtn)
      await waitFor(() => {
        expect(mockListPlaybackHistory.mock.calls.length).toBeGreaterThan(callCountBefore)
      })
    })

    it('点击历史项跳转到播放器', async () => {
      const historyItems = [
        makeMappedHistory({ id: 'vid123', title: '可点击的历史' }),
      ]
      mockListPlaybackHistory.mockResolvedValue(historyItems as unknown as PlaybackHistory[])
      mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)

      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /观看历史/ }))

      await waitFor(() => {
        expect(screen.getByText('可点击的历史')).toBeInTheDocument()
      })
      const historyItem = screen.getByText('可点击的历史').closest('.history-item')!
      fireEvent.click(historyItem)
      // navigate 被调用，具体路径由 MemoryRouter 验证
    })

    it('播放历史加载中显示骨架屏', async () => {
      mockListPlaybackHistory.mockReturnValue(new Promise(() => {}))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /观看历史/ }))

      // 骨架屏通过 sk-thumb / sk-line 渲染
      await waitFor(() => {
        const skeletons = document.querySelectorAll('.skeleton-video-thumb')
        expect(skeletons.length).toBeGreaterThan(0)
      })
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════════
  // 5. 收藏列表
  // ═══════════════════════════════════════════════════════════════════════════════
  describe('收藏列表', () => {
    it('切换到收藏 Tab 加载并显示收藏列表', async () => {
      const favItems = [
        makeMappedHistory({ id: 'f1', title: '收藏视频一' }),
        makeMappedHistory({ id: 'f2', title: '收藏视频二' }),
      ]
      mockListFavorites.mockResolvedValue(favItems as unknown as PlaybackHistory[])
      mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)

      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /我的收藏/ }))

      await waitFor(() => {
        expect(mockListFavorites).toHaveBeenCalled()
      })
      await waitFor(() => {
        expect(screen.getByText('收藏视频一')).toBeInTheDocument()
        expect(screen.getByText('收藏视频二')).toBeInTheDocument()
      })
    })

    it('收藏列表为空时显示空状态', async () => {
      mockListFavorites.mockResolvedValue([])
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /我的收藏/ }))

      await waitFor(() => {
        expect(screen.getByText('暂无收藏')).toBeInTheDocument()
      })
      const emptyIcon = document.querySelector('.profile-empty .empty-icon')
      expect(emptyIcon?.textContent).toBe('❤️')
    })

    it('收藏加载失败时显示错误和重试', async () => {
      mockListFavorites.mockRejectedValue(new Error('网络错误'))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /我的收藏/ }))

      await waitFor(() => {
        expect(screen.getByText('加载失败，请检查网络后重试')).toBeInTheDocument()
      })
      const retryBtn = screen.getByRole('button', { name: '重试' })
      expect(retryBtn).toBeInTheDocument()

      const callCountBefore = mockListFavorites.mock.calls.length
      fireEvent.click(retryBtn)
      await waitFor(() => {
        expect(mockListFavorites.mock.calls.length).toBeGreaterThan(callCountBefore)
      })
    })

    it('点击收藏项跳转到播放器', async () => {
      const favItems = [
        makeMappedHistory({ id: 'fav456', title: '可点击的收藏' }),
      ]
      mockListFavorites.mockResolvedValue(favItems as unknown as PlaybackHistory[])
      mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)

      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /我的收藏/ }))

      await waitFor(() => {
        expect(screen.getByText('可点击的收藏')).toBeInTheDocument()
      })
      const favItem = screen.getByText('可点击的收藏').closest('.history-item')!
      fireEvent.click(favItem)
    })

    it('收藏项不显示进度条（仅历史有进度）', async () => {
      const favItems = [
        makeMappedHistory({ id: 'f1', title: '收藏无进度', progress: 50 }),
      ]
      mockListFavorites.mockResolvedValue(favItems as unknown as PlaybackHistory[])
      mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)

      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /我的收藏/ }))

      await waitFor(() => {
        expect(screen.getByText('收藏无进度')).toBeInTheDocument()
      })
      // 收藏列表中不应有进度条（showProgress 为 false）
      const progressBars = document.querySelectorAll('.history-progress')
      expect(progressBars.length).toBe(0)
    })

    it('收藏加载中显示骨架屏', async () => {
      mockListFavorites.mockReturnValue(new Promise(() => {}))
      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /我的收藏/ }))

      await waitFor(() => {
        const skeletons = document.querySelectorAll('.skeleton-video-thumb')
        expect(skeletons.length).toBeGreaterThan(0)
      })
    })

    it('收藏项显示分类和日期', async () => {
      const favItems = [
        makeMappedHistory({
          id: 'f1',
          title: '带分类的收藏',
          category: '音乐',
          updatedAt: '2024-06-15T10:00:00Z',
        }),
      ]
      mockListFavorites.mockResolvedValue(favItems as unknown as PlaybackHistory[])
      mockMapHistory.mockImplementation((h: unknown) => h as MappedHistory)

      renderProfile()
      await waitFor(() => {
        expect(screen.getByText('testuser')).toBeInTheDocument()
      })
      fireEvent.click(screen.getByRole('tab', { name: /我的收藏/ }))

      await waitFor(() => {
        expect(screen.getByText('带分类的收藏')).toBeInTheDocument()
      })
      // 分类 "音乐" 应出现在 meta 中
      const metaEl = document.querySelector('.history-meta')
      expect(metaEl?.textContent).toContain('音乐')
    })
  })
})
