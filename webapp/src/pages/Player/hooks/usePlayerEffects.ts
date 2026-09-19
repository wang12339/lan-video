import { useCallback, useEffect } from 'react'
import { throttle } from '../../../utils/throttle'
import { MOUSE_MOVE_THROTTLE_MS, VOLUME_CHANGE_THROTTLE_MS } from '../constants'
import { useVideoSource } from './useVideoSource'

interface PlayerEffectsParams {
  videoId: string
  videoRef: React.RefObject<HTMLVideoElement | null>
  setVariants: (v: Array<{ resolution: string; filePath: string }>) => void
  setCurrentQuality: (v: string) => void
  setVideoError: (v: string) => void
  setShowLoading: (v: boolean) => void
  setSpeed: (v: number) => void
  setControlsVisible: (v: boolean) => void
  resetPreload: () => void
  resetHideTimer: () => void
  saveProgress: () => void
  saveProgressKeepalive: () => void
  stopSession: () => void
  cleanupPreload: () => void
  restoreRef: React.MutableRefObject<number>
  saveProgressRef: React.MutableRefObject<() => void>
  throttledMouseMoveRef: React.MutableRefObject<((e: React.MouseEvent) => void) | null>
  throttledVolumeChangeRef: React.MutableRefObject<(() => void) | null>
  shortcutTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  hideTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  lastVolumeRef: React.MutableRefObject<number>
  video: { stream?: string | null; thumb?: string | null; progress?: number } | null
  skipNativeSource?: boolean
}

export function usePlayerEffects(params: PlayerEffectsParams) {
  const {
    videoId, videoRef,
    setVariants, setCurrentQuality, setVideoError, setShowLoading,
    setSpeed, setControlsVisible,
    resetPreload, resetHideTimer,
    saveProgress, saveProgressKeepalive, stopSession, cleanupPreload,
    restoreRef, saveProgressRef,
    throttledMouseMoveRef, throttledVolumeChangeRef,
    shortcutTimerRef, hideTimerRef,
    lastVolumeRef, video,
    skipNativeSource,
  } = params

  useVideoSource({
    videoRef, videoId, video,
    setSpeed, setShowLoading, setControlsVisible,
    restoreRef,
    skipNativeSource,
  })

  useEffect(() => {
    setVariants([])
    setCurrentQuality('original')
    setVideoError('')
    resetPreload()
  }, [videoId, resetPreload, setVariants, setCurrentQuality, setVideoError])

  useEffect(() => {
    const handler = () => {
      if (document.hidden && !document.pictureInPictureElement) {
        saveProgress()
        videoRef.current?.pause()
      }
    }
    const onBeforeUnload = () => {
      saveProgressKeepalive()
      stopSession()
    }
    document.addEventListener('visibilitychange', handler)
    window.addEventListener('beforeunload', onBeforeUnload)
    return () => {
      document.removeEventListener('visibilitychange', handler)
      window.removeEventListener('beforeunload', onBeforeUnload)
    }
  }, [saveProgress, saveProgressKeepalive, stopSession, videoRef])

  // 卸载时需读取 ref 的最新值（清 timer / 上报进度）。通过稳定回调间接访问，
  // 既保留“取最新值”的语义，也避免 exhaustive-deps 对 cleanup 内 ref.current 的误报。
  const flushProgressOnUnmount = useCallback(() => {
    try { saveProgressRef.current() } catch { /* noop */ }
  }, [saveProgressRef])

  const clearPlayerTimers = useCallback(() => {
    if (shortcutTimerRef.current) clearTimeout(shortcutTimerRef.current)
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
  }, [shortcutTimerRef, hideTimerRef])

  useEffect(() => {
    return () => {
      flushProgressOnUnmount()
      try { stopSession() } catch { /* noop */ }
      clearPlayerTimers()
      try { cleanupPreload() } catch { /* noop */ }
    }
  }, [stopSession, cleanupPreload, flushProgressOnUnmount, clearPlayerTimers])

  useEffect(() => {
    throttledMouseMoveRef.current = throttle((_e: unknown) => {
      resetHideTimer()
    }, MOUSE_MOVE_THROTTLE_MS) as unknown as (e: React.MouseEvent) => void

    throttledVolumeChangeRef.current = throttle(() => {
      const v = videoRef.current
      if (!v) return
      if (v.volume > 0 && !v.muted) lastVolumeRef.current = v.volume
    }, VOLUME_CHANGE_THROTTLE_MS)

    return () => {
      throttledMouseMoveRef.current = null
      throttledVolumeChangeRef.current = null
    }
  }, [resetHideTimer, videoRef, lastVolumeRef, throttledMouseMoveRef, throttledVolumeChangeRef])
}
