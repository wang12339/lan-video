import { useCallback, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { savePlayback } from '../../../api'
import type { MappedVideo } from '../../../api/types'
import { SAVE_THROTTLE_MS } from '../constants'

const MAX_AUTO_RETRIES = 2
const AUTO_RETRY_DELAY_MS = 1500

export type VideoErrorType = 'network' | 'auth' | 'format' | 'unknown'

function classifyVideoError(videoEl: HTMLVideoElement | null): VideoErrorType {
  if (!videoEl) return 'unknown'
  const error = videoEl.error
  if (!error) return 'unknown'
  switch (error.code) {
    case MediaError.MEDIA_ERR_ABORTED: return 'unknown'
    case MediaError.MEDIA_ERR_NETWORK: return 'network'
    case MediaError.MEDIA_ERR_DECODE: return 'format'
    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED: {
      const msg = (error.message || '').toLowerCase()
      if (msg.includes('401') || msg.includes('403') || msg.includes('auth')) return 'auth'
      if (msg.includes('format') || msg.includes('codec')) return 'format'
      return 'network'
    }
    default: return 'unknown'
  }
}

// MediaError.message does not reliably carry the HTTP status (and browsers
// often report a 403/401 on the media source as SRC_NOT_SUPPORTED), so a
// plain "format" classification here misdiagnosed auth/session failures as
// "视频格式不支持". Probe the source directly to get the real status.
async function probeSourceStatus(src: string): Promise<number | null> {
  try {
    const res = await fetch(src, { headers: { Range: 'bytes=0-1' } })
    return res.status
  } catch {
    return null
  }
}

const VIDEO_ERROR_KEYS: Record<VideoErrorType, string> = {
  network: 'errors.videoNetwork',
  auth: 'errors.videoSessionExpired',
  format: 'errors.videoFormatUnsupported',
  unknown: 'errors.videoLoadFailed',
}

export interface UseVideoEventsReturn {
  onTimeUpdate: () => void
  onPlay: () => void
  onPause: () => void
  onLoadedMetadata: () => void
  onWaiting: () => void
  onCanPlay: () => void
  onPlaying: () => void
  onError: () => void
  onRateChange: () => void
  onEnded: () => void
}

export function useVideoEvents(
  videoRef: React.RefObject<HTMLVideoElement | null>,
  videoId: string,
  isShared: boolean,
  _video: MappedVideo | null,
  _related: MappedVideo[],
  startSession: () => void,
  stopSession: () => void,
  heartbeatTimerRef: React.MutableRefObject<ReturnType<typeof setInterval> | null>,
  resetHideTimer: () => void,
  showControls: () => void,
  checkPreload: () => void,
  debouncedSaveProgress: () => void,
  lastSaveTimeRef: React.MutableRefObject<number>,
  restoreRef: React.MutableRefObject<number>,
  pendingSeekRef: React.MutableRefObject<number>,
  setPaused: (v: boolean) => void,
  setVideoError: (v: string) => void,
  setShowLoading: (v: boolean) => void,
  setDuration: (v: number) => void,
  setSpeed: (v: number) => void,
  metrics: {
    recordFirstFrame: () => void
    recordStallStart: () => void
    recordStallEnd: () => void
    recordError: (type: string) => void
    recordCompletion: () => void
  },
): UseVideoEventsReturn {
  const { t } = useTranslation()
  const retryCountRef = useRef(0)
  const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const callbacksRef = useRef({ startSession, stopSession, resetHideTimer, showControls, checkPreload, debouncedSaveProgress, setPaused, setVideoError, setShowLoading, setDuration, setSpeed, t })
  callbacksRef.current = { startSession, stopSession, resetHideTimer, showControls, checkPreload, debouncedSaveProgress, setPaused, setVideoError, setShowLoading, setDuration, setSpeed, t }

  const onTimeUpdate = useCallback(() => {
    const v = videoRef.current
    if (!v || isShared) return
    const now = Date.now()
    if (now - lastSaveTimeRef.current >= SAVE_THROTTLE_MS && videoId && v.duration) {
      lastSaveTimeRef.current = now
      callbacksRef.current.debouncedSaveProgress()
    }
    callbacksRef.current.checkPreload()
  }, [videoRef, videoId, isShared, lastSaveTimeRef])

  const onPlay = useCallback(() => {
    callbacksRef.current.setPaused(false)
    callbacksRef.current.setVideoError('')
    retryCountRef.current = 0
    callbacksRef.current.resetHideTimer()
    callbacksRef.current.startSession()
  }, [])

  const onPause = useCallback(() => {
    callbacksRef.current.setPaused(true)
    callbacksRef.current.showControls()
    if (heartbeatTimerRef.current) {
      clearInterval(heartbeatTimerRef.current)
      heartbeatTimerRef.current = null
    }
  }, [heartbeatTimerRef])

  const onLoadedMetadata = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    callbacksRef.current.setDuration(v.duration)
    if (restoreRef.current > 0 && v.duration > 0) {
      const restoreSec = restoreRef.current / 1000
      if (restoreSec > v.duration * 0.95) v.currentTime = 0
      restoreRef.current = 0
    }
    if (pendingSeekRef.current > 0) {
      v.currentTime = pendingSeekRef.current
      pendingSeekRef.current = 0
    }
  }, [videoRef, restoreRef, pendingSeekRef])

  const onWaiting = useCallback(() => { callbacksRef.current.setShowLoading(true); metrics.recordStallStart() }, [])
  const onCanPlay = useCallback(() => { callbacksRef.current.setShowLoading(false); callbacksRef.current.setVideoError(''); retryCountRef.current = 0; metrics.recordStallEnd() }, [])
  const onPlaying = useCallback(() => { callbacksRef.current.setShowLoading(false); callbacksRef.current.setVideoError(''); retryCountRef.current = 0; metrics.recordFirstFrame(); metrics.recordStallEnd() }, [])
  const onError = useCallback(() => {
    callbacksRef.current.setShowLoading(false)
    const v = videoRef.current
    const quickType = classifyVideoError(v)

    const proceed = (errorType: VideoErrorType) => {
      metrics.recordError(errorType)
      const { t, setVideoError } = callbacksRef.current
      if (errorType === 'auth') {
        setVideoError(t('errors.videoSessionExpired'))
        return
      }
      if (retryCountRef.current < MAX_AUTO_RETRIES) {
        retryCountRef.current++
        retryTimerRef.current = setTimeout(() => {
          const v = videoRef.current
          if (v && v.src) {
            callbacksRef.current.setShowLoading(true)
            v.load()
          }
        }, AUTO_RETRY_DELAY_MS)
      } else {
        setVideoError(
          VIDEO_ERROR_KEYS[errorType]
            ? t(VIDEO_ERROR_KEYS[errorType])
            : t('errors.videoLoadFailed'),
        )
      }
    }

    // For decode/source errors, confirm against the source's real HTTP
    // status before blaming the format — 401/403 are auth/session failures.
    if ((quickType === 'format' || quickType === 'unknown') && v) {
      const src = v.currentSrc || v.src
      if (src) {
        probeSourceStatus(src).then((status) => {
          proceed(status === 401 || status === 403 ? 'auth' : quickType)
        })
        return
      }
    }
    proceed(quickType)
  }, [videoRef, metrics])

  const onRateChange = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    callbacksRef.current.setSpeed(v.playbackRate)
  }, [videoRef])

  const saveFinalProgress = useCallback(() => {
    const v = videoRef.current
    if (v && isFinite(v.currentTime) && isFinite(v.duration) && v.duration) {
      savePlayback(videoId, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {})
    }
  }, [videoRef, videoId])

  const onEnded = useCallback(() => {
    if (!videoId || isShared) return
    saveFinalProgress()
    stopSession()
    metrics.recordCompletion()
  }, [saveFinalProgress, videoId, isShared, stopSession, metrics])

  useEffect(() => {
    return () => {
      if (retryTimerRef.current) clearTimeout(retryTimerRef.current)
    }
  }, [])

  return {
    onTimeUpdate, onPlay, onPause, onLoadedMetadata,
    onWaiting, onCanPlay, onPlaying, onError,
    onRateChange, onEnded,
  }
}
