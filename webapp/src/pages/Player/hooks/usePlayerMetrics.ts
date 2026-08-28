import { useEffect, useRef, useCallback } from 'react'
import { trackClick } from '../../../utils/track'

export function usePlayerMetrics(
  videoRef: React.RefObject<HTMLVideoElement | null>,
  videoId: string,
  isShared: boolean,
) {
  const loadStartRef = useRef(performance.now())
  const firstFrameRecordedRef = useRef(false)
  const stallStartRef = useRef(0)
  const stallCountRef = useRef(0)
  const totalStallRef = useRef(0)
  const errorCountRef = useRef(0)
  const errorsByTypeRef = useRef<Record<string, number>>({})
  const visibleTimeRef = useRef(0)
  const visibleStartRef = useRef(performance.now())
  const isVisibleRef = useRef(true)
  const gestureCountsRef = useRef<Record<string, number>>({})
  const pendingQualityRef = useRef<{ from: string; to: string; time: number } | null>(null)

  useEffect(() => {
    loadStartRef.current = performance.now()
    firstFrameRecordedRef.current = false
    stallStartRef.current = 0
    stallCountRef.current = 0
    totalStallRef.current = 0
    errorCountRef.current = 0
    errorsByTypeRef.current = {}
    visibleTimeRef.current = 0
    visibleStartRef.current = performance.now()
    isVisibleRef.current = true
    gestureCountsRef.current = {}
    pendingQualityRef.current = null
  }, [videoId])

  useEffect(() => {
    const handleVisibility = () => {
      const now = performance.now()
      if (document.hidden) {
        if (isVisibleRef.current) {
          visibleTimeRef.current += now - visibleStartRef.current
          isVisibleRef.current = false
        }
      } else {
        visibleStartRef.current = now
        isVisibleRef.current = true
      }
    }
    document.addEventListener('visibilitychange', handleVisibility)
    return () => {
      document.removeEventListener('visibilitychange', handleVisibility)
      if (isVisibleRef.current) {
        visibleTimeRef.current += performance.now() - visibleStartRef.current
      }
    }
  }, [])

  useEffect(() => {
    if (isShared) return
    const conn = (navigator as unknown as { connection?: { effectiveType?: string } }).connection
    if (conn?.effectiveType) {
      trackClick('性能_网络', conn.effectiveType)
    }
    const perf = performance as unknown as { memory?: { usedJSHeapSize?: number; jsHeapSizeLimit?: number } }
    if (perf.memory && perf.memory.usedJSHeapSize != null && perf.memory.jsHeapSizeLimit) {
      const usage = perf.memory.usedJSHeapSize / perf.memory.jsHeapSizeLimit
      if (usage > 0.85) {
        trackClick('性能_内存压力', `${Math.round(usage * 100)}%`)
      }
    }
  }, [videoId, isShared])

  const recordFirstFrame = useCallback(() => {
    if (firstFrameRecordedRef.current) return
    firstFrameRecordedRef.current = true
    const ms = Math.round(performance.now() - loadStartRef.current)
    trackClick('性能_启动时间', `${ms}ms`)
  }, [])

  const recordStallStart = useCallback(() => {
    if (stallStartRef.current > 0) return
    stallStartRef.current = performance.now()
  }, [])

  const recordStallEnd = useCallback(() => {
    if (stallStartRef.current <= 0) return
    const ms = Math.round(performance.now() - stallStartRef.current)
    stallStartRef.current = 0
    totalStallRef.current += ms
    stallCountRef.current++
    if (ms > 2000) {
      trackClick('性能_长卡顿', `${ms}ms`)
    }
  }, [])

  const recordError = useCallback((errorType: string) => {
    errorCountRef.current++
    errorsByTypeRef.current[errorType] = (errorsByTypeRef.current[errorType] || 0) + 1
    trackClick('性能_播放错误', errorType)
  }, [])

  const recordGesture = useCallback((gestureType: string) => {
    gestureCountsRef.current[gestureType] = (gestureCountsRef.current[gestureType] || 0) + 1
  }, [])

  const recordCompletion = useCallback(() => {
    const v = videoRef.current
    const pct = v && isFinite(v.duration) && v.duration > 0 ? Math.round((v.currentTime / v.duration) * 100) : 100
    trackClick('性能_完播', `${videoId}:${pct}%`)
  }, [videoRef, videoId])

  const getConnectionInfo = useCallback(() => {
    const conn = (navigator as unknown as { connection?: { effectiveType?: string; downlink?: number } }).connection
    return conn ? { type: conn.effectiveType || 'unknown', downlink: conn.downlink ?? 0 } : { type: 'unknown', downlink: 0 }
  }, [])

  const getMemoryPressure = useCallback(() => {
    const perf = performance as unknown as { memory?: { usedJSHeapSize?: number; jsHeapSizeLimit?: number } }
    if (perf.memory && perf.memory.usedJSHeapSize != null && perf.memory.jsHeapSizeLimit) {
      return perf.memory.usedJSHeapSize / perf.memory.jsHeapSizeLimit > 0.85
    }
    return false
  }, [])

  const recordQualitySwitchStart = useCallback((from: string, to: string) => {
    pendingQualityRef.current = { from, to, time: performance.now() }
  }, [])

  const recordQualitySwitchResult = useCallback((success: boolean) => {
    const pending = pendingQualityRef.current
    if (!pending) return
    const ms = Math.round(performance.now() - pending.time)
    pendingQualityRef.current = null
    trackClick('性能_画质切换', `${pending.from}→${pending.to}:${success ? '成功' : '失败'}(${ms}ms)`)
  }, [])

  return {
    recordFirstFrame,
    recordStallStart,
    recordStallEnd,
    recordError,
    recordGesture,
    recordCompletion,
    getConnectionInfo,
    getMemoryPressure,
    recordQualitySwitchStart,
    recordQualitySwitchResult,
  }
}
