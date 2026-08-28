import { useState, useEffect, useCallback, useRef, useMemo, memo } from 'react'
import { useTranslation } from 'react-i18next'
import './SleepTimer.css'

interface SleepTimerProps {
  /** Called when timer expires */
  onExpire: () => void
  /** Called when timer is cancelled */
  onCancel?: () => void
}

interface TimerOption {
  label: string
  minutes: number
}

interface SavedTimer {
  endTime: number
  pausedRemaining?: number
  minutes: number
}

const STORAGE_KEY = 'atmos_sleep_timer'

function getSavedTimer(): SavedTimer | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    const data = JSON.parse(raw) as SavedTimer
    // If paused, restore with pausedRemaining
    if (data.pausedRemaining != null) {
      return data
    }
    if (data.endTime > Date.now()) {
      return data
    }
    localStorage.removeItem(STORAGE_KEY)
    return null
  } catch {
    return null
  }
}

function formatCountdown(totalSeconds: number): string {
  const h = Math.floor(totalSeconds / 3600)
  const m = Math.floor((totalSeconds % 3600) / 60)
  const s = totalSeconds % 60
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
  }
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
}

/** SVG circular progress ring */
function ProgressRing({ ratio }: { ratio: number }) {
  const radius = 16
  const circumference = 2 * Math.PI * radius
  const offset = circumference * (1 - Math.max(0, Math.min(1, ratio)))
  return (
    <svg className="sleep-timer-ring" width="40" height="40" viewBox="0 0 40 40">
      <circle
        className="sleep-timer-ring-bg"
        cx="20"
        cy="20"
        r={radius}
        fill="none"
        strokeWidth="3"
      />
      <circle
        className="sleep-timer-ring-fg"
        cx="20"
        cy="20"
        r={radius}
        fill="none"
        strokeWidth="3"
        strokeDasharray={circumference}
        strokeDashoffset={offset}
        strokeLinecap="round"
        transform="rotate(-90 20 20)"
      />
    </svg>
  )
}

