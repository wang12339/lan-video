import { useState, useEffect, useRef, useCallback, memo } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams, useNavigate } from 'react-router-dom'
import {
  getVideo, mapVideo, listVideos, incrementViews,
  savePlayback, getCatColor, deleteVideo,
  startPlaybackSession, heartbeatPlaybackSession, stopPlaybackSession,
  getSimilarVideos, createShareLink, getShareVideo, APIError,
} from '../../api'
import { request, mediaUrl, getToken, BASE } from '../../api/client'
import type { MappedVideo, VideoVariant } from '../../api/types'
import { getPref } from '../../api/prefs'
import { useAuth } from '../../context/AuthContext'
import { trackVideo } from '../../utils/track'
import { useHlsPlayer } from '../../hooks/useHlsPlayer'
import VideoCard from '../../components/VideoCard/VideoCard'
import Comments from '../../components/Comments/Comments'
import { usePlayerShortcuts } from './usePlayerShortcuts'
import { usePlayerTouch } from './usePlayerTouch'
import PlayerControls from './PlayerControls'
import { ConfirmDialog, AlertDialog } from '../../components/ui'
import './Player.css'

// 播放期间的 4Hz 重渲染已被隔离进 PlayerControls；Comments 再包一层 memo，
// 兜底父级偶发重渲染（暂停/菜单切换等）不波及评论区子树
const MemoComments = memo(Comments)

const SAVE_THROTTLE_MS = 10000
const MIN_PROGRESS_SAVE_S = 5
const HEARTBEAT_INTERVAL_MS = 45000

