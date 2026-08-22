import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, act, waitFor } from '@testing-library/react'
import { AuthProvider, useAuth } from '../context/AuthContext'
import type { UserInfo } from '../api/types'

// ── Mocks ──────────────────────────────────────────────────────────────────────

const mockGetUserInfo = vi.fn()
const mockApiLogin = vi.fn()
const mockApiLogout = vi.fn()
const mockApiRegister = vi.fn()
let onAuthRequiredCallback: ((msg?: string) => void) | null = null

// vi.mock 工厂会被提升到文件顶部，不能引用外部变量；
// 用 vi.mocked + beforeEach 动态注入回调。
vi.mock('../api', () => {
  return {
    getUserInfo: (...args: unknown[]) => mockGetUserInfo(...args),
    login: (...args: unknown[]) => mockApiLogin(...args),
    register: (...args: unknown[]) => mockApiRegister(...args),
    logout: (...args: unknown[]) => mockApiLogout(...args),
    AuthError: class AuthError extends Error {
      constructor(msg?: string) { super(msg ?? 'AuthError'); this.name = 'AuthError' }
    },
    setOnAuthRequired: (cb: (msg?: string) => void) => { onAuthRequiredCallback = cb },
  }
})

vi.mock('../i18n', () => ({
  default: { t: (key: string) => key },
}))

// ── 测试工具 ────────────────────────────────────────────────────────────────────

const mockUser: UserInfo = {
  id: 'user-1',
  username: 'testuser',
  isAdmin: false,
  avatarUrl: 'https://example.com/avatar.png',
  createdAt: '2024-01-01T00:00:00Z',
  email: 'test@example.com',
  emailVerified: true,
}

const mockAdminUser: UserInfo = {
  id: 'admin-1',
  username: 'admin',
  isAdmin: true,
  createdAt: '2024-01-01T00:00:00Z',
  emailVerified: true,
}

// 从 mock 模块拿到 AuthError 构造函数，供测试中构造实例
const { AuthError: AuthErrorClass } = await import('../api')

/** 读取 Context 值的测试组件 */
function Consumer({ onReady }: { onReady: (ctx: ReturnType<typeof useAuth>) => void }) {
  const ctx = useAuth()
  onReady(ctx)
  return <div data-testid="user-display">{ctx.user?.username ?? 'no-user'}</div>
}

function renderWithAuth(onReady: (ctx: ReturnType<typeof useAuth>) => void) {
  return render(
    <AuthProvider>
      <Consumer onReady={onReady} />
    </AuthProvider>,
  )
}

