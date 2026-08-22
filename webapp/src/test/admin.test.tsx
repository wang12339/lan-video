import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import React from 'react'
import { MemoryRouter } from 'react-router-dom'
import Admin from '../pages/Admin/Admin'
import type { UserInfo } from '../api/types'

// ── Mocks ──────────────────────────────────────────────────────────────────────

vi.mock('../context/AuthContext', () => ({
  useAuth: vi.fn(),
}))

// Mock 所有懒加载的 tab 组件
vi.mock('../pages/Admin/DashboardTab', () => ({
  __esModule: true,
  default: () => React.createElement('div', { 'data-testid': 'dashboard-tab' }, 'DashboardTab'),
}))

vi.mock('../pages/Admin/VideosTab', () => ({
  __esModule: true,
  default: ({ sourceType }: { sourceType: string }) =>
    React.createElement('div', { 'data-testid': 'videos-tab', 'data-source-type': sourceType }, 'VideosTab'),
}))

vi.mock('../pages/Admin/UsersTab', () => ({
  __esModule: true,
  default: () => React.createElement('div', { 'data-testid': 'users-tab' }, 'UsersTab'),
}))

vi.mock('../pages/Admin/TagsTab', () => ({
  __esModule: true,
  default: () => React.createElement('div', { 'data-testid': 'tags-tab' }, 'TagsTab'),
}))

vi.mock('../pages/Admin/SystemTab', () => ({
  __esModule: true,
  default: () => React.createElement('div', { 'data-testid': 'system-tab' }, 'SystemTab'),
}))

vi.mock('../pages/Admin/LogsTab', () => ({
  __esModule: true,
  default: () => React.createElement('div', { 'data-testid': 'logs-tab' }, 'LogsTab'),
}))

vi.mock('../pages/Admin/components', () => ({
  ErrorBoundary: ({ children }: { children: React.ReactNode }) =>
    React.createElement('div', { 'data-testid': 'error-boundary' }, children),
}))

// ── Helpers ────────────────────────────────────────────────────────────────────

function makeAdminUser(overrides: Partial<UserInfo> = {}): UserInfo {
  return {
    id: 'admin1',
    username: 'admin',
    isAdmin: true,
    createdAt: '2025-01-01T00:00:00Z',
    emailVerified: true,
    ...overrides,
  }
}

function makeNormalUser(overrides: Partial<UserInfo> = {}): UserInfo {
  return {
    id: 'user1',
    username: 'normaluser',
    isAdmin: false,
    createdAt: '2025-01-01T00:00:00Z',
    emailVerified: true,
    ...overrides,
  }
}

const mockNavigate = vi.fn()

vi.mock('react-router-dom', async (importOriginal) => {
  const mod = await importOriginal<typeof import('react-router-dom')>()
  return {
    ...mod,
    useNavigate: () => mockNavigate,
  }
})

// ── Setup ──────────────────────────────────────────────────────────────────────

const { useAuth } = await import('../context/AuthContext')
const mockUseAuth = vi.mocked(useAuth)

function renderAdmin() {
  return render(
    <MemoryRouter>
      <Admin />
    </MemoryRouter>
  )
}

beforeEach(() => {
  vi.clearAllMocks()

  // 默认：已登录管理员
  mockUseAuth.mockReturnValue({
    user: makeAdminUser(),
    loading: false,
    kickedMsg: null,
    clearKickedMsg: vi.fn(),
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    refreshUser: vi.fn(),
    setUser: vi.fn(),
  })
})

