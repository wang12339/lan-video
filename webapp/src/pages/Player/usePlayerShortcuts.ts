import { useEffect, useRef, useState, useCallback } from 'react'
import { SPEED_STEPS, SEEK_STEP_S, VOLUME_STEP } from './constants'

interface ShortcutHandlers {
  togglePlay: () => void
  toggleFullscreen: () => void
  toggleMute: () => void
  togglePiP: () => void
  setVolumeValue: (val: number) => void
  setSpeedValue: (s: number) => void
  showShortcut: (text: string) => void
  resetHideTimer: () => void
  t: (key: string, opts?: Record<string, unknown>) => string
}

export function usePlayerShortcuts(
  videoRef: React.RefObject<HTMLVideoElement | null>,
  handlers: ShortcutHandlers,
) {
  const handlersRef = useRef(handlers)
  handlersRef.current = handlers
  const [showShortcutHelp, setShowShortcutHelp] = useState(false)

  const toggleShortcutHelp = useCallback(() => setShowShortcutHelp(v => !v), [])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Skip shortcuts when user is typing in input/textarea fields
      const target = e.target as HTMLElement
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return

      const video = videoRef.current
      if (!video) return

      const key = e.key.toLowerCase()
      const {
        togglePlay,
        toggleFullscreen,
        toggleMute,
        togglePiP,
        setVolumeValue,
        setSpeedValue,
        showShortcut,
        resetHideTimer,
        t,
      } = handlersRef.current

      // Helper to get current volume percentage for display
      const getVolumePercent = () => Math.round(video.volume * 100)

      switch (key) {
        // Play/Pause: Space or K
        case ' ':
        case 'k':
          e.preventDefault()
          togglePlay()
          resetHideTimer()
          showShortcut(t('player.shortcutPlay'))
          break

        // Seek backward: Left arrow or J
        case 'arrowleft':
        case 'j':
          e.preventDefault()
          video.currentTime = Math.max(0, video.currentTime - SEEK_STEP_S)
          resetHideTimer()
          showShortcut(t('player.shortcutSeekBack'))
          break

        // Seek forward: Right arrow or L
        case 'arrowright':
        case 'l':
          e.preventDefault()
          video.currentTime = Math.min(video.duration || 0, video.currentTime + SEEK_STEP_S)
          resetHideTimer()
          showShortcut(t('player.shortcutSeekForward'))
          break

        // Volume up: Up arrow
        case 'arrowup':
          e.preventDefault()
          setVolumeValue(Math.min(1, video.volume + VOLUME_STEP))
          showShortcut(t('player.shortcutVolume', { val: getVolumePercent() }))
          break

        // Volume down: Down arrow
        case 'arrowdown':
          e.preventDefault()
          setVolumeValue(Math.max(0, video.volume - VOLUME_STEP))
          showShortcut(t('player.shortcutVolume', { val: getVolumePercent() }))
          break

        // Fullscreen: F
        case 'f':
          toggleFullscreen()
          break

        // Mute/Unmute: M
        case 'm':
          toggleMute()
          showShortcut(video.muted ? t('player.shortcutMuted') : t('player.shortcutVolume', { val: getVolumePercent() }))
          break

        // Picture-in-Picture: P
        case 'p':
          e.preventDefault()
          togglePiP()
          showShortcut(t('player.shortcutPiP'))
          break

        // Speed slower: ,
        case ',': {
          e.preventDefault()
          const currentIndex = SPEED_STEPS.indexOf(video.playbackRate)
          const slowerIndex = Math.max(0, currentIndex - 1)
          const slowerSpeed = SPEED_STEPS[slowerIndex]
          if (slowerSpeed !== undefined) {
            setSpeedValue(slowerSpeed)
            showShortcut(t('player.shortcutSpeedSlower', { val: slowerSpeed }))
          }
          break
        }

        // Speed faster: .
        case '.': {
          e.preventDefault()
          const currentIndex = SPEED_STEPS.indexOf(video.playbackRate)
          const fasterIndex = Math.min(SPEED_STEPS.length - 1, currentIndex + 1)
          const fasterSpeed = SPEED_STEPS[fasterIndex]
          if (fasterSpeed !== undefined) {
            setSpeedValue(fasterSpeed)
            showShortcut(t('player.shortcutSpeedFaster', { val: fasterSpeed }))
          }
          break
        }

        // Help overlay: ?
        case '?':
          e.preventDefault()
          e.stopPropagation()
          setShowShortcutHelp(v => !v)
          break

        // Jump to percentage: 0-9 keys
        default:
          if (key >= '0' && key <= '9' && video.duration) {
            e.preventDefault()
            const percentage = parseInt(key) * 10
            video.currentTime = (video.duration * percentage) / 100
            resetHideTimer()
            showShortcut(t('player.shortcutPercent', { val: percentage }))
          }
          break
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [videoRef])

  return { showShortcutHelp, toggleShortcutHelp }
}
