import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { MappedHistory } from '../../api/types'
import { HistoryRow, ListSkeleton, TabError } from './SharedComponents'

interface Props {
  history: MappedHistory[]
  pending: boolean
  error: boolean
  refetch: () => void
}

export default function HistoryTab({ history, pending, error, refetch }: Props) {
  const { t } = useTranslation()

  if (pending) {
    return (
      <div className="profile-content active" role="tabpanel">
        <ListSkeleton />
      </div>
    )
  }

  if (error) {
    return (
      <div className="profile-content active" role="tabpanel">
        <TabError onRetry={() => refetch()} />
      </div>
    )
  }

  if (history.length === 0) {
    return (
      <div className="profile-content active" role="tabpanel">
        <div className="profile-empty" role="status">
          <div className="empty-icon" aria-hidden="true">🕐</div>
          <div>{t('profile.noHistory')}</div>
          <Link to="/gallery" className="empty-cta">{t('profile.goDiscoverCta')}</Link>
        </div>
      </div>
    )
  }

  return (
    <div className="profile-content active" role="tabpanel">
      <div className="history-list">
        {history.map((h) => (
          <HistoryRow key={h.id} item={h} showProgress />
        ))}
      </div>
    </div>
  )
}
