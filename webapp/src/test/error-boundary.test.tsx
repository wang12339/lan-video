import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import ErrorBoundary from '../components/ui/ErrorBoundary'

function Bomb({ shouldThrow = true }: { shouldThrow?: boolean }) {
  if (shouldThrow) throw new Error('爆炸啦')
  return <div>正常内容</div>
}

describe('ErrorBoundary', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('shows the default fallback when a child throws', () => {
    const onError = vi.fn()
    render(<ErrorBoundary onError={onError}><Bomb /></ErrorBoundary>)
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.getByText('组件加载失败')).toBeInTheDocument()
    expect(screen.getByText('爆炸啦')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '重试' })).toBeInTheDocument()
    expect(onError).toHaveBeenCalledTimes(1)
    const [err] = onError.mock.calls[0] as [Error]
    expect(err.message).toBe('爆炸啦')
  })

  it('falls back to errorMessage when the error has no message', () => {
    function SilentBomb(): never { throw new Error() }
    render(<ErrorBoundary errorMessage="自定义错误信息"><SilentBomb /></ErrorBoundary>)
    expect(screen.getByText('自定义错误信息')).toBeInTheDocument()
  })

  it('renders custom fallback instead of the built-in UI', () => {
    render(<ErrorBoundary fallback={<div>自定义降级</div>}><Bomb /></ErrorBoundary>)
    expect(screen.getByText('自定义降级')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('uses custom title and retry text', () => {
    render(<ErrorBoundary errorTitle="出错了" retryText="再来"><Bomb /></ErrorBoundary>)
    expect(screen.getByText('出错了')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '再来' })).toBeInTheDocument()
  })

  it('recovers after clicking retry once the child stops throwing', () => {
    const { rerender } = render(<ErrorBoundary><Bomb shouldThrow /></ErrorBoundary>)
    expect(screen.getByRole('alert')).toBeInTheDocument()
    rerender(<ErrorBoundary><Bomb shouldThrow={false} /></ErrorBoundary>)
    fireEvent.click(screen.getByRole('button', { name: '重试' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByText('正常内容')).toBeInTheDocument()
  })

  it('renders children normally when nothing throws', () => {
    render(<ErrorBoundary><div>平安无事</div></ErrorBoundary>)
    expect(screen.getByText('平安无事')).toBeInTheDocument()
  })
})
