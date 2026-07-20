import { useEffect, useRef } from 'react'

export const SPEED_STEPS = [0.5, 0.75, 1, 1.25, 1.5, 2]
export const SEEK_STEP_S = 10
const VOLUME_STEP = 0.05

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

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      const v = videoRef.current
      if (!v) return
      const k = e.key.toLowerCase()
      const { togglePlay, toggleFullscreen, toggleMute, togglePiP, setVolumeValue, setSpeedValue, showShortcut, resetHideTimer, t } = handlersRef.current

      switch (k) {
        case ' ': case 'k':
          e.preventDefault()
          togglePlay()
          resetHideTimer()
          showShortcut(t('player.shortcutPlay'))
          break
        case 'arrowleft': case 'j':
          e.preventDefault()
          v.currentTime = Math.max(0, v.currentTime - SEEK_STEP_S)
          resetHideTimer()
          showShortcut(t('player.shortcutSeekBack'))
          break
        case 'arrowright': case 'l':
          e.preventDefault()
          v.currentTime = Math.min(v.duration || 0, v.currentTime + SEEK_STEP_S)
          resetHideTimer()
          showShortcut(t('player.shortcutSeekForward'))
          break
        case 'arrowup':
          e.preventDefault()
          setVolumeValue(Math.min(1, v.volume + VOLUME_STEP))
          showShortcut(t('player.shortcutVolume', { val: Math.round(v.volume * 100) }))
          break
        case 'arrowdown':
          e.preventDefault()
          setVolumeValue(Math.max(0, v.volume - VOLUME_STEP))
          showShortcut(t('player.shortcutVolume', { val: Math.round(v.volume * 100) }))
          break
        case 'f':
          toggleFullscreen()
          break
        case 'm':
          toggleMute()
          showShortcut(v.muted ? t('player.shortcutMuted') : t('player.shortcutVolume', { val: Math.round(v.volume * 100) }))
          break
        case 'p':
          e.preventDefault()
          togglePiP()
          showShortcut(t('player.shortcutPiP'))
          break
        case ',': {
          e.preventDefault()
          const curIdx = SPEED_STEPS.indexOf(v.playbackRate)
          const newIdx = curIdx > 0 ? curIdx - 1 : 0
          const newSpeed = SPEED_STEPS[newIdx]
          if (newSpeed !== undefined) {
            setSpeedValue(newSpeed)
            showShortcut(t('player.shortcutSpeedSlower', { val: newSpeed }))
          }
          break
        }
        case '.': {
          e.preventDefault()
          const curIdx = SPEED_STEPS.indexOf(v.playbackRate)
          const newIdx = curIdx < SPEED_STEPS.length - 1 ? curIdx + 1 : SPEED_STEPS.length - 1
          const newSpeed = SPEED_STEPS[newIdx]
          if (newSpeed !== undefined) {
            setSpeedValue(newSpeed)
            showShortcut(t('player.shortcutSpeedFaster', { val: newSpeed }))
          }
          break
        }
        default:
          if (k >= '0' && k <= '9' && v.duration) {
            e.preventDefault()
            const pct = parseInt(k) * 10
            v.currentTime = v.duration * pct / 100
            resetHideTimer()
            showShortcut(t('player.shortcutPercent', { val: pct }))
          }
          break
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [videoRef])
}
