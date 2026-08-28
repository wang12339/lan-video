import { useEffect, useCallback } from 'react'
import { getPref } from '../../../api/prefs'
import { MIN_PROGRESS_SAVE_S } from '../constants'

export interface UseVideoSourceParams {
  videoRef: React.RefObject<HTMLVideoElement | null>
  videoId: string
  video: { stream?: string | null; thumb?: string | null; progress?: number } | null
  setSpeed: (v: number) => void
  setShowLoading: (v: boolean) => void
  setControlsVisible: (v: boolean) => void
  restoreRef: React.MutableRefObject<number>
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
}: UseVideoSourceParams): UseVideoSourceReturn {
  const applySource = useCallback(() => {
    const v = videoRef.current
    if (!v || !video?.stream) return
    v.src = video.stream!
    v.poster = video.thumb || ''
    v.load()
    restoreRef.current = 0

    if (shouldRestoreProgress(video.progress || 0)) {
      restoreRef.current = video.progress!
      handleRestorePosition(v, video.progress!)
    }

    if (getPref('speedMem')) {
      const saved = localStorage.getItem('atmos_speed_' + videoId)
      if (saved) {
        const parsed = parseFloat(saved)
        v.playbackRate = parsed
        setSpeed(parsed)
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
