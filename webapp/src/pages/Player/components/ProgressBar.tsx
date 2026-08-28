import { memo, useCallback, useEffect, useRef } from 'react'
import { TFunction } from 'i18next'
import { formatDuration } from '../../../api'
import { trackClick } from '../../../utils/track'

const SEEK_KEY_STEP_S = 5

interface Props {
  videoRef: React.RefObject<HTMLVideoElement | null>
  currentTime: number
  buffered: number
  duration: number
  setCurrentTime: (time: number) => void
  onSeekingChange?: (seeking: boolean) => void
  resetHideTimer: () => void
  t: TFunction
}

function ProgressBarImpl({
  videoRef, currentTime, buffered, duration,
  setCurrentTime, onSeekingChange, resetHideTimer, t,
}: Props) {
  const progressRef = useRef<HTMLDivElement>(null)
  const seekingRef = useRef(false)

  const setSeeking = useCallback((value: boolean) => {
    seekingRef.current = value
    onSeekingChange?.(value)
  }, [onSeekingChange])

  const handleProgressClick = useCallback((clientX: number) => {
    const bar = progressRef.current
    const v = videoRef.current
    if (!bar || !v || !v.duration) return
    if (!isFinite(clientX)) return
    const rect = bar.getBoundingClientRect()
    if (rect.width <= 0) return
    const pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
    const target = pct * v.duration
    if (!isFinite(target) || target < 0) return
    const oldTime = v.currentTime
    v.currentTime = target
    setCurrentTime(target)
    const seekDelta = Math.round(target - oldTime)
    if (Math.abs(seekDelta) > 1) {
      trackClick(seekDelta > 0 ? '快进' : '快退', `${Math.abs(seekDelta)}s(拖动)`)
    }
  }, [videoRef, setCurrentTime])

  const handleProgressMouseDown = useCallback((e: React.MouseEvent) => {
    setSeeking(true)
    handleProgressClick(e.clientX)
  }, [handleProgressClick, setSeeking])

  const handleTouchProgress = useCallback((e: React.TouchEvent<HTMLDivElement>) => {
    setSeeking(true)
    const touch = e.touches[0]
    if (touch) {
      const bar = progressRef.current
      const v = videoRef.current
      if (!bar || !v || !v.duration) return
      if (!isFinite(touch.clientX)) return
      const rect = bar.getBoundingClientRect()
      if (rect.width <= 0) return
      const pct = Math.max(0, Math.min(1, (touch.clientX - rect.left) / rect.width))
      const target = pct * v.duration
      if (!isFinite(target) || target < 0) return
      const oldTime = v.currentTime
      v.currentTime = target
      setCurrentTime(target)
      const seekDelta = Math.round(target - oldTime)
      if (Math.abs(seekDelta) > 1) {
        trackClick(seekDelta > 0 ? '快进' : '快退', `${Math.abs(seekDelta)}s(触摸)`)
      }
    }
  }, [videoRef, setCurrentTime, setSeeking])

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (seekingRef.current) handleProgressClick(e.clientX)
    }
    const handleMouseUp = () => { setSeeking(false) }
    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)
    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [handleProgressClick, setSeeking])

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
    if (!isFinite(target) || target < 0) return
    v.currentTime = target
    setCurrentTime(target)
    resetHideTimer()
  }, [videoRef, resetHideTimer, setCurrentTime])

  const progressPct = duration > 0 ? (currentTime / duration) * 100 : 0

  return (
    <div
      className="player-progress-wrap"
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
      onTouchEnd={() => { setSeeking(false) }}
    >
      <div className="player-progress-bar">
        <div className="player-progress-buffered" style={{ width: buffered + '%' }} />
        <div className="player-progress-current" style={{ width: progressPct + '%' }} />
        <div className="player-progress-dot" style={{ left: progressPct + '%' }} />
      </div>
    </div>
  )
}

export default memo(ProgressBarImpl)
