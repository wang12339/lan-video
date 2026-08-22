import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { usePWA, useServiceWorker } from '../hooks/usePWA'

// Mock window.matchMedia
const mockMatchMedia = vi.fn()
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: mockMatchMedia,
})

// Mock navigator
Object.defineProperty(window, 'navigator', {
  writable: true,
  value: {
    standalone: false,
    serviceWorker: {
      register: vi.fn(),
      ready: Promise.resolve({
        update: vi.fn(),
        addEventListener: vi.fn(),
      }),
    },
  },
})

describe('usePWA', () => {
  let addEventListenerSpy: ReturnType<typeof vi.spyOn>
  let removeEventListenerSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    addEventListenerSpy = vi.spyOn(window, 'addEventListener')
    removeEventListenerSpy = vi.spyOn(window, 'removeEventListener')
    mockMatchMedia.mockReturnValue({ matches: false })
    window.navigator.standalone = false
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('安装提示', () => {
    it('初始状态应该是不可安装且未安装', () => {
      const { result } = renderHook(() => usePWA())

      expect(result.current.isInstallable).toBe(false)
      expect(result.current.isInstalled).toBe(false)
      expect(result.current.isStandalone).toBe(false)
      expect(result.current.install).toBeInstanceOf(Function)
    })

    it('监听 beforeinstallprompt 事件', () => {
      renderHook(() => usePWA())

      expect(addEventListenerSpy).toHaveBeenCalledWith(
        'beforeinstallprompt',
        expect.any(Function)
      )
      expect(addEventListenerSpy).toHaveBeenCalledWith(
        'appinstalled',
        expect.any(Function)
      )
    })

    it('当触发 beforeinstallprompt 时，isInstallable 应为 true', async () => {
      const { result } = renderHook(() => usePWA())

      const mockEvent = {
        preventDefault: vi.fn(),
        platforms: ['web'],
        userChoice: Promise.resolve({ outcome: 'accepted' as const, platform: 'web' }),
        prompt: vi.fn(),
      } as unknown as BeforeInstallPromptEvent

      // 获取注册的事件处理函数
      const beforeInstallPromptHandler = addEventListenerSpy.mock.calls.find(
        (call) => call[0] === 'beforeinstallprompt'
      )?.[1] as EventListener

      await act(async () => {
        beforeInstallPromptHandler(mockEvent)
      })

      expect(result.current.isInstallable).toBe(true)
      expect(mockEvent.preventDefault).toHaveBeenCalled()
    })

    it('当触发 appinstalled 时，isInstalled 应为 true 且 isInstallable 应为 false', async () => {
      const { result } = renderHook(() => usePWA())

      const appInstalledHandler = addEventListenerSpy.mock.calls.find(
        (call) => call[0] === 'appinstalled'
      )?.[1] as EventListener

      await act(async () => {
        appInstalledHandler()
      })

      expect(result.current.isInstalled).toBe(true)
      expect(result.current.isInstallable).toBe(false)
    })

    it('install 应该调用 prompt 并根据 userChoice 更新状态', async () => {
      const { result } = renderHook(() => usePWA())

      const mockPrompt = vi.fn()
      const mockUserChoice = Promise.resolve({ outcome: 'accepted' as const, platform: 'web' })

      const mockEvent = {
        preventDefault: vi.fn(),
        platforms: ['web'],
        userChoice: mockUserChoice,
        prompt: mockPrompt,
      } as unknown as BeforeInstallPromptEvent

      // 模拟 beforeinstallprompt 事件触发
      const beforeInstallPromptHandler = addEventListenerSpy.mock.calls.find(
        (call) => call[0] === 'beforeinstallprompt'
      )?.[1] as EventListener

      await act(async () => {
        beforeInstallPromptHandler(mockEvent)
      })

      await act(async () => {
        const accepted = await result.current.install()
        expect(accepted).toBe(true)
      })

      expect(mockPrompt).toHaveBeenCalled()
      expect(result.current.isInstalled).toBe(true)
      expect(result.current.isInstallable).toBe(false)
    })

    it('install 在用户拒绝时应返回 false', async () => {
      const { result } = renderHook(() => usePWA())

      const mockEvent = {
        preventDefault: vi.fn(),
        platforms: ['web'],
        userChoice: Promise.resolve({ outcome: 'dismissed' as const, platform: 'web' }),
        prompt: vi.fn(),
      } as unknown as BeforeInstallPromptEvent

      const beforeInstallPromptHandler = addEventListenerSpy.mock.calls.find(
        (call) => call[0] === 'beforeinstallprompt'
      )?.[1] as EventListener

      await act(async () => {
        beforeInstallPromptHandler(mockEvent)
      })

      await act(async () => {
        const accepted = await result.current.install()
        expect(accepted).toBe(false)
      })

      expect(result.current.isInstalled).toBe(false)
    })

    it('install 在没有 installPrompt 时应返回 false', async () => {
      const { result } = renderHook(() => usePWA())

      await act(async () => {
        const accepted = await result.current.install()
        expect(accepted).toBe(false)
      })
    })

    it('install 在 prompt 抛出异常时应返回 false', async () => {
      const { result } = renderHook(() => usePWA())

      const mockEvent = {
        preventDefault: vi.fn(),
        platforms: ['web'],
        userChoice: Promise.resolve({ outcome: 'accepted' as const, platform: 'web' }),
        prompt: vi.fn().mockRejectedValue(new Error('prompt failed')),
      } as unknown as BeforeInstallPromptEvent

      const beforeInstallPromptHandler = addEventListenerSpy.mock.calls.find(
        (call) => call[0] === 'beforeinstallprompt'
      )?.[1] as EventListener

      await act(async () => {
        beforeInstallPromptHandler(mockEvent)
      })

      await act(async () => {
        const accepted = await result.current.install()
        expect(accepted).toBe(false)
      })
    })
  })

  describe('standalone 模式检测', () => {
    it('当 display-mode: standalone 时，isStandalone 应为 true', () => {
      mockMatchMedia.mockReturnValue({ matches: true })
      const { result } = renderHook(() => usePWA())

      expect(result.current.isStandalone).toBe(true)
      expect(result.current.isInstalled).toBe(true)
    })

    it('当 iOS standalone 模式时，isStandalone 应为 true', () => {
      window.navigator.standalone = true
      const { result } = renderHook(() => usePWA())

      expect(result.current.isStandalone).toBe(true)
    })
  })

  describe('事件清理', () => {
    it('组件卸载时应移除事件监听器', () => {
      const { unmount } = renderHook(() => usePWA())

      unmount()

      expect(removeEventListenerSpy).toHaveBeenCalledWith(
        'beforeinstallprompt',
        expect.any(Function)
      )
      expect(removeEventListenerSpy).toHaveBeenCalledWith(
        'appinstalled',
        expect.any(Function)
      )
    })
  })
})

