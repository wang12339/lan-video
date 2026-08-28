import { memo } from 'react'
import type { GestureIndicatorType } from '../hooks/usePlayerGestures'

const LONG_PRESS_SPEED = 2.0

interface Props {
  gestureIndicator: GestureIndicatorType
  gestureValue: number
  isLongPressing: boolean
}

function GestureIndicatorImpl({ gestureIndicator, gestureValue, isLongPressing }: Props) {
  const renderContent = () => {
    if (!gestureIndicator) return null

    let icon: React.ReactNode
    let text: string

    switch (gestureIndicator) {
      case 'seek-forward':
        icon = (
          <svg viewBox="0 0 24 24" width="32" height="32" fill="currentColor">
            <path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z" />
          </svg>
        )
        text = `+${gestureValue}s`
        break
      case 'seek-backward':
        icon = (
          <svg viewBox="0 0 24 24" width="32" height="32" fill="currentColor">
            <path d="M12.5 8c-2.65 0-5.05 1-6.9 2.6L2 7v9h9l-3.62-3.62c1.39-1.16 3.16-1.88 5.12-1.88 3.54 0 6.55 2.31 7.6 5.5l2.37-.78C21.08 11.03 17.15 8 12.5 8z" />
          </svg>
        )
        text = `-${gestureValue}s`
        break
      case 'volume-up':
      case 'volume-down':
        icon = (
          <svg viewBox="0 0 24 24" width="32" height="32" fill="currentColor">
            <path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3A4.5 4.5 0 0 0 14 7.97v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z" />
          </svg>
        )
        text = `${gestureValue}%`
        break
      case 'speed':
        icon = (
          <svg viewBox="0 0 24 24" width="32" height="32" fill="currentColor">
            <path d="M20.38 8.57l-1.23 1.85a8 8 0 0 1-.22 7.58H5.07A8 8 0 0 1 15.58 6.85l1.85-1.23A10 10 0 0 0 3.35 19a2 2 0 0 0 1.72 1h13.85a2 2 0 0 0 1.74-1 10 10 0 0 0-.27-10.44zm-9.79 6.84a2 2 0 0 0 2.83 0l5.66-8.49-8.49 5.66a2 2 0 0 0 0 2.83z" />
          </svg>
        )
        text = `${gestureValue}x`
        break
      default:
        return null
    }

    return (
      <div className="gesture-indicator">
        <div className="gesture-indicator-icon">{icon}</div>
        <div className="gesture-indicator-text">{text}</div>
      </div>
    )
  }

  return (
    <>
      {renderContent()}
      {isLongPressing && (
        <div className="long-press-indicator">
          <span>{LONG_PRESS_SPEED}x</span>
        </div>
      )}
    </>
  )
}

export default memo(GestureIndicatorImpl)
