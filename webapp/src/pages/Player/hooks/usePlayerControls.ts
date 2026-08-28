import { useState, useRef, useCallback, useMemo } from 'react'
import { savePlayback } from '../../../api'
import { getToken, BASE, mediaUrl } from '../../../api/client'
import type { MappedVideo, VideoVariant } from '../../../api/types'
import { getPref } from '../../../api/prefs'
import { throttle, debounce } from '../../../utils/throttle'
import { trackClick } from '../../../utils/track'
import {
  PROGRESS_SAVE_DEBOUNCE_MS, SAVE_THROTTLE_MS, TIME_UPDATE_THROTTLE_MS,
  CONTROLS_HIDE_DELAY_MS, SHORTCUT_DISPLAY_MS,
} from '../constants'

const VALID_PLAYBACK_RATES = new Set([0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3])

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

interface PlaybackState {
  paused: boolean
  setPaused: (v: boolean) => void
  duration: number
  setDuration: (v: number) => void
  speed: number
  setSpeed: (v: number) => void
}

interface UIState {
  showLoading: boolean
  setShowLoading: (v: boolean) => void
  videoError: string
  setVideoError: (v: string) => void
  controlsVisible: boolean
  setControlsVisible: (v: boolean) => void
  shortcutText: string
}

interface MenuState {
  showSpeedMenu: boolean
  setShowSpeedMenu: (v: boolean | ((p: boolean) => boolean)) => void
  currentQuality: string
  setCurrentQuality: (v: string) => void
  showQualityMenu: boolean
  setShowQualityMenu: (v: boolean | ((p: boolean) => boolean)) => void
}

export interface UsePlayerControlsReturn extends PlaybackState, UIState, MenuState {
  lastVolumeRef: React.MutableRefObject<number>
  lastSaveTimeRef: React.MutableRefObject<number>
  hideTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  shortcutTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  pendingSeekRef: React.MutableRefObject<number>
  saveProgressRef: React.MutableRefObject<() => void>
  throttledMouseMoveRef: React.MutableRefObject<((e: React.MouseEvent) => void) | null>
  throttledVolumeChangeRef: React.MutableRefObject<(() => void) | null>
  showShortcut: (text: string) => void
  showControls: () => void
  hideControls: () => void
  resetHideTimer: () => void
  togglePlay: () => void
  setSpeedValue: (s: number) => void
  setVolumeValue: (val: number) => void
  toggleMute: () => void
  toggleFullscreen: () => void
  togglePiP: () => void
  seekBy: (delta: number) => void
  switchQuality: (quality: string, video: MappedVideo | null, variants: VideoVariant[]) => void
  retryLoad: (video: MappedVideo | null, variants: VideoVariant[]) => void
  onMouseMove: (e: React.MouseEvent) => void
  onVolumeChange: () => void
  saveProgress: (videoId: string, isShared: boolean) => void
  debouncedSaveProgress: (videoId: string, isShared: boolean) => void
  saveProgressKeepalive: (videoId: string, isShared: boolean) => void
  makeThrottledTimeUpdate: (videoId: string, isShared: boolean, debouncedSaveProgress: () => void, checkPreload: () => void) => () => void
  playerWrapClassName: string
  playerTopClassName: string
  loadingClassName: string
}

function clampVolume(val: number): number {
  return Math.max(0, Math.min(1, isNaN(val) ? 0.8 : val))
}

function normalizeSpeed(s: number): number {
  if (VALID_PLAYBACK_RATES.has(s)) return s
  let closest = 1
  let minDiff = Infinity
  for (const v of VALID_PLAYBACK_RATES) {
    const diff = Math.abs(s - v)
    if (diff < minDiff) { closest = v; minDiff = diff }
  }
  return closest
}

