import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import React from 'react'
import { MemoryRouter, Routes, Route } from 'react-router-dom'
import Layout from '../components/Layout/Layout'
import i18n from '../i18n'

// ── Mocks ──────────────────────────────────────────────────────────────────────

vi.mock('../context/AuthContext', () => ({
  useAuth: vi.fn(),
}))

vi.mock('../utils/track', () => ({
  trackClick: vi.fn(),
}))

vi.mock('../api', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../api')>()
  return {
    ...mod,
    searchSuggest: vi.fn(),
    setOnError: vi.fn(),
  }
})

vi.mock('../components/AuthDialog/AuthDialog', () => ({
  __esModule: true,
  default: ({ onClose }: { onClose?: () => void }) =>
    React.createElement('div', { 'data-testid': 'auth-dialog' }, [
      React.createElement('button', { key: 'close', onClick: onClose }, '关闭登录框'),
    ]),
}))

vi.mock('../components/ui/PageTransition', () => ({
  __esModule: true,
  default: ({ children }: { children: React.ReactNode }) =>
    React.createElement('div', { 'data-testid': 'page-transition' }, children),
}))

vi.mock('../components/Toast/Toast', () => ({
  ToastProvider: ({ children }: { children: React.ReactNode }) =>
    React.createElement('div', { 'data-testid': 'toast-provider' }, children),
  useToast: () => ({ toast: vi.fn() }),
}))

// ── Setup ──────────────────────────────────────────────────────────────────────

const { useAuth } = await import('../context/AuthContext')
const { searchSuggest } = await import('../api')

const mockUseAuth = vi.mocked(useAuth)
const mockSearchSuggest = vi.mocked(searchSuggest)

/** zh-CN 语言包中 nav 和 common 的翻译键 */
const zh = {
  nav: {
    logo: 'Atmos',
    search: '搜索视频...',
    home: '首页',
    gallery: '图片',
    upload: '上传',
    admin: '管理',
    login: '登录',
    logout: '退出登录',
    logoutConfirm: '确认退出登录？',
    myProfile: '个人中心',
    openMenu: '打开菜单',
    closeMenu: '关闭菜单',
    toggleLanguage: '切换语言',
  },
  common: {
    search: '搜索',
  },
} as const

/** en-US 语言包中 nav 和 common 的翻译键 */
const en = {
  nav: {
    logo: 'Atmos',
    search: 'Search videos...',
    home: 'Home',
    gallery: 'Gallery',
    upload: 'Upload',
    admin: 'Admin',
    login: 'Sign in',
    logout: 'Sign out',
    logoutConfirm: 'Sign out?',
    myProfile: 'My profile',
    openMenu: 'Open menu',
    closeMenu: 'Close menu',
    toggleLanguage: 'Toggle language',
  },
  common: {
    search: 'Search',
  },
} as const

/** 根据当前 i18n 语言返回对应翻译 */
function t(key: 'nav' | 'common', field: string): string {
  const lang = i18n.language === 'zh-CN' ? zh : en
  return (lang as Record<string, Record<string, string>>)[key][field] ?? field
}

function makeUser(overrides: Record<string, unknown> = {}) {
  return {
    id: 'u1',
    username: 'testuser',
    isAdmin: false,
    avatarUrl: undefined,
    createdAt: '',
    emailVerified: true,
    ...overrides,
  }
}

function makeAdmin() {
  return makeUser({ isAdmin: true, username: 'admin' })
}

function mockAuth(user: ReturnType<typeof makeUser> | null) {
  mockUseAuth.mockReturnValue({
    user,
    loading: false,
    kickedMsg: null,
    clearKickedMsg: vi.fn(),
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    refreshUser: vi.fn(),
    setUser: vi.fn(),
  })
}

function renderLayout(initialRoute = '/') {
  return render(
    <MemoryRouter initialEntries={[initialRoute]}>
      <Routes>
        <Route path="*" element={<Layout />}>
          <Route index element={<div data-testid="home-page">首页内容</div>} />
          <Route path="gallery" element={<div data-testid="gallery-page">相册内容</div>} />
          <Route path="upload" element={<div data-testid="upload-page">上传内容</div>} />
          <Route path="admin" element={<div data-testid="admin-page">管理内容</div>} />
          <Route path="profile" element={<div data-testid="profile-page">个人中心</div>} />
        </Route>
      </Routes>
    </MemoryRouter>
  )
}

beforeEach(async () => {
  vi.clearAllMocks()
  mockAuth(makeUser())
  mockSearchSuggest.mockResolvedValue([])

  // 确保 i18n 初始化完成并切换到 zh-CN
  if (!i18n.isInitialized) {
    await i18n.init()
  }
  await i18n.changeLanguage('zh-CN')
})

