import React, { useMemo, useCallback } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { MappedVideo } from '../../api/types'
import VideoCard, { VideoCardSkeleton } from '../../components/VideoCard/VideoCard'
import { useVirtualGrid, type RowHeightContext } from '../../hooks/useVirtualGrid'

const VideoCardMemo = React.memo(VideoCard)

/** 网格视图列间距（与 Home.css 的 --card-gap 默认值一致） */
const GRID_GAP = 16
/** 列表视图行间距（与 .video-grid.list-view 的 gap 一致） */
const LIST_GAP = 12
/** 列表视图单行估算高度（缩略图 135px + 信息区） */
const LIST_ROW_HEIGHT = 140
/** 网格视图卡片信息区估算高度（padding + 标题两行 + 元信息，偏保守） */
const GRID_INFO_HEIGHT = 96

/**
 * 网格视图行高估算：列宽 * 9/16（缩略图 16:9）+ 信息区。
 * 刻意取偏小值：低估只会多渲染几行，高估会导致视口底部出现空白。
 */
function estimateGridRowHeight({ containerWidth, columns, gap }: RowHeightContext): number {
  const safeColumns = Math.max(1, columns)
  const columnWidth = (containerWidth - Math.max(0, safeColumns - 1) * gap) / safeColumns
  return columnWidth * (9 / 16) + GRID_INFO_HEIGHT
}

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

  const isList = viewMode === 'list'

  // 窗口滚动的响应式网格虚拟化：只渲染可视区域 + overscan 行
  const { containerRef, windowed, virtualItems, startIndex, paddingTop, paddingBottom } =
    useVirtualGrid({
      itemCount: videos.length,
      minItemWidth: 280,
      rowHeight: isList ? LIST_ROW_HEIGHT : estimateGridRowHeight,
      gap: isList ? LIST_GAP : GRID_GAP,
      overscan: 2,
      fixedColumns: isList ? 1 : undefined,
      layoutKey: viewMode,
      onReachEnd: () => {
        // 窗口化后底部哨兵可能不再进入视口，由“可见范围触达末尾”兜底触发加载
        if (hasNextPage && !isFetchingNextPage) onLoadMore()
      },
    })

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
        <div
          ref={containerRef}
          className={`video-grid ${viewMode === 'list' ? 'list-view' : ''}`}
          style={windowed ? { paddingTop, paddingBottom } : undefined}
        >
          {virtualItems.map((index) => {
            const video = videos[index]
            if (!video) return null
            return (
              <div
                // 动画延迟用窗口内局部下标，避免深翻页后卡片等满 19 档延迟才出现
                key={video.id}
                style={{ '--card-index': index - startIndex } as React.CSSProperties}
              >
                <VideoCardMemo video={video} eager={index < 4} />
              </div>
            )
          })}
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
