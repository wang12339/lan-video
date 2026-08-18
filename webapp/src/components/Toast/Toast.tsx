import { useState, useEffect, useCallback, useMemo, createContext, useContext, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import './Toast.css'

type ToastType = 'success' | 'error' | 'warning' | 'info'

interface Toast {
  id: number
  message: string
  type: ToastType
  leaving: boolean
}

interface ToastContextValue {
  toast: (message: string, type?: ToastType) => void
}

const ToastContext = createContext<ToastContextValue>({ toast: () => {} })

export const useToast = () => useContext(ToastContext)

const TOAST_DURATION = 3200
const LEAVE_MS = 250
const MAX_TOASTS = 5

let nextId = 0

const TOAST_ICONS: Record<ToastType, string> = {
  success: '✓',
  error: '✕',
  warning: '!',
  info: 'ℹ',
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation()
  const [toasts, setToasts] = useState<Toast[]>([])
  const timersRef = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map())
  const toastsRef = useRef<Toast[]>([])

  useEffect(() => {
    toastsRef.current = toasts
  }, [toasts])

  const clearTimer = useCallback((id: number) => {
    const timer = timersRef.current.get(id)
    if (timer) {
      clearTimeout(timer)
      timersRef.current.delete(id)
    }
  }, [])

  const removeToast = useCallback((id: number) => {
    const target = toastsRef.current.find(x => x.id === id)
    if (!target) return
    setToasts(prev => {
      if (!prev.some(x => x.id === id)) return prev
      if (target.leaving) return prev.filter(x => x.id !== id)
      return prev.map(x => (x.id === id ? { ...x, leaving: true } : x))
    })
    clearTimer(id)
    if (!target.leaving) {
      timersRef.current.set(id, setTimeout(() => removeToast(id), LEAVE_MS))
    }
  }, [clearTimer])

  const scheduleDismiss = useCallback((id: number, delay: number) => {
    clearTimer(id)
    timersRef.current.set(id, setTimeout(() => removeToast(id), delay))
  }, [clearTimer, removeToast])

  const toast = useCallback((message: string, type: ToastType = 'info') => {
    const id = nextId++
    const list = toastsRef.current
    const dup = list.find(x => x.message === message && x.type === type && !x.leaving)
    if (dup) clearTimer(dup.id)
    let next = dup ? list.filter(x => x.id !== dup.id) : list
    next = [...next, { id, message, type, leaving: false }]
    if (next.length > MAX_TOASTS) {
      const oldest = next[0]
      if (oldest && !oldest.leaving) {
        next = next.slice(1)
        clearTimer(oldest.id)
        scheduleDismiss(oldest.id, LEAVE_MS)
      }
    }
    setToasts(next)
    scheduleDismiss(id, TOAST_DURATION)
  }, [clearTimer, scheduleDismiss])

  const pauseToast = useCallback((id: number) => {
    const target = toastsRef.current.find(x => x.id === id)
    if (!target || target.leaving) return
    clearTimer(id)
  }, [clearTimer])

  const resumeToast = useCallback((id: number) => {
    const target = toastsRef.current.find(x => x.id === id)
    if (!target || target.leaving) return
    scheduleDismiss(id, TOAST_DURATION)
  }, [scheduleDismiss])

  useEffect(() => {
    return () => {
      timersRef.current.forEach(timer => clearTimeout(timer))
      timersRef.current.clear()
    }
  }, [])

  const ctx = useMemo(() => ({ toast }), [toast])

  return (
    <ToastContext.Provider value={ctx}>
      {children}
      <div className="toast-container" aria-live="polite">
        {toasts.map(item => (
          <div
            key={item.id}
            className={`toast toast-${item.type}${item.leaving ? ' leaving' : ''}`}
            role={item.type === 'error' ? 'alert' : 'status'}
            onClick={() => removeToast(item.id)}
            onMouseEnter={() => pauseToast(item.id)}
            onMouseLeave={() => resumeToast(item.id)}
          >
            <span className="toast-icon" aria-hidden="true">{TOAST_ICONS[item.type]}</span>
            <span className="toast-message">{item.message}</span>
            <button
              type="button"
              className="toast-close"
              aria-label={t('toast.close')}
              onClick={(e) => {
                e.stopPropagation()
                removeToast(item.id)
              }}
            >
              ✕
            </button>
            {!item.leaving && (
              <span
                className="toast-progress"
                style={{ animationDuration: `${TOAST_DURATION}ms` }}
              />
            )}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}
