import { Component, type ReactNode } from 'react'
import i18n from '../../i18n'
import './ErrorBoundary.css'

// ─── 错误类型识别 ──────────────────────────────────────────────
type ErrorCategory = 'network' | 'chunk' | 'runtime' | 'unknown'

function classifyError(error: Error): ErrorCategory {
  const msg = error.message.toLowerCase()
  // 动态 import / 代码分割 chunk 加载失败
  if (
    msg.includes('failed to fetch dynamically imported module') ||
    msg.includes('loading chunk') ||
    msg.includes('loading css chunk') ||
    msg.includes('unexpected token') && msg.includes('<')
  ) {
    return 'chunk'
  }
  // 网络错误
  if (
    msg.includes('network error') ||
    msg.includes('networkerror') ||
    msg.includes('fetch') ||
    msg.includes('load failed') ||
    msg.includes('request failed')
  ) {
    return 'network'
  }
  return 'runtime'
}

// ─── 错误上报（动态加载 Sentry，失败时静默降级） ───────────────
async function reportError(error: Error, errorInfo: React.ErrorInfo, category: ErrorCategory) {
  // 1. 始终打 console
  console.error('[ErrorBoundary]', {
    message: error.message,
    category,
    componentStack: errorInfo.componentStack,
    stack: error.stack,
  })

  // 2. Sentry 集成（暂未安装，保留接口）
  // 如果需要 Sentry，取消注释并安装 @sentry/react
  // try {
  //   const Sentry = await import('@sentry/react')
  //   Sentry.withScope(scope => {
  //     scope.setTag('errorBoundary', true)
  //     scope.setTag('errorCategory', category)
  //     scope.setExtra('componentStack', errorInfo.componentStack)
  //     Sentry.captureException(error)
  //   })
  // } catch {
  //   // @sentry/react 未安装，静默跳过
  // }

  // 3. Navigator.sendBeacon 降级上报（页面卸载前也可靠）
  try {
    const payload = JSON.stringify({
      message: error.message,
      category,
      stack: error.stack?.slice(0, 2000),
      componentStack: errorInfo.componentStack?.slice(0, 1000),
      url: (() => {
        try {
          const u = new URL(window.location.href)
          u.searchParams.delete('token')
          u.searchParams.delete('share_token')
          u.hash = u.hash.replace(/share=[^&]*/g, 'share=[REDACTED]')
          return u.toString()
        } catch { return '[REDACTED]' }
      })(),
      ua: navigator.userAgent,
      ts: new Date().toISOString(),
    })
    if (navigator.sendBeacon) {
      navigator.sendBeacon('/api/errors', new Blob([payload], { type: 'application/json' }))
    }
  } catch {
    // 降级上报失败，静默
  }
}

// ─── Props / State ─────────────────────────────────────────────
interface Props {
  children: ReactNode
  fallback?: ReactNode
  /** 自定义标题（覆盖 i18n 默认） */
  errorTitle?: string
  /** 自定义消息（覆盖 i18n 默认） */
  errorMessage?: string
  /** 重试按钮文案 */
  retryText?: string
  /** 额外的错误回调（父组件级上报） */
  onError?: (error: Error, errorInfo: React.ErrorInfo, category: ErrorCategory) => void
  /** 最大自动重试次数，默认 3 */
  maxRetries?: number
  /** 是否默认展开错误详情，默认 DEV 模式下展开 */
  showDetails?: boolean
}

interface State {
  hasError: boolean
  error: Error | null
  errorInfo: React.ErrorInfo | null
  errorCategory: ErrorCategory
  retryCount: number
  isRetrying: boolean
}

// ─── ErrorBoundary 组件 ────────────────────────────────────────
export default class ErrorBoundary extends Component<Props, State> {
  private retryTimer: ReturnType<typeof setTimeout> | null = null

  constructor(props: Props) {
    super(props)
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
      errorCategory: 'unknown',
      retryCount: 0,
      isRetrying: false,
    }
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error, errorCategory: classifyError(error) }
  }

  async componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    const category = classifyError(error)
    this.setState({ errorInfo, errorCategory: category })

    // 错误上报（Sentry + sendBeacon + console）
    reportError(error, errorInfo, category)

    // 父组件回调
    this.props.onError?.(error, errorInfo, category)

    // 自动重试（仅 chunk 和网络错误，间隔指数递增）
    const maxRetries = this.props.maxRetries ?? 3
    const canAutoRetry = (category === 'chunk' || category === 'network') && this.state.retryCount < maxRetries
    if (canAutoRetry) {
      const delay = Math.min(1000 * Math.pow(2, this.state.retryCount), 10000)
      this.retryTimer = setTimeout(() => {
        this.handleRetry()
      }, delay)
    }
  }

  componentWillUnmount() {
    if (this.retryTimer) {
      clearTimeout(this.retryTimer)
    }
  }

  handleRetry = () => {
    this.setState(prev => ({
      hasError: false,
      error: null,
      errorInfo: null,
      retryCount: prev.retryCount + 1,
      isRetrying: false,
    }))
  }

  handleReload = () => {
    window.location.reload()
  }

  /** 根据错误类型生成建议文案 */
  private getAdvice(): string {
    switch (this.state.errorCategory) {
      case 'network':
        return i18n.t('errors.loadFailedNetwork')
      case 'chunk':
        return i18n.t('errors.loadFailedNetwork')
      default:
        return this.state.error?.message || this.props.errorMessage || i18n.t('errors.unknownError')
    }
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback

      const { maxRetries = 3, showDetails = import.meta.env.DEV } = this.props
      const { retryCount, errorCategory, errorInfo, error } = this.state
      const canRetry = retryCount < maxRetries

      // 图标映射
      const iconMap: Record<ErrorCategory, string> = {
        network: '📡',
        chunk: '📦',
        runtime: '⚠️',
        unknown: '❌',
      }

      return (
        <div className="eb-error" role="alert">
          <div className="eb-icon" aria-hidden="true">{iconMap[errorCategory]}</div>
          <h3 className="eb-title">
            {this.props.errorTitle || i18n.t('errors.componentError')}
          </h3>
          <p className="eb-message">{this.getAdvice()}</p>

          {showDetails && errorInfo && (
            <details className="eb-details">
              <summary>{i18n.t('errors.details')}</summary>
              <pre className="eb-stack">
                {error?.stack}
                {'\n\nComponent Stack:\n'}
                {errorInfo.componentStack}
              </pre>
            </details>
          )}

          <div className="eb-actions">
            {canRetry && (
              <button className="eb-retry-btn" onClick={this.handleRetry}>
                {this.props.retryText || i18n.t('common.retry')}
                {retryCount > 0 && ` (${retryCount}/${maxRetries})`}
              </button>
            )}
            <button className="eb-reload-btn" onClick={this.handleReload}>
              {i18n.t('errors.reloadPage')}
            </button>
          </div>
        </div>
      )
    }

    return this.props.children
  }
}

// ─── 高阶包装器 ────────────────────────────────────────────────
export function withErrorBoundary<P extends object>(
  WrappedComponent: React.ComponentType<P>,
  errorBoundaryProps?: Omit<Props, 'children'>
) {
  const displayName = WrappedComponent.displayName || WrappedComponent.name || 'Component'

  function WithErrorBoundary(props: P) {
    return (
      <ErrorBoundary {...errorBoundaryProps}>
        <WrappedComponent {...props} />
      </ErrorBoundary>
    )
  }

  WithErrorBoundary.displayName = `withErrorBoundary(${displayName})`
  return WithErrorBoundary
}

export type { ErrorCategory }
