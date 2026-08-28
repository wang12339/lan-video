import { useCallback, useEffect, useRef, useState } from 'react'
import {
  DOUBLE_TAP_DELAY_MS, LONG_PRESS_DELAY_MS, SWIPE_THRESHOLD_PX,
  SWIPE_VOLUME_STEP, LONG_PRESS_SPEED, VELOCITY_SWIPE_THRESHOLD,
} from '../constants'
import { trackClick } from '../../../utils/track'

export type GestureIndicatorType = 'seek-forward' | 'seek-backward' | 'volume-up' | 'volume-down' | 'brightness-up' | 'brightness-down' | 'speed' | null

interface UsePlayerGesturesOptions {
  videoRef: React.RefObject<HTMLVideoElement | null>
  speed: number
  setSpeedValue: (s: number) => void
  setVolumeValue: (val: number) => void
  setCurrentTime: (time: number) => void
  resetHideTimer: () => void
  togglePlay: () => void
}

export interface UsePlayerGesturesReturn {
  gestureIndicator: GestureIndicatorType
  gestureValue: number
  isLongPressing: boolean
  gestureAreaRef: React.RefObject<HTMLDivElement>
  handleTouchStart: (e: React.TouchEvent) => void
  handleTouchMove: (e: React.TouchEvent) => void
  handleTouchEnd: (e: React.TouchEvent) => void
  handleMouseDown: (e: React.MouseEvent) => void
  handleMouseUp: () => void
}

