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

describe('ToastProvider', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders the toast message after triggering', () => {
    renderProvider(<Trigger />)
    fireEvent.click(screen.getByRole('button', { name: 'show' }))
    expect(screen.getByText('测试消息')).toBeInTheDocument()
  })

  it('shows the correct type icon and role', () => {
    renderProvider(
      <>
        <Trigger message="成功" type="success" />
        <Trigger message="失败" type="error" />
        <Trigger message="提示" type="info" />
      </>
    )
    fireEvent.click(screen.getAllByRole('button', { name: 'show' })[0] as HTMLElement)
    fireEvent.click(screen.getAllByRole('button', { name: 'show' })[1] as HTMLElement)
    fireEvent.click(screen.getAllByRole('button', { name: 'show' })[2] as HTMLElement)
    const success = screen.getByText('成功').closest('.toast')
    const error = screen.getByText('失败').closest('.toast')
    const info = screen.getByText('提示').closest('.toast')
    expect(success).toHaveClass('toast-success')
    expect(success).toHaveAttribute('role', 'status')
    expect(success?.querySelector('.toast-icon')).toHaveTextContent('✓')
    expect(error).toHaveClass('toast-error')
    expect(error).toHaveAttribute('role', 'alert')
    expect(error?.querySelector('.toast-icon')).toHaveTextContent('✕')
    expect(info).toHaveClass('toast-info')
    expect(info?.querySelector('.toast-icon')).toHaveTextContent('ℹ')
  })

  it('auto-dismisses after the duration with a leaving animation', () => {
    renderProvider(<Trigger />)
    fireEvent.click(screen.getByRole('button', { name: 'show' }))
    act(() => { vi.advanceTimersByTime(3200) })
    const toast = screen.getByText('测试消息').closest('.toast')
    expect(toast).toHaveClass('leaving')
    act(() => { vi.advanceTimersByTime(260) })
    expect(screen.queryByText('测试消息')).not.toBeInTheDocument()
  })

  it('removes the toast immediately when the close button is clicked', () => {
    renderProvider(<Trigger />)
    fireEvent.click(screen.getByRole('button', { name: 'show' }))
    fireEvent.click(screen.getByRole('button', { name: '关闭提示' }))
    expect(screen.getByText('测试消息').closest('.toast')).toHaveClass('leaving')
    act(() => { vi.advanceTimersByTime(260) })
    expect(screen.queryByText('测试消息')).not.toBeInTheDocument()
  })

  it('deduplicates identical message + type and resets its timer', () => {
    renderProvider(<Trigger />)
    const btn = screen.getByRole('button', { name: 'show' })
    fireEvent.click(btn)
    act(() => { vi.advanceTimersByTime(3100) })
    fireEvent.click(btn)
    act(() => { vi.advanceTimersByTime(100) })
    expect(screen.getAllByText('测试消息')).toHaveLength(1)
    act(() => { vi.advanceTimersByTime(3200) })
    const toast = screen.getByText('测试消息').closest('.toast')
    expect(toast).toHaveClass('leaving')
  })

  it('drops the oldest toast when exceeding the 5-toast cap', () => {
    renderProvider(
      <>
        <Trigger message="消息1" />
        <Trigger message="消息2" />
        <Trigger message="消息3" />
        <Trigger message="消息4" />
        <Trigger message="消息5" />
        <Trigger message="消息6" />
      </>
    )
    const btns = screen.getAllByRole('button', { name: 'show' })
    btns.forEach(b => fireEvent.click(b))
    const toasts = document.querySelectorAll('.toast')
    expect(toasts).toHaveLength(5)
    expect(screen.queryByText('消息1')).not.toBeInTheDocument()
    expect(screen.getByText('消息6')).toBeInTheDocument()
    act(() => { vi.advanceTimersByTime(3200) })
    act(() => { vi.advanceTimersByTime(260) })
    expect(document.querySelectorAll('.toast')).toHaveLength(0)
  })

  it('pauses auto-dismiss on hover and resumes on leave', () => {
    renderProvider(<Trigger />)
    fireEvent.click(screen.getByRole('button', { name: 'show' }))
    fireEvent.mouseEnter(screen.getByText('测试消息'))
    act(() => { vi.advanceTimersByTime(10000) })
    expect(screen.getByText('测试消息')).toBeInTheDocument()
    fireEvent.mouseLeave(screen.getByText('测试消息'))
    act(() => { vi.advanceTimersByTime(3200) })
    act(() => { vi.advanceTimersByTime(260) })
    expect(screen.queryByText('测试消息')).not.toBeInTheDocument()
  })
})
