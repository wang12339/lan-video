import { useState, useEffect, useRef } from 'react'
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
  confirmText = '确定',
  cancelText = '取消',
  loadingText = '处理中...',
  danger = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const [loading, setLoading] = useState(false)
  const confirmRef = useRef<HTMLButtonElement>(null)

  const onConfirmRef = useRef(onConfirm)
  const onCancelRef = useRef(onCancel)
  useEffect(() => { onConfirmRef.current = onConfirm }, [onConfirm])
  useEffect(() => { onCancelRef.current = onCancel }, [onCancel])

  useEffect(() => {
    if (open) {
      setLoading(false)
      setTimeout(() => confirmRef.current?.focus(), 50)
    }
  }, [open])

  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancelRef.current()
      if (e.key === 'Enter' && !loading) handleConfirm()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [open, loading])

  const handleConfirm = async () => {
    setLoading(true)
    try {
      await onConfirmRef.current()
      onCancelRef.current()
    } finally {
      setLoading(false)
    }
  }

  if (!open) return null

  return (
    <div className="cd-overlay" onClick={onCancel}>
      <div className="cd-dialog" onClick={e => e.stopPropagation()}>
        <h3 className="cd-title">{title}</h3>
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

export function AlertDialog({ open, title = '提示', message, okText = '确定', onClose }: AlertDialogProps) {
  const onCloseRef = useRef(onClose)
  useEffect(() => { onCloseRef.current = onClose }, [onClose])

  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' || e.key === 'Enter') onCloseRef.current()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [open])

  if (!open) return null

  return (
    <div className="cd-overlay" onClick={onClose}>
      <div className="cd-dialog" onClick={e => e.stopPropagation()}>
        <h3 className="cd-title">{title}</h3>
        <p className="cd-message">{message}</p>
        <div className="cd-actions">
          <button className="cd-btn cd-btn-confirm" onClick={onClose} autoFocus>
            {okText}
          </button>
        </div>
      </div>
    </div>
  )
}