export default function Player() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const { t } = useTranslation()
  const { user } = useAuth()
  const videoId = searchParams.get('id') || ''
  const shareToken = (() => {
    const hash = window.location.hash
    const match = hash.match(/[#&]share=([^&]+)/)
    return match ? match[1] : null
  })()
  const isShared = !!shareToken

  const videoRef = useRef<HTMLVideoElement>(null)
  const playerRef = useRef<HTMLDivElement>(null)

  const [video, setVideo] = useState<MappedVideo | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [showLoading, setShowLoading] = useState(true)
  const [videoError, setVideoError] = useState('')

  const [paused, setPaused] = useState(true)
  const [duration, setDuration] = useState(0)
  const [speed, setSpeed] = useState(1)
  const [showSpeedMenu, setShowSpeedMenu] = useState(false)
  const [controlsVisible, setControlsVisible] = useState(true)
  const [shortcutText, setShortcutText] = useState('')
  const [related, setRelated] = useState<MappedVideo[]>([])

  // Quality/variant state
  const [variants, setVariants] = useState<VideoVariant[]>([])
  const [currentQuality, setCurrentQuality] = useState<string>('original')
  const [showQualityMenu, setShowQualityMenu] = useState(false)

  // Share state
  const [showShareTooltip, setShowShareTooltip] = useState(false)
  const [shareTooltipMsg, setShareTooltipMsg] = useState('')

  // Dialog state
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [deleteAlertMsg, setDeleteAlertMsg] = useState('')

  const lastVolumeRef = useRef(0.8)
  const lastSaveTimeRef = useRef(0)
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const shortcutTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const heartbeatTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  // 每个视频只启动一次播放会话（play 事件可能因暂停/恢复重复触发）
  const sessionStartedRef = useRef(false)
  const sessionVideoRef = useRef('')
  // 切清晰度后待恢复的播放位置：metadata 就绪前设置 currentTime 会被浏览器忽略
  const pendingSeekRef = useRef(0)
  // 上次观看位置（毫秒），用于"接近片尾则从头播"判断
  const restoreRef = useRef(0)
  const saveProgressRef = useRef<() => void>(() => {})
  const [hlsUrl, setHlsUrl] = useState<string | null>(null)

  // HLS播放器支持
  useHlsPlayer({
    videoRef,
    src: hlsUrl || (video?.stream ? mediaUrl(video.stream) : null),
    autoPlay: false,
  })

  const showShortcut = useCallback((text: string) => {
    setShortcutText(text)
    if (shortcutTimerRef.current) clearTimeout(shortcutTimerRef.current)
    shortcutTimerRef.current = setTimeout(() => setShortcutText(''), 700)
  }, [])

  const showControls = useCallback(() => {
    setControlsVisible(true)
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
  }, [])

  const hideControls = useCallback(() => {
    if (videoRef.current && !videoRef.current.paused) {
      setControlsVisible(false)
    }
  }, [])

  const resetHideTimer = useCallback(() => {
    showControls()
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
    hideTimerRef.current = setTimeout(hideControls, 3500)
  }, [showControls, hideControls])

  const togglePlay = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    if (v.paused) v.play().catch(() => {})
    else v.pause()
  }, [])

  // 停止播放会话：清心跳 + 通知后端（幂等，可安全重复调用）
  const stopSession = useCallback(() => {
    if (heartbeatTimerRef.current) {
      clearInterval(heartbeatTimerRef.current)
      heartbeatTimerRef.current = null
    }
    if (videoId && !isShared) stopPlaybackSession(videoId).catch(() => {})
  }, [videoId, isShared])

  // 断开视频连接：清空 src 停止浏览器预加载 + 停止播放会话
  const disconnectVideo = useCallback(() => {
    const v = videoRef.current
    if (v) {
      v.pause()
      v.removeAttribute('src')
      v.load()
    }
    stopSession()
  }, [stopSession])

  // 开始播放会话 + 心跳；每个视频只 start 一次，暂停后恢复只重启心跳
  const startSession = useCallback(() => {
    if (!videoId || isShared) return
    if (!sessionStartedRef.current || sessionVideoRef.current !== videoId) {
      sessionStartedRef.current = true
      sessionVideoRef.current = videoId
      startPlaybackSession(videoId).catch(() => {})
      trackVideo('开始播放', videoId)
    }
    if (heartbeatTimerRef.current) clearInterval(heartbeatTimerRef.current)
    heartbeatTimerRef.current = setInterval(() => {
      heartbeatPlaybackSession(videoId).catch(() => {})
    }, HEARTBEAT_INTERVAL_MS)
  }, [videoId, isShared])

  const setSpeedValue = useCallback((s: number) => {
    const v = videoRef.current
    if (!v) return
    v.playbackRate = s
    setSpeed(s)
    if (getPref('speedMem') && videoId) {
      try { localStorage.setItem('atmos_speed_' + videoId, String(s)) } catch { /* noop */ }
    }
  }, [videoId])

  const setVolumeValue = useCallback((val: number) => {
    const v = videoRef.current
    if (!v) return
    v.volume = val
    v.muted = val === 0
    if (val > 0) lastVolumeRef.current = val
  }, [])

  const toggleMute = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    setVolumeValue(v.volume > 0 ? 0 : lastVolumeRef.current)
  }, [setVolumeValue])

  const toggleFullscreen = useCallback(() => {
    const el = playerRef.current
    if (!el) return
    if (!document.fullscreenElement) el.requestFullscreen().catch(() => {})
    else document.exitFullscreen()
  }, [])

  const togglePiP = useCallback(async () => {
    const v = videoRef.current
    if (!v) return
    try {
      if (document.pictureInPictureElement) await document.exitPictureInPicture()
      else if (v.src) await v.requestPictureInPicture()
    } catch { /* PiP not supported */ }
  }, [])

  const saveProgress = useCallback(() => {
    if (!videoId || isShared) return
    const v = videoRef.current
    if (!v || !isFinite(v.currentTime) || !isFinite(v.duration) || !v.duration) return
    savePlayback(videoId, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {})
  }, [videoId, isShared])

  saveProgressRef.current = saveProgress

  // beforeunload 专用上报：页面卸载时普通 fetch 会被浏览器取消导致进度丢失
  // （seek 后马上关闭页面的场景），keepalive 请求可存活到页面销毁；
  // 播放进度接口需要 Bearer token（sendBeacon 无法携带自定义头，故不用）
  const saveProgressKeepalive = useCallback(() => {
    if (!videoId || isShared) return
    const v = videoRef.current
    if (!v || !isFinite(v.currentTime) || !isFinite(v.duration) || !v.duration) return
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'X-Requested-With': 'XMLHttpRequest',
    }
    const token = getToken()
    if (token) headers['Authorization'] = 'Bearer ' + token
    const body = JSON.stringify({
      video_id: videoId,
      position_ms: Math.max(0, Math.floor(v.currentTime * 1000)),
      duration_ms: Math.max(0, Math.floor(v.duration * 1000)),
    })
    try {
      fetch(BASE + '/playback/history', {
        method: 'POST',
        headers,
        body,
        keepalive: true,
        credentials: 'same-origin',
      }).catch(() => {})
    } catch { /* ignore */ }
  }, [videoId, isShared])

  const seekBy = useCallback((delta: number) => {
    const v = videoRef.current
    if (!v) return
    v.currentTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + delta))
    resetHideTimer()
  }, [resetHideTimer])

  // Keyboard shortcuts (extracted to hook)
  usePlayerShortcuts(videoRef, {
    togglePlay, toggleFullscreen, toggleMute, togglePiP,
    setVolumeValue, setSpeedValue, showShortcut, resetHideTimer,
    t,
  })

  // Touch gestures (extracted to hook)
  usePlayerTouch(playerRef, videoRef, { setVolumeValue, showControls })

  // Save on visibility change — pause, don't destroy
  useEffect(() => {
    const handler = () => {
      if (document.hidden && !document.pictureInPictureElement) {
        saveProgress()
        videoRef.current?.pause()
      }
    }
    const onBeforeUnload = () => {
      saveProgressKeepalive()
      disconnectVideo()
    }
    document.addEventListener('visibilitychange', handler)
    window.addEventListener('beforeunload', onBeforeUnload)
    return () => {
      document.removeEventListener('visibilitychange', handler)
      window.removeEventListener('beforeunload', onBeforeUnload)
    }
  }, [saveProgress, saveProgressKeepalive, disconnectVideo])

  // Cleanup on unmount / 切换到另一个视频：上报进度、停止会话、清理定时器
  useEffect(() => {
    return () => {
      saveProgressRef.current()
      stopSession()
      if (shortcutTimerRef.current) clearTimeout(shortcutTimerRef.current)
      if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
    }
  }, [stopSession])

  // 切换视频时重置清晰度/错误状态
  useEffect(() => {
    setVariants([])
    setCurrentQuality('original')
    setVideoError('')
  }, [videoId])

  // Load video
  useEffect(() => {
    let cancelled = false

    if (isShared && shareToken) {
      const load = async () => {
        setLoading(true)
        try {
          const sv = await getShareVideo(shareToken)
          if (cancelled) return
          const mv: MappedVideo = {
            id: sv.id,
            title: sv.title,
            category: sv.category,
            description: sv.description || '',
            thumb: sv.thumbUrl ? mediaUrl(sv.thumbUrl) : null,
            stream: mediaUrl(sv.streamUrl),
            cover: null,
            sourceType: sv.sourceType,
            duration: 0,
            views: 0,
            date: '',
            progress: 0,
          }
          setVideo(mv)
          document.title = mv.title + ' · ATMOS'
        } catch (e) {
          if (cancelled) return
          if (e instanceof APIError && (e.status === 404 || e.status === 410)) {
            setError(t('player.shareInvalid'))
          } else {
            setError(e instanceof Error && e.message ? e.message : t('player.shareInvalid'))
          }
        } finally {
          if (!cancelled) setLoading(false)
        }
      }
      load()
      return () => { cancelled = true }
    }

    if (!videoId) { setError(t('errors.missingVideoId')); setLoading(false); return }

    const load = async () => {
      setLoading(true)
      try {
        // 立即启动播放会话，这样视频加载后就能立即拖动进度条
        startSession()

        const v = await getVideo(videoId)
        const mv = mapVideo(v)
        if (cancelled || !mv) return
        setVideo(mv)
        const cleanTitle = mv.title.replace(/\.[^.]+$/, '').replace(/_/g, ' ').replace(/\s+/g, ' ').trim() || mv.title
        document.title = cleanTitle + ' · ATMOS'
        incrementViews(videoId).catch(() => {})
        loadRelated(mv.category)

        // 检查是否有 HLS 流可用
        try {
          const hlsRes = await request<{ status: string; masterUrl?: string }>(`/videos/${videoId}/hls`)
          if (!cancelled && hlsRes.status === 'ready' && hlsRes.masterUrl) {
            setHlsUrl(hlsRes.masterUrl)
          }
        } catch {
          // HLS not available, use direct video
        }

        // 通过公开的播放变体接口加载清晰度（admin 的 transcode/status 接口尚未实现，返回空列表）
        if (v.hasVariants) {
          try {
            const res = await request<Array<{ resolution: string; url: string; fileSize?: number; bitrate?: number }>>(`/videos/${videoId}/variants`)
            if (!cancelled) {
              setVariants(res.map(r => ({
                resolution: r.resolution,
                filePath: r.url,
                fileSize: r.fileSize ?? 0,
                bitrate: r.bitrate,
              })))
            }
          } catch {
            // Ignore variant loading errors
          }
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof Error && e.message ? e.message : t('errors.loadFailed'))
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    load()
    return () => { cancelled = true }
  }, [videoId, isShared, shareToken])

  const loadRelated = useCallback(async (category?: string) => {
    if (isShared) return
    try {
      const recommended = await getSimilarVideos(videoId)
      const items = recommended.filter(v => v.id !== videoId).slice(0, 8)
      if (items.length > 0) {
        setRelated(items)
        return
      }
    } catch { /* fall back to category filter */ }
    try {
      const r = await listVideos({ type: 'local_video', size: 20, category })
      const items = r.items.map(mapVideo).filter((v): v is MappedVideo => !!v && v.id !== videoId).slice(0, 8)
      setRelated(items)
    } catch { /* ignore */ }
  }, [videoId, isShared])

  // Quality switching
  const switchQuality = useCallback((quality: string) => {
    const v = videoRef.current
    if (!v) return
    if (quality === currentQuality) { setShowQualityMenu(false); return }

    const src = quality === 'original'
      ? (video?.stream || '')
      : mediaUrl(variants.find(variant => variant.resolution === quality)?.filePath || '')
    if (!src) { setShowQualityMenu(false); return }

    const wasPlaying = !v.paused
    // 元数据就绪前设置 currentTime 会被浏览器忽略，记下待恢复位置在 loadedmetadata 里重放
    pendingSeekRef.current = v.currentTime

    v.src = src
    v.load()
    if (wasPlaying) {
      v.play().catch(() => {})
    }

    setShowLoading(true)
    setCurrentQuality(quality)
    setShowQualityMenu(false)
    resetHideTimer()
  }, [video, variants, currentQuality, resetHideTimer])

  // Set video source and restore position
  useEffect(() => {
    const v = videoRef.current
    if (!v || !video?.stream) return

    // 启动播放会话（必须在加载视频源之前）
    // 使用 async/await 确保会话启动后再加载视频
    const initVideo = async () => {
      try {
        await startPlaybackSession(videoId)
      } catch {
        // 忽略错误，继续加载视频
      }

      v.src = video!.stream!
      v.poster = video!.thumb || ''
      v.load()

      restoreRef.current = 0
      if (video!.progress && video!.progress > MIN_PROGRESS_SAVE_S * 1000) {
        restoreRef.current = video!.progress
        v.currentTime = video!.progress / 1000
      }

      if (getPref('speedMem')) {
        const saved = localStorage.getItem('atmos_speed_' + videoId)
        if (saved) { v.playbackRate = parseFloat(saved); setSpeed(parseFloat(saved)) }
      }

      // 遵循"自动播放"偏好
      if (getPref('autoPlay')) {
        v.play().catch(() => setShowLoading(false))
      } else {
        setShowLoading(false)
        setControlsVisible(true)
      }
    }

    initVideo()
  }, [video, videoId])

  // 视频事件处理器：timeupdate 只做节流上报，不再 setState，
  // 进度条 UI 由 PlayerControls 内部订阅 video 事件驱动（H-1 高频重渲染隔离）
  const onTimeUpdate = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    if (isShared) return
    const now = Date.now()
    if (now - lastSaveTimeRef.current >= SAVE_THROTTLE_MS && videoId && v.duration) {
      lastSaveTimeRef.current = now
      savePlayback(videoId, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {})
    }
  }, [videoId, isShared])

  const onPlay = useCallback(() => {
    setPaused(false)
    setVideoError('')
    resetHideTimer()
    startSession()
  }, [resetHideTimer, startSession])

  const onPause = useCallback(() => {
    setPaused(true)
    showControls()
    // 暂停即停止心跳，避免"没在看但会话一直活跃"
    if (heartbeatTimerRef.current) {
      clearInterval(heartbeatTimerRef.current)
      heartbeatTimerRef.current = null
    }
  }, [showControls])

  const onLoadedMetadata = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    setDuration(v.duration)
    if (restoreRef.current > 0 && v.duration > 0) {
      // 已观看超过 95% 则从头播放，避免一进来就结束/自动跳到下一集
      const restoreSec = restoreRef.current / 1000
      if (restoreSec > v.duration * 0.95) {
        v.currentTime = 0
      }
      restoreRef.current = 0
    }
    if (pendingSeekRef.current > 0) {
      v.currentTime = pendingSeekRef.current
      pendingSeekRef.current = 0
    }
  }, [])

  const onWaiting = useCallback(() => setShowLoading(true), [])
  const onCanPlay = useCallback(() => { setShowLoading(false); setVideoError('') }, [])
  const onPlaying = useCallback(() => { setShowLoading(false); setVideoError('') }, [])
  const onError = useCallback(() => {
    setShowLoading(false)
    setVideoError(t('errors.videoLoadFailed'))
  }, [t])

  const onVolumeChange = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    // UI 状态（音量/静音）由 PlayerControls 内部订阅 volumechange 维护
    if (v.volume > 0 && !v.muted) lastVolumeRef.current = v.volume
  }, [])

  const onRateChange = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    setSpeed(v.playbackRate)
  }, [])

  const onEnded = useCallback(() => {
    // 播完：上报最终进度并结束会话
    saveProgress()
    stopSession()
    if (!getPref('autoPlay')) return
    const next = related[0]
    if (next) {
      navigate(`/player?id=${next.id}`)
    }
  }, [saveProgress, stopSession, related, navigate])

  const retryLoad = useCallback(() => {
    const v = videoRef.current
    if (!v || !video?.stream) return
    const src = currentQuality === 'original'
      ? video.stream
      : mediaUrl(variants.find(variant => variant.resolution === currentQuality)?.filePath || '')
    if (!src) return
    setVideoError('')
    setShowLoading(true)
    v.src = src
    v.load()
    v.play().catch(() => {})
  }, [video, currentQuality, variants])

  const handleDelete = useCallback(() => {
    if (!videoId) return
    setShowDeleteDialog(true)
  }, [videoId])

  const handleDeleteConfirm = useCallback(async () => {
    if (!videoId) return
    try {
      await deleteVideo(videoId)
      navigate('/')
    } catch {
      setDeleteAlertMsg(t('player.deleteError'))
    }
  }, [videoId, navigate])

  const handleShare = async () => {
    if (!user || !video) return
    try {
      const res = await createShareLink(video.id)
      await navigator.clipboard.writeText(res.shareUrl)
      setShareTooltipMsg(t('player.linkCopied'))
      setShowShareTooltip(true)
      setTimeout(() => setShowShareTooltip(false), 2000)
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : ''
      if (e instanceof APIError && e.status === 401) {
        setShareTooltipMsg(t('player.shareExpired'))
      } else if (e instanceof APIError && (e.status === 404 || e.status === 410)) {
        setShareTooltipMsg(t('player.shareOpFailed'))
      } else {
        setShareTooltipMsg(errMsg || t('player.shareFailed'))
      }
      setShowShareTooltip(true)
      setTimeout(() => setShowShareTooltip(false), 3000)
    }
  }

  if (error) {
    return (
      <div className="player-error">
        <span className="player-error-icon">⚠️</span>
        <p>{error}</p>
        <button onClick={() => navigate('/')}>{t('player.backToHome')}</button>
      </div>
    )
  }

  return (
    <div className="player-page">
      <div
        className={`player-wrap ${controlsVisible ? '' : 'controls-hidden'}`}
        ref={playerRef}
        onMouseMove={resetHideTimer}
        onDoubleClick={toggleFullscreen}
        onTouchStart={resetHideTimer}
        onMouseLeave={() => { if (hideTimerRef.current) clearTimeout(hideTimerRef.current); hideTimerRef.current = setTimeout(hideControls, 1000) }}
      >
        <video
          ref={videoRef}
          className="player-video"
          playsInline
          preload="auto"
          aria-label={video?.title || undefined}
          onTimeUpdate={onTimeUpdate}
          onPlay={onPlay}
          onPause={onPause}
          onLoadedMetadata={onLoadedMetadata}
          onWaiting={onWaiting}
          onCanPlay={onCanPlay}
          onPlaying={onPlaying}
          onEnded={onEnded}
          onError={onError}
          onVolumeChange={onVolumeChange}
          onRateChange={onRateChange}
          onClick={togglePlay}
        />

        {/* Top bar */}
        <header className={`player-top ${controlsVisible ? 'show' : ''}`}>
          <button className="player-back" onClick={() => navigate('/')} aria-label={t('player.backToHome')}>←</button>
          <span className="player-title">{video?.title || ''}</span>
          {user?.isAdmin && <button className="player-delete" onClick={handleDelete} title={t('player.delete')} aria-label={t('player.delete')}>🗑</button>}
        </header>

        {/* Loading spinner */}
        <div className={`player-loading ${showLoading ? 'show' : ''}`}>
          <div className="loading-ring" />
          <span>{t('common.loading')}</span>
        </div>

        {/* Video load error */}
        {videoError && (
          <div className="player-center" role="alert">
            <p style={{ color: '#fff', fontSize: 14, marginBottom: 12, textAlign: 'center' }}>{videoError}</p>
            <button className="center-play" onClick={retryLoad} aria-label={t('common.retry')}>↻</button>
          </div>
        )}

        {/* Center play button */}
        {!showLoading && paused && !videoError && (
          <div className="player-center">
            <button className="center-play" onClick={togglePlay}>▶</button>
          </div>
        )}

        {/* Shortcut indicator */}
        {shortcutText && (
          <div className="shortcut-indicator">{shortcutText}</div>
        )}

        {/* Controls */}
        <PlayerControls
            videoRef={videoRef}
            controlsVisible={controlsVisible}
            paused={paused}
            duration={duration}
            speed={speed}
            showQualityMenu={showQualityMenu}
            showSpeedMenu={showSpeedMenu}
            currentQuality={currentQuality}
            variants={variants}
            togglePlay={togglePlay}
            toggleMute={toggleMute}
            toggleFullscreen={toggleFullscreen}
            togglePiP={togglePiP}
            setSpeedValue={setSpeedValue}
            setVolumeValue={setVolumeValue}
            switchQuality={switchQuality}
            seekBy={seekBy}
            resetHideTimer={resetHideTimer}
            setShowQualityMenu={setShowQualityMenu}
            setShowSpeedMenu={setShowSpeedMenu}
            t={t}
          />
      </div>

      {/* Video info */}
      {video && (
        <div className="player-detail">
          <div className="pd-meta">
            <span style={{ background: getCatColor(video.category) + '1a', color: getCatColor(video.category) }}>{video.category || t('common.other')}</span>
            {user && (
              <button className="pd-share-btn" onClick={handleShare}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7c.05-.23.09-.46.09-.7s-.04-.47-.09-.7l7.05-4.11c.54.5 1.25.81 2.04.81 1.66 0 3-1.34 3-3s-1.34-3-3-3-3 1.34-3 3c0 .24.04.47.09.7L8.04 9.81C7.5 9.31 6.79 9 6 9c-1.66 0-3 1.34-3 3s1.34 3 3 3c.79 0 1.5-.31 2.04-.81l7.12 4.16c-.05.21-.08.43-.08.65 0 1.61 1.31 2.92 2.92 2.92 1.61 0 2.92-1.31 2.92-2.92s-1.31-2.92-2.92-2.92z"/></svg>
                {t('player.share')}
              </button>
            )}
            {showShareTooltip && <span className="pd-share-tooltip">{shareTooltipMsg}</span>}
          </div>
          <h1 className="pd-title">
            {video.title.replace(/\.[^.]+$/, '').replace(/_/g, ' ').replace(/\s+/g, ' ').trim() || video.title}
          </h1>
          {video.description && <p className="pd-desc">{video.description}</p>}
        </div>
      )}

      {/* Comments */}
      {videoId && <MemoComments videoId={videoId} />}

      {/* Related videos */}
      {related.length > 0 && (
        <section className="player-related">
          <h2 className="prs-title">{t('player.related')}</h2>
          <div className="prs-grid">
            {related.map((v) => (
              <VideoCard key={v.id} video={v} compact />
            ))}
          </div>
        </section>
      )}

      {deleteAlertMsg && (
        <AlertDialog
          open={!!deleteAlertMsg}
          message={deleteAlertMsg}
          onClose={() => setDeleteAlertMsg('')}
        />
      )}
      <ConfirmDialog
        open={showDeleteDialog}
        title={t('player.deleteConfirm')}
        message={t('player.deleteConfirmMessage')}
        danger
        onConfirm={handleDeleteConfirm}
        onCancel={() => setShowDeleteDialog(false)}
      />
      {loading && <div className="player-loading-text">{t('common.loading')}</div>}
    </div>
  )
}
