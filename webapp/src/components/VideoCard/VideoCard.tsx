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
}

interface VideoCardProps {
  video: Video;
  onClick?: (video: Video) => void;
  compact?: boolean;  // 添加compact属性支持
  eager?: boolean; // 首屏前4张设为 eager + high priority，优化 LCP
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
              <span aria-hidden="true">🎬</span>
            </div>
          }
        />
      </div>
      <div className="video-info">
        <h3>{video.title}</h3>
        {viewsText
          ? <span aria-label={`${video.views} 次观看`}>{viewsText}</span>
          : <span className="video-badge-new" aria-label="新视频">新</span>
        }
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
