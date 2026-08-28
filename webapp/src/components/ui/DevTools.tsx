import { memo } from 'react'
import { useDevTools } from '../../hooks/useDevTools'
import './DevTools.css'

function DevToolsImpl() {
  const { isDev, showPanel, setShowPanel, metrics, debugInfo, refreshMetrics } = useDevTools()

  if (!isDev) return null

  return (
    <>
      {/* 浮动按钮 */}
      <button
        className="dev-tools-trigger"
        onClick={() => setShowPanel(!showPanel)}
        title="开发工具 (Ctrl+Shift+D)"
        aria-label="开发工具"
      >
        🛠️
      </button>

      {/* 调试面板 */}
      {showPanel && (
        <div className="dev-tools-panel" role="dialog" aria-label="开发工具">
          <div className="dev-tools-header">
            <h3>🛠️ 开发工具</h3>
            <button type="button" className="dev-tools-close" onClick={() => setShowPanel(false)} aria-label="关闭开发工具">
              ✕
            </button>
          </div>

          <div className="dev-tools-content">
            {/* 环境信息 */}
            {debugInfo && (
              <section className="dev-tools-section">
                <h4>环境信息</h4>
                <div className="dev-tools-grid">
                  <div className="dev-tools-item">
                    <span className="label">屏幕</span>
                    <span className="value">{debugInfo.screenSize}</span>
                  </div>
                  <div className="dev-tools-item">
                    <span className="label">DPR</span>
                    <span className="value">{debugInfo.devicePixelRatio}</span>
                  </div>
                  <div className="dev-tools-item">
                    <span className="label">网络</span>
                    <span className="value">{debugInfo.connection}</span>
                  </div>
                  <div className="dev-tools-item">
                    <span className="label">内存</span>
                    <span className="value">{debugInfo.memory}</span>
                  </div>
                  <div className="dev-tools-item">
                    <span className="label">语言</span>
                    <span className="value">{debugInfo.language}</span>
                  </div>
                  <div className="dev-tools-item">
                    <span className="label">Cookie</span>
                    <span className="value">{debugInfo.cookiesEnabled ? '✅' : '❌'}</span>
                  </div>
                  <div className="dev-tools-item">
                    <span className="label">LocalStorage</span>
                    <span className="value">{debugInfo.localStorageEnabled ? '✅' : '❌'}</span>
                  </div>
                </div>
              </section>
            )}

            {/* 性能指标 */}
            <section className="dev-tools-section">
              <div className="dev-tools-section-header">
                <h4>性能指标</h4>
                <button className="dev-tools-refresh" onClick={refreshMetrics}>
                  🔄
                </button>
              </div>
              <div className="dev-tools-metrics">
                {metrics.map(metric => (
                  <div key={metric.name} className="dev-tools-metric">
                    <span className="metric-name">{metric.name}</span>
                    <span className="metric-value">
                      {metric.value} {metric.unit}
                    </span>
                  </div>
                ))}
              </div>
            </section>

            {/* 快捷操作 */}
            <section className="dev-tools-section">
              <h4>快捷操作</h4>
              <div className="dev-tools-actions">
                <button type="button" onClick={() => localStorage.clear()}>
                  清除LocalStorage
                </button>
                <button type="button" onClick={() => sessionStorage.clear()}>
                  清除SessionStorage
                </button>
                <button type="button" onClick={() => window.location.reload()}>
                  刷新页面
                </button>
                <button type="button" onClick={() => console.log('Debug Info:', debugInfo)}>
                  输出调试信息
                </button>
              </div>
            </section>
          </div>
        </div>
      )}
    </>
  )
}

export default memo(DevToolsImpl)
