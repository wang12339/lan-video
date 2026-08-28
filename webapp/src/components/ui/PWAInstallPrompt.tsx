import { useState, useEffect, useCallback, memo } from 'react'
import { usePWA } from '../../hooks/usePWA'
import './PWAInstallPrompt.css'

// localStorage key，用于记住用户关闭提示
const DISMISSED_KEY = 'pwa-install-dismissed'
// 提示延迟显示时间（毫秒），避免首次加载就弹出
const SHOW_DELAY = 3000

// 检测是否为 iOS 设备
function isIOS(): boolean {
  return /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
}

// 检测是否在独立模式（已安装）运行
function isStandalone(): boolean {
  return window.matchMedia('(display-mode: standalone)').matches ||
    (window.navigator as unknown as Record<string, boolean>).standalone === true
}

function PWAInstallPromptImpl() {
  const { isInstallable, isInstalled, install } = usePWA()
  const [isVisible, setIsVisible] = useState(false)
  const [isIOSDevice] = useState(isIOS)
  const [dismissed, setDismissed] = useState(() => {
    try {
      return localStorage.getItem(DISMISSED_KEY) === 'true'
    } catch {
      return false
    }
  })

  // 延迟显示提示，避免首次加载就弹出
  useEffect(() => {
    if (isInstalled || isStandalone() || dismissed || isIOSDevice) {
      return
    }

    const timer = setTimeout(() => {
      setIsVisible(true)
    }, SHOW_DELAY)

    return () => clearTimeout(timer)
  }, [isInstalled, dismissed, isIOSDevice])

  // 安装后自动隐藏
  useEffect(() => {
    if (isInstalled || isStandalone()) {
      setIsVisible(false)
    }
  }, [isInstalled])

  const handleDismiss = useCallback(() => {
    setIsVisible(false)
    setDismissed(true)
    try {
      localStorage.setItem(DISMISSED_KEY, 'true')
    } catch {
      // localStorage 不可用时静默失败
    }
  }, [])

  // 安装处理
  const handleInstall = useCallback(async () => {
    const success = await install()
    if (success) {
      setIsVisible(false)
    }
  }, [install])

  // Android/Desktop: 显示标准安装提示
  if (isVisible && isInstallable && !isIOSDevice) {
    return (
      <div className="pwa-install-banner" role="dialog" aria-label="安装应用提示">
        <div className="pwa-install-content">
          <div className="pwa-install-icon">📱</div>
          <div className="pwa-install-text">
            <strong>安装 Atmos</strong>
            <span>添加到主屏幕，获得更好的体验</span>
          </div>
        </div>
        <div className="pwa-install-actions">
          <button className="pwa-install-btn" onClick={handleInstall}>
            安装
          </button>
          <button
            className="pwa-install-close"
            onClick={handleDismiss}
            aria-label="关闭提示"
          >
            ✕
          </button>
        </div>
      </div>
    )
  }

  // iOS: 显示 Safari 添加到主屏幕的引导
  if (isIOSDevice && !isStandalone() && !dismissed) {
    return (
      <div className="pwa-install-banner pwa-install-ios" role="dialog" aria-label="iOS安装引导">
        <div className="pwa-install-content">
          <div className="pwa-install-icon">🍎</div>
          <div className="pwa-install-text">
            <strong>添加到主屏幕</strong>
            <span>在 Safari 中点击分享按钮</span>
          </div>
        </div>
        <div className="pwa-install-actions">
          <button
            className="pwa-install-close"
            onClick={handleDismiss}
            aria-label="关闭提示"
          >
            ✕
          </button>
        </div>
        {/* iOS 操作指引 */}
        <div className="pwa-ios-guide">
          <div className="pwa-ios-step">
            <span className="pwa-ios-step-number">1</span>
            <span>点击底部 <strong>分享按钮</strong> □↑</span>
          </div>
          <div className="pwa-ios-step">
            <span className="pwa-ios-step-number">2</span>
            <span>选择 <strong>添加到主屏幕</strong></span>
          </div>
        </div>
      </div>
    )
  }

  return null
}

export default memo(PWAInstallPromptImpl)