export function usePlayerGestures({
  videoRef, speed, setSpeedValue, setVolumeValue,
  setCurrentTime, resetHideTimer, togglePlay,
}: UsePlayerGesturesOptions): UsePlayerGesturesReturn {
  const [gestureIndicator, setGestureIndicator] = useState<GestureIndicatorType>(null)
  const [gestureValue, setGestureValue] = useState(0)
  const [isLongPressing, setIsLongPressing] = useState(false)
  const [originalSpeed, setOriginalSpeed] = useState(speed)
  const gestureAreaRef = useRef<HTMLDivElement>(null)
  const lastTapRef = useRef(0)
  const tapCountRef = useRef(0)
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const touchStartRef = useRef<{ x: number; y: number; time: number } | null>(null)
  const isSwipingRef = useRef(false)
  const gestureTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const showGestureIndicator = useCallback((type: GestureIndicatorType, value: number) => {
    if (gestureTimeoutRef.current) clearTimeout(gestureTimeoutRef.current)
    setGestureIndicator(type)
    setGestureValue(value)
    gestureTimeoutRef.current = setTimeout(() => { setGestureIndicator(null); setGestureValue(0) }, 700)
  }, [])

  const clearLongPressTimer = useCallback(() => {
    if (longPressTimerRef.current) { clearTimeout(longPressTimerRef.current); longPressTimerRef.current = null }
  }, [])

  const handleDoubleTap = useCallback((side: 'left' | 'right') => {
    const v = videoRef.current
    if (!v || !isFinite(v.currentTime)) return
    try {
      const delta = side === 'right' ? 10 : -10
      const newTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + delta))
      if (!isFinite(newTime)) return
      v.currentTime = newTime
      setCurrentTime(newTime)
      trackClick('手势_双击', side === 'right' ? '快进10s' : '快退10s')
      showGestureIndicator(side === 'right' ? 'seek-forward' : 'seek-backward', 10)
      resetHideTimer()
    } catch { /* seek failed */ }
  }, [videoRef, resetHideTimer, showGestureIndicator, setCurrentTime])

  const handleLongPressStart = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    try {
    setOriginalSpeed(speed)
    setSpeedValue(LONG_PRESS_SPEED)
    setIsLongPressing(true)
    trackClick('手势_长按', `${speed}x→${LONG_PRESS_SPEED}x`)
    showGestureIndicator('speed', LONG_PRESS_SPEED)
    } catch { /* speed change failed */ }
  }, [videoRef, speed, setSpeedValue, showGestureIndicator])

  const handleLongPressEnd = useCallback(() => {
    if (isLongPressing) { setSpeedValue(originalSpeed); setIsLongPressing(false); clearLongPressTimer() }
  }, [isLongPressing, originalSpeed, setSpeedValue, clearLongPressTimer])

  const handleSwipe = useCallback((direction: 'up' | 'down' | 'left' | 'right', magnitude: number, velocity: number) => {
    const v = videoRef.current
    if (!v) return
    try {
      const safeMagnitude = Math.max(0, Math.min(10000, isFinite(magnitude) ? magnitude : 0))
      const safeVelocity = Math.max(0, Math.min(100, isFinite(velocity) ? velocity : 0))
      const velocityBoost = Math.max(1, Math.min(3, safeVelocity / 0.5))
      const steps = Math.max(1, Math.floor((safeMagnitude * velocityBoost) / SWIPE_THRESHOLD_PX))
    if (direction === 'up' || direction === 'down') {
      const delta = direction === 'up' ? SWIPE_VOLUME_STEP * steps : -SWIPE_VOLUME_STEP * steps
      const newVolume = Math.max(0, Math.min(1, v.volume + delta))
      setVolumeValue(newVolume)
      trackClick('手势_滑动', direction === 'up' ? '音量+' : '音量-')
      showGestureIndicator(direction === 'up' ? 'volume-up' : 'volume-down', Math.round(newVolume * 100))
    } else {
      const seekAmount = 5 * steps
      const delta = direction === 'right' ? seekAmount : -seekAmount
      const newTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + delta))
      v.currentTime = newTime
      setCurrentTime(newTime)
      trackClick('手势_滑动', direction === 'right' ? `快进${seekAmount}s` : `快退${seekAmount}s`)
      showGestureIndicator(direction === 'right' ? 'seek-forward' : 'seek-backward', seekAmount)
    }
      resetHideTimer()
    } catch { /* gesture failed */ }
  }, [videoRef, setVolumeValue, resetHideTimer, showGestureIndicator, setCurrentTime])

  const handleTouchStart = useCallback((e: React.TouchEvent) => {
    if ((e.target as HTMLElement).closest('.progress-wrap')) return
    const touch = e.touches[0]
    if (!touch) return
    touchStartRef.current = { x: touch.clientX, y: touch.clientY, time: Date.now() }
    isSwipingRef.current = false
    longPressTimerRef.current = setTimeout(() => { handleLongPressStart() }, LONG_PRESS_DELAY_MS)
  }, [handleLongPressStart])

  const handleTouchMove = useCallback((e: React.TouchEvent) => {
    if (!touchStartRef.current || (e.target as HTMLElement).closest('.progress-wrap')) return
    const touch = e.touches[0]
    if (!touch) return
    const deltaX = touch.clientX - touchStartRef.current.x
    const deltaY = touch.clientY - touchStartRef.current.y
    if (Math.sqrt(deltaX * deltaX + deltaY * deltaY) > 10) {
      if (longPressTimerRef.current) clearLongPressTimer()
      isSwipingRef.current = true
      tapCountRef.current = 0
    }
  }, [clearLongPressTimer])

  const handleTouchEnd = useCallback((e: React.TouchEvent) => {
    if (!touchStartRef.current || (e.target as HTMLElement).closest('.progress-wrap')) return
    clearLongPressTimer()
    if (isLongPressing) { handleLongPressEnd(); touchStartRef.current = null; return }
    const touch = e.changedTouches[0]
    if (!touch) { touchStartRef.current = null; return }
    const deltaX = touch.clientX - touchStartRef.current.x
    const deltaY = touch.clientY - touchStartRef.current.y
    const deltaTime = Math.max(1, Date.now() - touchStartRef.current.time)
    const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY)
    const velocity = distance / deltaTime
    if (distance > SWIPE_THRESHOLD_PX || velocity > VELOCITY_SWIPE_THRESHOLD) {
      const absX = Math.abs(deltaX); const absY = Math.abs(deltaY)
      handleSwipe(absX > absY ? (deltaX > 0 ? 'right' : 'left') : (deltaY > 0 ? 'down' : 'up'), Math.max(absX, absY), velocity)
    } else if (distance < 10 && deltaTime < 300) {
      const now = Date.now()
      if (now - lastTapRef.current < DOUBLE_TAP_DELAY_MS) {
        tapCountRef.current++
        if (tapCountRef.current >= 2) {
          const playerWidth = gestureAreaRef.current?.clientWidth || window.innerWidth
          handleDoubleTap(touch.clientX < playerWidth / 2 ? 'left' : 'right')
          tapCountRef.current = 0
          lastTapRef.current = 0
        }
      } else {
        tapCountRef.current = 1
        const capturedCount = tapCountRef.current
        setTimeout(() => { if (tapCountRef.current === capturedCount && tapCountRef.current === 1) { togglePlay(); resetHideTimer() } }, DOUBLE_TAP_DELAY_MS)
      }
      lastTapRef.current = now
    }
    touchStartRef.current = null; isSwipingRef.current = false
  }, [isLongPressing, handleLongPressEnd, clearLongPressTimer, handleSwipe, handleDoubleTap, togglePlay, resetHideTimer])

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!(e.target as HTMLElement).closest('.progress-wrap, .controls-row')) {
      longPressTimerRef.current = setTimeout(() => { handleLongPressStart() }, LONG_PRESS_DELAY_MS)
    }
  }, [handleLongPressStart])

  const handleMouseUp = useCallback(() => {
    clearLongPressTimer()
    if (isLongPressing) handleLongPressEnd()
  }, [isLongPressing, handleLongPressEnd, clearLongPressTimer])

  useEffect(() => {
    return () => {
      clearLongPressTimer()
      if (gestureTimeoutRef.current) clearTimeout(gestureTimeoutRef.current)
      setGestureIndicator(null)
      setGestureValue(0)
      setIsLongPressing(false)
    }
  }, [clearLongPressTimer])

  return { gestureIndicator, gestureValue, isLongPressing, gestureAreaRef, handleTouchStart, handleTouchMove, handleTouchEnd, handleMouseDown, handleMouseUp }
}
