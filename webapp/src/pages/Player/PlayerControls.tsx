import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { TFunction } from 'i18next'
import { formatDuration } from '../../api'
import { SPEED_STEPS, SEEK_STEP_S } from './usePlayerShortcuts'
import type { VideoVariant } from '../../api/types'

// 进度条键盘步进（←/→），全局快捷键 usePlayerShortcuts 仍为 10s
const SEEK_KEY_STEP_S = 5

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

  // 高频 UI 状态（当前时间/缓冲/音量）隔离在本组件内部：
  // 直接订阅 video 元素事件，播放中只有本组件随 timeupdate（约 4Hz）重渲染，
  // 评论区、相关视频等兄弟子树不再参与
  const [currentTime, setCurrent] = useState(0)
  const [buffered, setBuffered] = useState(0)
  const [volume, setVolume] = useState(() => videoRef.current?.volume ?? 0.8)
  const [muted, setMuted] = useState(() => videoRef.current?.muted ?? false)

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
    // 拖动时实时更新进度条（timeupdate 在 seeking 期间被跳过）
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

  // 拖动进度条：document 级 mousemove/mouseup 跟随
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
    // 阻止全局快捷键（10s 步进）与本条（5s）重复触发
    e.preventDefault()
    e.stopPropagation()
    v.currentTime = target
    setCurrent(target)
    resetHideTimer()
  }, [videoRef, resetHideTimer])

  const progressPct = duration > 0 ? (currentTime / duration) * 100 : 0

  return (
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
  )
}

const PlayerControls = memo(PlayerControlsImpl)
export default PlayerControls