function SleepTimerImpl({ onExpire, onCancel }: SleepTimerProps) {
  const { t } = useTranslation()
  const [dropdownVisible, setDropdownVisible] = useState(false)
  const [activeMinutes, setActiveMinutes] = useState<number | null>(null)
  const [remainingSeconds, setRemainingSeconds] = useState(0)
  const [totalSeconds, setTotalSeconds] = useState(0)
  const [isPaused, setIsPaused] = useState(false)
  const [showToast, setShowToast] = useState(false)
  const [toastMessage, setToastMessage] = useState('')
  const [showCustom, setShowCustom] = useState(false)
  const [customValue, setCustomValue] = useState('')
  const [customUnit, setCustomUnit] = useState<'min' | 'hour'>('min')
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const customInputRef = useRef<HTMLInputElement>(null)

  const options: TimerOption[] = [
    { label: t('timer.15min'), minutes: 15 },
    { label: t('timer.30min'), minutes: 30 },
    { label: t('timer.45min'), minutes: 45 },
    { label: t('timer.1hour'), minutes: 60 },
    { label: t('timer.2hours'), minutes: 120 },
  ]

  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const showNotification = useCallback(
    (message: string) => {
      // Toast
      setToastMessage(message)
      setShowToast(true)
      setTimeout(() => setShowToast(false), 4000)

      // Browser notification (if permitted)
      if (typeof Notification !== 'undefined' && Notification.permission === 'granted') {
        new Notification(t('timer.title'), {
          body: message,
          icon: '/webapp/favicon.ico',
          tag: 'sleep-timer-expire',
        })
      }
    },
    [t],
  )

  const handleExpire = useCallback(() => {
    clearTimer()
    localStorage.removeItem(STORAGE_KEY)
    setActiveMinutes(null)
    setRemainingSeconds(0)
    setTotalSeconds(0)
    setIsPaused(false)
    showNotification(t('timer.expired'))
    onExpire()
  }, [clearTimer, onExpire, showNotification, t])

  const startCountdown = useCallback(
    (seconds: number) => {
      clearTimer()
      setRemainingSeconds(seconds)
      setIsPaused(false)

      timerRef.current = setInterval(() => {
        setRemainingSeconds((prev) => {
          if (prev <= 1) {
            handleExpire()
            return 0
          }
          return prev - 1
        })
      }, 1000)
    },
    [clearTimer, handleExpire],
  )

  const startCountdownRef = useRef(startCountdown)
  startCountdownRef.current = startCountdown

  // Restore saved timer on mount
  useEffect(() => {
    const saved = getSavedTimer()
    if (saved) {
      setActiveMinutes(saved.minutes)
      if (saved.pausedRemaining != null) {
        setRemainingSeconds(saved.pausedRemaining)
        setTotalSeconds(saved.minutes * 60)
        setIsPaused(true)
      } else {
        const remaining = Math.max(0, Math.floor((saved.endTime - Date.now()) / 1000))
        setTotalSeconds(saved.minutes * 60)
        startCountdownRef.current(remaining)
      }
    }

    if (typeof Notification !== 'undefined' && Notification.permission === 'default') {
      Notification.requestPermission()
    }
  }, [])

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownVisible(false)
        setShowCustom(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const handleSelect = (minutes: number) => {
    const seconds = minutes * 60
    const endTime = Date.now() + seconds * 1000

    localStorage.setItem(STORAGE_KEY, JSON.stringify({ endTime, minutes } satisfies SavedTimer))

    setActiveMinutes(minutes)
    setTotalSeconds(seconds)
    setDropdownVisible(false)
    setShowCustom(false)
    setToastMessage(
      `${t('timer.set')} ${minutes >= 60 ? `${Math.floor(minutes / 60)}${t('timer.hourUnit')}${minutes % 60 > 0 ? ` ${minutes % 60}${t('timer.minUnit')}` : ''}` : `${minutes}${t('timer.minUnit')}`} ${t('timer.after')}`,
    )
    setShowToast(true)
    setTimeout(() => setShowToast(false), 3000)

    startCountdown(seconds)
  }

  const handleCustomSubmit = () => {
    const num = parseFloat(customValue)
    if (isNaN(num) || num <= 0) return

    const minutes = customUnit === 'hour' ? Math.round(num * 60) : Math.round(num)
    if (minutes < 1) return
    // Cap at 24 hours
    const capped = Math.min(minutes, 24 * 60)
    handleSelect(capped)
    setCustomValue('')
  }

  const handlePause = () => {
    if (!isPaused) {
      // Pause
      clearTimer()
      setIsPaused(true)
      const saved: SavedTimer = { endTime: 0, minutes: activeMinutes ?? 0, pausedRemaining: remainingSeconds }
      localStorage.setItem(STORAGE_KEY, JSON.stringify(saved))
    } else {
      // Resume
      const endTime = Date.now() + remainingSeconds * 1000
      const saved: SavedTimer = { endTime, minutes: activeMinutes ?? 0 }
      localStorage.setItem(STORAGE_KEY, JSON.stringify(saved))
      startCountdown(remainingSeconds)
    }
  }

  const handleCancel = () => {
    clearTimer()
    localStorage.removeItem(STORAGE_KEY)
    setActiveMinutes(null)
    setRemainingSeconds(0)
    setTotalSeconds(0)
    setIsPaused(false)
    setDropdownVisible(false)
    setShowCustom(false)
    onCancel?.()
  }

  const ratio = useMemo(
    () => (totalSeconds > 0 ? remainingSeconds / totalSeconds : 0),
    [remainingSeconds, totalSeconds],
  )

  // Cleanup on unmount
  useEffect(() => {
    return () => clearTimer()
  }, [clearTimer])

  const isActive = activeMinutes != null && remainingSeconds > 0
  const isEnding = isActive && remainingSeconds <= 60 // Last minute: pulse

  return (
    <>
      <div className="sleep-timer" ref={dropdownRef}>
        <button
          className={`sleep-timer-trigger ${isActive ? 'active' : ''} ${isEnding ? 'ending' : ''}`}
          onClick={() => setDropdownVisible(!dropdownVisible)}
          aria-label={t('timer.title')}
        >
          {isActive ? (
            <>
              <ProgressRing ratio={ratio} />
              <span className="sleep-timer-countdown">{formatCountdown(remainingSeconds)}</span>
              {isPaused && <span className="sleep-timer-paused-badge">{t('timer.paused')}</span>}
            </>
          ) : (
            <>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                <circle cx="12" cy="12" r="10" />
                <polyline points="12,6 12,12 16,14" />
              </svg>
              <span>{t('timer.title')}</span>
            </>
          )}
        </button>

        <div className={`sleep-timer-dropdown ${dropdownVisible ? 'visible' : ''}`}>
          <div className="sleep-timer-header">{t('timer.selectDuration')}</div>

          {/* Countdown display when active */}
          {isActive && (
            <div className="sleep-timer-current">
              <ProgressRing ratio={ratio} />
              <div className="sleep-timer-current-info">
                <span className="sleep-timer-current-time">{formatCountdown(remainingSeconds)}</span>
                <span className="sleep-timer-current-label">
                  {isPaused ? t('timer.paused') : t('timer.remaining')}
                </span>
              </div>
            </div>
          )}

          <div className="sleep-timer-options">
            {options.map((option) => (
              <button
                key={option.minutes}
                type="button"
                className={`sleep-timer-option ${activeMinutes === option.minutes ? 'active' : ''}`}
                onClick={() => handleSelect(option.minutes)}
              >
                <span className="sleep-timer-option-label">{option.label}</span>
                <svg
                  className="sleep-timer-option-icon"
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  aria-hidden="true"
                >
                  <polyline points="20,6 9,17 4,12" />
                </svg>
              </button>
            ))}
          </div>

          {/* Custom time */}
          <div className="sleep-timer-custom">
            {showCustom ? (
              <div className="sleep-timer-custom-form">
                <div className="sleep-timer-custom-row">
                  <input
                    ref={customInputRef}
                    type="number"
                    className="sleep-timer-custom-input"
                    min="1"
                    step={customUnit === 'hour' ? '0.5' : '1'}
                    placeholder={customUnit === 'hour' ? '0.5' : '30'}
                    value={customValue}
                    onChange={(e) => setCustomValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleCustomSubmit()
                      if (e.key === 'Escape') setShowCustom(false)
                    }}
                    autoFocus
                  />
                  <div className="sleep-timer-unit-toggle">
                    <button
                      className={`sleep-timer-unit-btn ${customUnit === 'min' ? 'active' : ''}`}
                      onClick={() => setCustomUnit('min')}
                    >
                      {t('timer.minUnit')}
                    </button>
                    <button
                      className={`sleep-timer-unit-btn ${customUnit === 'hour' ? 'active' : ''}`}
                      onClick={() => setCustomUnit('hour')}
                    >
                      {t('timer.hourUnit')}
                    </button>
                  </div>
                </div>
                <div className="sleep-timer-custom-actions">
                  <button type="button" className="sleep-timer-custom-cancel" onClick={() => setShowCustom(false)}>
                    {t('common.cancel')}
                  </button>
                  <button
                    type="button"
                    className="sleep-timer-custom-confirm"
                    onClick={handleCustomSubmit}
                    disabled={!customValue || parseFloat(customValue) <= 0}
                  >
                    {t('common.confirm')}
                  </button>
                </div>
              </div>
            ) : (
              <button type="button" className="sleep-timer-custom-trigger" onClick={() => setShowCustom(true)}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                  <line x1="12" y1="5" x2="12" y2="19" />
                  <line x1="5" y1="12" x2="19" y2="12" />
                </svg>
                {t('timer.customTime')}
              </button>
            )}
          </div>

          {/* Footer: pause / resume + cancel */}
          {activeMinutes != null && (
            <div className="sleep-timer-footer">
              <button type="button" className="sleep-timer-pause" onClick={handlePause}>
                {isPaused ? (
                  <>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                      <polygon points="5,3 19,12 5,21" />
                    </svg>
                    {t('timer.resume')}
                  </>
                ) : (
                  <>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                      <rect x="6" y="4" width="4" height="16" />
                      <rect x="14" y="4" width="4" height="16" />
                    </svg>
                    {t('timer.pause')}
                  </>
                )}
              </button>
              <button type="button" className="sleep-timer-cancel" onClick={handleCancel}>
                {t('timer.cancel')}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Toast notification */}
      <div className={`sleep-timer-toast ${showToast ? 'visible' : ''}`}>
        <svg
          className="sleep-timer-toast-icon"
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <circle cx="12" cy="12" r="10" />
          <polyline points="12,6 12,12 16,14" />
        </svg>
        <span className="sleep-timer-toast-text">{toastMessage}</span>
      </div>
    </>
  )
}

export default memo(SleepTimerImpl)
