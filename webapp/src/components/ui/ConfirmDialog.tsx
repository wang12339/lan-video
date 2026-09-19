import { useState, useEffect, useRef, useId, useCallback } from 'react'
import i18n from '../../i18n'
import { useFocusTrap } from '../../hooks/useFocusTrap'
import { useScrollLock } from '../../hooks/useScrollLock'
import './ConfirmDialog.css'

type ButtonVariant = 'primary' | 'danger' | 'ghost' | 'outline'

interface CustomButton {
  text: string
  variant?: ButtonVariant
  onClick: () => void | Promise<void>
  disabled?: boolean
  loading?: boolean
}

interface ConfirmDialogProps {
  open: boolean
  title: string
  message: string
  /** 消息描述（可选，用于无障碍 aria-describedby） */
  description?: string
  confirmText?: string
  cancelText?: string
  loadingText?: string
  /** 确认按钮变体：primary(默认) | danger | ghost | outline */
  confirmVariant?: ButtonVariant
  danger?: boolean
  /** 自定义额外按钮（显示在取消按钮左侧） */
  extraButtons?: CustomButton[]
  /** 点击遮罩层是否可关闭（默认 true） */
  closeOnOverlay?: boolean
  onConfirm: () => void | Promise<void>
  /** 用户取消（取消按钮/遮罩/Esc）时调用；确认成功后不会再调用 */
  onCancel: () => void
  /** 关闭动画结束后调用（无论取消还是确认成功），用于区分「取消」与「已关闭」 */
  onClosed?: () => void
}

