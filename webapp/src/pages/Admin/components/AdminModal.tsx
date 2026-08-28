import { useRef, useEffect } from 'react'
import type { ReactNode } from 'react'
import { useModalEscape } from './useModalEscape'
import './AdminModal.css'

interface AdminModalProps {
  title: string
  onClose: () => void
  children: ReactNode
  actions?: ReactNode
  maxWidth?: number
  /** When true, no close button in header (e.g. for forms that handle their own close) */
  hideCloseButton?: boolean
}

export default function AdminModal({
  title,
  onClose,
  children,
  actions,
  maxWidth,
  hideCloseButton = false,
}: AdminModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null)

  useModalEscape(onClose)

  useEffect(() => {
    const el = dialogRef.current
    if (!el) return
    const focusable = el.querySelector<HTMLElement>(
      'input:not([disabled]):not([type="hidden"]), textarea:not([disabled]), select:not([disabled]), button:not([disabled])',
    )
    focusable?.focus()
  }, [])

  return (
    <div className="admin-modal-overlay" onClick={onClose}>
      <div
        ref={dialogRef}
        className="admin-modal"
        style={maxWidth ? { maxWidth } : undefined}
        onClick={e => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="admin-modal-header">
          <h3>{title}</h3>
          {!hideCloseButton && (
            <button
              type="button"
              className="admin-modal-close"
              onClick={onClose}
              aria-label="Close"
            >
              ×
            </button>
          )}
        </div>
        <div className="admin-modal-body">
          {children}
        </div>
        {actions && (
          <div className="admin-modal-actions">
            {actions}
          </div>
        )}
      </div>
    </div>
  )
}
