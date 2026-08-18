import { useOfflineAlert } from '../../hooks/useNetworkState'
import './OfflineAlert.css'

export default function OfflineAlert() {
  const { isOnline, showAlert, dismissAlert } = useOfflineAlert()

  if (!showAlert) return null

  return (
    <div 
      className={`offline-alert ${isOnline ? 'online' : 'offline'}`}
      role="alert"
      aria-live="assertive"
    >
      <span className="offline-alert-icon">
        {isOnline ? '✅' : '📡'}
      </span>
      <span className="offline-alert-message">
        {isOnline ? '网络已恢复' : '网络连接已断开，部分功能可能不可用'}
      </span>
      <button 
        className="offline-alert-close"
        onClick={dismissAlert}
        aria-label="关闭"
      >
        ✕
      </button>
    </div>
  )
}
