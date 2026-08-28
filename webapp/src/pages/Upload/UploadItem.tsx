import { memo, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import i18n from '../../i18n'
import type { Category } from '../../config/categories'
import type { UploadItem } from './hooks/useUploadManager'
import { formatSize } from './hooks/useUploadManager'

interface Props {
  item: UploadItem
  uploading: boolean
  categories: Category[]
  onCategoryChange: (item: UploadItem, category: string) => void
  onRetry: () => void
  onRemove: () => void
}

function getStatusText(item: UploadItem, t: (key: string) => string): string {
  switch (item.status) {
    case 'pending': return t('upload.pending')
    case 'hashing': return t('upload.hashing')
    case 'uploading': return `${t('upload.uploading')} ${item.progress}%`
    case 'done': return `✅ ${t('upload.success')}`
    case 'error': return '❌ ' + (item.errorMsg || t('upload.failed'))
  }
}

function getStatusClass(item: UploadItem): string {
  if (item.status === 'done') return 'done'
  if (item.status === 'error') return 'error'
  return ''
}

const UploadItemRow = memo(function UploadItemRow({
  item, uploading, categories, onCategoryChange, onRetry, onRemove,
}: Props) {
  const { t } = useTranslation()
  const navigate = useNavigate()

  const catEditable = !uploading && (item.status === 'pending' || item.status === 'error')

  const handleCatChange = useCallback((e: React.ChangeEvent<HTMLSelectElement>) => {
    onCategoryChange(item, e.target.value)
  }, [onCategoryChange, item])

  const handleView = useCallback(() => {
    if (item.videoId !== undefined) navigate(`/player?id=${item.videoId}`)
  }, [navigate, item.videoId])

  return (
    <div className={`upload-item ${item.status === 'done' ? 'is-done' : ''} ${item.status === 'error' ? 'is-error' : ''} ${item.status === 'uploading' ? 'is-uploading' : ''} ${item.status === 'hashing' ? 'is-hashing' : ''}`}>
      <div className="upload-item-icon">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
          <polygon points="23 7 16 12 23 17 23 7" />
          <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
        </svg>
      </div>
      <div className="upload-item-info">
        <div className="upload-item-top">
          <div className="upload-item-name">{item.name}</div>
          <select
            className="upload-item-cat"
            value={item.category}
            aria-label={i18n.t('upload.categoryAria', { name: item.name })}
            disabled={!catEditable}
            onChange={handleCatChange}
          >
            {categories.map((cat) => (
              <option key={cat.key} value={cat.key}>{i18n.t(cat.i18nKey)}</option>
            ))}
          </select>
        </div>
        <div className="upload-item-size">{formatSize(item.size)}</div>
        <div className="upload-item-status">
          <div
            className="upload-progress-bar"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={item.progress}
            aria-label={i18n.t('upload.progressAria', { name: item.name })}
          >
            <div className="upload-progress-fill" style={{ width: item.progress + '%' }} />
          </div>
          <span className={`status-text ${getStatusClass(item)}`}>
            {getStatusText(item, t)}
          </span>
        </div>
      </div>
      <div className="upload-item-actions">
        {item.status === 'done' && item.videoId !== undefined && (
          <button
            className="upload-item-view"
            title={i18n.t('upload.goToPlayer')}
            onClick={handleView}
          >
            {i18n.t('upload.view')}
          </button>
        )}
        {item.status === 'error' && !uploading && (
          <button
            className="upload-item-resume"
            title={i18n.t('upload.retryTitle')}
            onClick={onRetry}
          >
            {i18n.t('common.retry')}
          </button>
        )}
        {(item.status === 'pending' || item.status === 'error') && (
          <button
            className="upload-item-remove"
            onClick={onRemove}
            disabled={uploading}
            title={i18n.t('upload.remove')}
            aria-label={i18n.t('upload.removeAria', { name: item.name })}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        )}
      </div>
    </div>
  )
})

export default UploadItemRow