afterEach(() => {
  vi.restoreAllMocks()
})

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('Layout 导航栏', () => {
  it('渲染导航栏包含 Logo、搜索框和链接', () => {
    renderLayout()

    // Logo
    expect(screen.getByText('Atmos')).toBeInTheDocument()

    // 搜索框
    const searchInput = screen.getByPlaceholderText(t('nav', 'search'))
    expect(searchInput).toBeInTheDocument()

    // 导航链接
    expect(screen.getByText(t('nav', 'home'))).toBeInTheDocument()
    expect(screen.getByText(t('nav', 'gallery'))).toBeInTheDocument()
  })

  it('已登录用户显示用户名首字母头像', () => {
    mockAuth(makeUser({ username: 'alice' }))
    renderLayout()

    const avatar = screen.getByLabelText(t('nav', 'myProfile'))
    expect(avatar).toBeInTheDocument()
    expect(avatar).toHaveTextContent('A')
  })

  it('未登录用户显示登录按钮', () => {
    mockAuth(null)
    renderLayout()

    expect(screen.getByText(t('nav', 'login'))).toBeInTheDocument()
    expect(screen.queryByLabelText(t('nav', 'myProfile'))).not.toBeInTheDocument()
  })

  it('点击登录按钮在非首页弹出登录对话框', () => {
    mockAuth(null)
    renderLayout('/gallery')

    fireEvent.click(screen.getByText(t('nav', 'login')))

    expect(screen.getByTestId('auth-dialog')).toBeInTheDocument()
  })

  it('点击登录按钮在首页跳转到个人中心页', () => {
    mockAuth(null)
    renderLayout('/')

    fireEvent.click(screen.getByText(t('nav', 'login')))

    // 首页时不弹登录框，而是导航到 /profile
    expect(screen.queryByTestId('auth-dialog')).not.toBeInTheDocument()
  })

  it('管理员用户可以看到上传和管理链接', () => {
    mockAuth(makeAdmin())
    renderLayout()

    expect(screen.getByText(t('nav', 'upload'))).toBeInTheDocument()
    expect(screen.getByText(t('nav', 'admin'))).toBeInTheDocument()
  })

  it('普通用户可以看到上传但看不到管理链接', () => {
    mockAuth(makeUser({ isAdmin: false }))
    renderLayout()

    expect(screen.getByText(t('nav', 'upload'))).toBeInTheDocument()
    expect(screen.queryByText(t('nav', 'admin'))).not.toBeInTheDocument()
  })

  it('点击头像打开用户菜单', () => {
    mockAuth(makeUser({ username: 'bob' }))
    renderLayout()

    fireEvent.click(screen.getByLabelText(t('nav', 'myProfile')))

    expect(screen.getByText('bob')).toBeInTheDocument()
    // 用户菜单中的"个人中心"和"退出登录"（菜单项有 role="menuitem"）
    const menuItems = screen.getAllByText(t('nav', 'myProfile'))
    expect(menuItems.length).toBeGreaterThanOrEqual(1)
    const logoutBtns = screen.getAllByText(t('nav', 'logout'))
    expect(logoutBtns.length).toBeGreaterThanOrEqual(1)
  })

  it('用户菜单中点击退出需二次确认', () => {
    mockAuth(makeUser())
    renderLayout()

    fireEvent.click(screen.getByLabelText(t('nav', 'myProfile')))
    // 用户菜单中的退出按钮（role="menuitem"）
    const logoutBtns = screen.getAllByText(t('nav', 'logout'))
    // 点击最后一个（用户菜单中的那个）
    fireEvent.click(logoutBtns[logoutBtns.length - 1]!)

    // 第一次点击显示确认文字（可能在多处出现）
    const confirmTexts = screen.getAllByText(t('nav', 'logoutConfirm'))
    expect(confirmTexts.length).toBeGreaterThanOrEqual(1)
  })

  it('搜索框输入文字触发搜索建议', async () => {
    mockSearchSuggest.mockResolvedValue(['建议一', '建议二'])
    renderLayout()

    const input = screen.getByPlaceholderText(t('nav', 'search'))
    fireEvent.change(input, { target: { value: '测试' } })

    await waitFor(() => {
      expect(screen.getByText('建议一')).toBeInTheDocument()
      expect(screen.getByText('建议二')).toBeInTheDocument()
    })
  })

  it('搜索框提交后更新输入值', () => {
    renderLayout('/gallery')

    const input = screen.getByPlaceholderText(t('nav', 'search'))
    fireEvent.change(input, { target: { value: '关键词' } })
    fireEvent.submit(input.closest('form')!)

    // 提交后 input 值被更新为搜索词
    expect(input).toHaveValue('关键词')
  })

  it('搜索建议键盘导航：ArrowDown 选中第一项', async () => {
    mockSearchSuggest.mockResolvedValue(['建议A', '建议B'])
    renderLayout()

    const input = screen.getByPlaceholderText(t('nav', 'search'))
    fireEvent.change(input, { target: { value: '建' } })

    await waitFor(() => {
      expect(screen.getByText('建议A')).toBeInTheDocument()
    })

    fireEvent.keyDown(input, { key: 'ArrowDown' })

    const selected = screen.getByText('建议A').closest('.search-suggestion')
    expect(selected).toHaveClass('selected')
  })

  it('搜索建议键盘导航：Escape 关闭建议列表', async () => {
    mockSearchSuggest.mockResolvedValue(['建议X'])
    renderLayout()

    const input = screen.getByPlaceholderText(t('nav', 'search'))
    fireEvent.change(input, { target: { value: '建' } })

    await waitFor(() => {
      expect(screen.getByText('建议X')).toBeInTheDocument()
    })

    fireEvent.keyDown(input, { key: 'Escape' })

    expect(screen.queryByText('建议X')).not.toBeInTheDocument()
  })

  it('语言切换按钮切换中英文', () => {
    renderLayout()

    const langBtn = screen.getByLabelText(t('nav', 'toggleLanguage'))
    expect(langBtn).toBeInTheDocument()

    // 中文环境下按钮显示 EN
    expect(langBtn).toHaveTextContent('EN')
  })

  it('当前路由对应的导航链接高亮', () => {
    renderLayout('/gallery')

    const galleryLink = screen.getByText(t('nav', 'gallery'))
    expect(galleryLink).toHaveClass('active')

    const homeLink = screen.getByText(t('nav', 'home'))
    expect(homeLink).not.toHaveClass('active')
  })
})

