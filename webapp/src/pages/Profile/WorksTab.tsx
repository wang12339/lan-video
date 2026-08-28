import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { MappedVideo } from '../../api/types'
import VideoCard, { VideoCardSkeleton } from '../../components/VideoCard/VideoCard'
import { TabError } from './SharedComponents'

interface Props {
  works: MappedVideo[]
  pending: boolean
  error: boolean
  isFetchingNextPage: boolean
  hasNextPage: boolean | undefined
  fetchNextPage: () => void
  refetch: () => void
}

export default function WorksTab({
  works, pending, error, isFetchingNextPage, hasNextPage, fetchNextPage, refetch,
}: Props) {
  const { t } = useTranslation()

  if (pending && works.length === 0) {
    return (
      <div className="profile-content active" role="tabpanel">
        <div className="profile-grid">
          <VideoCardSkeleton count={8} />
        </div>
      </div>
    )
  }

  if (error && works.length === 0) {
    return (
      <div className="profile-content active" role="tabpanel">
        <TabError onRetry={() => refetch()} />
      </div>
    )
  }

  if (works.length === 0) {
    return (
      <div className="profile-content active" role="tabpanel">
        <div className="profile-empty" role="status" aria-live="polite">
          <div className="empty-icon" aria-hidden="true">🎬</div>
          <div>{t('profile.noWorks')}</div>
          <Link to="/upload" className="empty-cta">{t('profile.goUploadCta')}</Link>
        </div>
      </div>
    )
  }

  return (
    <div className="profile-content active" role="tabpanel">
      <div className="profile-grid">
        {works.map((v) => (
          <VideoCard key={v.id} video={v} />
        ))}
      </div>
      {error && (
        <div className="load-more-error">
          <span>{t('errors.loadFailed')}</span>
          <button className="profile-retry" onClick={() => refetch()}>{t('common.retry')}</button>
        </div>
      )}
      {hasNextPage ? (
        <button className="profile-load-more-btn" onClick={() => fetchNextPage()} disabled={isFetchingNextPage}>
          {isFetchingNextPage ? t('common.loading') : t('common.loadMore')}
        </button>
      ) : (
        works.length > 0 && <div className="pm-no-more">{t('common.noMore')}</div>
      )}
    </div>
  )
}
