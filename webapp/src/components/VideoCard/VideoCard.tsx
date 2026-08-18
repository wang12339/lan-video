import { useState, useMemo, useCallback, memo } from 'react'
import { useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { formatDuration, formatViews, getCatColor } from '../../api'
import type { MappedVideo } from '../../api/types'
import { trackClick } from '../../utils/track'
import './VideoCard.css'

interface VideoCardProps {
  video: MappedVideo;
  isList?: boolean;
  compact?: boolean;
  selected?: boolean;
  onSelect?: (id: string, e: React.MouseEvent | React.KeyboardEvent) => void;
}

// getCatColor 对未知分类返回白色，用此标记跳过 --cat-color，避免白底白字
const UNKNOWN_CATEGORY_COLOR = '#ffffff'

const CATEGORY_EMOJI: Record<string, string> = {
  科技: '💻',
  设计: '🎨',
  音乐: '🎵',
  教程: '📚',
  娱乐: '🎮',
  运动: '⚽',
  记录: '📷',
  外部: '🌐',
}

function VideoCardImpl({ video, isList = false, compact = false, selected = false, onSelect }: VideoCardProps) {
  const navigate = useNavigate()
  const { t } = useTranslation()
  const thumb = video.thumb
  const [loaded, setLoaded] = useState(false)
  const [thumbFailed, setThumbFailed] = useState(false)
  const [prevThumb, setPrevThumb] = useState(thumb)

  // 缩略图变化时同步重置加载/失败状态（React 官方推荐的渲染期状态调整模式）
  if (prevThumb !== thumb) {
    setPrevThumb(thumb)
    setLoaded(false)
    setThumbFailed(false)
  }

  const openVideo = useCallback((e: React.SyntheticEvent) => {
    if (onSelect) {
      onSelect(video.id, e as React.MouseEvent | React.KeyboardEvent)
    } else {
      trackClick('点击视频', video.title)
      navigate(`/player?id=${video.id}`)
    }
  }, [onSelect, video.id, video.title, navigate])

  const style = useMemo(() => {
    const color = getCatColor(video.category)
    if (!color || color === UNKNOWN_CATEGORY_COLOR) return undefined
    return { '--cat-color': color } as React.CSSProperties
  }, [video.category])

  const catLabel = video.category || t('common.other')
  const dur = video.duration > 0 ? formatDuration(video.duration) : ''
  // 清理标题：去掉文件扩展名，替换下划线为空格
  const cleanTitle = video.title
    .replace(/\.[^.]+$/, '') // 去掉扩展名
    .replace(/_/g, ' ') // 下划线替换为空格
    .replace(/\s+/g, ' ') // 多个空格合并
    .trim()
  const views = t('common.views', { n: formatViews(video.views ?? 0) })

  return (
    <div
      className={`card ${isList ? 'list-mode' : ''} ${selected ? 'selected' : ''}`}
      data-cat={video.category}
      data-id={video.id}
      style={style}
      onClick={openVideo}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          openVideo(e)
        }
      }}
      role="button"
      tabIndex={0}
      title={video.title}
      aria-label={video.title}
    >
      <div className="thumb-wrap">
        {thumb && !thumbFailed ? (
          <img
            src={thumb}
            alt={video.title}
            className={`card-img ${loaded ? 'loaded' : ''}`}
            loading="lazy"
            decoding="async"
            onLoad={() => setLoaded(true)}
            onError={() => setThumbFailed(true)}
          />
        ) : (
          <div className="thumb-fallback" role="img" aria-label={video.title}>
            <span aria-hidden="true">{CATEGORY_EMOJI[video.category] || '🎬'}</span>
          </div>
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

        {video.progress > 0 && (
          <div className="progress-bar" aria-hidden="true">
            <div className="progress-fill" style={{ width: `${Math.min(100, video.progress)}%` }} />
          </div>
        )}
      </div>

      <div className="info">
        <div className="info-top">
          <span className="cat-badge">{catLabel}</span>
          <span className="views">{views}</span>
        </div>
        <div className="title" title={video.title}>{cleanTitle || video.title}</div>
      </div>
    </div>
  )
}

const VideoCard = memo(VideoCardImpl)

export default VideoCard

export function VideoCardSkeleton({ count = 6 }: { count?: number }) {
  return (
    <>
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="card-skeleton" aria-hidden="true">
          <div className="skeleton-thumb" />
          <div className="skeleton-info">
            <div className="skeleton-line" style={{ width: '30%' }} />
            <div className="skeleton-line" style={{ width: '90%' }} />
            <div className="skeleton-line" style={{ width: '60%' }} />
          </div>
        </div>
      ))}
    </>
  )
}