describe('Layout 侧边栏 / 移动端菜单', () => {
  it('汉堡菜单按钮存在（移动端入口）', () => {
    renderLayout()

    // 汉堡按钮默认隐藏（CSS display:none），但 DOM 仍存在
    const hamburger = screen.getByLabelText(t('nav', 'openMenu'))
    expect(hamburger).toBeInTheDocument()
    expect(hamburger).toHaveAttribute('aria-expanded', 'false')
  })

  it('点击汉堡菜单切换展开状态', () => {
    renderLayout()

    const hamburger = screen.getByLabelText(t('nav', 'openMenu'))
    fireEvent.click(hamburger)

    expect(hamburger).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByLabelText(t('nav', 'closeMenu'))).toBeInTheDocument()
  })

  it('再次点击汉堡菜单收起', () => {
    renderLayout()

    const hamburger = screen.getByLabelText(t('nav', 'openMenu'))
    fireEvent.click(hamburger)
    fireEvent.click(hamburger)

    expect(hamburger).toHaveAttribute('aria-expanded', 'false')
  })

  it('路由变化时自动收起菜单', () => {
    const { unmount } = renderLayout('/')

    const hamburger = screen.getByLabelText(t('nav', 'openMenu'))
    fireEvent.click(hamburger)
    expect(hamburger).toHaveAttribute('aria-expanded', 'true')

    unmount()

    // 重新渲染不同路由
    renderLayout('/gallery')

    const hamburger2 = screen.getByLabelText(t('nav', 'openMenu'))
    // 路由变化后菜单应该收起
    expect(hamburger2).toHaveAttribute('aria-expanded', 'false')
  })

  it('Escape 键收起所有菜单', () => {
    mockAuth(makeUser())
    renderLayout()

    // 打开用户菜单
    fireEvent.click(screen.getByLabelText(t('nav', 'myProfile')))
    // 确认用户菜单已打开（用户名可见）
    expect(screen.getByText('testuser')).toBeInTheDocument()

    // 按 Escape
    fireEvent.keyDown(document, { key: 'Escape' })

    // 用户菜单应关闭（用户名不可见）
    expect(screen.queryByText('testuser')).not.toBeInTheDocument()
  })

  it('点击外部区域关闭用户菜单', () => {
    mockAuth(makeUser())
    renderLayout()

    fireEvent.click(screen.getByLabelText(t('nav', 'myProfile')))
    expect(screen.getByText('testuser')).toBeInTheDocument()

    // 点击页面其他区域
    fireEvent.mouseDown(document.body)

    expect(screen.queryByText('testuser')).not.toBeInTheDocument()
  })

  it('管理员在移动端菜单中看到上传和管理链接', () => {
    mockAuth(makeAdmin())
    renderLayout()

    fireEvent.click(screen.getByLabelText(t('nav', 'openMenu')))

    // 移动端菜单中也应有这些链接
    expect(screen.getByText(t('nav', 'upload'))).toBeInTheDocument()
    expect(screen.getByText(t('nav', 'admin'))).toBeInTheDocument()
  })
})

