import { useState, useEffect, useRef, useCallback, useMemo, memo } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams, useNavigate } from 'react-router-dom'
import {
  getVideo, mapVideo, listVideos, incrementViews,
  savePlayback, getCatColor, deleteVideo,
  startPlaybackSession, heartbeatPlaybackSession, stopPlaybackSession,
  getSimilarVideos, createShareLink, getShareVideo, APIError,
  toggleFavorite, getFavoriteStatus,
} from '../../api'
import { listMyPlaylists, addVideoToPlaylist } from '../../api/playlists'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { request, mediaUrl, getToken, BASE } from '../../api/client'
import type { MappedVideo, VideoVariant } from '../../api/types'
import { getPref } from '../../api/prefs'
import { useAuth } from '../../context/AuthContext'
import { useToast } from '../../components/Toast/Toast'
import { trackClick, trackVideo } from '../../utils/track'
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

// ============================================================
// 性能优化常量
// ============================================================

const SAVE_THROTTLE_MS = 10000
const MIN_PROGRESS_SAVE_S = 5
const HEARTBEAT_INTERVAL_MS = 45000

// 预加载配置
const PRELOAD_THRESHOLD = 0.85 // 播放到 85% 时开始预加载下一集

// 节流配置
const TIME_UPDATE_THROTTLE_MS = 1000 // timeupdate 事件节流间隔
const MOUSE_MOVE_THROTTLE_MS = 100 // 鼠标移动节流间隔
const VOLUME_CHANGE_THROTTLE_MS = 100 // 音量变化节流间隔

// 内存管理配置
const MAX_RELATED_VIDEOS = 6 // 最多保留的相关视频数量
const PROGRESS_SAVE_DEBOUNCE_MS = 3000 // 进度保存防抖时间

// ============================================================
// 工具函数
// ============================================================
import { throttle, debounce } from '../../utils/throttle'
import { cleanupVideoElement, safeGetDuration } from '../../utils/videoUtils'
import { usePreloadManager, useMemoryManager } from './usePlayerHooks'

