import React, { useCallback, useMemo, memo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import SkeletonLoader from '../ui/SkeletonLoader'
import LazyImage from '../ui/LazyImage'

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

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60)
  const s = Math.floor(seconds % 60)
  return `${m}:${s.toString().padStart(2, '0')}`
}

function relativeTime(dateStr: string): string {
  const now = Date.now()
  const then = new Date(dateStr).getTime()
  const diff = now - then
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return '刚刚'
  if (mins < 60) return `${mins}分钟前`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}小时前`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days}天前`
  const months = Math.floor(days / 30)
  if (months < 12) return `${months}个月前`
  return `${Math.floor(months / 12)}年前`
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

  // 获取缩略图URL，优先使用thumbnail_url，然后是thumb
  const thumbnailUrl = video.thumbnail_url || video.thumb || '';

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
          <div className="play-btn">▶</div>
        </div>
        {video.duration != null && video.duration > 0 && (
          <span className="dur">
            {formatDuration(video.duration)}
          </span>
        )}
        {video.progress != null && video.progress > 0 && (
          <div className="progress-bar">
            <div className="progress-fill" style={{ width: `${video.progress}%` }} />
          </div>
        )}
      </div>
      <div className="video-info">
        {video.category && (
          <span className="cat-badge" data-cat={video.category}>
            {video.category}
          </span>
        )}
        <h3>{video.title}</h3>
        <div className="video-meta">
          {viewsText
            ? <span className="views" aria-label={`${video.views} views`}>
                <svg className="views-icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
                  <path d="M8 3C4.5 3 1.5 5.5 0.5 8c1 2.5 4 5 7.5 5s6.5-2.5 7.5-5c-1-2.5-4-5-7.5-5zm0 8a3 3 0 110-6 3 3 0 010 6zm0-5a2 2 0 100 4 2 2 0 000-4z"/>
                </svg>
                {viewsText}
              </span>
            : <span className="video-badge-new" aria-label="新视频">新</span>
          }
          {video.date && <span className="video-date">{relativeTime(video.date)}</span>}
        </div>
      </div>
    </div>
  );
});

VideoCard.displayName = 'VideoCard';

// VideoCard 骨架屏组件（委托给统一 SkeletonLoader）
export function VideoCardSkeleton({ count = 1 }: { count?: number }) {
  return <SkeletonLoader type="video-card" lines={count} />;
}

export default VideoCard;