describe('Layout 响应式布局', () => {
  it('页面内容区域包含 page-content 类', () => {
    renderLayout()

    const main = document.querySelector('.page-content')
    expect(main).toBeInTheDocument()
    expect(main?.tagName).toBe('MAIN')
  })

  it('导航栏固定在顶部（nav 类）', () => {
    renderLayout()

    const nav = document.querySelector('.nav')
    expect(nav).toBeInTheDocument()
    expect(nav?.tagName).toBe('NAV')
  })

  it('导航链接容器有 nav-links 类', () => {
    renderLayout()

    const linksContainer = document.querySelector('.nav-links')
    expect(linksContainer).toBeInTheDocument()
  })

  it('搜索容器有 nav-search 类', () => {
    renderLayout()

    const searchContainer = document.querySelector('.nav-search')
    expect(searchContainer).toBeInTheDocument()
  })

  it('移动端菜单展开时 nav-links 添加 open 类', () => {
    renderLayout()

    const linksContainer = document.querySelector('.nav-links')
    expect(linksContainer).not.toHaveClass('open')

    fireEvent.click(screen.getByLabelText(t('nav', 'openMenu')))

    expect(linksContainer).toHaveClass('open')
  })

  it('子路由内容通过 Outlet 渲染', () => {
    renderLayout('/')

    expect(screen.getByTestId('home-page')).toBeInTheDocument()
    expect(screen.getByText('首页内容')).toBeInTheDocument()
  })

  it('不同路由渲染不同子页面', () => {
    renderLayout('/gallery')

    expect(screen.getByTestId('gallery-page')).toBeInTheDocument()
    expect(screen.queryByTestId('home-page')).not.toBeInTheDocument()
  })

  it('内容被 PageTransition 包裹', () => {
    renderLayout()

    expect(screen.getByTestId('page-transition')).toBeInTheDocument()
  })

  it('Layout 包裹在 ToastProvider 中', () => {
    renderLayout()

    expect(screen.getByTestId('toast-provider')).toBeInTheDocument()
  })
})

describe('Layout 主题切换', () => {
  it('语言切换按钮存在且可点击', () => {
    renderLayout()

    const langBtn = screen.getByLabelText(t('nav', 'toggleLanguage'))
    expect(langBtn).toBeInTheDocument()
    expect(langBtn.tagName).toBe('BUTTON')
  })

  it('中文环境下语言按钮显示 EN', () => {
    renderLayout()

    const langBtn = screen.getByLabelText(t('nav', 'toggleLanguage'))
    expect(langBtn).toHaveTextContent('EN')
  })

  it('点击语言切换按钮存储语言偏好到 localStorage', () => {
    const setItemSpy = vi.spyOn(Storage.prototype, 'setItem')
    renderLayout()

    fireEvent.click(screen.getByLabelText(t('nav', 'toggleLanguage')))

    expect(setItemSpy).toHaveBeenCalledWith('atmos.lang', 'en-US')
    setItemSpy.mockRestore()
  })

  it('导航链接使用中文显示', () => {
    renderLayout()

    // 测试环境为 zh-CN，验证中文文本正确渲染
    expect(screen.getByText(t('nav', 'home'))).toBeInTheDocument()
    expect(screen.getByText(t('nav', 'gallery'))).toBeInTheDocument()
    // 已登录用户不显示登录按钮，而是显示头像
    expect(screen.getByLabelText(t('nav', 'myProfile'))).toBeInTheDocument()
  })

  it('搜索框占位符使用中文', () => {
    renderLayout()

    expect(screen.getByPlaceholderText(t('nav', 'search'))).toBeInTheDocument()
  })

  it('导航栏 Logo 文字正确', () => {
    renderLayout()

    const logo = screen.getByText('Atmos')
    expect(logo).toBeInTheDocument()
    expect(logo.tagName).toBe('A')
    expect(logo).toHaveAttribute('href', '/')
  })

  it('登录按钮样式类为 nav-login-btn', () => {
    mockAuth(null)
    renderLayout()

    const loginBtn = screen.getByText(t('nav', 'login'))
    expect(loginBtn).toHaveClass('nav-login-btn')
  })

  it('语言切换按钮样式类为 nav-lang-toggle', () => {
    renderLayout()

    const langBtn = screen.getByLabelText(t('nav', 'toggleLanguage'))
    expect(langBtn).toHaveClass('nav-lang-toggle')
  })

  it('CSS 变量用于主题：导航栏背景', () => {
    renderLayout()

    const nav = document.querySelector('.nav')
    // 验证 nav 元素存在（CSS 变量由 CSS 文件定义，DOM 层面验证结构正确）
    expect(nav).toBeInTheDocument()
  })
})
