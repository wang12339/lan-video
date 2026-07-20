import { useEffect, useRef } from 'react'

interface TouchHandlers {
  setVolumeValue: (val: number) => void
  showControls: () => void
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
    let startX = 0, startY = 0, startTime = 0

    const onTouchStart = (e: TouchEvent) => {
      const touch = e.touches[0]
      if (!touch) return
      startX = touch.clientX
      startY = touch.clientY
      startTime = Date.now()
    }

    const onTouchEnd = (e: TouchEvent) => {
      const touch = e.changedTouches[0]
      if (!touch) return
      const dx = touch.clientX - startX
      const dy = touch.clientY - startY
      const dt = Date.now() - startTime
      if (dt > 300) return
      const v = videoRef.current
      if (!v) return
      const rect = el.getBoundingClientRect()
      const { setVolumeValue, showControls } = handlersRef.current
      if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy)) {
        v.currentTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + (dx > 0 ? 10 : -10)))
        showControls()
      } else if (Math.abs(dy) > 50 && Math.abs(dy) > Math.abs(dx)) {
        if (startX < rect.width / 2) {
          const newVol = v.volume + (dy > 0 ? -0.1 : 0.1)
          setVolumeValue(Math.max(0, Math.min(1, newVol)))
        }
        showControls()
      }
    }

    el.addEventListener('touchstart', onTouchStart, { passive: true })
    el.addEventListener('touchend', onTouchEnd, { passive: true })
    return () => {
      el.removeEventListener('touchstart', onTouchStart)
      el.removeEventListener('touchend', onTouchEnd)
    }
  }, [playerRef, videoRef])
}
