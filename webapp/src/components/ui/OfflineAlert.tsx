import { useOfflineAlert, useOfflineCapabilities } from '../../hooks/useNetworkState'
import { useTranslation } from 'react-i18next'
import './OfflineAlert.css'

export default function OfflineAlert() {
  const { t } = useTranslation()
  const { 
    isOnline, 
    showAlert, 
    dismissAlert, 
    retryConnection,
    isReconnecting,
    reconnectAttempts,
    connectionType,
    isSlowConnection,
    offlineDuration,
    syncStatus,
    wasOffline
  } = useOfflineAlert()
  
  const capabilities = useOfflineCapabilities()

  if (!showAlert) return null

  const formatDuration = (seconds: number): string => {
    if (seconds < 60) return `${seconds}${t('offline.seconds')}`
    const minutes = Math.floor(seconds / 60)
    const remainingSeconds = seconds % 60
    return `${minutes}${t('offline.minutes')} ${remainingSeconds}${t('offline.seconds')}`
  }

  return (
    <div 
      className={`offline-alert ${isOnline ? 'online' : 'offline'} ${isReconnecting ? 'reconnecting' : ''}`}
      role="alert"
      aria-live="assertive"
    >
      <div className="offline-alert-header">
        <span className="offline-alert-icon">
          {isOnline ? '✅' : '📡'}
        </span>
        <span className="offline-alert-message">
          {isOnline 
            ? (wasOffline ? t('offline.reconnected') : t('offline.online'))
            : t('offline.disconnected')
          }
        </span>
        <button 
          className="offline-alert-close"
          onClick={dismissAlert}
          aria-label={t('offline.close')}
        >
          ✕
        </button>
      </div>

      {/* Connection status details */}
      {!isOnline && (
        <div className="offline-alert-details">
          <div className="offline-detail-item">
            <span className="detail-label">{t('offline.duration')}:</span>
            <span className="detail-value">{formatDuration(offlineDuration)}</span>
          </div>
          
          {reconnectAttempts > 0 && (
            <div className="offline-detail-item">
              <span className="detail-label">{t('offline.reconnectAttempts')}:</span>
              <span className="detail-value">{reconnectAttempts}/5</span>
            </div>
          )}

          {connectionType !== 'unknown' && (
            <div className="offline-detail-item">
              <span className="detail-label">{t('offline.connectionType')}:</span>
              <span className="detail-value">{connectionType}</span>
            </div>
          )}
        </div>
      )}

      {/* Reconnection status */}
      {isReconnecting && (
        <div className="reconnecting-status">
          <div className="reconnecting-spinner"></div>
          <span>{t('offline.reconnecting')}</span>
        </div>
      )}

      {/* Slow connection warning */}
      {isOnline && isSlowConnection && (
        <div className="slow-connection-warning">
          <span className="warning-icon">⚠️</span>
          <span>{t('offline.slowConnection')}</span>
        </div>
      )}

      {/* Offline capabilities */}
      {!isOnline && (
        <div className="offline-capabilities">
          <div className="capabilities-title">{t('offline.availableOffline')}:</div>
          <div className="capabilities-list">
            {capabilities.canPlayCached && (
              <div className="capability-item">
                <span className="capability-icon">▶️</span>
                <span>{t('offline.playCachedVideos')}</span>
              </div>
            )}
            {capabilities.canViewHistory && (
              <div className="capability-item">
                <span className="capability-icon">📋</span>
                <span>{t('offline.viewHistory')}</span>
              </div>
            )}
            {capabilities.canBrowseFavorites && (
              <div className="capability-item">
                <span className="capability-icon">⭐</span>
                <span>{t('offline.browseFavorites')}</span>
              </div>
            )}
            {capabilities.canManagePlaylists && (
              <div className="capability-item">
                <span className="capability-icon">📁</span>
                <span>{t('offline.managePlaylists')}</span>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Sync status */}
      {isOnline && syncStatus.isSyncing && (
        <div className="sync-status">
          <div className="sync-spinner"></div>
          <span>{t('offline.syncing')}</span>
        </div>
      )}

      {isOnline && syncStatus.lastSyncTime && !syncStatus.isSyncing && (
        <div className="sync-status synced">
          <span className="sync-icon">✓</span>
          <span>{t('offline.synced')}</span>
        </div>
      )}

      {/* Action buttons */}
      <div className="offline-actions">
        {!isOnline && (
          <button 
            className="offline-action-btn primary"
            onClick={retryConnection}
            disabled={isReconnecting}
          >
            {isReconnecting ? t('offline.reconnecting') : t('offline.retryConnection')}
          </button>
        )}
        
        {isOnline && syncStatus.pending > 0 && (
          <button className="offline-action-btn secondary">
            {t('offline.syncNow')} ({syncStatus.pending})
          </button>
        )}
      </div>
    </div>
  )
}
