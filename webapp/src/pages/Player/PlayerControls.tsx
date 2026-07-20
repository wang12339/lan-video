import { TFunction } from 'i18next'
import { formatDuration } from '../../api'
import { SPEED_STEPS, SEEK_STEP_S } from './usePlayerShortcuts'
import type { VideoVariant } from '../../api/types'

interface Props {
  controlsVisible: boolean
  paused: boolean
  currentTime: number
  duration: number
  buffered: number
  speed: number
  volume: number
  muted: boolean
  progressRef: React.RefObject<HTMLDivElement>
  progressPct: number
  seekingRef: React.MutableRefObject<boolean>
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
  handleProgressMouseDown: (e: React.MouseEvent) => void
  handleTouchProgress: (e: React.TouchEvent<HTMLDivElement>) => void
  seekBy: (delta: number) => void
  setShowQualityMenu: (v: boolean | ((p: boolean) => boolean)) => void
  setShowSpeedMenu: (v: boolean | ((p: boolean) => boolean)) => void
  t: TFunction
}

export default function PlayerControls({
  controlsVisible, paused, currentTime, duration, buffered, speed, volume, muted,
  progressRef, progressPct, seekingRef,
  showQualityMenu, showSpeedMenu, currentQuality, variants,
  togglePlay, toggleMute, toggleFullscreen, togglePiP,
  setSpeedValue, setVolumeValue, switchQuality,
  handleProgressMouseDown, handleTouchProgress,
  seekBy,
  setShowQualityMenu, setShowSpeedMenu, t,
}: Props) {
  return (
    <div className={`player-controls ${controlsVisible ? 'show' : ''}`}>
      <div
        className="progress-wrap"
        ref={progressRef}
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
            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M18 13c0 3.31-2.69 6-6 6s-6-2.69-6-6 2.69-6 6-6v4l5-5-5-5v4c-4.42 0-8 3.58-8 8s3.58 8 8 8 8-3.58 8-8h-2z"/></svg>
          </button>
          <span className="time-display">
            <span>{formatDuration(currentTime)}</span>
            <span className="time-sep">/</span>
            <span>{formatDuration(duration)}</span>
          </span>
        </div>
        <div className="controls-right">
          <div className="quality-wrap">
            <button className="ctrl-btn quality-btn" onClick={(e) => { e.stopPropagation(); setShowQualityMenu(!showQualityMenu) }}>
              {currentQuality === 'original' ? t('player.original') : currentQuality}
            </button>
            {showQualityMenu && (
              <div className="quality-menu" onClick={(e) => e.stopPropagation()}>
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
          <div className="speed-wrap">
            <button className="ctrl-btn speed-btn" onClick={(e) => { e.stopPropagation(); setShowSpeedMenu(!showSpeedMenu) }}>
              {speed}×
            </button>
            {showSpeedMenu && (
              <div className="speed-menu" onClick={(e) => e.stopPropagation()}>
                {SPEED_STEPS.map((s) => (
                  <button key={s} className={`speed-opt ${speed === s ? 'active' : ''}`} onClick={() => { setSpeedValue(s); setShowSpeedMenu(false) }}>
                    {s}×
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="volume-wrap">
            <button className="ctrl-btn" onClick={toggleMute} aria-label={t('player.mute')}>
              <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3A4.5 4.5 0 0 0 14 7.97v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/></svg>
            </button>
            <input
              type="range"
              className="volume-slider"
              min="0"
              max="1"
              step="0.05"
              value={muted ? 0 : volume}
              onChange={(e) => setVolumeValue(parseFloat(e.target.value))}
              aria-label={t('player.volume')}
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
