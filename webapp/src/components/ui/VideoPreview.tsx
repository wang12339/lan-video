import { useState, useRef, useEffect, useCallback } from 'react'
import { mediaUrl } from '../../api/client'
import './VideoPreview.css'

interface VideoPreviewProps {
  videoId: string
  title: string
  thumbUrl?: string
  duration?: number
  views?: number
  visible: boolean
  position?: { x: number; y: number }
}

export default function VideoPreview({
  videoId,
  title,
  thumbUrl,
  duration,
  views,
  visible,
}: VideoPreviewProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const [loading, setLoading] = useState(true)
  const [currentTime, setCurrentTime] = useState(0)
  const [isPlaying, setIsPlaying] = useState(false)
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Start preview after hovering for 500ms
  useEffect(() => {
    if (visible) {
      hoverTimerRef.current = setTimeout(() => {
        if (videoRef.current) {
          videoRef.current.currentTime = 0
          videoRef.current.play().catch(() => {
            // Autoplay might be blocked
          })
          setIsPlaying(true)
        }
      }, 500)
    } else {
      if (hoverTimerRef.current) {
        clearTimeout(hoverTimerRef.current)
      }
      if (videoRef.current) {
        videoRef.current.pause()
        videoRef.current.currentTime = 0
        setIsPlaying(false)
      }
    }

    return () => {
      if (hoverTimerRef.current) {
        clearTimeout(hoverTimerRef.current)
      }
    }
  }, [visible])

  const handleTimeUpdate = useCallback(() => {
    if (videoRef.current) {
      setCurrentTime(videoRef.current.currentTime)
    }
  }, [])

  const handleLoadedData = useCallback(() => {
    setLoading(false)
  }, [])

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60)
    const secs = Math.floor(seconds % 60)
    return `${mins}:${secs.toString().padStart(2, '0')}`
  }

  const formatViews = (count?: number) => {
    if (!count) return ''
    if (count >= 10000) {
      return `${(count / 10000).toFixed(1)}万`
    }
    return count.toLocaleString()
  }

  if (!visible) return null

  return (
    <div className={`video-preview ${visible ? 'visible' : ''}`}>
      <video
        ref={videoRef}
        className="video-preview-video"
        src={mediaUrl(videoId) ?? undefined}
        poster={thumbUrl ?? undefined}
        muted
        playsInline
        preload="metadata"
        onTimeUpdate={handleTimeUpdate}
        onLoadedData={handleLoadedData}
      />

      {loading && isPlaying && (
        <div className="video-preview-loading">
          <div className="video-preview-spinner" />
        </div>
      )}

      <div className="video-preview-progress">
        <div
          className="video-preview-progress-fill"
          style={{
            width: duration ? `${(currentTime / duration) * 100}%` : '0%',
          }}
        />
      </div>

      <div className="video-preview-info">
        <div className="video-preview-title">{title}</div>
        <div className="video-preview-meta">
          {duration && <span>{formatTime(duration)}</span>}
          {views !== undefined && <span>{formatViews(views)}次播放</span>}
        </div>
      </div>
    </div>
  )
}
