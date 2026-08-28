import { useMemo, useCallback, useRef } from 'react'
import { useSearchParams } from 'react-router-dom'
import { useHlsPlayer } from '../../../hooks/useHlsPlayer'
import { mediaUrl } from '../../../api/client'
import { usePlayerSession } from './usePlayerSession'
import { usePlayerPreload } from './usePlayerPreload'
import { useVideoData } from './useVideoData'
import { useVideoEvents } from './useVideoEvents'
import { usePlayerControls } from './usePlayerControls'
import { usePlayerEffects } from './usePlayerEffects'
import { usePlayerMetrics } from './usePlayerMetrics'

export function usePlayerState(
  videoRef: React.RefObject<HTMLVideoElement | null>,
  playerRef: React.RefObject<HTMLDivElement | null>,
) {
  const [searchParams] = useSearchParams()
  const videoId = searchParams.get('id') || ''
  const shareToken = (() => {
    const hash = window.location.hash
    const match = hash.match(/[#&]share=([^&]+)/)
    return match ? match[1] : null
  })()
  const isShared = !!shareToken

  const restoreRef = useRef(0)

  // ── Controls (playback, UI, menus, progress) ──
  const metrics = usePlayerMetrics(videoRef, videoId, isShared)
  const controls = usePlayerControls(videoRef, playerRef, metrics)
  const {
    showLoading, setShowLoading,
    videoError, setVideoError,
    paused, setPaused,
    duration, setDuration,
    speed, setSpeed,
    showSpeedMenu, setShowSpeedMenu,
    controlsVisible, setControlsVisible,
    shortcutText,
    currentQuality, setCurrentQuality,
    showQualityMenu, setShowQualityMenu,
    lastVolumeRef, lastSaveTimeRef,
    hideTimerRef, shortcutTimerRef,
    pendingSeekRef, saveProgressRef,
    throttledMouseMoveRef, throttledVolumeChangeRef,
    showShortcut, showControls, resetHideTimer,
    togglePlay, setSpeedValue, setVolumeValue, toggleMute,
    toggleFullscreen, togglePiP, seekBy,
    switchQuality: switchQualityRaw, retryLoad: retryLoadRaw,
    onMouseMove, onVolumeChange,
    saveProgress: saveProgressRaw, debouncedSaveProgress: debouncedSaveProgressRaw,
    saveProgressKeepalive: saveProgressKeepaliveRaw,
    makeThrottledTimeUpdate,
    playerWrapClassName, playerTopClassName, loadingClassName,
  } = controls

  const saveProgress = useCallback(() => { saveProgressRaw(videoId, isShared) }, [saveProgressRaw, videoId, isShared])
  const debouncedSaveProgress = useCallback(() => { debouncedSaveProgressRaw(videoId, isShared) }, [debouncedSaveProgressRaw, videoId, isShared])
  const saveProgressKeepalive = useCallback(() => { saveProgressKeepaliveRaw(videoId, isShared) }, [saveProgressKeepaliveRaw, videoId, isShared])

  // ── Session (heartbeat, tracking) ──
  const { stopSession, startSession, heartbeatTimerRef } = usePlayerSession(videoId, isShared)

  // ── Video data (fetch, related, variants) ──
  const {
    video, loading, error, setError,
    related, variants, setVariants, hlsUrl,
  } = useVideoData(videoId, isShared, shareToken ?? null, startSession)
  const { preloadingNext, checkPreload, resetPreload, cleanupPreload } = usePlayerPreload(videoId, related, videoRef)

  // ── HLS player ──
  useHlsPlayer({
    videoRef,
    src: hlsUrl || (video?.stream ? mediaUrl(video.stream) : null),
    autoPlay: false,
  })

  const switchQuality = useCallback((quality: string) => {
    switchQualityRaw(quality, video, variants)
  }, [switchQualityRaw, video, variants])

  const retryLoad = useCallback(() => {
    retryLoadRaw(video, variants)
  }, [retryLoadRaw, video, variants])

  // ── Video events ──
  const videoEvents = useVideoEvents(
    videoRef, videoId, isShared, video, related,
    startSession, stopSession, heartbeatTimerRef,
    resetHideTimer, showControls,
    checkPreload, debouncedSaveProgress,
    lastSaveTimeRef, restoreRef, pendingSeekRef,
    setPaused, setVideoError, setShowLoading, setDuration, setSpeed,
    metrics,
  )

  const throttledTimeUpdate = useMemo(
    () => makeThrottledTimeUpdate(videoId, isShared, debouncedSaveProgress, checkPreload),
    [makeThrottledTimeUpdate, videoId, isShared, debouncedSaveProgress, checkPreload]
  )

  const onTimeUpdate = useCallback(() => {
    throttledTimeUpdate()
  }, [throttledTimeUpdate])

  // ── Side effects ──
  usePlayerEffects({
    videoId, videoRef,
    setVariants: setVariants as (v: Array<{ resolution: string; filePath: string }>) => void,
    setCurrentQuality, setVideoError, setShowLoading,
    setSpeed, setControlsVisible,
    resetPreload, resetHideTimer,
    saveProgress, saveProgressKeepalive, stopSession, cleanupPreload,
    restoreRef, saveProgressRef,
    throttledMouseMoveRef, throttledVolumeChangeRef,
    shortcutTimerRef, hideTimerRef,
    lastVolumeRef, video,
  })

  // ── Debug snapshot ──
  const getStateSnapshot = useCallback(() => ({
    videoId, isShared, paused, duration, speed,
    showLoading, videoError, controlsVisible,
    currentQuality, preloadingNext,
  }), [videoId, isShared, paused, duration, speed, showLoading, videoError, controlsVisible, currentQuality, preloadingNext])

  // ── Return: grouped by domain ──
  return {
    videoId, shareToken, isShared,
    video, loading, error, setError,
    showLoading, videoError,
    paused, duration, speed,
    showSpeedMenu, setShowSpeedMenu,
    controlsVisible, shortcutText,
    related, variants, currentQuality,
    showQualityMenu, setShowQualityMenu,
    preloadingNext, hideTimerRef,
    playerWrapClassName, playerTopClassName, loadingClassName,
    resetHideTimer,
    togglePlay, toggleFullscreen, toggleMute, togglePiP,
    setSpeedValue, setVolumeValue,
    showShortcut, seekBy,
    switchQuality, retryLoad,
    onTimeUpdate,
    onPlay: videoEvents.onPlay,
    onPause: videoEvents.onPause,
    onLoadedMetadata: videoEvents.onLoadedMetadata,
    onWaiting: videoEvents.onWaiting,
    onCanPlay: videoEvents.onCanPlay,
    onPlaying: videoEvents.onPlaying,
    onError: videoEvents.onError,
    onRateChange: videoEvents.onRateChange,
    onEnded: videoEvents.onEnded,
    onMouseMove, onVolumeChange,
    getStateSnapshot,
  }
}
