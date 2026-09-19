import { useEffect, useCallback, useRef } from 'react'
import { getPref } from '../../../api/prefs'
import { normalizeSpeed } from './usePlayerControls'
import { MIN_PROGRESS_SAVE_S } from '../constants'

export interface UseVideoSourceParams {
  videoRef: React.RefObject<HTMLVideoElement | null>
  videoId: string
  video: { stream?: string | null; thumb?: string | null; progress?: number } | null
  setSpeed: (v: number) => void
  setShowLoading: (v: boolean) => void
  setControlsVisible: (v: boolean) => void
  restoreRef: React.MutableRefObject<number>
  // true 时只应用 poster/进度/倍速等非源逻辑，不触碰原生 v.src/v.load()，
  // 源由 HLS hook 统一设置，避免 MSE 与原生源互相覆盖。
  skipNativeSource?: boolean
}

export interface UseVideoSourceReturn {
  applySource: () => void
  applySpeed: (speed: number) => void
}

function shouldRestoreProgress(progress: number): boolean {
  return !!progress && progress > MIN_PROGRESS_SAVE_S * 1000
}

function handleRestorePosition(v: HTMLVideoElement, progress: number): void {
  v.currentTime = progress / 1000
}

export function useVideoSource({
  videoRef, videoId, video,
  setSpeed, setShowLoading, setControlsVisible,
  restoreRef,
  skipNativeSource = false,
}: UseVideoSourceParams): UseVideoSourceReturn {
  // skipNativeSource 不放进 applySource 依赖：画质切到 variant 时它会 true→false，
  // 若因此重跑 applySource，会把 HLS hook 刚设好的 variant 源覆盖回默认源。
  // 用 ref 读取最新值，applySource 仅在 video/videoId 变化时重跑。
  const skipNativeSourceRef = useRef(skipNativeSource)
  skipNativeSourceRef.current = skipNativeSource

  const applySource = useCallback(() => {
    const v = videoRef.current
    if (!v || !video) return
    if (!skipNativeSourceRef.current) {
      if (!video.stream) return
      v.src = video.stream
      v.load()
    }
    v.poster = video.thumb || ''
    restoreRef.current = 0

    if (shouldRestoreProgress(video.progress || 0)) {
      restoreRef.current = video.progress!
      handleRestorePosition(v, video.progress!)
    }

    if (getPref('speedMem')) {
      const saved = localStorage.getItem('atmos_speed_' + videoId)
      if (saved) {
        const parsed = parseFloat(saved)
        // 非法值（NaN/超出范围）不赋值：直接写 playbackRate 会抛错导致白屏
        if (Number.isFinite(parsed)) {
          const valid = normalizeSpeed(parsed)
          v.playbackRate = valid
          setSpeed(valid)
        }
      }
    }

    if (getPref('autoPlay')) {
      v.play().catch(() => setShowLoading(false))
    } else {
      setShowLoading(false)
      setControlsVisible(true)
    }
  }, [video, videoId, videoRef, setSpeed, setShowLoading, setControlsVisible, restoreRef])

  const applySpeed = useCallback((speed: number) => {
    const v = videoRef.current
    if (!v) return
    v.playbackRate = speed
    setSpeed(speed)
  }, [videoRef, setSpeed])

  useEffect(() => {
    applySource()
  }, [applySource])

  return { applySource, applySpeed }
}