export default function ConfirmDialog({
  open,
  title,
  message,
  description,
  confirmText = i18n.t('common.confirm'),
  cancelText = i18n.t('common.cancel'),
  loadingText = i18n.t('common.processing'),
  confirmVariant,
  danger = false,
  extraButtons,
  closeOnOverlay = true,
  onConfirm,
  onCancel,
  onClosed,
}: ConfirmDialogProps) {
  const [loading, setLoading] = useState(false)
  const [extraLoading, setExtraLoading] = useState<Record<number, boolean>>({})
  const [closing, setClosing] = useState(false)
  const confirmRef = useRef<HTMLButtonElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const lastFocusedRef = useRef<HTMLElement | null>(null)
  const titleId = useId()
  const descId = useId()

  const onConfirmRef = useRef(onConfirm)
  const onCancelRef = useRef(onCancel)
  const onClosedRef = useRef(onClosed)
  useEffect(() => { onConfirmRef.current = onConfirm }, [onConfirm])
  useEffect(() => { onCancelRef.current = onCancel }, [onCancel])
  useEffect(() => { onClosedRef.current = onClosed }, [onClosed])

  // 关闭动画；notifyCancel=false 用于确认成功路径——此时不能调用 onCancel，
  // 否则调用方无法区分「用户取消」与「确认成功」
  const closeWithNotify = useCallback((notifyCancel: boolean) => {
    if (loading) return
    setClosing(true)
    setTimeout(() => {
      setClosing(false)
      if (notifyCancel) onCancelRef.current()
      onClosedRef.current?.()
    }, 200)
  }, [loading])

  const handleClose = useCallback(() => closeWithNotify(true), [closeWithNotify])

  const handleConfirm = useCallback(async () => {
    if (loading) return
    setLoading(true)
    try {
      await onConfirmRef.current()
      closeWithNotify(false)
    } catch (e) {
      console.error('ConfirmDialog action failed:', e)
    } finally {
      setLoading(false)
    }
  }, [loading, closeWithNotify])

  const handleExtraClick = async (index: number, btn: CustomButton) => {
    if (btn.disabled || extraLoading[index]) return
    setExtraLoading(prev => ({ ...prev, [index]: true }))
    try {
      await btn.onClick()
    } catch (e) {
      console.error('Extra button action failed:', e)
    } finally {
      setExtraLoading(prev => ({ ...prev, [index]: false }))
    }
  }

  // 计算确认按钮的最终变体
  const finalConfirmVariant: ButtonVariant = confirmVariant ?? (danger ? 'danger' : 'primary')

  // 打开时聚焦确认按钮，关闭时还原焦点到触发元素
  useEffect(() => {
    if (!open) return
    setLoading(false)
    setClosing(false)
    setExtraLoading({})
    lastFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const timer = setTimeout(() => confirmRef.current?.focus(), 50)
    return () => {
      clearTimeout(timer)
      lastFocusedRef.current?.focus()
    }
  }, [open])

  // 键盘支持：Esc 关闭、Enter 确认（按钮已聚焦时避免与原生点击重复触发）
  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      const onButton = document.activeElement instanceof HTMLButtonElement
      if (e.key === 'Escape') {
        e.preventDefault()
        if (!loading) handleClose()
      } else if (e.key === 'Enter' && !loading && !onButton) {
        e.preventDefault()
        void handleConfirm()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [open, loading, handleClose, handleConfirm])

  // 焦点陷阱 + 移动端锁背景滚动（防穿透）
  useFocusTrap(dialogRef, open, { autoFocus: false })
  useScrollLock(open)

  if (!open && !closing) return null

  const overlayClickHandler = closeOnOverlay && !loading ? handleClose : undefined

  return (
    <div
      className={`cd-overlay${closing ? ' cd-overlay-closing' : ''}`}
      onClick={overlayClickHandler}
    >
      <div
        ref={dialogRef}
        className={`cd-dialog${closing ? ' cd-dialog-closing' : ''}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descId : undefined}
        onClick={e => e.stopPropagation()}
      >
        <h3 className="cd-title" id={titleId}>{title}</h3>
        <p className="cd-message" id={description ? descId : undefined}>{message}</p>
        <div className="cd-actions">
          {extraButtons?.map((btn, i) => (
            <button
              key={i}
              type="button"
              className={`cd-btn cd-btn-${btn.variant ?? 'outline'}`}
              onClick={() => handleExtraClick(i, btn)}
              disabled={btn.disabled || extraLoading[i] || loading}
            >
              {extraLoading[i] ? (btn.loading ?? i18n.t('common.processing')) : btn.text}
            </button>
          ))}
          <button type="button" className="cd-btn cd-btn-cancel" onClick={handleClose} disabled={loading}>
            {cancelText}
          </button>
          <button
            ref={confirmRef}
            type="button"
            className={`cd-btn cd-btn-${finalConfirmVariant}`}
            onClick={handleConfirm}
            disabled={loading}
          >
            {loading ? loadingText : confirmText}
          </button>
        </div>
      </div>
    </div>
  )
}

interface AlertDialogProps {
  open: boolean
  title?: string
  message: string
  okText?: string
  /** 确认按钮变体 */
  okVariant?: ButtonVariant
  onClose: () => void
}

export function AlertDialog({ 
  open, 
  title = i18n.t('common.alertTitle'), 
  message, 
  okText = i18n.t('common.confirm'), 
  okVariant = 'primary',
  onClose 
}: AlertDialogProps) {
  const onCloseRef = useRef(onClose)
  const okRef = useRef<HTMLButtonElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)
  const titleId = useId()
  const descId = useId()
  const [closing, setClosing] = useState(false)
  useEffect(() => { onCloseRef.current = onClose }, [onClose])

  // 关闭动画
  const handleClose = useCallback(() => {
    setClosing(true)
    setTimeout(() => {
      setClosing(false)
      onCloseRef.current()
    }, 200)
  }, [])

  // 打开时重置状态
  useEffect(() => {
    if (!open) return
    setClosing(false)
  }, [open])

  // 键盘支持：Esc / Enter 关闭（按钮已聚焦时避免与原生点击重复触发）
  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        handleClose()
      } else if (e.key === 'Enter' && !(document.activeElement instanceof HTMLButtonElement)) {
        e.preventDefault()
        handleClose()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [open, handleClose])

  // 焦点陷阱 + 移动端锁背景滚动（防穿透）
  useFocusTrap(dialogRef, open)
  useScrollLock(open)

  if (!open && !closing) return null

  return (
    <div className={`cd-overlay${closing ? ' cd-overlay-closing' : ''}`} onClick={handleClose}>
      <div
        ref={dialogRef}
        className={`cd-dialog${closing ? ' cd-dialog-closing' : ''}`}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
        onClick={e => e.stopPropagation()}
      >
        <h3 className="cd-title" id={titleId}>{title}</h3>
        <p className="cd-message" id={descId}>{message}</p>
        <div className="cd-actions">
          <button ref={okRef} type="button" className={`cd-btn cd-btn-${okVariant}`} onClick={handleClose}>
            {okText}
          </button>
        </div>
      </div>
    </div>
  )
}
