import React, { useCallback, useMemo, memo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { formatDuration } from '../../api/utils'
import SkeletonLoader from '../ui/SkeletonLoader'
import LazyImage from '../ui/LazyImage'
import './VideoCard.css'

interface Video {
  id: string;
  title: string;
  thumbnail_url?: string;
  thumb?: string | null;
  views: number;
  category?: string;
  duration?: number;
  date?: string;
  progress?: number;
}

interface VideoCardProps {
  video: Video;
  onClick?: (video: Video) => void;
  compact?: boolean;  // 添加compact属性支持
  eager?: boolean; // 首屏前4张设为 eager + high priority，优化 LCP
}

function getCategoryKey(cat: string): string {
  const map: Record<string, string> = {
    '科技': 'tech', '设计': 'design', '音乐': 'music',
    '教程': 'tutorial', '娱乐': 'entertainment', '运动': 'sports', '记录': 'record'
  }
  return map[cat] || cat
}

function relativeTime(dateStr: string, t: (k: string, opts?: Record<string, unknown>) => string): string {
  const now = Date.now()
  const then = new Date(dateStr).getTime()
  const diff = now - then
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return t('time.justNow')
  if (mins < 60) return t('time.minutesAgo', { n: mins })
  const hours = Math.floor(mins / 60)
  if (hours < 24) return t('time.hoursAgo', { n: hours })
  const days = Math.floor(hours / 24)
  if (days < 30) return t('time.daysAgo', { n: days })
  const months = Math.floor(days / 30)
  if (months < 12) return t('time.monthsAgo', { n: months })
  return t('time.yearsAgo', { n: Math.floor(months / 12) })
}

const VideoCard: React.FC<VideoCardProps> = memo(({ video, onClick, compact = false, eager = false }) => {
  const navigate = useNavigate();
  const { t } = useTranslation();

  const handleClick = useCallback(
    (_e: React.SyntheticEvent) => {
      if (onClick) {
        onClick(video);
      } else {
        navigate(`/player?id=${encodeURIComponent(video.id)}`);
      }
    },
    [onClick, video, navigate]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        if (onClick) {
          onClick(video);
        } else {
          navigate(`/player?id=${encodeURIComponent(video.id)}`);
        }
      }
    },
    [onClick, video, navigate]
  );

  const viewsText = useMemo(
    () => t('common.views', { count: video.views }),
    [video.views, t]
  );

  const thumbnailUrl = useMemo(
    () => video.thumbnail_url || video.thumb || '',
    [video.thumbnail_url, video.thumb]
  );

  const categoryBadge = useMemo(() => {
    if (!video.category || ['local_video', 'local', 'external'].includes(video.category)) return null
    return (
      <span className="cat-badge" data-cat={video.category}>
        {t('home.categories.' + getCategoryKey(video.category), { defaultValue: video.category })}
      </span>
    )
  }, [video.category, t]);

  const durationBadge = useMemo(() => {
    if (video.duration == null || video.duration <= 0) return null
    return (
      <span className="dur">
        {formatDuration(video.duration)}
      </span>
    )
  }, [video.duration]);

  const progressBar = useMemo(() => {
    if (video.progress == null || video.progress <= 0) return null
    return (
      <div className="vc-progress-bar">
        <div className="vc-progress-fill" style={{ width: `${video.progress}%` }} />
      </div>
    )
  }, [video.progress]);

  return (
    <div
      className={`video-card ${compact ? 'compact' : ''}`}
      role="button"
      tabIndex={0}
      aria-label={video.title}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
    >
      <div className="thumb-wrap">
        <LazyImage
          src={thumbnailUrl}
          alt={video.title}
          className="card-img"
          imageContext="card"
          eager={eager}
          showPlaceholder={false}
          fallback={
            <div className="thumb-fallback" role="img" aria-label={video.title}>
              <span className="thumb-fallback-icon" aria-hidden="true">🎬</span>
              <span className="thumb-fallback-text">{video.title.slice(0, 20)}</span>
            </div>
          }
        />
        <div className="thumb-overlay" aria-hidden="true" />
        <div className="play-over" aria-hidden="true">
          <div className="play-btn">
            <svg viewBox="0 0 24 24" fill="currentColor" width="24" height="24" aria-hidden="true">
              <path d="M8 5v14l11-7z"/>
            </svg>
          </div>
        </div>
        {durationBadge}
        {progressBar}
      </div>
      <div className="video-info">
        {categoryBadge}
        <h3 className="title">{video.title}</h3>
        <div className="video-meta">
          {video.views > 0
            ? <span className="views" aria-label={`${video.views} views`}>
                <svg className="views-icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
                  <path d="M8 3C4.5 3 1.5 5.5 0.5 8c1 2.5 4 5 7.5 5s6.5-2.5 7.5-5c-1-2.5-4-5-7.5-5zm0 8a3 3 0 110-6 3 3 0 010 6zm0-5a2 2 0 100 4 2 2 0 000-4z"/>
                </svg>
                {viewsText}
              </span>
            : <span className="video-badge-new" aria-label={t('gallery.newBadge')}>{t('gallery.newBadge')}</span>
          }
          {video.views > 0 && video.date && <span className="meta-sep" aria-hidden="true">·</span>}
          {video.date && <span className="video-date">{relativeTime(video.date, t)}</span>}
        </div>
      </div>
    </div>
  );
});

VideoCard.displayName = 'VideoCard';

interface VideoCardSkeletonProps {
  count?: number
}

export const VideoCardSkeleton = memo(function VideoCardSkeleton({ count = 1 }: VideoCardSkeletonProps) {
  return <SkeletonLoader type="video-card" lines={count} aria-hidden="true" />
})

export default VideoCard;
