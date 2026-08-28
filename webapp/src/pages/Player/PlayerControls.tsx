import { memo, useEffect, useRef, useState } from 'react'
import { TFunction } from 'i18next'
import { formatDuration } from '../../api'
import { SEEK_STEP_S } from './constants'
import type { VideoVariant } from '../../api/types'
import { usePlayerGestures } from './hooks/usePlayerGestures'
import ProgressBar from './components/ProgressBar'
import VolumeControl from './components/VolumeControl'
import SpeedMenu from './components/SpeedMenu'
import QualityMenu from './components/QualityMenu'
import GestureIndicator from './components/GestureIndicator'

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
  const seekingRef = useRef(false)
  const [currentTime, setCurrentTime] = useState(0)
  const [buffered, setBuffered] = useState(0)
  const [volume, setVolume] = useState(() => videoRef.current?.volume ?? 0.8)
  const [muted, setMuted] = useState(() => videoRef.current?.muted ?? false)

  const {
    gestureIndicator, gestureValue, isLongPressing,
    gestureAreaRef, handleTouchStart, handleTouchMove,
    handleTouchEnd, handleMouseDown, handleMouseUp,
  } = usePlayerGestures({
    videoRef, speed, setSpeedValue, setVolumeValue,
    setCurrentTime, resetHideTimer, togglePlay,
  })

  useEffect(() => {
    const v = videoRef.current
    if (!v) return
    const onTime = () => {
      if (seekingRef.current) return
      setCurrentTime(v.currentTime)
    }
    const onProgress = () => {
      if (v.duration) {
        const last = v.buffered.length > 0 ? v.buffered.end(v.buffered.length - 1) : 0
        setBuffered((last / v.duration) * 100)
      }
    }
    const onVolumeChange = () => { setVolume(v.volume); setMuted(v.muted) }
    v.addEventListener('timeupdate', onTime)
    v.addEventListener('progress', onProgress)
    v.addEventListener('volumechange', onVolumeChange)
    return () => {
      v.removeEventListener('timeupdate', onTime)
      v.removeEventListener('progress', onProgress)
      v.removeEventListener('volumechange', onVolumeChange)
    }
  }, [videoRef])

  return (
    <>
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

      <GestureIndicator
        gestureIndicator={gestureIndicator}
        gestureValue={gestureValue}
        isLongPressing={isLongPressing}
      />

      <div className={`player-controls ${controlsVisible ? 'show' : ''}`} style={{ transition: 'opacity var(--player-transition-speed, 280ms) cubic-bezier(0.4, 0, 0.2, 1)' }}>
        <ProgressBar
          videoRef={videoRef}
          currentTime={currentTime}
          buffered={buffered}
          duration={duration}
          setCurrentTime={setCurrentTime}
          onSeekingChange={(seeking) => { seekingRef.current = seeking }}
          resetHideTimer={resetHideTimer}
          t={t}
        />

        <div className="controls-row" role="toolbar" aria-label={t('player.videoControls')}>
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
            <span className="time-display" aria-label={t('player.timeDisplay', { current: formatDuration(currentTime), total: formatDuration(duration) })}>
              <span>{formatDuration(currentTime)}</span>
              <span className="time-sep">/</span>
              <span>{formatDuration(duration)}</span>
            </span>
          </div>
          <div className="controls-right">
            <QualityMenu
              currentQuality={currentQuality}
              variants={variants}
              showQualityMenu={showQualityMenu}
              setShowQualityMenu={setShowQualityMenu}
              switchQuality={switchQuality}
              t={t}
            />
            <SpeedMenu
              speed={speed}
              showSpeedMenu={showSpeedMenu}
              setShowSpeedMenu={setShowSpeedMenu}
              setSpeedValue={setSpeedValue}
              t={t}
            />
            <VolumeControl
              volume={volume}
              muted={muted}
              toggleMute={toggleMute}
              setVolumeValue={setVolumeValue}
              setVolume={setVolume}
              t={t}
            />
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
