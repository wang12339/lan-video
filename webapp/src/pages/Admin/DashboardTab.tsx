import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { getStats, getSystemInfo } from '../../api/admin'
import type { AdminStats, SystemInfo } from '../../api/admin'
import { formatDuration } from '../../api/utils'
import { SkeletonLoader } from '../../components/ui'

export default function DashboardTab() {
  const { t } = useTranslation()
  const { data: stats, isLoading: statsLoading, error: statsError } = useQuery<AdminStats>({
    queryKey: ['admin-stats'],
    queryFn: getStats,
  })
  const { data: sys } = useQuery<SystemInfo>({
    queryKey: ['admin-system-info'],
    queryFn: getSystemInfo,
  })

  if (statsLoading) return <SkeletonLoader type="stats" />
  if (statsError || !stats) return <div className="admin-error">{t('admin.stats.loadFailed')}</div>

  return (
    <div className="admin-tab-content">
      <div className="admin-stats-grid">
        <div className="admin-stat-card">
          <div className="admin-stat-value">{stats.videoCount}</div>
          <div className="admin-stat-label">{t('admin.stats.videos')}</div>
        </div>
        <div className="admin-stat-card">
          <div className="admin-stat-value">{stats.imageCount}</div>
          <div className="admin-stat-label">{t('admin.stats.images')}</div>
        </div>
        <div className="admin-stat-card">
          <div className="admin-stat-value">{stats.totalViews.toLocaleString()}</div>
          <div className="admin-stat-label">{t('admin.stats.totalViews')}</div>
        </div>
        <div className="admin-stat-card">
          <div className="admin-stat-value">{stats.userCount}</div>
          <div className="admin-stat-label">{t('admin.stats.users')}</div>
        </div>
        {stats.pendingCount > 0 && (
          <div className="admin-stat-card admin-stat-card-warn">
            <div className="admin-stat-value">{stats.pendingCount}</div>
            <div className="admin-stat-label">{t('admin.stats.pending')}</div>
          </div>
        )}
        <div className="admin-stat-card">
          <div className="admin-stat-value">{formatDuration(stats.totalDurationSecs, '--:--')}</div>
          <div className="admin-stat-label">{t('admin.stats.totalDuration')}</div>
        </div>
        <div className="admin-stat-card">
          <div className="admin-stat-value">{sys?.mediaSizeHuman || '--'}</div>
          <div className="admin-stat-label">{t('admin.stats.storage')}</div>
        </div>
      </div>

      {stats.byCategory.length > 0 && (
        <div className="admin-section">
          <h3 className="admin-section-title">{t('admin.stats.byCategory')}</h3>
          <div className="admin-card">
            {stats.byCategory.map(({ category, count }) => (
              <div key={category} className="admin-bar-row">
                <span className="admin-bar-label">{category}</span>
                <div className="admin-bar-track">
                  <div
                    className="admin-bar-fill"
                    style={{ width: `${(count / stats.totalVideos) * 100}%` }}
                  />
                </div>
                <span className="admin-bar-count">{count}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
