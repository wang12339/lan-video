import { useMemo, useCallback, useRef, useState, useEffect } from 'react'
import { useSearchParams } from 'react-router-dom'
import { useHlsPlayer } from '../../../hooks/useHlsPlayer'
import { mediaUrl } from '../../../api/client'
import { trackClick } from '../../../utils/track'
import { usePlayerSession } from './usePlayerSession'
import { usePlayerPreload } from './usePlayerPreload'
import { useVideoData } from './useVideoData'
import { useVideoEvents } from './useVideoEvents'
import { usePlayerControls } from './usePlayerControls'
import { usePlayerEffects } from './usePlayerEffects'
import { usePlayerMetrics } from './usePlayerMetrics'

function isValidMediaUrl(url: string): boolean {
  if (!url) return false
  try {
    const parsed = new URL(url, window.location.origin)
    const protocol = parsed.protocol
    return protocol === 'http:' || protocol === 'https:' || protocol === 'blob:'
  } catch {
    return false
  }
}

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
  const controls = usePlayerControls(videoRef, playerRef, videoId, metrics)
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
    retryLoad: retryLoadRaw,
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
  // 画质覆盖源：null 表示用默认源（HLS 优先，其次原生 stream）。
  const [qualityOverrideSrc, setQualityOverrideSrc] = useState<string | null>(null)
  const [hlsReloadKey, setHlsReloadKey] = useState(0)
  const pendingPlayRef = useRef(false)
  const baseSrc = hlsUrl || (video?.stream ? mediaUrl(video.stream) : null)
  const activeSrc = qualityOverrideSrc ?? baseSrc
  // `?? {}` 兜底 hook 未返回对象的情况（如测试替身），destroyHls 用可选调用保护。
  const { destroy: destroyHls } = useHlsPlayer({
    videoRef,
    src: activeSrc,
    autoPlay: false,
    reloadKey: hlsReloadKey,
  }) ?? {}

  useEffect(() => {
    setQualityOverrideSrc(null)
  }, [videoId])

  // 源的设置统一交给 useHlsPlayer（HLS 分支 attachMedia，非 HLS 分支设 v.src）；
  // 本 hook 只负责状态与进度交接，避免与 MSE 互相覆盖。
  const switchQuality = useCallback((quality: string) => {
    if (quality === currentQuality) { setShowQualityMenu(false); return }
    const v = videoRef.current
    if (!v) return
    trackClick('画质', `${currentQuality}→${quality}`)
    metrics.recordQualitySwitchStart(currentQuality, quality)
    const src = quality === 'original'
      ? (video?.stream || '')
      : (mediaUrl(variants.find(variant => variant.resolution === quality)?.filePath || '') || '')
    if (!src || !isValidMediaUrl(src)) { setShowQualityMenu(false); return }
    pendingPlayRef.current = !v.paused
    pendingSeekRef.current = v.currentTime
    setQualityOverrideSrc(quality === 'original' ? null : src)
    setCurrentQuality(quality)
    setShowLoading(true)
    setShowQualityMenu(false)
  }, [videoRef, currentQuality, video, variants, metrics, pendingSeekRef, setCurrentQuality, setShowLoading, setShowQualityMenu])

  const retryLoad = useCallback(() => {
    const isHls = !!baseSrc && (/\.m3u8(\?|$)/.test(baseSrc) || baseSrc.includes('/hls/'))
    if (isHls) {
      const v = videoRef.current
      pendingPlayRef.current = !!v && !v.paused
      setShowLoading(true)
      setHlsReloadKey(k => k + 1)
      return
    }
    destroyHls?.()
    retryLoadRaw(video, variants)
  }, [baseSrc, videoRef, destroyHls, retryLoadRaw, video, variants, setShowLoading])

  // 只有默认 HLS 源需要跳过原生设源；切到 variant 后由 useHlsPlayer 负责。
  const skipNativeSource = !!hlsUrl && qualityOverrideSrc === null

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

  const { onLoadedMetadata: handleLoadedMetadata } = videoEvents

  const onLoadedMetadata = useCallback(() => {
    handleLoadedMetadata()
    if (pendingPlayRef.current) {
      pendingPlayRef.current = false
      videoRef.current?.play().catch(() => {})
    }
  }, [handleLoadedMetadata, videoRef])

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
    skipNativeSource,
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
    onLoadedMetadata,
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