/** 构造 AuthError 实例 */
function authError(msg?: string) {
  return new (AuthErrorClass as new (msg?: string) => Error)(msg)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('AuthContext', () => {
  let ctx: ReturnType<typeof useAuth>

  beforeEach(() => {
    vi.resetAllMocks()
    onAuthRequiredCallback = null
    // 默认 getUserInfo 抛出 AuthError（未登录）
    mockGetUserInfo.mockRejectedValue(authError('未登录'))
  })

  // =========================================================================
  // 1) 登录状态
  // =========================================================================
  describe('登录状态', () => {
    it('初始状态 loading=true，user=null', async () => {
      let receivedLoading = true
      renderWithAuth((c) => {
        ctx = c
        receivedLoading = c.loading
      })

      // getUserInfo 被调用前 loading 应为 true
      expect(receivedLoading).toBe(true)
    })

    it('getUserInfo 成功后 loading=false，user 有值', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)

      renderWithAuth((c) => { ctx = c })

      await waitFor(() => {
        expect(ctx.loading).toBe(false)
        expect(ctx.user).toEqual(mockUser)
      })
    })

    it('getUserInfo 失败（AuthError）后 loading=false，user=null', async () => {
      mockGetUserInfo.mockRejectedValue(authError('未登录'))

      renderWithAuth((c) => { ctx = c })

      await waitFor(() => {
        expect(ctx.loading).toBe(false)
        expect(ctx.user).toBeNull()
      })
    })

    it('getUserInfo 网络错误时保留 user=null，loading=false', async () => {
      mockGetUserInfo.mockRejectedValue(new Error('Network Error'))

      renderWithAuth((c) => { ctx = c })

      await waitFor(() => {
        expect(ctx.loading).toBe(false)
        expect(ctx.user).toBeNull()
      })
    })

    it('useAuth 在 AuthProvider 外调用时抛出错误', () => {
      const OutsideConsumer = () => {
        useAuth()
        return null
      }

      const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
      expect(() => render(<OutsideConsumer />)).toThrow('useAuth must be used within AuthProvider')
      spy.mockRestore()
    })
  })

  // =========================================================================
  // 2) Token 管理（登录/登出流程中的 session 递增 & refreshUser 行为）
  // =========================================================================
  describe('Token 管理（session 代数）', () => {
    it('login 调用后 session 递增，refreshUser 拿到新用户', async () => {
      mockGetUserInfo.mockRejectedValue(authError('未登录'))
      mockApiLogin.mockResolvedValue(undefined)

      renderWithAuth((c) => { ctx = c })

      await waitFor(() => expect(ctx.loading).toBe(false))

      // 模拟登录成功
      mockGetUserInfo.mockResolvedValueOnce(mockUser)
      await act(async () => {
        await ctx.login('testuser', 'password123')
      })

      expect(mockApiLogin).toHaveBeenCalledWith('testuser', 'password123')
      expect(ctx.user).toEqual(mockUser)
    })

    it('login 后旧的 refreshUser 结果被丢弃（session 不匹配）', async () => {
      mockGetUserInfo.mockRejectedValue(authError('未登录'))
      mockApiLogin.mockResolvedValue(undefined)

      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.loading).toBe(false))

      // 第一次 refreshUser 返回旧用户（延迟），第二次返回新用户
      let resolveFirst!: (v: UserInfo) => void
      const firstCall = new Promise<UserInfo>((r) => { resolveFirst = r })
      mockGetUserInfo
        .mockImplementationOnce(() => firstCall)
        .mockResolvedValueOnce(mockAdminUser)

      const loginPromise = act(async () => {
        await ctx.login('admin', 'adminpass')
      })

      resolveFirst(mockUser)
      await loginPromise

      await waitFor(() => {
        expect(ctx.user).toBeDefined()
      })
    })

    it('refreshUser 中 AuthError 导致 session 递增并清空 user', async () => {
      mockGetUserInfo.mockReset()
      mockGetUserInfo.mockResolvedValue(mockUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      mockGetUserInfo.mockReset()
      mockGetUserInfo.mockRejectedValue(authError('Token expired'))
      await act(async () => {
        await ctx.refreshUser()
      })

      expect(ctx.user).toBeNull()
    })

    it('refreshUser 中非 AuthError（网络错误）保留现有 user', async () => {
      mockGetUserInfo.mockReset()
      mockGetUserInfo.mockResolvedValue(mockUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      mockGetUserInfo.mockReset()
      mockGetUserInfo.mockRejectedValue(new Error('Network timeout'))
      await act(async () => {
        await ctx.refreshUser()
      })

      expect(ctx.user).toEqual(mockUser)
    })
  })

  // =========================================================================
  // 3) 用户信息
  // =========================================================================
  describe('用户信息', () => {
    it('setUser 可手动更新用户信息', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      act(() => {
        ctx.setUser(mockAdminUser)
      })
      expect(ctx.user).toEqual(mockAdminUser)
    })

    it('setUser(null) 清除用户', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      act(() => {
        ctx.setUser(null)
      })
      expect(ctx.user).toBeNull()
    })

    it('用户信息包含所有预期字段', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      expect(ctx.user).toMatchObject({
        id: 'user-1',
        username: 'testuser',
        isAdmin: false,
        avatarUrl: 'https://example.com/avatar.png',
        email: 'test@example.com',
        emailVerified: true,
      })
    })

    it('管理员用户 isAdmin=true', async () => {
      mockGetUserInfo.mockResolvedValue(mockAdminUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockAdminUser))

      expect(ctx.user?.isAdmin).toBe(true)
      expect(ctx.user?.username).toBe('admin')
    })

    it('用户无 avatarUrl 时 avatarUrl 为 undefined', async () => {
      const userNoAvatar: UserInfo = { ...mockUser, avatarUrl: undefined }
      mockGetUserInfo.mockResolvedValue(userNoAvatar)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(userNoAvatar))

      expect(ctx.user?.avatarUrl).toBeUndefined()
    })
  })

  // =========================================================================
  // 4) 登出功能
  // =========================================================================
  describe('登出功能', () => {
    it('logout 清空 user 并调用 apiLogout', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      mockApiLogout.mockResolvedValue(undefined)

      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      await act(async () => {
        await ctx.logout()
      })

      expect(mockApiLogout).toHaveBeenCalledTimes(1)
      expect(ctx.user).toBeNull()
    })

    it('logout 清除 kickedMsg', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      mockApiLogout.mockResolvedValue(undefined)

      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      // 模拟被踢
      act(() => {
        onAuthRequiredCallback?.('Token expired')
      })

      await act(async () => {
        await ctx.logout()
      })

      expect(ctx.kickedMsg).toBeNull()
    })

    it('logout 后 session 递增，旧 session 的在途结果被丢弃', async () => {
      mockGetUserInfo.mockReset()
      mockGetUserInfo.mockResolvedValue(mockUser)
      mockApiLogout.mockResolvedValue(undefined)

      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      // 模拟一个在 logout 之前启动的 refreshUser（旧 session 代数）
      // 它会在 logout 之后才 resolve，结果应被丢弃
      let resolveStaleRefresh!: (v: UserInfo) => void
      const staleCall = new Promise<UserInfo>((r) => { resolveStaleRefresh = r })
      mockGetUserInfo.mockReset()
      mockGetUserInfo.mockImplementationOnce(() => staleCall) // 旧 session 的 refreshUser

      // 手动调用 refreshUser（旧 session 代数），不等待
      const refreshPromise = ctx.refreshUser()

      // logout 递增 sessionRef
      await act(async () => {
        await ctx.logout()
      })
      expect(ctx.user).toBeNull()

      // 现在旧 session 的 refreshUser 才 resolve
      await act(async () => {
        resolveStaleRefresh(mockAdminUser)
        await refreshPromise
      })

      // 旧 session 的结果应被丢弃，user 仍为 null
      expect(ctx.user).toBeNull()
    })

    it('连续调用 logout 不会出错', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      mockApiLogout.mockResolvedValue(undefined)

      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      await act(async () => {
        await ctx.logout()
        await ctx.logout()
      })

      expect(mockApiLogout).toHaveBeenCalledTimes(2)
      expect(ctx.user).toBeNull()
    })
  })

  // =========================================================================
  // 5) kickedMsg（被踢提示）
  // =========================================================================
  describe('kickedMsg', () => {
    it('onAuthRequired 回调触发后设置 kickedMsg', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      // 模拟 401 触发 onAuthRequired，复验也失败则设置 kickedMsg
      mockGetUserInfo.mockRejectedValue(authError('Token expired'))
      await act(async () => {
        onAuthRequiredCallback?.('您的会话已过期，请重新登录')
      })

      await waitFor(() => {
        expect(ctx.kickedMsg).toBe('您的会话已过期，请重新登录')
      })
    })

    it('未登录时 onAuthRequired 不设置 kickedMsg', async () => {
      mockGetUserInfo.mockRejectedValue(authError('未登录'))
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.loading).toBe(false))

      // userRef.current 为 null，回调应忽略
      act(() => {
        onAuthRequiredCallback?.('Token expired')
      })

      expect(ctx.kickedMsg).toBeNull()
    })

    it('clearKickedMsg 清除 kickedMsg', async () => {
      mockGetUserInfo.mockResolvedValue(mockUser)
      renderWithAuth((c) => { ctx = c })
      await waitFor(() => expect(ctx.user).toEqual(mockUser))

      mockGetUserInfo.mockRejectedValue(authError('expired'))
      await act(async () => {
        onAuthRequiredCallback?.('会话过期')
      })
      await waitFor(() => expect(ctx.kickedMsg).toBe('会话过期'))

      act(() => {
        ctx.clearKickedMsg()
      })
      expect(ctx.kickedMsg).toBeNull()
    })
  })
})
