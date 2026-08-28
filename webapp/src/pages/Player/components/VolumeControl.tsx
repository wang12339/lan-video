import { memo } from 'react'
import { TFunction } from 'i18next'

interface Props {
  volume: number
  muted: boolean
  toggleMute: () => void
  setVolumeValue: (val: number) => void
  setVolume: (val: number) => void
  t: TFunction
}

function VolumeControlImpl({ volume, muted, toggleMute, setVolumeValue, setVolume, t }: Props) {
  return (
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
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round((muted ? 0 : volume) * 100)}
        aria-valuetext={`${Math.round((muted ? 0 : volume) * 100)}%`}
      />
    </div>
  )
}

export default memo(VolumeControlImpl)
