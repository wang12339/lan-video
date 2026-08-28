import { useState, useRef, useEffect, useCallback, useMemo, memo } from 'react'
import { mediaUrl } from '../../api/client'
import './VideoPreview.css'

function formatPreviewTime(seconds: number): string {
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

function formatPreviewViews(count?: number): string {
  if (!count) return ''
  if (count >= 10000) {
    return `${(count / 10000).toFixed(1)}万`
  }
  return count.toLocaleString()
}

interface VideoPreviewProps {
  videoId: string
  title: string
  thumbUrl?: string
  duration?: number
  views?: number
  visible: boolean
  position?: { x: number; y: number }
}

function VideoPreviewImpl({
  videoId,
  title,
  thumbUrl,
  duration,
  views,
  visible,
}: VideoPreviewProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const progressRef = useRef<HTMLDivElement>(null)
  const [loading, setLoading] = useState(true)
  const [isPlaying, setIsPlaying] = useState(false)
  const [progress, setProgress] = useState(0)
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const animationFrameRef = useRef<number | null>(null)
  const lastUpdateTimeRef = useRef<number>(0)

  // 智能预加载 - 使用 IntersectionObserver 实现懒加载
  const [shouldLoad, setShouldLoad] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          setShouldLoad(true)
          observer.disconnect()
        }
      },
      { threshold: 0.1 }
    )

    observer.observe(container)
    return () => observer.disconnect()
  }, [])

  // 使用 requestAnimationFrame 优化进度条更新
  const updateProgress = useCallback(() => {
    const video = videoRef.current
    if (!video || !video.duration || !duration) return

    const now = performance.now()
    // 限制更新频率为每 16ms（约 60fps）
    if (now - lastUpdateTimeRef.current < 16) {
      animationFrameRef.current = requestAnimationFrame(updateProgress)
      return
    }

    lastUpdateTimeRef.current = now
    const newProgress = (video.currentTime / video.duration) * 100
    setProgress(newProgress)

    if (video.currentTime < video.duration) {
      animationFrameRef.current = requestAnimationFrame(updateProgress)
    }
  }, [duration])

  // 优化的悬停预览逻辑
  useEffect(() => {
    if (visible) {
      // 增加悬停延迟到 800ms，减少误触
      hoverTimerRef.current = setTimeout(() => {
        const video = videoRef.current
        if (video && shouldLoad) {
          video.currentTime = 0
          video.play().catch(() => {
            // Autoplay might be blocked
          })
          setIsPlaying(true)
          // 开始进度条更新循环
          animationFrameRef.current = requestAnimationFrame(updateProgress)
        }
      }, 800)
    } else {
      if (hoverTimerRef.current) {
        clearTimeout(hoverTimerRef.current)
      }
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current)
      }
      if (videoRef.current) {
        videoRef.current.pause()
        videoRef.current.currentTime = 0
        setIsPlaying(false)
        setProgress(0)
      }
    }

    return () => {
      if (hoverTimerRef.current) {
        clearTimeout(hoverTimerRef.current)
      }
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current)
      }
    }
  }, [visible, shouldLoad, updateProgress])

  // 优化的事件处理器
  const handleLoadedData = useCallback(() => {
    setLoading(false)
  }, [])

  const handleWaiting = useCallback(() => {
    setLoading(true)
  }, [])

  const handlePlaying = useCallback(() => {
    setLoading(false)
  }, [])

  const handleEnded = useCallback(() => {
    setIsPlaying(false)
    setProgress(100)
  }, [])

  const handleSeeked = useCallback(() => {
    const video = videoRef.current
    if (video && video.duration) {
      setProgress((video.currentTime / video.duration) * 100)
    }
  }, [])

  // 视频源 URL 缓存
  const videoSrc = useMemo(() => mediaUrl(videoId), [videoId])

  // 清理动画帧
  useEffect(() => {
    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current)
      }
    }
  }, [])

  if (!visible) return null

  return (
    <div ref={containerRef} className={`video-preview ${visible ? 'visible' : ''}`}>
      {shouldLoad && (
        <video
          ref={videoRef}
          className="video-preview-video"
          src={videoSrc ?? undefined}
          poster={thumbUrl ?? undefined}
          muted
          playsInline
          preload="metadata"
          aria-label={title}
          onLoadedData={handleLoadedData}
          onWaiting={handleWaiting}
          onPlaying={handlePlaying}
          onEnded={handleEnded}
          onSeeked={handleSeeked}
        />
      )}

      {loading && isPlaying && (
        <div className="video-preview-loading">
          <div className="video-preview-spinner" />
        </div>
      )}

      <div ref={progressRef} className="video-preview-progress">
        <div
          className="video-preview-progress-fill"
          style={{
            width: `${progress}%`,
            willChange: 'width',
          }}
        />
      </div>

      <div className="video-preview-info">
        <div className="video-preview-title">{title}</div>
        <div className="video-preview-meta">
          {duration && <span>{formatPreviewTime(duration)}</span>}
          {views !== undefined && <span>{formatPreviewViews(views)}次播放</span>}
        </div>
      </div>
    </div>
  )
}

export default memo(VideoPreviewImpl)
