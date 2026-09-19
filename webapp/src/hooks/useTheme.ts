import { useCallback, useSyncExternalStore } from 'react'

type Theme = 'dark' | 'light' | 'system'
type ResolvedTheme = 'dark' | 'light'

interface ThemeState {
  theme: Theme
  resolvedTheme: ResolvedTheme
}

const THEME_KEY = 'atmos.theme'

const META_COLORS: Record<ResolvedTheme, string> = {
  dark: '#0c0c10',
  light: '#ffffff',
}

function getSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined') return 'dark'
  try {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  } catch {
    return 'dark'
  }
}

function isValidTheme(v: string | null): v is Theme {
  return v === 'dark' || v === 'light' || v === 'system'
}

function readStoredTheme(): Theme {
  if (typeof window === 'undefined') return 'system'
  try {
    const raw = localStorage.getItem(THEME_KEY)
    return isValidTheme(raw) ? raw : 'system'
  } catch {
    return 'system'
  }
}

function resolveTheme(theme: Theme): ResolvedTheme {
  return theme === 'system' ? getSystemTheme() : theme
}

function applyDomTheme(resolved: ResolvedTheme): void {
  if (typeof document === 'undefined') return
  document.documentElement.dataset.theme = resolved
  const meta = document.querySelector('meta[name="theme-color"]')
  if (meta) meta.setAttribute('content', META_COLORS[resolved])
}

// ── 模块级外部 store：所有 useTheme 共享同一份状态，并通过 storage 事件跨标签同步 ──

const initialTheme = readStoredTheme()
let state: ThemeState = {
  theme: initialTheme,
  resolvedTheme: resolveTheme(initialTheme),
}
const listeners = new Set<() => void>()

let mediaQueryList: MediaQueryList | null = null
let mediaQueryHandler: ((event: MediaQueryListEvent) => void) | null = null
let storageListenerAttached = false

function emit(): void {
  listeners.forEach((listener) => listener())
}

function commit(next: ThemeState): void {
  state = next
  applyDomTheme(next.resolvedTheme)
  emit()
}

function attachSystemListener(): void {
  if (mediaQueryHandler || typeof window === 'undefined') return
  try {
    const mql = window.matchMedia('(prefers-color-scheme: dark)')
    const handler = (event: MediaQueryListEvent) => {
      if (state.theme !== 'system') return
      const resolvedTheme = event.matches ? 'dark' : 'light'
      if (resolvedTheme === state.resolvedTheme) return
      commit({ theme: 'system', resolvedTheme })
    }
    mql.addEventListener('change', handler)
    mediaQueryList = mql
    mediaQueryHandler = handler
  } catch {
    mediaQueryList = null
    mediaQueryHandler = null
  }
}

function detachSystemListener(): void {
  if (!mediaQueryList || !mediaQueryHandler) return
  try {
    mediaQueryList.removeEventListener('change', mediaQueryHandler)
  } catch {
    /* ignore */
  }
  mediaQueryList = null
  mediaQueryHandler = null
}

function syncSystemListener(): void {
  if (state.theme === 'system' && listeners.size > 0) attachSystemListener()
  else detachSystemListener()
}

function handleStorageEvent(event: StorageEvent): void {
  if (event.key !== null && event.key !== THEME_KEY) return
  const stored = readStoredTheme()
  if (stored === state.theme) return
  commit({ theme: stored, resolvedTheme: resolveTheme(stored) })
  syncSystemListener()
}

function attachStorageListener(): void {
  if (storageListenerAttached || typeof window === 'undefined') return
  window.addEventListener('storage', handleStorageEvent)
  storageListenerAttached = true
}

function detachStorageListener(): void {
  if (!storageListenerAttached || typeof window === 'undefined') return
  window.removeEventListener('storage', handleStorageEvent)
  storageListenerAttached = false
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  if (listeners.size === 1) {
    // 首个订阅者挂载时与 localStorage 对齐（也是初始值恢复的兜底）
    const stored = readStoredTheme()
    const resolvedTheme = resolveTheme(stored)
    if (stored !== state.theme || resolvedTheme !== state.resolvedTheme) {
      state = { theme: stored, resolvedTheme }
    }
    applyDomTheme(resolvedTheme)
    attachStorageListener()
  }
  syncSystemListener()
  return () => {
    listeners.delete(listener)
    if (listeners.size === 0) {
      detachSystemListener()
      detachStorageListener()
    }
  }
}

function getSnapshot(): ThemeState {
  return state
}

const SERVER_STATE: ThemeState = { theme: 'system', resolvedTheme: 'dark' }

function getServerSnapshot(): ThemeState {
  return SERVER_STATE
}

function setTheme(newTheme: Theme): void {
  try {
    localStorage.setItem(THEME_KEY, newTheme)
  } catch {
    void 0
  }
  const resolvedTheme = resolveTheme(newTheme)
  if (newTheme !== state.theme || resolvedTheme !== state.resolvedTheme) {
    commit({ theme: newTheme, resolvedTheme })
  }
  syncSystemListener()
}

export function useTheme() {
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot)

  const toggleTheme = useCallback(() => {
    setTheme(state.resolvedTheme === 'dark' ? 'light' : 'dark')
  }, [])

  return {
    theme: snapshot.theme,
    resolvedTheme: snapshot.resolvedTheme,
    setTheme,
    toggleTheme,
  }
}
