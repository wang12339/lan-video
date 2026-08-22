import { describe, it, expect, afterEach } from 'vitest'
import { vi } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import ConfirmDialog, { AlertDialog } from '../components/ui/ConfirmDialog'

function renderDialog(overrides: Partial<Parameters<typeof ConfirmDialog>[0]> = {}) {
  const props = {
    open: true,
    title: '删除确认',
    message: '确定要删除这条视频吗？',
    onConfirm: vi.fn(),
    onCancel: vi.fn(),
    ...overrides,
  }
  const utils = render(<ConfirmDialog {...props} />)
  return { ...utils, props }
}

describe('ConfirmDialog', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders nothing when closed', () => {
    const { container } = renderDialog({ open: false })
    expect(container).toBeEmptyDOMElement()
  })

  it('renders title, message, and default button texts', () => {
    renderDialog()
    expect(screen.getByRole('dialog', { name: '删除确认' })).toBeInTheDocument()
    expect(screen.getByText('确定要删除这条视频吗？')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '确定' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '取消' })).toBeInTheDocument()
  })

  it('uses custom button texts and danger style', () => {
    renderDialog({ confirmText: '立即删除', cancelText: '返回', danger: true })
    const confirm = screen.getByRole('button', { name: '立即删除' })
    expect(screen.getByRole('button', { name: '返回' })).toBeInTheDocument()
    expect(confirm.className).toContain('cd-btn-danger')
  })

  it('calls onCancel when the cancel button is clicked', () => {
    vi.useFakeTimers()
    const { props } = renderDialog()
    fireEvent.click(screen.getByRole('button', { name: '取消' }))
    act(() => { vi.advanceTimersByTime(250) })
    expect(props.onCancel).toHaveBeenCalledTimes(1)
    expect(props.onConfirm).not.toHaveBeenCalled()
  })

  it('calls onCancel when the overlay is clicked', () => {
    vi.useFakeTimers()
    const { props, container } = renderDialog()
    fireEvent.click(container.querySelector('.cd-overlay') as HTMLElement)
    act(() => { vi.advanceTimersByTime(250) })
    expect(props.onCancel).toHaveBeenCalledTimes(1)
  })

  it('calls onConfirm when the confirm button is clicked', async () => {
    vi.useFakeTimers()
    const { props } = renderDialog()
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '确定' }))
    })
    act(() => { vi.advanceTimersByTime(250) })
    expect(props.onConfirm).toHaveBeenCalledTimes(1)
    expect(props.onCancel).toHaveBeenCalledTimes(1)
  })

  it('shows loading state while the async confirm is pending and blocks Esc', async () => {
    vi.useFakeTimers()
    let resolveConfirm!: () => void
    const onConfirm = vi.fn(() => new Promise<void>(r => { resolveConfirm = r }))
    const { props } = renderDialog({ onConfirm })
    fireEvent.click(screen.getByRole('button', { name: '确定' }))
    expect(screen.getByRole('button', { name: '处理中...' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '取消' })).toBeDisabled()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(props.onCancel).not.toHaveBeenCalled()
    await act(async () => { resolveConfirm() })
    act(() => { vi.advanceTimersByTime(250) })
    expect(props.onCancel).toHaveBeenCalledTimes(1)
  })

  it('calls onCancel when Esc is pressed', () => {
    vi.useFakeTimers()
    const { props } = renderDialog()
    fireEvent.keyDown(window, { key: 'Escape' })
    act(() => { vi.advanceTimersByTime(250) })
    expect(props.onCancel).toHaveBeenCalledTimes(1)
  })

  it('calls onConfirm when Enter is pressed while focus is not on a button', async () => {
    const { props } = renderDialog()
    ;(document.activeElement as HTMLElement | null)?.blur?.()
    await act(async () => {
      fireEvent.keyDown(window, { key: 'Enter' })
    })
    expect(props.onConfirm).toHaveBeenCalledTimes(1)
  })

  it('does not trigger confirm via Enter while the confirm button is focused (guard against double trigger)', async () => {
    vi.useFakeTimers()
    const { props } = renderDialog()
    act(() => { vi.advanceTimersByTime(50) })
    expect(screen.getByRole('button', { name: '确定' })).toHaveFocus()
    await act(async () => {
      fireEvent.keyDown(screen.getByRole('button', { name: '确定' }), { key: 'Enter' })
    })
    expect(props.onConfirm).not.toHaveBeenCalled()
    expect(props.onCancel).not.toHaveBeenCalled()
  })

  it('focuses the confirm button shortly after opening', () => {
    vi.useFakeTimers()
    renderDialog()
    act(() => { vi.advanceTimersByTime(50) })
    expect(screen.getByRole('button', { name: '确定' })).toHaveFocus()
  })
})

describe('AlertDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = render(<AlertDialog open={false} message="x" onClose={() => {}} />)
    expect(container).toBeEmptyDOMElement()
  })

  it('renders message with default title and ok text', () => {
    render(<AlertDialog open message="操作成功" onClose={() => {}} />)
    expect(screen.getByRole('alertdialog', { name: '提示' })).toBeInTheDocument()
    expect(screen.getByText('操作成功')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '确定' })).toBeInTheDocument()
  })

  it('closes on ok click, Esc, and Enter', () => {
    vi.useFakeTimers()
    const onClose = vi.fn()
    render(<AlertDialog open message="x" onClose={onClose} />)
    fireEvent.click(screen.getByRole('button', { name: '确定' }))
    act(() => { vi.advanceTimersByTime(250) })
    expect(onClose).toHaveBeenCalledTimes(1)
    fireEvent.keyDown(window, { key: 'Escape' })
    act(() => { vi.advanceTimersByTime(250) })
    expect(onClose).toHaveBeenCalledTimes(2)
    ;(document.activeElement as HTMLElement | null)?.blur?.()
    fireEvent.keyDown(window, { key: 'Enter' })
    act(() => { vi.advanceTimersByTime(250) })
    expect(onClose).toHaveBeenCalledTimes(3)
  })
})