export function usePlayerControls(
  videoRef: React.RefObject<HTMLVideoElement | null>,
  playerRef: React.RefObject<HTMLDivElement | null>,
  metrics?: {
    recordQualitySwitchStart: (from: string, to: string) => void
    recordQualitySwitchResult: (success: boolean) => void
  },
): UsePlayerControlsReturn {
  const [showLoading, setShowLoading] = useState(true)
  const [videoError, setVideoError] = useState('')
  const [paused, setPaused] = useState(true)
  const [duration, setDuration] = useState(0)
  const [speed, setSpeed] = useState(1)
  const [showSpeedMenu, setShowSpeedMenu] = useState(false)
  const [controlsVisible, setControlsVisible] = useState(true)
  const [shortcutText, setShortcutText] = useState('')
  const [currentQuality, setCurrentQuality] = useState<string>('original')
  const [showQualityMenu, setShowQualityMenu] = useState(false)

  const lastVolumeRef = useRef(clampVolume(0.8))
  const lastSaveTimeRef = useRef(0)
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const shortcutTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pendingSeekRef = useRef(0)
  const saveProgressRef = useRef<() => void>(() => {})
  const throttledMouseMoveRef = useRef<((e: React.MouseEvent) => void) | null>(null)
  const throttledVolumeChangeRef = useRef<(() => void) | null>(null)
  const lastVolumeTrackRef = useRef(0)

  const showShortcut = useCallback((text: string) => {
    setShortcutText(text)
    if (shortcutTimerRef.current) clearTimeout(shortcutTimerRef.current)
    shortcutTimerRef.current = setTimeout(() => setShortcutText(''), SHORTCUT_DISPLAY_MS)
  }, [])

  const showControls = useCallback(() => {
    setControlsVisible(true)
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
  }, [])

  const hideControls = useCallback(() => {
    if (videoRef.current && !videoRef.current.paused) setControlsVisible(false)
  }, [videoRef])

  const resetHideTimer = useCallback(() => {
    showControls()
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
    hideTimerRef.current = setTimeout(hideControls, CONTROLS_HIDE_DELAY_MS)
  }, [showControls, hideControls])

  const togglePlay = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    if (v.paused) { v.play().catch(() => {}); trackClick('播放') }
    else { v.pause(); trackClick('暂停') }
  }, [videoRef])

  const setSpeedValue = useCallback((s: number) => {
    const v = videoRef.current
    if (!v) return
    const old = v.playbackRate
    const valid = normalizeSpeed(s)
    try {
      v.playbackRate = valid
    } catch { /* playback rate not supported */ }
    setSpeed(valid)
    trackClick('倍速', `${old}x→${valid}x`)
    if (getPref('speedMem')) {
      try { localStorage.setItem('atmos_speed_video', String(valid)) } catch { /* noop */ }
    }
  }, [videoRef])

  const setVolumeValue = useCallback((val: number) => {
    const v = videoRef.current
    if (!v) return
    const clamped = clampVolume(val)
    v.volume = clamped
    v.muted = clamped === 0
    if (clamped > 0) lastVolumeRef.current = clamped
    const now = Date.now()
    if (now - lastVolumeTrackRef.current > 2000) {
      lastVolumeTrackRef.current = now
      trackClick('音量', `${Math.round(clamped * 100)}%`)
    }
  }, [videoRef])

  const toggleMute = useCallback(() => { const v = videoRef.current; if (!v) return; setVolumeValue(v.volume > 0 ? 0 : lastVolumeRef.current) }, [videoRef, setVolumeValue])

  const toggleFullscreen = useCallback(() => {
    const el = playerRef.current
    if (!el) return
    try {
      if (!document.fullscreenElement) { el.requestFullscreen().catch(() => {}); trackClick('全屏', '进入') }
      else { document.exitFullscreen().catch(() => {}); trackClick('全屏', '退出') }
    } catch { /* fullscreen API not available */ }
  }, [playerRef])

  const togglePiP = useCallback(async () => {
    const v = videoRef.current
    if (!v) return
    try {
      if (document.pictureInPictureElement) { await document.exitPictureInPicture(); trackClick('画中画', '退出') }
      else if (v.src) { await v.requestPictureInPicture(); trackClick('画中画', '进入') }
    } catch { /* PiP not supported */ }
  }, [videoRef])

  const seekBy = useCallback((delta: number) => {
    const v = videoRef.current
    if (!v) return
    try {
      v.currentTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + delta))
      trackClick(delta > 0 ? '快进' : '快退', `${Math.abs(Math.round(delta))}s`)
      resetHideTimer()
    } catch { /* seek failed */ }
  }, [videoRef, resetHideTimer])

  const switchQuality = useCallback((quality: string, video: MappedVideo | null, variants: VideoVariant[]) => {
    const v = videoRef.current
    if (!v) return
    if (quality === currentQuality) { setShowQualityMenu(false); return }
    trackClick('画质', `${currentQuality}→${quality}`)
    metrics?.recordQualitySwitchStart(currentQuality, quality)
    const src = quality === 'original'
      ? (video?.stream || '')
      : mediaUrl(variants.find(variant => variant.resolution === quality)?.filePath || '')
    if (!src || !isValidMediaUrl(src)) { setShowQualityMenu(false); return }
    const wasPlaying = !v.paused
    pendingSeekRef.current = v.currentTime
    v.src = src
    v.load()
    if (wasPlaying) v.play().catch(() => {})
    setShowLoading(true)
    setCurrentQuality(quality)
    setShowQualityMenu(false)
    resetHideTimer()
  }, [videoRef, currentQuality, resetHideTimer])

  const retryLoad = useCallback((video: MappedVideo | null, variants: VideoVariant[]) => {
    const v = videoRef.current
    if (!v || !video?.stream) return
    const src = currentQuality === 'original'
      ? video.stream
      : mediaUrl(variants.find(variant => variant.resolution === currentQuality)?.filePath || '')
    if (!src || !isValidMediaUrl(src)) return
    setVideoError('')
    setShowLoading(true)
    v.src = src
    v.load()
    v.play().catch(() => {})
  }, [videoRef, currentQuality])

  const onMouseMove = useCallback((e: React.MouseEvent) => { if (throttledMouseMoveRef.current) throttledMouseMoveRef.current(e) }, [])
  const onVolumeChange = useCallback(() => { if (throttledVolumeChangeRef.current) throttledVolumeChangeRef.current() }, [])

  const getPlaybackPayload = useCallback((videoId: string, isShared: boolean) => {
    if (!videoId || isShared) return null
    const v = videoRef.current
    if (!v || !isFinite(v.currentTime) || !isFinite(v.duration) || !v.duration) return null
    return { video_id: videoId, position_ms: Math.max(0, Math.floor(v.currentTime * 1000)), duration_ms: Math.max(0, Math.floor(v.duration * 1000)) }
  }, [videoRef])

  const saveProgress = useCallback((videoId: string, isShared: boolean) => {
    const payload = getPlaybackPayload(videoId, isShared)
    if (!payload) return
    savePlayback(videoId, payload.position_ms, payload.duration_ms).catch(() => {})
  }, [getPlaybackPayload])

  const debouncedSaveProgress = useMemo(() => debounce((videoId: string, isShared: boolean) => saveProgress(videoId, isShared), PROGRESS_SAVE_DEBOUNCE_MS), [saveProgress])

  const saveProgressKeepalive = useCallback((videoId: string, isShared: boolean) => {
    const payload = getPlaybackPayload(videoId, isShared)
    if (!payload) return
    const headers: Record<string, string> = { 'Content-Type': 'application/json', 'X-Requested-With': 'XMLHttpRequest' }
    const token = getToken()
    if (token) headers['Authorization'] = 'Bearer ' + token
    try { fetch(BASE + '/playback/history', { method: 'POST', headers, body: JSON.stringify(payload), keepalive: true, credentials: 'same-origin' }).catch(() => {}) } catch { /* ignore */ }
  }, [getPlaybackPayload])

  const classNames = useMemo(() => ({
    playerWrap: `player-wrap ${controlsVisible ? '' : 'controls-hidden'}`,
    playerTop: `player-top ${controlsVisible ? 'show' : ''}`,
    loading: `player-loading ${showLoading ? 'show' : ''}`,
  }), [controlsVisible, showLoading])
  const playerWrapClassName = classNames.playerWrap
  const playerTopClassName = classNames.playerTop
  const loadingClassName = classNames.loading

  const makeThrottledTimeUpdate = useCallback((videoId: string, isShared: boolean, debouncedSaveProgress: () => void, checkPreload: () => void) => {
    return throttle(() => {
      const v = videoRef.current
      if (!v || isShared) return
      const now = Date.now()
      if (now - lastSaveTimeRef.current >= SAVE_THROTTLE_MS && videoId && v.duration) {
        lastSaveTimeRef.current = now
        debouncedSaveProgress()
      }
      checkPreload()
    }, TIME_UPDATE_THROTTLE_MS)
  }, [videoRef])

  return {
    showLoading, setShowLoading, videoError, setVideoError,
    paused, setPaused, duration, setDuration, speed, setSpeed,
    showSpeedMenu, setShowSpeedMenu, controlsVisible, setControlsVisible,
    shortcutText, currentQuality, setCurrentQuality, showQualityMenu, setShowQualityMenu,
    lastVolumeRef, lastSaveTimeRef, hideTimerRef, shortcutTimerRef,
    pendingSeekRef, saveProgressRef, throttledMouseMoveRef, throttledVolumeChangeRef,
    showShortcut, showControls, hideControls, resetHideTimer,
    togglePlay, setSpeedValue, setVolumeValue, toggleMute,
    toggleFullscreen, togglePiP, seekBy,
    switchQuality, retryLoad, onMouseMove, onVolumeChange,
    saveProgress, debouncedSaveProgress, saveProgressKeepalive,
    makeThrottledTimeUpdate,
    playerWrapClassName, playerTopClassName, loadingClassName,
  }
}
