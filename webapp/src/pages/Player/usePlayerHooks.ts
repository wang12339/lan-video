import { useRef, useCallback, useEffect } from 'react'

// ============================================================
// 预加载管理器
// ============================================================
interface PreloadManager {
  preloadVideo: (videoId: string) => void
  cleanup: () => void
}

export function usePreloadManager(): PreloadManager {
  const preloadedRef = useRef<Set<string>>(new Set())
  const linkElementsRef = useRef<Map<string, HTMLLinkElement>>(new Map())
  
  const preloadVideo = useCallback((videoId: string) => {
    if (preloadedRef.current.has(videoId)) return
    preloadedRef.current.add(videoId)
    const link = document.createElement('link')
    link.rel = 'prefetch'
    link.href = `/api/videos/${videoId}`
    link.as = 'fetch'
    document.head.appendChild(link)
    linkElementsRef.current.set(videoId, link)
  }, [])
  
  const cleanup = useCallback(() => {
    linkElementsRef.current.forEach((link) => {
      if (link.parentNode) link.parentNode.removeChild(link)
    })
    linkElementsRef.current.clear()
    preloadedRef.current.clear()
  }, [])
  
  useEffect(() => cleanup, [cleanup])
  
  return { preloadVideo, cleanup }
}

// ============================================================
// 内存管理器（DEV only）
// ============================================================
interface MemoryManager {
  trackMemory: () => void
  optimizeMemory: () => void
}

export function useMemoryManager(): MemoryManager {
  const memoryCheckIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null)
  
  const trackMemory = useCallback(() => {
    if (import.meta.env.DEV && 'memory' in performance) {
      const memory = (performance as unknown as { memory: { usedJSHeapSize: number; jsHeapSizeLimit: number } }).memory
      console.log('Memory usage:', {
        used: Math.round(memory.usedJSHeapSize / 1024 / 1024) + ' MB',
        limit: Math.round(memory.jsHeapSizeLimit / 1024 / 1024) + ' MB',
        usage: Math.round((memory.usedJSHeapSize / memory.jsHeapSizeLimit) * 100) + '%'
      })
    }
  }, [])
  
  const optimizeMemory = useCallback(() => {
    if ('gc' in window && typeof (window as unknown as { gc: () => void }).gc === 'function') {
      (window as unknown as { gc: () => void }).gc()
    }
    trackMemory()
  }, [trackMemory])
  
  useEffect(() => {
    if (import.meta.env.DEV) {
      memoryCheckIntervalRef.current = setInterval(trackMemory, 30000)
    }
    return () => {
      if (memoryCheckIntervalRef.current) clearInterval(memoryCheckIntervalRef.current)
    }
  }, [trackMemory])
  
  return { trackMemory, optimizeMemory }
}
