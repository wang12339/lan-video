import { Component, type ReactNode } from 'react'
import './ErrorBoundary.css'

interface Props {
  children: ReactNode
  fallback?: ReactNode
  errorTitle?: string
  errorMessage?: string
  retryText?: string
}

interface State {
  hasError: boolean
  error: Error | null
}

export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Admin component error:', error, errorInfo)
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null })
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback

      return (
        <div className="eb-error">
          <div className="eb-icon">⚠️</div>
          <h3 className="eb-title">{this.props.errorTitle || '组件加载失败'}</h3>
          <p className="eb-message">
            {this.state.error?.message || this.props.errorMessage || '发生未知错误'}
          </p>
          <button className="eb-retry-btn" onClick={this.handleRetry}>
            {this.props.retryText || '重试'}
          </button>
        </div>
      )
    }

    return this.props.children
  }
}
