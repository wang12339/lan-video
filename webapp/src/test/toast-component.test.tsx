import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import { ToastProvider, useToast } from '../components/Toast/Toast'
import '../i18n'

function Trigger({ message = '测试消息', type }: { message?: string; type?: 'success' | 'error' | 'info' }) {
  const { toast } = useToast()
  return <button onClick={() => toast(message, type)}>show</button>
}

function renderProvider(children: React.ReactNode) {
  return render(<ToastProvider>{children}</ToastProvider>)
}

describe('Toast', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('显示消息', () => {
    renderProvider(<Trigger />)
    fireEvent.click(screen.getByRole('button', { name: 'show' }))
    expect(screen.getByText('测试消息')).toBeInTheDocument()
  })

  it('自动消失', () => {
    renderProvider(<Trigger />)
    fireEvent.click(screen.getByRole('button', { name: 'show' }))
    expect(screen.getByText('测试消息')).toBeInTheDocument()
    
    act(() => { vi.advanceTimersByTime(3200) })
    const toast = screen.getByText('测试消息').closest('.toast')
    expect(toast).toHaveClass('leaving')
    
    act(() => { vi.advanceTimersByTime(260) })
    expect(screen.queryByText('测试消息')).not.toBeInTheDocument()
  })

  it('手动关闭', () => {
    renderProvider(<Trigger />)
    fireEvent.click(screen.getByRole('button', { name: 'show' }))
    fireEvent.click(screen.getByRole('button', { name: '关闭提示' }))
    
    const toast = screen.getByText('测试消息').closest('.toast')
    expect(toast).toHaveClass('leaving')
    
    act(() => { vi.advanceTimersByTime(260) })
    expect(screen.queryByText('测试消息')).not.toBeInTheDocument()
  })

  it('不同类型的 Toast 显示', () => {
    renderProvider(
      <>
        <Trigger message="成功消息" type="success" />
        <Trigger message="错误消息" type="error" />
        <Trigger message="提示消息" type="info" />
      </>
    )
    
    fireEvent.click(screen.getAllByRole('button', { name: 'show' })[0])
    fireEvent.click(screen.getAllByRole('button', { name: 'show' })[1])
    fireEvent.click(screen.getAllByRole('button', { name: 'show' })[2])
    
    const success = screen.getByText('成功消息').closest('.toast')
    const error = screen.getByText('错误消息').closest('.toast')
    const info = screen.getByText('提示消息').closest('.toast')
    
    expect(success).toHaveClass('toast-success')
    expect(success).toHaveAttribute('role', 'status')
    expect(success?.querySelector('.toast-icon')).toHaveTextContent('✓')
    
    expect(error).toHaveClass('toast-error')
    expect(error).toHaveAttribute('role', 'alert')
    expect(error?.querySelector('.toast-icon')).toHaveTextContent('✕')
    
    expect(info).toHaveClass('toast-info')
    expect(info).toHaveAttribute('role', 'status')
    expect(info?.querySelector('.toast-icon')).toHaveTextContent('ℹ')
  })
})