import { useState, useEffect, useCallback, useRef } from 'react'
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

const STORAGE_KEY = 'atmos_sleep_timer'

function getSavedTimer(): { endTime: number; minutes: number } | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    const data = JSON.parse(raw) as { endTime: number; minutes: number }
    if (data.endTime > Date.now()) {
      return data
    }
    localStorage.removeItem(STORAGE_KEY)
    return null
  } catch {
    return null
  }
}

export default function SleepTimer({ onExpire, onCancel }: SleepTimerProps) {
  const { t } = useTranslation()
  const [dropdownVisible, setDropdownVisible] = useState(false)
  const [activeMinutes, setActiveMinutes] = useState<number | null>(null)
  const [remainingSeconds, setRemainingSeconds] = useState(0)
  const [showToast, setShowToast] = useState(false)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

  const options: TimerOption[] = [
    { label: t('timer.15min'), minutes: 15 },
    { label: t('timer.30min'), minutes: 30 },
    { label: t('timer.45min'), minutes: 45 },
    { label: t('timer.1hour'), minutes: 60 },
    { label: t('timer.2hours'), minutes: 120 },
  ]

  // Restore saved timer on mount
  useEffect(() => {
    const saved = getSavedTimer()
    if (saved) {
      setActiveMinutes(saved.minutes)
      const remaining = Math.max(0, Math.floor((saved.endTime - Date.now()) / 1000))
      setRemainingSeconds(remaining)
      startTimer()
    }
  }, [])

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownVisible(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const startTimer = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current)
    }

    timerRef.current = setInterval(() => {
      setRemainingSeconds((prev) => {
        if (prev <= 1) {
          // Timer expired
          if (timerRef.current) {
            clearInterval(timerRef.current)
          }
          localStorage.removeItem(STORAGE_KEY)
          setActiveMinutes(null)
          onExpire()
          return 0
        }
        return prev - 1
      })
    }, 1000)
  }, [onExpire])

  const handleSelect = (minutes: number) => {
    const seconds = minutes * 60
    const endTime = Date.now() + seconds * 1000

    // Save to localStorage
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ endTime, minutes }))

    setActiveMinutes(minutes)
    setRemainingSeconds(seconds)
    setDropdownVisible(false)
    setShowToast(true)

    // Hide toast after 3 seconds
    setTimeout(() => setShowToast(false), 3000)

    startTimer()
  }

  const handleCancel = () => {
    if (timerRef.current) {
      clearInterval(timerRef.current)
    }
    localStorage.removeItem(STORAGE_KEY)
    setActiveMinutes(null)
    setRemainingSeconds(0)
    setDropdownVisible(false)
    onCancel?.()
  }

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60)
    const secs = seconds % 60
    return `${mins}:${secs.toString().padStart(2, '0')}`
  }

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current)
      }
    }
  }, [])

  return (
    <>
      <div className="sleep-timer" ref={dropdownRef}>
        <button
          className={`sleep-timer-trigger ${activeMinutes ? 'active' : ''}`}
          onClick={() => setDropdownVisible(!dropdownVisible)}
          aria-label={t('timer.title')}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="10" />
            <polyline points="12,6 12,12 16,14" />
          </svg>
          {activeMinutes && remainingSeconds > 0 ? formatTime(remainingSeconds) : t('timer.title')}
        </button>

        <div className={`sleep-timer-dropdown ${dropdownVisible ? 'visible' : ''}`}>
          <div className="sleep-timer-header">{t('timer.selectDuration')}</div>
          <div className="sleep-timer-options">
            {options.map((option) => (
              <div
                key={option.minutes}
                className={`sleep-timer-option ${activeMinutes === option.minutes ? 'active' : ''}`}
                onClick={() => handleSelect(option.minutes)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    handleSelect(option.minutes)
                  }
                }}
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
                >
                  <polyline points="20,6 9,17 4,12" />
                </svg>
              </div>
            ))}
          </div>
          {activeMinutes && (
            <div className="sleep-timer-footer">
              <button className="sleep-timer-cancel" onClick={handleCancel}>
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
        <span className="sleep-timer-toast-text">
          {t('timer.set')} <span className="sleep-timer-toast-time">{activeMinutes}分钟</span> {t('timer.after')}
        </span>
      </div>
    </>
  )
}
