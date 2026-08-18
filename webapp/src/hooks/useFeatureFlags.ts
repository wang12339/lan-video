import { useState, useEffect, useCallback } from 'react'

// 功能开关配置
interface FeatureFlags {
  [key: string]: boolean | string | number
}

// 默认功能开关
const DEFAULT_FLAGS: FeatureFlags = {
  // 功能开关
  enableComments: true,
  enableShare: true,
  enablePlaylists: true,
  enableRecommendations: true,
  enableDarkMode: true,
  enablePWA: true,
  enableAnalytics: false,
  enableDevTools: import.meta.env.DEV,
  
  // 实验功能
  enableNewPlayer: false,
  enableNewGallery: false,
  enableNewUpload: false,
  
  // 配置值
  maxUploadSize: 50 * 1024 * 1024 * 1024, // 50GB
  maxCommentsPerVideo: 1000,
  maxPlaylistItems: 500
}

class FeatureFlagManager {
  private static instance: FeatureFlagManager
  private flags: FeatureFlags = { ...DEFAULT_FLAGS }
  private listeners: Set<(flags: FeatureFlags) => void> = new Set()

  private constructor() {
    // 从localStorage加载用户自定义的开关
    this.loadFromStorage()
  }

  static getInstance(): FeatureFlagManager {
    if (!FeatureFlagManager.instance) {
      FeatureFlagManager.instance = new FeatureFlagManager()
    }
    return FeatureFlagManager.instance
  }

  private loadFromStorage() {
    try {
      const saved = localStorage.getItem('featureFlags')
      if (saved) {
        const parsed = JSON.parse(saved) as FeatureFlags
        this.flags = { ...DEFAULT_FLAGS, ...parsed }
      }
    } catch {
      // 忽略解析错误
    }
  }

  private saveToStorage() {
    try {
      localStorage.setItem('featureFlags', JSON.stringify(this.flags))
    } catch {
      // 忽略存储错误
    }
  }

  isEnabled(flag: string): boolean {
    const value = this.flags[flag]
    if (typeof value === 'boolean') return value
    if (typeof value === 'string') return value === 'true'
    return false
  }

  getValue<T>(flag: string, defaultValue: T): T {
    const value = this.flags[flag]
    return value !== undefined ? (value as T) : defaultValue
  }

  setFlag(flag: string, value: boolean | string | number) {
    this.flags[flag] = value
    this.saveToStorage()
    this.notifyListeners()
  }

  resetFlag(flag: string) {
    if (flag in DEFAULT_FLAGS) {
      this.flags[flag] = DEFAULT_FLAGS[flag] ?? false
    } else {
      delete this.flags[flag]
    }
    this.saveToStorage()
    this.notifyListeners()
  }

  resetAll() {
    this.flags = { ...DEFAULT_FLAGS }
    this.saveToStorage()
    this.notifyListeners()
  }

  getAllFlags(): FeatureFlags {
    return { ...this.flags }
  }

  subscribe(listener: (flags: FeatureFlags) => void) {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  private notifyListeners() {
    this.listeners.forEach(listener => listener(this.flags))
  }
}

export const featureFlags = FeatureFlagManager.getInstance()

// React Hook
export function useFeatureFlag(flag: string): boolean {
  const [isEnabled, setIsEnabled] = useState(() => featureFlags.isEnabled(flag))

  useEffect(() => {
    const unsubscribe = featureFlags.subscribe((flags) => {
      const value = flags[flag]
      setIsEnabled(typeof value === 'boolean' ? value : false)
    })
    return () => { unsubscribe() }
  }, [flag])

  return isEnabled
}

export function useFeatureFlags() {
  const [flags, setFlags] = useState(() => featureFlags.getAllFlags())

  useEffect(() => {
    const unsubscribe = featureFlags.subscribe(setFlags)
    return () => { unsubscribe() }
  }, [])

  const isEnabled = useCallback((flag: string) => featureFlags.isEnabled(flag), [])
  const setFlag = useCallback((flag: string, value: boolean | string | number) => featureFlags.setFlag(flag, value), [])
  const resetFlag = useCallback((flag: string) => featureFlags.resetFlag(flag), [])
  const resetAll = useCallback(() => featureFlags.resetAll(), [])

  return { flags, isEnabled, setFlag, resetFlag, resetAll }
}