describe('useServiceWorker', () => {
  let originalServiceWorker: typeof navigator.serviceWorker

  beforeEach(() => {
    originalServiceWorker = window.navigator.serviceWorker
    window.navigator.serviceWorker = {
      register: vi.fn().mockResolvedValue({
        addEventListener: vi.fn(),
        installing: null,
      }),
      ready: Promise.resolve({
        update: vi.fn(),
        addEventListener: vi.fn(),
      }),
    } as unknown as typeof navigator.serviceWorker
  })

  afterEach(() => {
    window.navigator.serviceWorker = originalServiceWorker
    vi.restoreAllMocks()
  })

  describe('更新检测', () => {
    it('初始状态应为未注册且无更新', async () => {
      const { result } = renderHook(() => useServiceWorker())

      await waitFor(() => {
        expect(result.current.isRegistered).toBe(false)
        expect(result.current.updateAvailable).toBe(false)
        expect(result.current.update).toBeInstanceOf(Function)
      })
    })

    it('当 serviceWorker 可用时应注册成功', async () => {
      const mockRegister = vi.fn().mockResolvedValue({
        addEventListener: vi.fn(),
        installing: null,
      })
      window.navigator.serviceWorker.register = mockRegister

      const { result } = renderHook(() => useServiceWorker())

      await waitFor(() => {
        expect(result.current.isRegistered).toBe(true)
      })

      expect(mockRegister).toHaveBeenCalledWith('/webapp/sw.js')
    })

    it('当 serviceWorker 不可用时应保持未注册', async () => {
      // @ts-expect-error - 测试不支持的情况
      delete window.navigator.serviceWorker

      const { result } = renderHook(() => useServiceWorker())

      await waitFor(() => {
        expect(result.current.isRegistered).toBe(false)
      })
    })

    it('注册失败时应捕获错误', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
      const mockRegister = vi.fn().mockRejectedValue(new Error('Registration failed'))
      window.navigator.serviceWorker.register = mockRegister

      renderHook(() => useServiceWorker())

      await waitFor(() => {
        expect(consoleSpy).toHaveBeenCalledWith(
          'SW registration failed:',
          expect.any(Error)
        )
      })

      consoleSpy.mockRestore()
    })

    it('当新 worker 激活时应设置 updateAvailable', async () => {
      const mockStateChangeHandler = vi.fn()
      const mockAddEventListener = vi.fn()
      const mockInstallingWorker = {
        addEventListener: mockAddEventListener,
        state: 'installing',
      }

      const mockRegistration = {
        addEventListener: vi.fn((event, handler) => {
          if (event === 'updatefound') {
            // 立即调用 updatefound 处理函数
            setTimeout(() => handler(), 0)
          }
        }),
        installing: mockInstallingWorker,
      }

      window.navigator.serviceWorker.register = vi.fn().mockResolvedValue(mockRegistration)

      const { result } = renderHook(() => useServiceWorker())

      await waitFor(() => {
        expect(result.current.isRegistered).toBe(true)
      })

      expect(mockRegistration.addEventListener).toHaveBeenCalledWith(
        'updatefound',
        expect.any(Function)
      )

      // 模拟 statechange 事件
      const stateChangeHandler = mockAddEventListener.mock.calls.find(
        (call) => call[0] === 'statechange'
      )?.[1]

      if (stateChangeHandler) {
        mockInstallingWorker.state = 'activated'
        await act(async () => {
          stateChangeHandler()
        })

        expect(result.current.updateAvailable).toBe(true)
      }
    })

    it('update 应该调用 registration.update()', async () => {
      const mockUpdate = vi.fn()
      window.navigator.serviceWorker.ready = Promise.resolve({
        update: mockUpdate,
        addEventListener: vi.fn(),
      })

      const { result } = renderHook(() => useServiceWorker())

      await act(async () => {
        result.current.update()
      })

      await waitFor(() => {
        expect(mockUpdate).toHaveBeenCalled()
      })
    })

    it('当 serviceWorker 不可用时 update 不应抛出错误', async () => {
      // @ts-expect-error - 测试不支持的情况
      delete window.navigator.serviceWorker

      const { result } = renderHook(() => useServiceWorker())

      await waitFor(() => {
        expect(() => {
          result.current.update()
        }).not.toThrow()
      })
    })
  })
})
