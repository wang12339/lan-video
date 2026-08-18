import { useState, useEffect, useRef, useId } from 'react'
import i18n from '../../i18n'
import './ConfirmDialog.css'

interface ConfirmDialogProps {
  open: boolean
  title: string
  message: string
  confirmText?: string
  cancelText?: string
  loadingText?: string
  danger?: boolean
  onConfirm: () => void | Promise<void>
  onCancel: () => void
}

export default function ConfirmDialog({
  open,
  title,
  message,
  confirmText = i18n.t('common.confirm'),
  cancelText = i18n.t('common.cancel'),
  loadingText = i18n.t('common.processing'),
  danger = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const [loading, setLoading] = useState(false)
  const confirmRef = useRef<HTMLButtonElement>(null)
  const lastFocusedRef = useRef<HTMLElement | null>(null)
  const titleId = useId()

  const onConfirmRef = useRef(onConfirm)
  const onCancelRef = useRef(onCancel)
  useEffect(() => { onConfirmRef.current = onConfirm }, [onConfirm])
  useEffect(() => { onCancelRef.current = onCancel }, [onCancel])

  const handleConfirm = async () => {
    if (loading) return
    setLoading(true)
    try {
      await onConfirmRef.current()
      onCancelRef.current()
    } catch (e) {
      console.error('ConfirmDialog action failed:', e)
    } finally {
      setLoading(false)
    }
  }

  // 打开时聚焦确认按钮，关闭时还原焦点到触发元素
  useEffect(() => {
    if (!open) return
    setLoading(false)
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
        if (!loading) onCancelRef.current()
      } else if (e.key === 'Enter' && !loading && !onButton) {
        e.preventDefault()
        void handleConfirm()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [open, loading])

  if (!open) return null

  return (
    <div className="cd-overlay" onClick={loading ? undefined : onCancel}>
      <div
        className="cd-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onClick={e => e.stopPropagation()}
      >
        <h3 className="cd-title" id={titleId}>{title}</h3>
        <p className="cd-message">{message}</p>
        <div className="cd-actions">
          <button className="cd-btn cd-btn-cancel" onClick={onCancel} disabled={loading}>
            {cancelText}
          </button>
          <button
            ref={confirmRef}
            className={`cd-btn cd-btn-confirm ${danger ? 'cd-btn-danger' : ''}`}
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
  onClose: () => void
}

export function AlertDialog({ open, title = i18n.t('common.alertTitle'), message, okText = i18n.t('common.confirm'), onClose }: AlertDialogProps) {
  const onCloseRef = useRef(onClose)
  const okRef = useRef<HTMLButtonElement>(null)
  const lastFocusedRef = useRef<HTMLElement | null>(null)
  const titleId = useId()
  useEffect(() => { onCloseRef.current = onClose }, [onClose])

  // 打开时聚焦确定按钮，关闭时还原焦点
  useEffect(() => {
    if (!open) return
    lastFocusedRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    okRef.current?.focus()
    return () => { lastFocusedRef.current?.focus() }
  }, [open])

  // 键盘支持：Esc / Enter 关闭（按钮已聚焦时避免与原生点击重复触发）
  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        onCloseRef.current()
      } else if (e.key === 'Enter' && !(document.activeElement instanceof HTMLButtonElement)) {
        e.preventDefault()
        onCloseRef.current()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [open])

  if (!open) return null

  return (
    <div className="cd-overlay" onClick={onClose}>
      <div
        className="cd-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onClick={e => e.stopPropagation()}
      >
        <h3 className="cd-title" id={titleId}>{title}</h3>
        <p className="cd-message">{message}</p>
        <div className="cd-actions">
          <button ref={okRef} className="cd-btn cd-btn-confirm" onClick={onClose}>
            {okText}
          </button>
        </div>
      </div>
    </div>
  )
}
