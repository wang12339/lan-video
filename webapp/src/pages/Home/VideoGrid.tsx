import React, { useMemo, useCallback } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { MappedVideo } from '../../api/types'
import VideoCard, { VideoCardSkeleton } from '../../components/VideoCard/VideoCard'

const VideoCardMemo = React.memo(VideoCard)

function useSkeletonCount() {
  return useMemo(() => {
    if (typeof window === 'undefined') return 6
    const width = window.innerWidth
    if (width <= 380) return 2
    if (width <= 640) return 4
    if (width <= 1024) return 6
    return 8
  }, [])
}

interface VideoGridProps {
  videos: MappedVideo[]
  viewMode: 'grid' | 'list'
  isPending: boolean
  isError: boolean
  hasNextPage: boolean
  isFetchingNextPage: boolean
  onRetry: () => void
  onLoadMore: () => void
}

export default function VideoGrid({
  videos,
  viewMode,
  isPending,
  isError,
  hasNextPage,
  isFetchingNextPage,
  onRetry,
  onLoadMore,
}: VideoGridProps) {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const query = (searchParams.get('q') || '').trim()
  const skeletonCount = useSkeletonCount()

  const showInitialError = isError && videos.length === 0 && !isPending
  const showEmpty = !isPending && !isError && videos.length === 0

  const clearSearch = useCallback(() => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      next.delete('q')
      return next
    }, { replace: true })
  }, [setSearchParams])

  return (
    <>
      {videos.length > 0 ? (
        <div className={`video-grid ${viewMode === 'list' ? 'list-view' : ''}`}>
          {videos.map((video, i) => (
            <div key={video.id} style={{ '--card-index': i } as React.CSSProperties}>
              <VideoCardMemo video={video} eager={i < 4} />
            </div>
          ))}
        </div>
      ) : isPending ? (
        <div className="video-grid">
          <VideoCardSkeleton count={skeletonCount} />
        </div>
      ) : showInitialError ? (
        <div className="empty-state">
          <div className="empty-icon">⚠️</div>
          <div className="empty-text">{t('errors.network')}</div>
          <p className="empty-hint">{t('home.errorHint')}</p>
          <button className="home-retry-btn" onClick={onRetry}>
            {t('common.retry')}
          </button>
        </div>
      ) : showEmpty ? (
        <div className="empty-state" role="status" aria-live="polite">
          <div className="empty-icon" aria-hidden="true">
            {query ? '🔍' : '🎬'}
          </div>
          <div className="empty-text">
            {query ? t('home.searchEmpty', { query }) : t('home.empty')}
          </div>
          {query ? (
            <button className="empty-cta" onClick={clearSearch}>
              {t('common.clearSearch')}
            </button>
          ) : (
            <Link to="/upload" className="empty-cta">
              {t('common.goUpload')} →
            </Link>
          )}
        </div>
      ) : null}

      {!isError && hasNextPage && !isFetchingNextPage && videos.length > 0 && (
        <div className="load-more-wrap">
          <button className="home-load-more-btn" onClick={onLoadMore}>
            {t('common.loadMore')}
          </button>
        </div>
      )}

      {isError && videos.length > 0 && (
        <div className="load-more-error">
          <span>{t('errors.network')}</span>
          <button className="home-retry-btn" onClick={onRetry}>
            {t('common.retry')}
          </button>
        </div>
      )}

      {!isError && isFetchingNextPage && (
        <div className="video-grid loading-grid" aria-label={t('common.loading')}>
          <VideoCardSkeleton count={skeletonCount} />
        </div>
      )}

      {!isError && !isFetchingNextPage && !hasNextPage && videos.length > 0 && (
        <div className="no-more">{t('common.noMore')}</div>
      )}
    </>
  )
}
