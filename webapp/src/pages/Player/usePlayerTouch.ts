import { useEffect, useRef } from 'react'

interface TouchHandlers {
  setVolumeValue: (val: number) => void
  showControls: () => void
  togglePlay?: () => void
  setPlaybackRate?: (rate: number) => void
}

export function usePlayerTouch(
  playerRef: React.RefObject<HTMLDivElement | null>,
  videoRef: React.RefObject<HTMLVideoElement | null>,
  handlers: TouchHandlers,
) {
  const handlersRef = useRef(handlers)
  handlersRef.current = handlers

  useEffect(() => {
    const el = playerRef.current
    if (!el) return

    let startX = 0
    let startY = 0
    let startTime = 0
    let lastTapTime = 0
    let longPressTimer: ReturnType<typeof setTimeout> | null = null
    let isLongPress = false
    let originalPlaybackRate = 1

    const clearLongPress = () => {
      if (longPressTimer) {
        clearTimeout(longPressTimer)
        longPressTimer = null
      }
    }

    const onTouchStart = (e: TouchEvent) => {
      const touch = e.touches[0]
      if (!touch) return
      startX = touch.clientX
      startY = touch.clientY
      startTime = Date.now()
      isLongPress = false

      // 长按检测：500ms 后触发倍速
      clearLongPress()
      longPressTimer = setTimeout(() => {
        isLongPress = true
        const v = videoRef.current
        if (v && handlersRef.current.setPlaybackRate) {
          originalPlaybackRate = v.playbackRate
          handlersRef.current.setPlaybackRate(3) // 长按 3 倍速
        }
        handlersRef.current.showControls()
      }, 500)
    }

    const onTouchEnd = (e: TouchEvent) => {
      const touch = e.changedTouches[0]
      if (!touch) return

      clearLongPress()

      // 如果是长按，恢复原播放速率
      if (isLongPress) {
        isLongPress = false
        if (handlersRef.current.setPlaybackRate) {
          handlersRef.current.setPlaybackRate(originalPlaybackRate)
        }
        return
      }

      const dx = touch.clientX - startX
      const dy = touch.clientY - startY
      const dt = Date.now() - startTime
      const v = videoRef.current
      if (!v) return

      const now = Date.now()

      // 双击检测：300ms 内连续点击
      if (dt < 200 && Math.abs(dx) < 20 && Math.abs(dy) < 20) {
        if (now - lastTapTime < 300) {
          // 双击暂停/播放
          if (handlersRef.current.togglePlay) {
            handlersRef.current.togglePlay()
          }
          lastTapTime = 0
          return
        }
        lastTapTime = now
      }

      // 快速滑动才识别为手势（<300ms）
      if (dt > 300) return

      const rect = el.getBoundingClientRect()
      const { setVolumeValue, showControls } = handlersRef.current

      // 水平滑动：快进/快退
      if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy)) {
        const seekTime = dx > 0 ? 10 : -10
        v.currentTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + seekTime))
        showControls()
      }
      // 垂直滑动：音量调节（左侧区域）
      else if (Math.abs(dy) > 50 && Math.abs(dy) > Math.abs(dx)) {
        if (startX < rect.width / 2) {
          const volumeChange = dy > 0 ? -0.1 : 0.1
          const newVol = v.volume + volumeChange
          setVolumeValue(Math.max(0, Math.min(1, newVol)))
        }
        showControls()
      }
    }

    const onTouchMove = (e: TouchEvent) => {
      // 如果移动距离过大，取消长按
      if (longPressTimer) {
        const touch = e.touches[0]
        if (touch) {
          const dx = Math.abs(touch.clientX - startX)
          const dy = Math.abs(touch.clientY - startY)
          if (dx > 20 || dy > 20) {
            clearLongPress()
          }
        }
      }
    }

    el.addEventListener('touchstart', onTouchStart, { passive: true })
    el.addEventListener('touchend', onTouchEnd, { passive: true })
    el.addEventListener('touchmove', onTouchMove, { passive: true })

    return () => {
      clearLongPress()
      el.removeEventListener('touchstart', onTouchStart)
      el.removeEventListener('touchend', onTouchEnd)
      el.removeEventListener('touchmove', onTouchMove)
    }
  }, [playerRef, videoRef])
}
