import { memo, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { formatDuration } from '../../api'
import type { MappedHistory } from '../../api/types'
import { SkeletonLoader } from '../../components/ui'
import { trackClick } from '../../utils/track'

export const HistoryRow = memo(function HistoryRow({ item, showProgress }: { item: MappedHistory; showProgress?: boolean }) {
  const navigate = useNavigate()
  const { t } = useTranslation()

  const open = useCallback(() => {
    trackClick(showProgress ? 'continueWatching' : 'openFavorite', item.title)
    navigate(`/player?id=${item.id}`)
  }, [navigate, item.id, item.title, showProgress])

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      open()
    }
  }

  const progress = Math.max(0, Math.min(100, item.progress))

  return (
    <div
      className="history-item"
      role="button"
      tabIndex={0}
      onClick={open}
      onKeyDown={onKeyDown}
      aria-label={item.title}
    >
      <div className="history-thumb">
        {item.thumb ? (
          <img
            src={item.thumb}
            alt=""
            loading="lazy"
            onError={(e) => { e.currentTarget.style.display = 'none' }}
          />
        ) : null}
        {item.durationMs > 0 && (
          <span className="history-dur">{formatDuration(Math.floor(item.durationMs / 1000))}</span>
        )}
      </div>
      <div className="history-info">
        <div className="history-title">
          {item.title}
          {showProgress && progress > 0 && (
            <span className="history-continue">{t('common.continue', { progress })}</span>
          )}
        </div>
        <div className="history-meta">
          {[item.category, item.updatedAt ? new Date(item.updatedAt).toLocaleDateString() : ''].filter(Boolean).join(' · ')}
        </div>
        {showProgress && item.durationMs > 0 && (
          <div className="history-progress" aria-hidden="true">
            <div className="hp-fill" style={{ width: `${progress}%` }} />
          </div>
        )}
      </div>
    </div>
  )
})

export function ListSkeleton() {
  return (
    <div className="history-list" aria-hidden="true">
      <SkeletonLoader type="video-card" lines={4} />
    </div>
  )
}

export function TabError({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation()
  return (
    <div className="tab-error">
      <div className="empty-icon">⚠️</div>
      <div className="tab-error-text">{t('errors.loadFailedNetwork')}</div>
      <button className="profile-retry" onClick={onRetry}>{t('common.retry')}</button>
    </div>
  )
}

export function formatWatchTime(ms: number, t: (k: string, o?: Record<string, unknown>) => string): string {
  const totalMin = Math.floor((ms || 0) / 60000)
  if (totalMin < 60) return t('common.minutes', { n: totalMin })
  const h = Math.floor(totalMin / 60)
  const m = totalMin % 60
  return m > 0 ? t('common.hoursMin', { h, m }) : t('common.hoursOnly', { h })
}
