import { Component, type ReactNode } from 'react'
import i18n from '../../i18n'
import './ErrorBoundary.css'

interface Props {
  children: ReactNode
  fallback?: ReactNode
  errorTitle?: string
  errorMessage?: string
  retryText?: string
  onError?: (error: Error, errorInfo: React.ErrorInfo) => void
  maxRetries?: number
  showDetails?: boolean
}

interface State {
  hasError: boolean
  error: Error | null
  errorInfo: React.ErrorInfo | null
  retryCount: number
}

export default class ErrorBoundary extends Component<Props, State> {
  private retryTimer: ReturnType<typeof setTimeout> | null = null

  constructor(props: Props) {
    super(props)
    this.state = { 
      hasError: false, 
      error: null,
      errorInfo: null,
      retryCount: 0
    }
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('[ErrorBoundary]', error, errorInfo.componentStack)
    this.setState({ errorInfo })
    this.props.onError?.(error, errorInfo)

    // 自动重试（最多3次，间隔递增）
    const maxRetries = this.props.maxRetries ?? 3
    if (this.state.retryCount < maxRetries) {
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
      retryCount: prev.retryCount + 1
    }))
  }

  handleReload = () => {
    window.location.reload()
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback

      const maxRetries = this.props.maxRetries ?? 3
      const canRetry = this.state.retryCount < maxRetries
      const showDetails = this.props.showDetails ?? import.meta.env.DEV

      return (
        <div className="eb-error" role="alert">
          <div className="eb-icon" aria-hidden="true">⚠️</div>
          <h3 className="eb-title">
            {this.props.errorTitle || i18n.t('errors.componentError')}
          </h3>
          <p className="eb-message">
            {this.state.error?.message || this.props.errorMessage || i18n.t('errors.unknownError')}
          </p>
          
          {showDetails && this.state.errorInfo && (
            <details className="eb-details">
              <summary>{i18n.t('errors.details')}</summary>
              <pre className="eb-stack">
                {this.state.error?.stack}
                {'\n\nComponent Stack:\n'}
                {this.state.errorInfo.componentStack}
              </pre>
            </details>
          )}

          <div className="eb-actions">
            {canRetry && (
              <button className="eb-retry-btn" onClick={this.handleRetry}>
                {this.props.retryText || i18n.t('common.retry')}
                {this.state.retryCount > 0 && ` (${this.state.retryCount}/${maxRetries})`}
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

// 函数式组件的错误边界包装器
export function withErrorBoundary<P extends object>(
  Component: React.ComponentType<P>,
  errorBoundaryProps?: Omit<Props, 'children'>
) {
  return function WithErrorBoundary(props: P) {
    return (
      <ErrorBoundary {...errorBoundaryProps}>
        <Component {...props} />
      </ErrorBoundary>
    )
  }
}
