import { memo } from 'react'
import { useTranslation } from 'react-i18next'
import type { LogEntry as LogEntryType } from '../../../api/logs'
import { TYPE_STYLES, TYPE_ICONS } from '../utils/logFormatter'

interface LogEntryProps {
  entry: LogEntryType
  formatted: { desc: string; type: string }
  nodeKey: string
  isExpanded: boolean
  onToggle: (key: string) => void
}

function LogEntry({ entry, formatted, nodeKey, isExpanded, onToggle }: LogEntryProps) {
  const { t } = useTranslation()
  const style = TYPE_STYLES[formatted.type] || { color: '#6b7280', labelKey: 'admin.logs.types.default' }

  return (
    <div
      className={`a-route-node ${isExpanded ? 'expanded' : ''}`}
      style={{ '--node-color': style.color } as React.CSSProperties}
      onClick={() => onToggle(nodeKey)}
    >
      <div className="a-node-dot">{TYPE_ICONS[formatted.type] || '·'}</div>
      <div className="a-node-content">
        <span className="a-node-desc">{formatted.desc}</span>
        {entry.video_id && <span className="a-node-vid">{t('admin.logs.videoId', { id: entry.video_id })}</span>}
      </div>
      {isExpanded && (
        <div className="a-node-details">
          {entry.path && (
            <div className="a-node-detail">
              <span className="a-detail-key">{t('admin.logs.path')}</span>
              <span className="a-detail-value">{entry.method} {entry.path}</span>
            </div>
          )}
          {entry.status && (
            <div className="a-node-detail">
              <span className="a-detail-key">{t('admin.logs.status')}</span>
              <span className="a-detail-value">{entry.status}</span>
            </div>
          )}
          {entry.duration_ms && (
            <div className="a-node-detail">
              <span className="a-detail-key">{t('admin.logs.duration')}</span>
              <span className="a-detail-value">{entry.duration_ms}ms</span>
            </div>
          )}
          {entry.request_id && (
            <div className="a-node-detail">
              <span className="a-detail-key">{t('admin.logs.requestId')}</span>
              <span className="a-detail-value">{entry.request_id.slice(0, 8)}</span>
            </div>
          )}
          {entry.error && (
            <div className="a-node-detail a-detail-error">
              <span className="a-detail-key">{t('admin.logs.error')}</span>
              <span className="a-detail-value">{entry.error}</span>
            </div>
          )}
        </div>
      )}
      <svg className="a-node-arrow" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ transform: isExpanded ? 'rotate(180deg)' : 'none' }}>
        <polyline points="6 9 12 15 18 9"/>
      </svg>
    </div>
  )
}

export default memo(LogEntry)
