import { useRef, useEffect, useState, useMemo, useCallback, memo } from 'react'
import { useNavigate } from 'react-router-dom'
import { formatDuration, formatViews, getCatColor } from '../../api'
import type { MappedVideo } from '../../api/types'
import { trackClick } from '../../utils/track'
import './VideoCard.css'

interface VideoCardProps {
  video: MappedVideo;
  isList?: boolean;
  compact?: boolean;
  selected?: boolean;
  onSelect?: (id: number, e: React.MouseEvent | React.KeyboardEvent) => void;
}

const VideoCard = memo(function VideoCard({ video, isList = false, compact = false, selected = false, onSelect }: VideoCardProps) {
  const navigate = useNavigate()
  const imgRef = useRef<HTMLImageElement>(null)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    const img = imgRef.current
    if (!img || !video.thumb) return

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            img.src = video.thumb!
            observer.unobserve(img)
          }
        })
      },
      { rootMargin: '200px' }
    )
    observer.observe(img)
    return () => observer.disconnect()
  }, [video.thumb])

  // Also re-observe when thumb changes: reset loaded state so that
  // the new file starts loading when it scrolls into view again.
  const prevThumbRef = useRef(video.thumb)
  useEffect(() => {
    if (prevThumbRef.current !== video.thumb) {
      setLoaded(false)
      prevThumbRef.current = video.thumb
    }
  }, [video.thumb])

  const handleClick = useCallback((e: React.MouseEvent) => {
    if (onSelect) {
      onSelect(video.id, e)
    } else {
      trackClick('点击视频', video.title)
      navigate(`/player?id=${video.id}`)
    }
  }, [onSelect, video.id, video.title, navigate])

  const style = useMemo(() => ({
    '--cat-color': getCatColor(video.category)
  } as React.CSSProperties), [video.category])

  const dur = video.duration ? formatDuration(video.duration) : ''
  const views = video.views ? formatViews(video.views) + ' 次播放' : ''

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      if (onSelect) {
        onSelect(video.id, e)
      } else {
        trackClick('点击视频', video.title)
        navigate(`/player?id=${video.id}`)
      }
    }
  }, [onSelect, video.id, video.title, navigate])

  return (
    <div
      className={`card ${isList ? 'list-mode' : ''} ${selected ? 'selected' : ''}`}
      data-cat={video.category}
      data-id={video.id}
      style={style}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
      aria-label={video.title}
    >
      <div className="thumb-wrap">
        {video.thumb && (
          <img
            ref={imgRef}
            alt={video.title}
            className={`card-img ${loaded ? 'loaded' : ''}`}
            onLoad={() => setLoaded(true)}
            onError={(e) => {
              const el = e.target as HTMLImageElement
              el.style.display = 'none'
              console.warn('Image failed to load:', el.src)
            }}
          />
        )}
        {!isList && (
          <>
            <div className="thumb-overlay" />
            <div className="play-over">
              <span className="play-btn" style={compact ? { width: 42, height: 42, fontSize: 15 } : undefined}>▶</span>
            </div>
          </>
        )}
        {dur && <span className="dur">{dur}</span>}
      </div>

      <div className="info">
        {isList ? (
          <div className="info-top">
            {views && <span className="views">{views}</span>}
          </div>
        ) : (
          <>
            <div className="info-top">
              <span className="cat-badge">{video.category || '其他'}</span>
              {views && <span className="views">{views}</span>}
            </div>
            <div className="title">{video.title}</div>
          </>
        )}
      </div>
    </div>
  )
})

export default VideoCard
