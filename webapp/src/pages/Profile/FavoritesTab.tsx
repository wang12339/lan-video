import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { MappedHistory } from '../../api/types'
import { HistoryRow, ListSkeleton, TabError } from './SharedComponents'

interface Props {
  favorites: MappedHistory[]
  pending: boolean
  error: boolean
  refetch: () => void
}

export default function FavoritesTab({ favorites, pending, error, refetch }: Props) {
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

  if (favorites.length === 0) {
    return (
      <div className="profile-content active" role="tabpanel">
        <div className="profile-empty">
          <div className="empty-icon">❤️</div>
          <div>{t('profile.noFavorites')}</div>
          <Link to="/gallery" className="empty-cta">{t('profile.goBrowseCta')}</Link>
        </div>
      </div>
    )
  }

  return (
    <div className="profile-content active" role="tabpanel">
      <div className="history-list">
        {favorites.map((h) => (
          <HistoryRow key={h.id} item={h} />
        ))}
      </div>
    </div>
  )
}
