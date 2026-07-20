import { useState, useEffect, useRef, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams, useNavigate } from 'react-router-dom'
import {
  getVideo, mapVideo, listVideos, incrementViews,
  savePlayback, getCatColor, deleteVideo,
  startPlaybackSession, heartbeatPlaybackSession, stopPlaybackSession,
  getTranscodeStatus, getSimilarVideos, createShareLink, getShareVideo, APIError,
} from '../../api'
import type { MappedVideo, VideoVariant } from '../../api/types'
import { getPref } from '../../api/prefs'
import { useAuth } from '../../context/AuthContext'
import { trackVideo } from '../../utils/track'
import VideoCard from '../../components/VideoCard/VideoCard'
import Comments from '../../components/Comments/Comments'
import { usePlayerShortcuts } from './usePlayerShortcuts'
import { usePlayerTouch } from './usePlayerTouch'
import PlayerControls from './PlayerControls'
import { ConfirmDialog, AlertDialog } from '../../components/ui'
import './Player.css'

const SAVE_THROTTLE_MS = 3000
const MIN_PROGRESS_SAVE_S = 5
const HEARTBEAT_INTERVAL_MS = 15000

export default function Player() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const { t } = useTranslation()
  const { user } = useAuth()
  const videoId = Number(searchParams.get('id') || '0')
  const shareToken = (() => {
    const hash = window.location.hash
    const match = hash.match(/[#&]share=([^&]+)/)
    return match ? match[1] : null
  })()
  const isShared = !!shareToken

  const videoRef = useRef<HTMLVideoElement>(null)
  const playerRef = useRef<HTMLDivElement>(null)
  const progressRef = useRef<HTMLDivElement>(null)

  const [video, setVideo] = useState<MappedVideo | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [showLoading, setShowLoading] = useState(true)

  const [paused, setPaused] = useState(true)
  const [currentTime, setCurrent] = useState(0)
  const [duration, setDuration] = useState(0)
  const [buffered, setBuffered] = useState(0)
  const [speed, setSpeed] = useState(1)
  const [volume, setVolume] = useState(0.8)
  const [muted, setMuted] = useState(false)
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

  const seekingRef = useRef(false)
  const lastVolumeRef = useRef(0.8)
  const lastSaveTimeRef = useRef(0)
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const shortcutTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const heartbeatTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)

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

  // 断开视频连接：清空 src 停止浏览器预加载 + 停止播放会话
  const disconnectVideo = useCallback(() => {
    const v = videoRef.current
    if (v) {
      v.pause()
      v.removeAttribute('src')
      v.load()
    }
    if (heartbeatTimerRef.current) {
      clearInterval(heartbeatTimerRef.current)
      heartbeatTimerRef.current = null
    }
    if (videoId && !isShared) stopPlaybackSession(videoId).catch(() => {})
  }, [videoId, isShared])

  // 开始播放会话 + 心跳
  const startSession = useCallback(() => {
    if (!videoId || isShared) return
    startPlaybackSession(videoId).catch(() => {})
    trackVideo('开始播放', videoId)
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
    setVolume(val)
    setMuted(val === 0)
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
    if (!videoId || isShared || !videoRef.current?.duration) return
    const v = videoRef.current
    savePlayback(videoId, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {})
  }, [videoId, isShared])

  const seekBy = useCallback((delta: number) => {
    const v = videoRef.current
    if (!v) return
    v.currentTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + delta))
    resetHideTimer()
  }, [resetHideTimer])

  const handleProgressClick = useCallback((e: React.MouseEvent) => {
    const bar = progressRef.current
    const v = videoRef.current
    if (!bar || !v || !v.duration) return
    const rect = bar.getBoundingClientRect()
    const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width))
    v.currentTime = pct * v.duration
  }, [])

  const handleProgressMouseDown = useCallback((e: React.MouseEvent) => {
    seekingRef.current = true
    handleProgressClick(e)
  }, [handleProgressClick])

  const handleTouchProgress = (e: React.TouchEvent<HTMLDivElement>) => {
    seekingRef.current = true
    const touch = e.touches[0]
    if (touch) {
      handleProgressClick({ clientX: touch.clientX } as React.MouseEvent)
    }
  }

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (seekingRef.current) {
        const synthetic = { clientX: e.clientX } as React.MouseEvent
        handleProgressClick(synthetic)
      }
    }
    const handleMouseUp = () => { seekingRef.current = false }
    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)
    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [handleProgressClick])

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
      if (document.hidden) {
        saveProgress()
        videoRef.current?.pause()
      }
    }
    const onBeforeUnload = () => {
      saveProgress()
      disconnectVideo()
    }
    document.addEventListener('visibilitychange', handler)
    window.addEventListener('beforeunload', onBeforeUnload)
    return () => {
      document.removeEventListener('visibilitychange', handler)
      window.removeEventListener('beforeunload', onBeforeUnload)
    }
  }, [saveProgress, disconnectVideo])

  // Cleanup on unmount: stop session + clear heartbeat
  useEffect(() => {
    return () => {
      if (heartbeatTimerRef.current) clearInterval(heartbeatTimerRef.current)
      if (videoId && !isShared) stopPlaybackSession(videoId).catch(() => {})
    }
  }, [videoId, isShared])

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
            thumb: sv.thumbUrl,
            stream: sv.streamUrl,
            cover: null,
            sourceType: sv.sourceType,
            duration: 0,
            views: 0,
            date: '',
            progress: 0,
          }
          setVideo(mv)
          document.title = mv.title + ' · ATMOS'
        } catch {
          if (!cancelled) setError('分享链接无效或已过期')
        } finally {
          if (!cancelled) setLoading(false)
        }
      }
      load()
      return () => { cancelled = true }
    }

    if (!videoId) { setError('缺少视频ID'); setLoading(false); return }

    const load = async () => {
      setLoading(true)
      try {
        const v = await getVideo(videoId)
        const mv = mapVideo(v)
        if (cancelled || !mv) return
        setVideo(mv)
        document.title = mv.title + ' · ATMOS'
        incrementViews(videoId).catch(() => {})
        loadRelated(mv.category)
        
        // Load variants if video has them
        if (v.hasVariants) {
          try {
            const status = await getTranscodeStatus(videoId)
            if (!cancelled) {
              setVariants(status.variants)
            }
          } catch {
            // Ignore variant loading errors
          }
        }
      } catch {
        if (!cancelled) setError('加载失败')
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
    
    const currentTime = v.currentTime
    const wasPlaying = !v.paused
    
    if (quality === 'original') {
      v.src = video?.stream || ''
    } else {
      const variant = variants.find(variant => variant.resolution === quality)
      if (variant) {
        v.src = `/media/${variant.filePath}`
      }
    }
    
    v.load()
    v.currentTime = currentTime
    if (wasPlaying) {
      v.play().catch(() => {})
    }
    
    setCurrentQuality(quality)
    setShowQualityMenu(false)
    resetHideTimer()
  }, [video, variants, resetHideTimer])

  // Set video source and restore position
  useEffect(() => {
    const v = videoRef.current
    if (!v || !video?.stream) return
    v.src = video.stream
    v.poster = video.thumb || ''
    v.load()

    if (video.progress && video.progress > MIN_PROGRESS_SAVE_S * 1000) {
      v.currentTime = video.progress / 1000
    }

    if (getPref('speedMem')) {
      const saved = localStorage.getItem('atmos_speed_' + videoId)
      if (saved) { v.playbackRate = parseFloat(saved); setSpeed(parseFloat(saved)) }
    }

    v.play().catch(() => {})
  }, [video, videoId])

  // Video event handlers
  const onTimeUpdate = useCallback(() => {
    const v = videoRef.current
    if (!v || seekingRef.current) return
    setCurrent(v.currentTime)
    if (v.duration) setBuffered(v.buffered.length > 0 ? v.buffered.end(v.buffered.length - 1) / v.duration * 100 : 0)
    if (isShared) return
    const now = Date.now()
    if (now - lastSaveTimeRef.current >= SAVE_THROTTLE_MS && videoId && v.duration) {
      lastSaveTimeRef.current = now
      savePlayback(videoId, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {})
    }
  }, [videoId, isShared])

  const onPlay = useCallback(() => {
    setPaused(false)
    resetHideTimer()
    startSession()
  }, [resetHideTimer, startSession])

  const onPause = useCallback(() => {
    setPaused(true)
    showControls()
  }, [showControls])
  const onLoadedMetadata = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    setDuration(v.duration)
  }, [])
  const onWaiting = useCallback(() => setShowLoading(true), [])
  const onCanPlay = useCallback(() => setShowLoading(false), [])
  const onPlaying = useCallback(() => setShowLoading(false), [])

  const onEnded = useCallback(() => {
    if (!getPref('autoPlay')) return
    const next = related[0]
    if (next) {
      navigate(`/player?id=${next.id}`)
    }
  }, [related, navigate])

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

  const progressPct = duration > 0 ? (currentTime / duration) * 100 : 0

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
        setShareTooltipMsg('登录已过期，请刷新页面')
      } else if (e instanceof APIError && (e.status === 404 || e.status === 410)) {
        setShareTooltipMsg('操作失败，请确认视频存在')
      } else {
        setShareTooltipMsg(errMsg || '分享失败，请稍后重试')
      }
      setShowShareTooltip(true)
      setTimeout(() => setShowShareTooltip(false), 3000)
    }
  }

  if (error) {
    return (
      <div className="player-error">
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
        onMouseLeave={() => { if (hideTimerRef.current) clearTimeout(hideTimerRef.current); hideTimerRef.current = setTimeout(hideControls, 1000) }}
      >
        <video
          ref={videoRef}
          className="player-video"
          playsInline
          onTimeUpdate={onTimeUpdate}
          onPlay={onPlay}
          onPause={onPause}
          onLoadedMetadata={onLoadedMetadata}
          onWaiting={onWaiting}
          onCanPlay={onCanPlay}
          onPlaying={onPlaying}
          onEnded={onEnded}
          onClick={togglePlay}
        />

        {/* Top bar */}
        <header className={`player-top ${controlsVisible ? 'show' : ''}`}>
          <button className="player-back" onClick={() => navigate('/')}>←</button>
          <span className="player-title">{video?.title || ''}</span>
          {user?.isAdmin && <button className="player-delete" onClick={handleDelete} title={t('player.delete')}>🗑</button>}
        </header>

        {/* Loading spinner */}
        <div className={`player-loading ${showLoading ? 'show' : ''}`}>
          <div className="loading-ring" />
          <span>{t('common.loading')}</span>
        </div>

        {/* Center play button */}
        {!showLoading && paused && (
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
            controlsVisible={controlsVisible}
            paused={paused}
            currentTime={currentTime}
            duration={duration}
            buffered={buffered}
            speed={speed}
            volume={volume}
            muted={muted}
            progressRef={progressRef}
            progressPct={progressPct}
            seekingRef={seekingRef}
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
            handleProgressMouseDown={handleProgressMouseDown}
            handleTouchProgress={handleTouchProgress}
            seekBy={seekBy}
            setShowQualityMenu={setShowQualityMenu}
            setShowSpeedMenu={setShowSpeedMenu}
            t={t}
          />
      </div>

      {/* Video info */}
      {video && (
        <div className="player-detail">
          <div className="pd-meta">
            <span style={{ background: getCatColor(video.category) + '1a', color: getCatColor(video.category) }}>{video.category || '其他'}</span>
            {user && (
              <button className="pd-share-btn" onClick={handleShare}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7c.05-.23.09-.46.09-.7s-.04-.47-.09-.7l7.05-4.11c.54.5 1.25.81 2.04.81 1.66 0 3-1.34 3-3s-1.34-3-3-3-3 1.34-3 3c0 .24.04.47.09.7L8.04 9.81C7.5 9.31 6.79 9 6 9c-1.66 0-3 1.34-3 3s1.34 3 3 3c.79 0 1.5-.31 2.04-.81l7.12 4.16c-.05.21-.08.43-.08.65 0 1.61 1.31 2.92 2.92 2.92 1.61 0 2.92-1.31 2.92-2.92s-1.31-2.92-2.92-2.92z"/></svg>
                {t('player.share')}
              </button>
            )}
            {showShareTooltip && <span className="pd-share-tooltip">{shareTooltipMsg}</span>}
          </div>
          <h1 className="pd-title">{video.title}</h1>
          {video.description && <p className="pd-desc">{video.description}</p>}
        </div>
      )}

      {/* Comments */}
      {videoId > 0 && <Comments videoId={videoId} />}

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
