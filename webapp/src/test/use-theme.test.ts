import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useTheme } from '../hooks/useTheme'

// ── helpers ──────────────────────────────────────────────────────────────────

/** Mock matchMedia with a controllable listener store */
function setupMatchMedia(initialDark: boolean) {
  let listener: ((e: MediaQueryListEvent) => void) | null = null
  const mql = {
    matches: initialDark,
    addEventListener: vi.fn((_type: string, fn: (e: MediaQueryListEvent) => void) => {
      listener = fn
    }),
    removeEventListener: vi.fn(() => {
      listener = null
    }),
  }
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockReturnValue(mql as unknown as MediaQueryList),
  )
  return {
    mql,
    /** Simulate the OS switching between light ↔ dark */
    emitChange(dark: boolean) {
      listener?.({ matches: dark } as MediaQueryListEvent)
    },
  }
}

const THEME_KEY = 'atmos.theme'

// ── tests ────────────────────────────────────────────────────────────────────

describe('useTheme', () => {
  beforeEach(() => {
    localStorage.clear()
    document.documentElement.removeAttribute('data-theme')
    // Remove any existing theme-color meta
    document.querySelector('meta[name="theme-color"]')?.remove()
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  // ── 1. 主题切换 ──────────────────────────────────────────────────────────

  describe('主题切换', () => {
    it('默认主题为 system', () => {
      setupMatchMedia(true) // system = dark
      const { result } = renderHook(() => useTheme())
      expect(result.current.theme).toBe('system')
    })

    it('setTheme 切换到 dark / light 并更新 resolvedTheme', () => {
      setupMatchMedia(false)
      const { result } = renderHook(() => useTheme())

      act(() => result.current.setTheme('dark'))
      expect(result.current.theme).toBe('dark')
      expect(result.current.resolvedTheme).toBe('dark')

      act(() => result.current.setTheme('light'))
      expect(result.current.theme).toBe('light')
      expect(result.current.resolvedTheme).toBe('light')
    })

    it('setTheme("system") 回退到系统主题', () => {
      const { emitChange } = setupMatchMedia(true)
      const { result } = renderHook(() => useTheme())

      act(() => result.current.setTheme('dark'))
      expect(result.current.resolvedTheme).toBe('dark')

      act(() => result.current.setTheme('system'))
      expect(result.current.resolvedTheme).toBe('dark') // 系统当前 dark

      act(() => emitChange(false)) // 系统切到 light
      expect(result.current.resolvedTheme).toBe('light')
    })

    it('toggleTheme 在 dark ↔ light 之间切换', () => {
      setupMatchMedia(false)
      const { result } = renderHook(() => useTheme())

      // 初始 system → resolved = light
      expect(result.current.resolvedTheme).toBe('light')

      act(() => result.current.toggleTheme())
      expect(result.current.theme).toBe('dark')
      expect(result.current.resolvedTheme).toBe('dark')

      act(() => result.current.toggleTheme())
      expect(result.current.theme).toBe('light')
      expect(result.current.resolvedTheme).toBe('light')
    })

    it('resolvedTheme 变化后设置 data-theme 属性', () => {
      setupMatchMedia(false)
      const { result } = renderHook(() => useTheme())

      act(() => result.current.setTheme('dark'))
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark')

      act(() => result.current.setTheme('light'))
      expect(document.documentElement.getAttribute('data-theme')).toBe('light')
    })

    it('resolvedTheme 变化后更新 meta theme-color', () => {
      const meta = document.createElement('meta')
      meta.setAttribute('name', 'theme-color')
      document.head.appendChild(meta)

      setupMatchMedia(false)
      const { result } = renderHook(() => useTheme())

      act(() => result.current.setTheme('dark'))
      expect(meta.getAttribute('content')).toBe('#0c0c10')

      act(() => result.current.setTheme('light'))
      expect(meta.getAttribute('content')).toBe('#ffffff')
    })
  })

  // ── 2. 持久化 ──────────────────────────────────────────────────────────

  describe('持久化', () => {
    it('setTheme 将选择写入 localStorage', () => {
      setupMatchMedia(false)
      const { result } = renderHook(() => useTheme())

      act(() => result.current.setTheme('dark'))
      expect(localStorage.getItem(THEME_KEY)).toBe('dark')

      act(() => result.current.setTheme('light'))
      expect(localStorage.getItem(THEME_KEY)).toBe('light')

      act(() => result.current.setTheme('system'))
      expect(localStorage.getItem(THEME_KEY)).toBe('system')
    })

    it('初始渲染时从 localStorage 恢复已保存的主题', () => {
      setupMatchMedia(false)
      localStorage.setItem(THEME_KEY, 'dark')

      const { result } = renderHook(() => useTheme())
      expect(result.current.theme).toBe('dark')
      expect(result.current.resolvedTheme).toBe('dark')
    })

    it('localStorage 无值时回退为 system', () => {
      setupMatchMedia(true)

      const { result } = renderHook(() => useTheme())
      expect(result.current.theme).toBe('system')
      expect(result.current.resolvedTheme).toBe('dark')
    })

    it('localStorage 存储无效值时仍回退为 system', () => {
      setupMatchMedia(false)
      localStorage.setItem(THEME_KEY, 'invalid-value')

      const { result } = renderHook(() => useTheme())
      // 修复后：无效值应回退为 system
      expect(result.current.theme).toBe('system')
    })
  })

  // ── 3. 系统主题检测 ──────────────────────────────────────────────────────

  describe('系统主题检测', () => {
    it('theme 为 system 时 resolvedTheme 跟随系统', () => {
      const { emitChange } = setupMatchMedia(true)
      const { result } = renderHook(() => useTheme())

      expect(result.current.resolvedTheme).toBe('dark')

      act(() => emitChange(false))
      expect(result.current.resolvedTheme).toBe('light')

      act(() => emitChange(true))
      expect(result.current.resolvedTheme).toBe('dark')
    })

    it('theme 非 system 时不监听系统变化', () => {
      const { mql, emitChange } = setupMatchMedia(false)
      const { result } = renderHook(() => useTheme())

      act(() => result.current.setTheme('light'))
      // mql.addEventListener 只在 theme === 'system' 时调用
      // setTheme('light') 后 effect 清理了 listener
      expect(mql.removeEventListener).toHaveBeenCalled()

      // 系统切换不应影响 resolvedTheme
      act(() => emitChange(true))
      expect(result.current.resolvedTheme).toBe('light')
    })

    it('cleanup 时正确移除 matchMedia listener', () => {
      const { mql } = setupMatchMedia(true)
      const { unmount } = renderHook(() => useTheme())

      unmount()
      expect(mql.removeEventListener).toHaveBeenCalledTimes(1)
    })

    it('从 system 切到固定主题后停止监听系统变化', () => {
      const { mql, emitChange } = setupMatchMedia(true)
      const { result } = renderHook(() => useTheme())

      expect(result.current.resolvedTheme).toBe('dark')

      // 切到固定主题
      act(() => result.current.setTheme('light'))
      expect(mql.removeEventListener).toHaveBeenCalled()

      // 系统变化不再影响
      act(() => emitChange(true))
      expect(result.current.resolvedTheme).toBe('light')
    })

    it('从固定主题切回 system 重新开始监听', () => {
      const { mql, emitChange } = setupMatchMedia(false)
      const { result } = renderHook(() => useTheme())

      act(() => result.current.setTheme('dark'))
      expect(result.current.resolvedTheme).toBe('dark')

      // 切回 system
      act(() => result.current.setTheme('system'))
      expect(result.current.resolvedTheme).toBe('light') // 系统当前 light

      // 系统切 dark 应再次生效
      act(() => emitChange(true))
      expect(result.current.resolvedTheme).toBe('dark')
      expect(mql.addEventListener).toHaveBeenCalled()
    })
  })
})