// ============================================================
// Player 组件
// ============================================================

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

  const { toast } = useToast()
  const queryClient = useQueryClient()
  const [favorited, setFavorited] = useState(false)
  const [showPlaylistPicker, setShowPlaylistPicker] = useState(false)

  const { data: favStatus } = useQuery({
    queryKey: ['favorite-status', videoId],
    queryFn: () => getFavoriteStatus(videoId),
    enabled: !!user && !!videoId && !isShared,
  })
  useEffect(() => { if (favStatus) setFavorited(favStatus.favorited) }, [favStatus])

  const { data: myPlaylists = [] } = useQuery({
    queryKey: ['my-playlists', user?.id],
    queryFn: listMyPlaylists,
    enabled: !!user && showPlaylistPicker,
  })

  const handleFavorite = async () => {
    if (!user || !video) { toast(t('auth.pleaseLogin') || '请先登录', 'error'); return }
    try {
      const res = await toggleFavorite(video.id)
      setFavorited(res.favorited)
      toast(res.favorited ? (t('player.favorited') as string || '已收藏') : (t('player.unfavorited') as string || '已取消收藏'), 'success')
      queryClient.invalidateQueries({ queryKey: ['my-favorites'] })
      trackClick(res.favorited ? '收藏' : '取消收藏', video.title)
    } catch (e: any) {
      toast(e.message || t('player.favoriteFailed') || '收藏失败', 'error')
    }
  }
  const handleAddToPlaylist = async (playlistId: string) => {
    if (!video) return
    try {
      await addVideoToPlaylist(playlistId, video.id)
      toast((t('player.addedToPlaylist') as string) || '已加入播放列表', 'success')
      setShowPlaylistPicker(false)
      queryClient.invalidateQueries({ queryKey: ['my-playlists'] })
      trackClick('加入播放列表', video.title)
    } catch (e: any) {
      toast(e.message || (t('player.addToPlaylistFailed') as string) || '加入失败', 'error')
    }
  }
  const [showQualityMenu, setShowQualityMenu] = useState(false)

  // Share state
  const [showShareTooltip, setShowShareTooltip] = useState(false)
  const [shareTooltipMsg, setShareTooltipMsg] = useState('')

  // Dialog state
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [deleteAlertMsg, setDeleteAlertMsg] = useState('')

  // 预加载状态
  const [preloadingNext, setPreloadingNext] = useState(false)

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

  // 节流事件处理器引用
  const throttledMouseMoveRef = useRef<((e: React.MouseEvent) => void) | null>(null)
  const throttledVolumeChangeRef = useRef<(() => void) | null>(null)

  // 使用自定义 hooks
  const { preloadVideo, cleanup: cleanupPreload } = usePreloadManager()
  const { optimizeMemory } = useMemoryManager()

  // HLS播放器支持
  useHlsPlayer({
    videoRef,
    src: hlsUrl || (video?.stream ? mediaUrl(video.stream) : null),
    autoPlay: false,
  })

  // ============================================================
  // 优化的回调函数
  // ============================================================

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
    cleanupVideoElement(videoRef.current)
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

  // 优化的进度保存 - 使用防抖
  const saveProgress = useCallback(() => {
    if (!videoId || isShared) return
    const v = videoRef.current
    if (!v || !isFinite(v.currentTime) || !isFinite(v.duration) || !v.duration) return
    savePlayback(videoId, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {})
  }, [videoId, isShared])

  // 使用防抖版本的进度保存
  const debouncedSaveProgress = useMemo(
    () => debounce(saveProgress, PROGRESS_SAVE_DEBOUNCE_MS),
    [saveProgress]
  )

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

  // ============================================================
  // 预加载下一集逻辑
  // ============================================================

  const preloadNextVideo = useCallback(() => {
    if (preloadingNext || related.length === 0) return
    
    const nextVideo = related[0]
    if (!nextVideo || nextVideo.id === videoId) return
    
    if (import.meta.env.DEV) console.log('Preloading next video:', nextVideo.id)
    setPreloadingNext(true)
    preloadVideo(nextVideo.id)
    
    // 预加载后优化内存
    setTimeout(optimizeMemory, 1000)
  }, [preloadingNext, related, videoId, preloadVideo, optimizeMemory])

  // ============================================================
  // Keyboard shortcuts (extracted to hook)
  // ============================================================

  usePlayerShortcuts(videoRef, {
    togglePlay, toggleFullscreen, toggleMute, togglePiP,
    setVolumeValue, setSpeedValue, showShortcut, resetHideTimer,
    t,
  })

  // Touch gestures (extracted to hook)
  usePlayerTouch(playerRef, videoRef, { setVolumeValue, showControls })

  // ============================================================
  // 初始化节流函数
  // ============================================================

  useEffect(() => {
    // 节流鼠标移动
    throttledMouseMoveRef.current = throttle((_e: unknown) => {
      resetHideTimer()
    }, MOUSE_MOVE_THROTTLE_MS) as unknown as (e: React.MouseEvent) => void

    // 节流音量变化
    throttledVolumeChangeRef.current = throttle(() => {
      const v = videoRef.current
      if (!v) return
      if (v.volume > 0 && !v.muted) lastVolumeRef.current = v.volume
    }, VOLUME_CHANGE_THROTTLE_MS)

    return () => {
      throttledMouseMoveRef.current = null
      throttledVolumeChangeRef.current = null
    }
  }, [resetHideTimer])

  // ============================================================
  // 事件监听器
  // ============================================================

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
      cleanupPreload()
    }
  }, [stopSession, cleanupPreload])

  // 切换视频时重置清晰度/错误状态
  useEffect(() => {
    setVariants([])
    setCurrentQuality('original')
    setVideoError('')
    setPreloadingNext(false)
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
          const thumbUrl = sv.thumbUrl ? mediaUrl(sv.thumbUrl) : null;
          const mv: MappedVideo = {
            id: sv.id,
            title: sv.title,
            category: sv.category,
            description: sv.description || '',
            thumb: thumbUrl,
            thumbnail_url: thumbUrl || '',  // 添加此字段
            stream: mediaUrl(sv.streamUrl),
            cover: null,
            sourceType: (sv.sourceType ?? 'local_video') as 'local_video' | 'external',
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
      // 限制相关视频数量以节省内存
      const items = recommended.filter(v => v.id !== videoId).slice(0, MAX_RELATED_VIDEOS)
      if (items.length > 0) {
        setRelated(items)
        return
      }
    } catch { /* fall back to category filter */ }
    try {
      const r = await listVideos({ type: 'local_video', size: 20, category })
      const items = r.items.map(mapVideo).filter((v): v is MappedVideo => !!v && v.id !== videoId).slice(0, MAX_RELATED_VIDEOS)
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

    let cancelled = false

    const initVideo = async () => {
      if (cancelled) return

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

    return () => {
      cancelled = true
    }
  }, [video, videoId])

  // ============================================================
  // 优化的视频事件处理器
  // ============================================================

  // 节流的 timeupdate 处理器
  const throttledTimeUpdate = useMemo(
    () => throttle(() => {
      const v = videoRef.current
      if (!v) return
      if (isShared) return
      const now = Date.now()
      if (now - lastSaveTimeRef.current >= SAVE_THROTTLE_MS && videoId && v.duration) {
        lastSaveTimeRef.current = now
        debouncedSaveProgress()
      }
      
      // 检查是否需要预加载下一集
      const duration = safeGetDuration(v)
      if (duration > 0 && v.currentTime / duration >= PRELOAD_THRESHOLD) {
        preloadNextVideo()
      }
    }, TIME_UPDATE_THROTTLE_MS),
    [videoId, isShared, debouncedSaveProgress, preloadNextVideo]
  )

  const onTimeUpdate = useCallback(() => {
    throttledTimeUpdate()
  }, [throttledTimeUpdate])

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

  // 节流的音量变化处理器
  const onVolumeChange = useCallback(() => {
    if (throttledVolumeChangeRef.current) {
      throttledVolumeChangeRef.current()
    }
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

  // 优化的鼠标移动处理器
  const onMouseMove = useCallback((e: React.MouseEvent) => {
    if (throttledMouseMoveRef.current) {
      throttledMouseMoveRef.current(e)
    }
  }, [])

  // ============================================================
  // 记忆化的样式和类名
  // ============================================================

  const playerWrapClassName = useMemo(() => 
    `player-wrap ${controlsVisible ? '' : 'controls-hidden'}`,
    [controlsVisible]
  )

  const playerTopClassName = useMemo(() => 
    `player-top ${controlsVisible ? 'show' : ''}`,
    [controlsVisible]
  )

  const loadingClassName = useMemo(() => 
    `player-loading ${showLoading ? 'show' : ''}`,
    [showLoading]
  )

  // ============================================================
  // 渲染
  // ============================================================

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
        className={playerWrapClassName}
        ref={playerRef}
        onMouseMove={onMouseMove}
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
        <header className={playerTopClassName}>
          <button className="player-back" onClick={() => navigate('/')} aria-label={t('player.backToHome')}>←</button>
          <span className="player-title">{video?.title || ''}</span>
          {user?.isAdmin && <button className="player-delete" onClick={handleDelete} title={t('player.delete')} aria-label={t('player.delete')}>🗑</button>}
        </header>

        {/* Loading spinner */}
        <div className={loadingClassName}>
          <div className="loading-ring" />
          <span>{t('common.loading')}</span>
          {preloadingNext && (
            <div className="preload-indicator">
              {t('player.preloadingNext')}
            </div>
          )}
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
            {user && !isShared && (
              <button className={`pd-action-btn ${favorited ? 'favorited' : ''}`} onClick={handleFavorite} aria-label={favorited ? '取消收藏' : '收藏'}>
                {favorited ? '❤️' : '♡'} {favorited ? (t('player.favorited') as string || '已收藏') : (t('player.favorite') as string || '收藏')}
              </button>
            )}
            {user && !isShared && (
              <button className="pd-action-btn" onClick={() => setShowPlaylistPicker(true)}>
                ➕ {t('player.addToPlaylist') as string || '加入播放列表'}
              </button>
            )}
            {user && (
              <button className="pd-share-btn" onClick={handleShare}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7c.05-.23.09-.46.09-.7s-.04-.47-.09-.7l7.05-4.11c.54.5 1.25.81 2.04.81 1.66 0 3-1.34 3-3s-1.34-3-3-3-3 1.34-3 3c0 .24.04.47.09.7L8.04 9.81C7.5 9.31 6.79 9 6 9c-1.66 0-3 1.34-3 3s1.34 3 3 3c.79 0 1.5-.31 2.04-.81l7.12 4.16c-.05.21-.08.43-.08.65 0 1.61 1.31 2.92 2.92 2.92 1.61 0 2.92-1.31 2.92-2.92s-1.31-2.92-2.92-2.92z"/></svg>
                {t('player.share')}
              </button>
            )}
            {showShareTooltip && <span className="pd-share-tooltip">{shareTooltipMsg}</span>}
          </div>
          {showPlaylistPicker && (
            <div className="cd-overlay" onClick={() => setShowPlaylistPicker(false)}>
              <div className="cd-dialog" role="dialog" aria-modal="true" aria-label="选择播放列表" onClick={e => e.stopPropagation()}>
                <h3 className="cd-title">选择播放列表</h3>
                <div style={{ maxHeight: 300, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 8, margin: '12px 0' }}>
                  {myPlaylists.length === 0 ? (
                    <p style={{ color: 'var(--text3)', fontSize: 14 }}>暂无播放列表，去个人中心创建</p>
                  ) : (
                    myPlaylists.map((pl: any) => (
                      <button key={pl.id} className="cd-btn cd-btn-outline" style={{ width: '100%', justifyContent: 'flex-start' }} onClick={() => handleAddToPlaylist(pl.id)}>
                        {pl.name} ({pl.item_count})
                      </button>
                    ))
                  )}
                </div>
                <div className="cd-actions">
                  <button className="cd-btn cd-btn-cancel" onClick={() => setShowPlaylistPicker(false)}>取消</button>
                </div>
              </div>
            </div>
          )}
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
