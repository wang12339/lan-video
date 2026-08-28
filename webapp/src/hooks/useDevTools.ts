import { useState, useEffect, useCallback } from 'react'

const isDev = import.meta.env.DEV

interface PerformanceMetric {
  name: string
  value: number
  unit: string
  timestamp: number
}

interface DebugInfo {
  userAgent: string
  screenSize: string
  devicePixelRatio: number
  connection: string
  memory: string
  language: string
  cookiesEnabled: boolean
  localStorageEnabled: boolean
}

export function useDevTools() {
  const [showPanel, setShowPanel] = useState(false)
  const [metrics, setMetrics] = useState<PerformanceMetric[]>([])
  const [debugInfo, setDebugInfo] = useState<DebugInfo | null>(null)

  // 收集调试信息
  const collectDebugInfo = useCallback(() => {
    const connection = (navigator as unknown as { connection?: { effectiveType?: string } }).connection
    
    setDebugInfo({
      userAgent: navigator.userAgent,
      screenSize: `${window.innerWidth}x${window.innerHeight}`,
      devicePixelRatio: window.devicePixelRatio,
      connection: connection?.effectiveType || 'unknown',
      memory: (performance as unknown as { memory?: { usedJSHeapSize?: number } }).memory 
        ? `${Math.round(((performance as unknown as { memory: { usedJSHeapSize: number } }).memory.usedJSHeapSize / 1024 / 1024))}MB`
        : 'N/A',
      language: navigator.language,
      cookiesEnabled: navigator.cookieEnabled,
      localStorageEnabled: (() => {
        try {
          localStorage.setItem('test', 'test')
          localStorage.removeItem('test')
          return true
        } catch {
          return false
        }
      })()
    })
  }, [])

  // 收集性能指标
  const collectMetrics = useCallback(() => {
    const newMetrics: PerformanceMetric[] = []
    
    // 内存使用
    const memory = (performance as unknown as { memory?: { usedJSHeapSize?: number; totalJSHeapSize?: number } }).memory
    if (memory) {
      newMetrics.push({
        name: 'JS堆内存',
        value: Math.round(memory.usedJSHeapSize! / 1024 / 1024),
        unit: 'MB',
        timestamp: Date.now()
      })
    }
    
    // DOM节点数
    newMetrics.push({
      name: 'DOM节点数',
      value: document.querySelectorAll('*').length,
      unit: '个',
      timestamp: Date.now()
    })
    
    // 事件监听器数（近似）
    const listeners = document.querySelectorAll('[onclick], [onchange], [onsubmit]').length
    newMetrics.push({
      name: '事件监听器',
      value: listeners,
      unit: '个',
      timestamp: Date.now()
    })
    
    setMetrics(newMetrics)
  }, [])

  // 定期更新
  useEffect(() => {
    if (!isDev || !showPanel) return
    
    collectDebugInfo()
    collectMetrics()
    
    const interval = setInterval(collectMetrics, 2000)
    return () => clearInterval(interval)
  }, [showPanel, collectDebugInfo, collectMetrics])

  // 快捷键切换面板
  useEffect(() => {
    if (!isDev) return
    
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl + Shift + D 切换调试面板
      if (e.ctrlKey && e.shiftKey && e.key === 'D') {
        e.preventDefault()
        setShowPanel(prev => !prev)
      }
    }
    
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  return {
    isDev,
    showPanel,
    setShowPanel,
    metrics,
    debugInfo,
    refreshMetrics: collectMetrics,
    refreshDebugInfo: collectDebugInfo
  }
}