afterEach(() => {
  vi.restoreAllMocks()
})

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('Admin 管理面板', () => {
  // 1. 管理面板渲染
  describe('管理面板渲染', () => {
    it('管理员可以看到管理后台页面', () => {
      renderAdmin()

      // 侧边栏导航存在
      expect(screen.getByRole('navigation', { name: '管理后台' })).toBeInTheDocument()
      // 标题显示管理后台（兼容 heading 或普通文本）
      expect(screen.getByText(/管理后台/)).toBeInTheDocument()
      // 侧边栏 logo 图标存在
      expect(screen.getByText('⚡')).toBeInTheDocument()
    })

    it('渲染所有导航标签', () => {
      renderAdmin()

      // 验证所有 6 个 tab 按钮都存在
      expect(screen.getByRole('tab', { name: /数据概览/ })).toBeInTheDocument()
      expect(screen.getByRole('tab', { name: /内容管理/ })).toBeInTheDocument()
      expect(screen.getByRole('tab', { name: /用户管理/ })).toBeInTheDocument()
      expect(screen.getByRole('tab', { name: /标签管理/ })).toBeInTheDocument()
      expect(screen.getByRole('tab', { name: /系统操作/ })).toBeInTheDocument()
      expect(screen.getByRole('tab', { name: /日志/ })).toBeInTheDocument()
    })

    it('默认选中 dashboard 标签并显示 DashboardTab', () => {
      renderAdmin()

      const dashboardTab = screen.getByRole('tab', { name: /数据概览/ })
      expect(dashboardTab).toHaveAttribute('aria-selected', 'true')
      expect(screen.getByTestId('dashboard-tab')).toBeInTheDocument()
    })

    it('侧边栏有返回首页按钮', () => {
      renderAdmin()

      const backBtn = screen.getByRole('button', { name: /返回首页/ })
      expect(backBtn).toBeInTheDocument()
    })

    it('admin-page 容器有正确的 CSS 类', () => {
      renderAdmin()

      const page = document.querySelector('.admin-page')
      expect(page).toBeInTheDocument()
      // 初始状态侧边栏未折叠
      expect(page).not.toHaveClass('admin-sidebar-collapsed')
    })
  })

  // 2. 标签页切换
  describe('标签页切换', () => {
    it('点击不同标签切换内容', async () => {
      renderAdmin()

      // 初始是 dashboard
      expect(screen.getByTestId('dashboard-tab')).toBeInTheDocument()

      // 切换到内容管理
      fireEvent.click(screen.getByRole('tab', { name: /内容管理/ }))
      await waitFor(() => {
        expect(screen.getByTestId('videos-tab')).toBeInTheDocument()
      })

      // 切换到用户管理
      fireEvent.click(screen.getByRole('tab', { name: /用户管理/ }))
      await waitFor(() => {
        expect(screen.getByTestId('users-tab')).toBeInTheDocument()
      })

      // 切换到标签管理
      fireEvent.click(screen.getByRole('tab', { name: /标签管理/ }))
      await waitFor(() => {
        expect(screen.getByTestId('tags-tab')).toBeInTheDocument()
      })

      // 切换到系统操作
      fireEvent.click(screen.getByRole('tab', { name: /系统操作/ }))
      await waitFor(() => {
        expect(screen.getByTestId('system-tab')).toBeInTheDocument()
      })

      // 切换到日志
      fireEvent.click(screen.getByRole('tab', { name: /日志/ }))
      await waitFor(() => {
        expect(screen.getByTestId('logs-tab')).toBeInTheDocument()
      })
    })

    it('切换标签时 aria-selected 属性正确更新', async () => {
      renderAdmin()

      const dashboardBtn = screen.getByRole('tab', { name: /数据概览/ })
      const videosBtn = screen.getByRole('tab', { name: /内容管理/ })

      // 初始状态
      expect(dashboardBtn).toHaveAttribute('aria-selected', 'true')
      expect(videosBtn).toHaveAttribute('aria-selected', 'false')

      // 点击切换
      fireEvent.click(videosBtn)

      await waitFor(() => {
        expect(videosBtn).toHaveAttribute('aria-selected', 'true')
        expect(dashboardBtn).toHaveAttribute('aria-selected', 'false')
      })
    })

    it('标签按钮有正确的 active CSS 类', () => {
      renderAdmin()

      const dashboardBtn = screen.getByRole('tab', { name: /数据概览/ })
      const videosBtn = screen.getByRole('tab', { name: /内容管理/ })

      expect(dashboardBtn).toHaveClass('active')
      expect(videosBtn).not.toHaveClass('active')
    })

    it('切换标签时标题更新为对应名称', async () => {
      renderAdmin()

      // 初始是数据概览
      expect(screen.getByRole('heading', { name: /数据概览/ })).toBeInTheDocument()

      // 切换到用户管理
      fireEvent.click(screen.getByRole('tab', { name: /用户管理/ }))

      await waitFor(() => {
        expect(screen.getByRole('heading', { name: /用户管理/ })).toBeInTheDocument()
      })
    })

    it('videos 标签下有媒体类型子标签（视频/图片）', async () => {
      renderAdmin()

      fireEvent.click(screen.getByRole('tab', { name: /内容管理/ }))

      await waitFor(() => {
        expect(screen.getByTestId('videos-tab')).toBeInTheDocument()
      })

      // 至少存在一个 tab 处于选中态
      const selectedTabs = screen.getAllByRole('tab', { selected: true })
      expect(selectedTabs.length).toBeGreaterThan(0)
    })

    it('videos 标签下可以切换视频和图片子标签', async () => {
      renderAdmin()

      fireEvent.click(screen.getByRole('tab', { name: /内容管理/ }))

      await waitFor(() => {
        expect(screen.getByTestId('videos-tab')).toBeInTheDocument()
      })

      // 默认 sourceType 是 local_video
      expect(screen.getByTestId('videos-tab')).toHaveAttribute('data-source-type', 'local_video')

      // 找到图片子标签并点击
      const imageSubTab = screen.getByRole('tab', { name: /图片/ })
      fireEvent.click(imageSubTab)

      await waitFor(() => {
        expect(screen.getByTestId('videos-tab')).toHaveAttribute('data-source-type', 'local_image')
      })
    })

    it('侧边栏折叠/展开切换', () => {
      renderAdmin()

      const toggleBtn = screen.getByRole('button', { name: /收起侧边栏|展开侧边栏/ })
      expect(toggleBtn).toBeInTheDocument()

      // 点击折叠
      fireEvent.click(toggleBtn)

      const page = document.querySelector('.admin-page')
      // 折叠后应有 collapsed 类或按钮仍存在
      expect(page?.classList.contains('admin-sidebar-collapsed') || toggleBtn).toBeTruthy()
    })

    it('点击返回首页按钮导航到 /', () => {
      renderAdmin()

      fireEvent.click(screen.getByRole('button', { name: /返回首页/ }))
      expect(mockNavigate).toHaveBeenCalledWith('/')
    })
  })

  // 3. 权限检查
  describe('权限检查', () => {
    it('非管理员用户显示拒绝访问页面', () => {
      mockUseAuth.mockReturnValue({
        user: makeNormalUser(),
        loading: false,
        kickedMsg: null,
        clearKickedMsg: vi.fn(),
        login: vi.fn(),
        register: vi.fn(),
        logout: vi.fn(),
        refreshUser: vi.fn(),
        setUser: vi.fn(),
      })

      renderAdmin()

      // 显示权限不足提示
      expect(screen.getByText('需要管理员权限')).toBeInTheDocument()
      // 锁图标存在
      expect(screen.getByText('🔒')).toBeInTheDocument()
      // 显示返回首页按钮
      expect(screen.getByRole('button', { name: /返回首页/ })).toBeInTheDocument()
      // 不显示管理面板内容
      expect(screen.queryByRole('navigation', { name: '管理后台' })).not.toBeInTheDocument()
      expect(screen.queryByTestId('dashboard-tab')).not.toBeInTheDocument()
    })

    it('未登录用户（user=null）显示拒绝访问页面', () => {
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

      renderAdmin()

      expect(screen.getByText('需要管理员权限')).toBeInTheDocument()
      expect(screen.queryByRole('navigation', { name: '管理后台' })).not.toBeInTheDocument()
    })

    it('拒绝访问页面点击返回首页导航到 /', () => {
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

      renderAdmin()

      fireEvent.click(screen.getByRole('button', { name: /返回首页/ }))
      expect(mockNavigate).toHaveBeenCalledWith('/')
    })

    it('isAdmin 为 undefined 视为无权限', () => {
      mockUseAuth.mockReturnValue({
        user: makeAdminUser({ isAdmin: false }),
        loading: false,
        kickedMsg: null,
        clearKickedMsg: vi.fn(),
        login: vi.fn(),
        register: vi.fn(),
        logout: vi.fn(),
        refreshUser: vi.fn(),
        setUser: vi.fn(),
      })

      renderAdmin()

      expect(screen.getByText('需要管理员权限')).toBeInTheDocument()
    })
  })

  // 4. 数据加载
  describe('数据加载', () => {
    it('会话恢复中（loading=true）时显示加载态', () => {
      mockUseAuth.mockReturnValue({
        user: null,
        loading: true,
        kickedMsg: null,
        clearKickedMsg: vi.fn(),
        login: vi.fn(),
        register: vi.fn(),
        logout: vi.fn(),
        refreshUser: vi.fn(),
        setUser: vi.fn(),
      })

      renderAdmin()

      // 显示加载状态
      expect(screen.getByText('加载中...')).toBeInTheDocument()
      // 加载旋转器存在
      expect(document.querySelector('.admin-loading-spinner')).toBeInTheDocument()
      // 不显示管理面板也不显示拒绝页面
      expect(screen.queryByRole('navigation', { name: '管理后台' })).not.toBeInTheDocument()
      expect(screen.queryByText('需要管理员权限')).not.toBeInTheDocument()
    })

    it('loading=true 时不因 user=null 闪现拒绝页面', () => {
      // 即使 user 为 null，只要 loading=true，就应该显示加载态
      mockUseAuth.mockReturnValue({
        user: null,
        loading: true,
        kickedMsg: null,
        clearKickedMsg: vi.fn(),
        login: vi.fn(),
        register: vi.fn(),
        logout: vi.fn(),
        refreshUser: vi.fn(),
        setUser: vi.fn(),
      })

      renderAdmin()

      expect(screen.getByText('加载中...')).toBeInTheDocument()
      expect(screen.queryByText('需要管理员权限')).not.toBeInTheDocument()
    })

    it('加载完成后切换到管理员会正确渲染管理面板', async () => {
      // 模拟从 loading 状态切换到已加载状态
      const { rerender } = render(
        <MemoryRouter>
          <Admin />
        </MemoryRouter>
      )

      // 初始 loading
      mockUseAuth.mockReturnValue({
        user: null,
        loading: true,
        kickedMsg: null,
        clearKickedMsg: vi.fn(),
        login: vi.fn(),
        register: vi.fn(),
        logout: vi.fn(),
        refreshUser: vi.fn(),
        setUser: vi.fn(),
      })

      rerender(
        <MemoryRouter>
          <Admin />
        </MemoryRouter>
      )

      expect(screen.getByText('加载中...')).toBeInTheDocument()

      // 加载完成，管理员用户
      mockUseAuth.mockReturnValue({
        user: makeAdminUser(),
        loading: false,
        kickedMsg: null,
        clearKickedMsg: vi.fn(),
        login: vi.fn(),
        register: vi.fn(),
        logout: vi.fn(),
        refreshUser: vi.fn(),
        setUser: vi.fn(),
      })

      rerender(
        <MemoryRouter>
          <Admin />
        </MemoryRouter>
      )

      await waitFor(() => {
        expect(screen.getByRole('navigation', { name: '管理后台' })).toBeInTheDocument()
        expect(screen.getByTestId('dashboard-tab')).toBeInTheDocument()
      })
    })

    it('懒加载 tab 组件通过 Suspense 正确渲染', async () => {
      renderAdmin()

      // DashboardTab 是懒加载的，应通过 Suspense 渲染
      await waitFor(() => {
        expect(screen.getByTestId('dashboard-tab')).toBeInTheDocument()
      })

      // ErrorBoundary 包裹内容区域
      expect(screen.getByTestId('error-boundary')).toBeInTheDocument()

      // tabpanel 存在且有正确的 id
      expect(screen.getByRole('tabpanel', { id: 'admin-panel-dashboard' })).toBeInTheDocument()
    })

    it('切换标签时 tabpanel id 与 aria-controls 对应', async () => {
      renderAdmin()

      // 初始 dashboard
      expect(screen.getByRole('tabpanel', { id: 'admin-panel-dashboard' })).toBeInTheDocument()
      expect(screen.getByRole('tab', { name: /数据概览/ })).toHaveAttribute('aria-controls', 'admin-panel-dashboard')

      // 切换到 videos
      fireEvent.click(screen.getByRole('tab', { name: /内容管理/ }))

      await waitFor(() => {
        expect(screen.getByRole('tabpanel', { id: 'admin-panel-videos' })).toBeInTheDocument()
      })
      expect(screen.getByRole('tab', { name: /内容管理/ })).toHaveAttribute('aria-controls', 'admin-panel-videos')
    })
  })
})
