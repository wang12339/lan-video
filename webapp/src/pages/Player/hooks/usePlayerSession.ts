import { useRef, useCallback } from 'react'
import { startPlaybackSession, heartbeatPlaybackSession, stopPlaybackSession } from '../../../api'
import { cleanupVideoElement } from '../../../utils/videoUtils'
import { trackVideo } from '../../../utils/track'
import { HEARTBEAT_INTERVAL_MS } from '../constants'

type SessionState = 'idle' | 'started' | 'heartbeat' | 'stopped'

const VIDEO_ID_PATTERN = /^[a-zA-Z0-9_-]+$/

function sanitizeVideoId(id: string): string {
  if (!id) return ''
  return VIDEO_ID_PATTERN.test(id) ? id : ''
}

export interface UsePlayerSessionReturn {
  stopSession: () => void
  startSession: () => void
  disconnectVideo: (videoEl: HTMLVideoElement | null) => void
  heartbeatTimerRef: React.MutableRefObject<ReturnType<typeof setInterval> | null>
}

export function usePlayerSession(videoId: string, isShared: boolean): UsePlayerSessionReturn {
  const heartbeatTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const sessionStateRef = useRef<SessionState>('idle')
  const sessionVideoRef = useRef('')
  const pendingStopRef = useRef(false)

  const stopSession = useCallback(() => {
    if (sessionStateRef.current === 'stopped' || sessionStateRef.current === 'idle') return
    pendingStopRef.current = false
    sessionStateRef.current = 'stopped'
    if (heartbeatTimerRef.current) {
      clearInterval(heartbeatTimerRef.current)
      heartbeatTimerRef.current = null
    }
    const safeId = sanitizeVideoId(videoId)
    if (safeId && !isShared) stopPlaybackSession(safeId).catch(() => {})
  }, [videoId, isShared])

  const startSession = useCallback(() => {
    const safeId = sanitizeVideoId(videoId)
    if (!safeId || isShared) return
    if (pendingStopRef.current) return
    if (sessionStateRef.current === 'started' || sessionStateRef.current === 'heartbeat') {
      if (sessionVideoRef.current === safeId) {
        if (heartbeatTimerRef.current) clearInterval(heartbeatTimerRef.current)
        heartbeatTimerRef.current = setInterval(() => {
          heartbeatPlaybackSession(safeId).catch(() => {})
        }, HEARTBEAT_INTERVAL_MS)
        sessionStateRef.current = 'heartbeat'
        return
      }
      stopSession()
    }
    sessionStateRef.current = 'started'
    sessionVideoRef.current = safeId
    startPlaybackSession(safeId).catch(() => {})
    trackVideo('Start playback', safeId)
    if (heartbeatTimerRef.current) clearInterval(heartbeatTimerRef.current)
    heartbeatTimerRef.current = setInterval(() => {
      heartbeatPlaybackSession(safeId).catch(() => {})
    }, HEARTBEAT_INTERVAL_MS)
    sessionStateRef.current = 'heartbeat'
  }, [videoId, isShared, stopSession])

  const disconnectVideo = useCallback((videoEl: HTMLVideoElement | null) => {
    pendingStopRef.current = true
    cleanupVideoElement(videoEl)
    stopSession()
  }, [stopSession])

  return { stopSession, startSession, disconnectVideo, heartbeatTimerRef }
}
