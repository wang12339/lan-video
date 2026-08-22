import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { TFunction } from 'i18next'
import { formatDuration } from '../../api'
import { SPEED_STEPS, SEEK_STEP_S } from './usePlayerShortcuts'
import type { VideoVariant } from '../../api/types'

// 进度条键盘步进（←/→），全局快捷键 usePlayerShortcuts 仍为 10s
const SEEK_KEY_STEP_S = 5

// 手势相关常量
const DOUBLE_TAP_DELAY_MS = 300
const LONG_PRESS_DELAY_MS = 500
const SWIPE_THRESHOLD_PX = 50
const SWIPE_VOLUME_STEP = 0.05
const LONG_PRESS_SPEED = 2.0

interface Props {
  videoRef: React.RefObject<HTMLVideoElement | null>
  controlsVisible: boolean
  paused: boolean
  duration: number
  speed: number
  showQualityMenu: boolean
  showSpeedMenu: boolean
  currentQuality: string
  variants: VideoVariant[]
  togglePlay: () => void
  toggleMute: () => void
  toggleFullscreen: () => void
  togglePiP: () => void
  setSpeedValue: (s: number) => void
  setVolumeValue: (val: number) => void
  switchQuality: (quality: string) => void
  seekBy: (delta: number) => void
  resetHideTimer: () => void
  setShowQualityMenu: (v: boolean | ((p: boolean) => boolean)) => void
  setShowSpeedMenu: (v: boolean | ((p: boolean) => boolean)) => void
  t: TFunction
}

// 手势状态指示器类型
type GestureIndicatorType = 'seek-forward' | 'seek-backward' | 'volume-up' | 'volume-down' | 'brightness-up' | 'brightness-down' | 'speed' | null

