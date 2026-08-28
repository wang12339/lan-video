import { useState, useCallback } from 'react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { ShareListItem } from '../../api'
import { TabError } from './SharedComponents'

interface Props {
  shares: ShareListItem[]
  pending: boolean
  error: boolean
  refetch: () => void
  onRevoke: (shareId: string) => Promise<void>
}

export default function SharesTab({ shares, pending, error, refetch, onRevoke }: Props) {
  const { t } = useTranslation()
  const [revokingId, setRevokingId] = useState<string | null>(null)

  const handleRevoke = useCallback(async (shareId: string) => {
    if (revokingId === shareId) return
    setRevokingId(shareId)
    try {
      await onRevoke(shareId)
    } finally {
      setRevokingId(null)
    }
  }, [revokingId, onRevoke])

  return (
    <div className="profile-content active" role="tabpanel">
      {pending ? (
        <div className="tab-loading">{t('common.loading')}</div>
      ) : error ? (
        <TabError onRetry={() => refetch()} />
      ) : shares.length > 0 ? (
        <div className="shares-list">
          {shares.map(s => (
            <div key={s.id} className="share-item">
              <div className="share-info">
                <span className={`share-status ${s.active ? 'active' : 'expired'}`}>
                  {s.active ? t('profile.shareActive') : t('profile.shareExpired')}
                </span>
                <span className="share-meta">
                  {t('profile.createdAt', { date: new Date(s.createdAt).toLocaleDateString() })}
                  {s.expiresAt ? ` · ${t('profile.expiresAt', { date: new Date(s.expiresAt).toLocaleDateString() })}` : ` · ${t('profile.neverExpires')}`}
                </span>
              </div>
              <button className="profile-btn-danger" onClick={() => handleRevoke(s.id)} disabled={revokingId === s.id}>
                {revokingId === s.id ? t('common.loading') : t('profile.revokeShare')}
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="profile-empty" role="status">
          <div className="empty-icon" aria-hidden="true">🔗</div>
          <div>{t('profile.noShares')}</div>
          <p style={{fontSize:'13px', color:'var(--text3)', marginTop:'8px'}}>{t('profile.shareHint')}</p>
          <Link to="/" className="empty-cta">{t('profile.goHomeCta')}</Link>
        </div>
      )}
    </div>
  )
}
