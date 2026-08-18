import { usePWA } from '../../hooks/usePWA'
import './PWAInstallPrompt.css'

export default function PWAInstallPrompt() {
  const { isInstallable, install } = usePWA()

  if (!isInstallable) return null

  return (
    <div className="pwa-install-banner" role="alert">
      <div className="pwa-install-content">
        <span className="pwa-install-icon">📱</span>
        <div className="pwa-install-text">
          <strong>安装 Atmos</strong>
          <span>添加到主屏幕，获得更好的体验</span>
        </div>
      </div>
      <div className="pwa-install-actions">
        <button className="pwa-install-btn" onClick={install}>
          安装
        </button>
      </div>
    </div>
  )
}