function PlayerControlsImpl({
  videoRef, controlsVisible, paused, duration, speed,
  showQualityMenu, showSpeedMenu, currentQuality, variants,
  togglePlay, toggleMute, toggleFullscreen, togglePiP,
  setSpeedValue, setVolumeValue, switchQuality,
  seekBy, resetHideTimer,
  setShowQualityMenu, setShowSpeedMenu, t,
}: Props) {
  const progressRef = useRef<HTMLDivElement>(null)
  const seekingRef = useRef(false)
  const gestureAreaRef = useRef<HTMLDivElement>(null)

  // 高频 UI 状态（当前时间/缓冲/音量）隔离在本组件内部
  const [currentTime, setCurrent] = useState(0)
  const [buffered, setBuffered] = useState(0)
  const [volume, setVolume] = useState(() => videoRef.current?.volume ?? 0.8)
  const [muted, setMuted] = useState(() => videoRef.current?.muted ?? false)

  // 手势状态
  const [gestureIndicator, setGestureIndicator] = useState<GestureIndicatorType>(null)
  const [gestureValue, setGestureValue] = useState<number>(0)
  const [isLongPressing, setIsLongPressing] = useState(false)
  const [originalSpeed, setOriginalSpeed] = useState(speed)

  // 手势引用
  const lastTapRef = useRef<number>(0)
  const tapCountRef = useRef<number>(0)
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const touchStartRef = useRef<{ x: number; y: number; time: number } | null>(null)
  const isSwipingRef = useRef(false)
  const gestureTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    const v = videoRef.current
    if (!v) return
    const onTime = () => {
      if (seekingRef.current) return
      setCurrent(v.currentTime)
    }
    const onProgress = () => {
      if (v.duration) {
        const last = v.buffered.length > 0 ? v.buffered.end(v.buffered.length - 1) : 0
        setBuffered((last / v.duration) * 100)
      }
    }
    const onVolumeChange = () => {
      setVolume(v.volume)
      setMuted(v.muted)
    }
    v.addEventListener('timeupdate', onTime)
    v.addEventListener('progress', onProgress)
    v.addEventListener('volumechange', onVolumeChange)
    return () => {
      v.removeEventListener('timeupdate', onTime)
      v.removeEventListener('progress', onProgress)
      v.removeEventListener('volumechange', onVolumeChange)
    }
  }, [videoRef])

  const handleProgressClick = useCallback((clientX: number) => {
    const bar = progressRef.current
    const v = videoRef.current
    if (!bar || !v || !v.duration) return
    const rect = bar.getBoundingClientRect()
    const pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
    const target = pct * v.duration
    v.currentTime = target
    setCurrent(target)
  }, [videoRef])

  const handleProgressMouseDown = useCallback((e: React.MouseEvent) => {
    seekingRef.current = true
    handleProgressClick(e.clientX)
  }, [handleProgressClick])

  const handleTouchProgress = useCallback((e: React.TouchEvent<HTMLDivElement>) => {
    seekingRef.current = true
    const touch = e.touches[0]
    if (touch) handleProgressClick(touch.clientX)
  }, [handleProgressClick])

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (seekingRef.current) handleProgressClick(e.clientX)
    }
    const handleMouseUp = () => { seekingRef.current = false }
    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)
    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [handleProgressClick])

  const handleProgressKeyDown = useCallback((e: React.KeyboardEvent<HTMLDivElement>) => {
    const v = videoRef.current
    if (!v) return
    let target: number | null = null
    switch (e.key) {
      case 'ArrowLeft': target = Math.max(0, v.currentTime - SEEK_KEY_STEP_S); break
      case 'ArrowRight': target = Math.min(v.duration || 0, v.currentTime + SEEK_KEY_STEP_S); break
      case 'Home': target = 0; break
      case 'End': target = v.duration || 0; break
      default: return
    }
    e.preventDefault()
    e.stopPropagation()
    v.currentTime = target
    setCurrent(target)
    resetHideTimer()
  }, [videoRef, resetHideTimer])

  // 显示手势指示器
  const showGestureIndicator = useCallback((type: GestureIndicatorType, value: number) => {
    if (gestureTimeoutRef.current) {
      clearTimeout(gestureTimeoutRef.current)
    }
    setGestureIndicator(type)
    setGestureValue(value)
    gestureTimeoutRef.current = setTimeout(() => {
      setGestureIndicator(null)
      setGestureValue(0)
    }, 700)
  }, [])

  // 清除长按定时器
  const clearLongPressTimer = useCallback(() => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current)
      longPressTimerRef.current = null
    }
  }, [])

  // 处理双击快进/快退
  const handleDoubleTap = useCallback((side: 'left' | 'right') => {
    const v = videoRef.current
    if (!v) return

    if (side === 'right') {
      // 右侧双击快进 10 秒
      const newTime = Math.min(v.duration || 0, v.currentTime + 10)
      v.currentTime = newTime
      setCurrent(newTime)
      showGestureIndicator('seek-forward', 10)
    } else {
      // 左侧双击快退 10 秒
      const newTime = Math.max(0, v.currentTime - 10)
      v.currentTime = newTime
      setCurrent(newTime)
      showGestureIndicator('seek-backward', 10)
    }
    resetHideTimer()
  }, [videoRef, resetHideTimer, showGestureIndicator])

  // 处理长按倍速
  const handleLongPressStart = useCallback(() => {
    const v = videoRef.current
    if (!v) return

    setOriginalSpeed(speed)
    setSpeedValue(LONG_PRESS_SPEED)
    setIsLongPressing(true)
    showGestureIndicator('speed', LONG_PRESS_SPEED)
  }, [videoRef, speed, setSpeedValue, showGestureIndicator])

  const handleLongPressEnd = useCallback(() => {
    if (isLongPressing) {
      setSpeedValue(originalSpeed)
      setIsLongPressing(false)
      clearLongPressTimer()
    }
  }, [isLongPressing, originalSpeed, setSpeedValue, clearLongPressTimer])

  // 处理滑动调节（音量/进度）
  const handleSwipe = useCallback((direction: 'up' | 'down' | 'left' | 'right', magnitude: number) => {
    const v = videoRef.current
    if (!v) return

    const steps = Math.max(1, Math.floor(magnitude / SWIPE_THRESHOLD_PX))

    switch (direction) {
      case 'up':
        // 上滑增加音量
        {
          const newVolume = Math.min(1, v.volume + (SWIPE_VOLUME_STEP * steps))
          setVolumeValue(newVolume)
          showGestureIndicator('volume-up', Math.round(newVolume * 100))
        }
        break
      case 'down':
        // 下滑减少音量
        {
          const newVolume = Math.max(0, v.volume - (SWIPE_VOLUME_STEP * steps))
          setVolumeValue(newVolume)
          showGestureIndicator('volume-down', Math.round(newVolume * 100))
        }
        break
      case 'left':
        // 左滑快退
        {
          const seekAmount = 5 * steps
          const newTime = Math.max(0, v.currentTime - seekAmount)
          v.currentTime = newTime
          setCurrent(newTime)
          showGestureIndicator('seek-backward', seekAmount)
        }
        break
      case 'right':
        // 右滑快进
        {
          const seekAmount = 5 * steps
          const newTime = Math.min(v.duration || 0, v.currentTime + seekAmount)
          v.currentTime = newTime
          setCurrent(newTime)
          showGestureIndicator('seek-forward', seekAmount)
        }
        break
    }
    resetHideTimer()
  }, [videoRef, setVolumeValue, resetHideTimer, showGestureIndicator])

  // 触摸事件处理
  const handleTouchStart = useCallback((e: React.TouchEvent) => {
    // 忽略进度条区域的触摸
    if ((e.target as HTMLElement).closest('.progress-wrap')) return

    const touch = e.touches[0]
    if (!touch) return

    touchStartRef.current = {
      x: touch.clientX,
      y: touch.clientY,
      time: Date.now()
    }
    isSwipingRef.current = false

    // 设置长按定时器
    longPressTimerRef.current = setTimeout(() => {
      handleLongPressStart()
    }, LONG_PRESS_DELAY_MS)
  }, [handleLongPressStart])

  const handleTouchMove = useCallback((e: React.TouchEvent) => {
    if (!touchStartRef.current) return
    if ((e.target as HTMLElement).closest('.progress-wrap')) return

    const touch = e.touches[0]
    if (!touch) return

    const deltaX = touch.clientX - touchStartRef.current.x
    const deltaY = touch.clientY - touchStartRef.current.y
    const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY)

    // 如果移动距离超过阈值，取消长按
    if (distance > 10 && longPressTimerRef.current) {
      clearLongPressTimer()
      isSwipingRef.current = true
    }
  }, [clearLongPressTimer])

  const handleTouchEnd = useCallback((e: React.TouchEvent) => {
    if (!touchStartRef.current) return
    if ((e.target as HTMLElement).closest('.progress-wrap')) return

    clearLongPressTimer()

    // 如果是长按状态，结束长按
    if (isLongPressing) {
      handleLongPressEnd()
      touchStartRef.current = null
      return
    }

    const touch = e.changedTouches[0]
    if (!touch) {
      touchStartRef.current = null
      return
    }

    const deltaX = touch.clientX - touchStartRef.current.x
    const deltaY = touch.clientY - touchStartRef.current.y
    const deltaTime = Date.now() - touchStartRef.current.time
    const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY)

    // 判断是否为滑动手势
    if (distance > SWIPE_THRESHOLD_PX && deltaTime < 300) {
      const absX = Math.abs(deltaX)
      const absY = Math.abs(deltaY)

      if (absX > absY) {
        // 水平滑动
        handleSwipe(deltaX > 0 ? 'right' : 'left', absX)
      } else {
        // 垂直滑动
        handleSwipe(deltaY > 0 ? 'down' : 'up', absY)
      }
    }
    // 判断是否为点击（非滑动）
    else if (distance < 10 && deltaTime < 300) {
      const now = Date.now()
      const timeSinceLastTap = now - lastTapRef.current

      if (timeSinceLastTap < DOUBLE_TAP_DELAY_MS) {
        // 双击
        tapCountRef.current++
        if (tapCountRef.current >= 2) {
          // 判断双击位置（左侧或右侧）
          const playerWidth = gestureAreaRef.current?.clientWidth || window.innerWidth
          const tapX = touch.clientX
          const side = tapX < playerWidth / 2 ? 'left' : 'right'
          handleDoubleTap(side)
          tapCountRef.current = 0
        }
      } else {
        // 单击
        tapCountRef.current = 1
        // 延迟执行单击（等待可能的双击）
        setTimeout(() => {
          if (tapCountRef.current === 1) {
            togglePlay()
            resetHideTimer()
          }
        }, DOUBLE_TAP_DELAY_MS)
      }

      lastTapRef.current = now
    }

    touchStartRef.current = null
    isSwipingRef.current = false
  }, [isLongPressing, handleLongPressEnd, clearLongPressTimer, handleSwipe, handleDoubleTap, togglePlay, resetHideTimer])

  // 鼠标事件处理（用于桌面端长按倍速）
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // 忽略进度条和控制按钮区域
    if ((e.target as HTMLElement).closest('.progress-wrap, .controls-row')) return

    longPressTimerRef.current = setTimeout(() => {
      handleLongPressStart()
    }, LONG_PRESS_DELAY_MS)
  }, [handleLongPressStart])

  const handleMouseUp = useCallback(() => {
    clearLongPressTimer()
    if (isLongPressing) {
      handleLongPressEnd()
    }
  }, [isLongPressing, handleLongPressEnd, clearLongPressTimer])

  // 清理定时器
  useEffect(() => {
    return () => {
      clearLongPressTimer()
      if (gestureTimeoutRef.current) {
        clearTimeout(gestureTimeoutRef.current)
      }
    }
  }, [clearLongPressTimer])

  const progressPct = duration > 0 ? (currentTime / duration) * 100 : 0

  // 渲染手势指示器
  const renderGestureIndicator = () => {
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
      {/* 手势检测区域 */}
      <div
        ref={gestureAreaRef}
        className="gesture-area"
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
      />

      {/* 手势指示器 */}
      {renderGestureIndicator()}

      {/* 长按倍速指示器 */}
      {isLongPressing && (
        <div className="long-press-indicator">
          <span>{LONG_PRESS_SPEED}x</span>
        </div>
      )}

      {/* 原有控制组件 */}
      <div className={`player-controls ${controlsVisible ? 'show' : ''}`}>
        <div
          className="progress-wrap"
          ref={progressRef}
          role="slider"
          tabIndex={0}
          aria-label={t('player.progressAria')}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(progressPct)}
          aria-valuetext={`${formatDuration(currentTime)} / ${formatDuration(duration)}`}
          onKeyDown={handleProgressKeyDown}
          onMouseDown={handleProgressMouseDown}
          onTouchStart={handleTouchProgress}
          onTouchMove={handleTouchProgress}
          onTouchEnd={() => { seekingRef.current = false }}
        >
          <div className="progress-bar">
            <div className="progress-buffered" style={{ width: buffered + '%' }} />
            <div className="progress-current" style={{ width: progressPct + '%' }} />
            <div className="progress-dot" style={{ left: progressPct + '%' }} />
          </div>
        </div>

        <div className="controls-row">
          <div className="controls-left">
            <button className="ctrl-btn" onClick={togglePlay} aria-label={t('player.playPause')}>
              {paused ? (
                <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
              ) : (
                <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M6 4h4v16H6V4zm8 0h4v16h-4V4z"/></svg>
              )}
            </button>
            <button className="ctrl-btn" onClick={() => seekBy(-SEEK_STEP_S)} aria-label={t('player.seekBackward')}>
              <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M12.5 8c-2.65 0-5.05 1-6.9 2.6L2 7v9h9l-3.62-3.62c1.39-1.16 3.16-1.88 5.12-1.88 3.54 0 6.55 2.31 7.6 5.5l2.37-.78C21.08 11.03 17.15 8 12.5 8z"/></svg>
            </button>
            <button className="ctrl-btn" onClick={() => seekBy(SEEK_STEP_S)} aria-label={t('player.seekForward')}>
              <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z"/></svg>
            </button>
            <span className="time-display">
              <span>{formatDuration(currentTime)}</span>
              <span className="time-sep">/</span>
              <span>{formatDuration(duration)}</span>
            </span>
          </div>
          <div className="controls-right">
            {variants.length > 0 && (
            <div className="quality-wrap">
              <button className="ctrl-btn quality-btn" aria-label={t('player.quality')} aria-haspopup="menu" aria-expanded={showQualityMenu} onClick={(e) => { e.stopPropagation(); setShowQualityMenu(!showQualityMenu) }}>
                {currentQuality === 'original' ? t('player.original') : currentQuality}
              </button>
              {showQualityMenu && (
                <div className="quality-menu" role="menu" onClick={(e) => e.stopPropagation()}>
                  <button
                    className={`quality-opt ${currentQuality === 'original' ? 'active' : ''}`}
                    onClick={() => switchQuality('original')}
                  >
                    {t('player.original')}
                  </button>
                  {variants.map((variant) => (
                    <button
                      key={variant.resolution}
                      className={`quality-opt ${currentQuality === variant.resolution ? 'active' : ''}`}
                      onClick={() => switchQuality(variant.resolution)}
                    >
                      {variant.resolution}
                    </button>
                  ))}
                </div>
              )}
            </div>
            )}
            <div className="speed-wrap">
              <button className="ctrl-btn speed-btn" aria-label={t('player.speed')} aria-haspopup="menu" aria-expanded={showSpeedMenu} onClick={(e) => { e.stopPropagation(); setShowSpeedMenu(!showSpeedMenu) }}>
                {speed}×
              </button>
              {showSpeedMenu && (
                <div className="speed-menu" role="menu" onClick={(e) => e.stopPropagation()}>
                  {SPEED_STEPS.map((s) => (
                    <button key={s} className={`speed-opt ${speed === s ? 'active' : ''}`} onClick={() => { setSpeedValue(s); setShowSpeedMenu(false) }}>
                      {s}×
                    </button>
                  ))}
                </div>
              )}
            </div>
            <div className="volume-wrap">
              <button className="ctrl-btn" onClick={toggleMute} aria-label={muted ? t('player.unmute') : t('player.mute')}>
                <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3A4.5 4.5 0 0 0 14 7.97v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/></svg>
              </button>
              <input
                type="range"
                className="volume-slider"
                min="0"
                max="1"
                step="0.05"
                value={muted ? 0 : volume}
                onChange={(e) => { const val = parseFloat(e.target.value); setVolume(val); setVolumeValue(val) }}
                aria-label={t('player.volume')}
                aria-valuetext={`${Math.round((muted ? 0 : volume) * 100)}%`}
              />
            </div>
            <button className="ctrl-btn" onClick={toggleFullscreen} aria-label={t('player.fullscreen')}>
              <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z"/></svg>
            </button>
            <button className="ctrl-btn" onClick={togglePiP} aria-label={t('player.pictureInPicture')}>
              <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M19 11h-8v6h8v-6zm4 8V4.98C23 3.88 22.1 3 21 3H3c-1.1 0-2 .88-2 1.98V19c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2zm-2 .02H3V4.97h18v14.05z"/></svg>
            </button>
          </div>
        </div>
      </div>
    </>
  )
}

const PlayerControls = memo(PlayerControlsImpl)
export default PlayerControls
