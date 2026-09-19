import { useRef } from 'react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { useModalEscape } from './useModalEscape'
import { useFocusTrap } from '../../../hooks/useFocusTrap'
import { useScrollLock } from '../../../hooks/useScrollLock'
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
  const { t } = useTranslation()

  useModalEscape(onClose)
  // 弹窗打开期间锁背景滚动（移动端防穿透）
  useScrollLock(true)
  // 焦点陷阱：打开时自动聚焦首个可聚焦元素，关闭/卸载后还原触发元素焦点
  useFocusTrap(dialogRef, true)

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
              aria-label={t('common.close')}
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
