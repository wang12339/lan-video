import { memo } from 'react'
import { TFunction } from 'i18next'
import { SPEED_STEPS } from '../constants'

interface Props {
  speed: number
  showSpeedMenu: boolean
  setShowSpeedMenu: (v: boolean | ((p: boolean) => boolean)) => void
  setSpeedValue: (s: number) => void
  t: TFunction
}

function SpeedMenuImpl({ speed, showSpeedMenu, setShowSpeedMenu, setSpeedValue, t }: Props) {
  return (
    <div className="speed-wrap">
      <button className="ctrl-btn speed-btn" aria-label={t('player.speed')} aria-haspopup="menu" aria-expanded={showSpeedMenu} onClick={(e) => { e.stopPropagation(); setShowSpeedMenu(!showSpeedMenu) }}>
        {speed}×
      </button>
      {showSpeedMenu && (
        <div className="speed-menu" role="menu" onClick={(e) => e.stopPropagation()}>
          {SPEED_STEPS.map((s) => (
            <button key={s} className={`speed-opt ${speed === s ? 'active' : ''}`} aria-current={speed === s ? 'true' : undefined} onClick={() => { setSpeedValue(s); setShowSpeedMenu(false) }}>
              {s}×
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

export default memo(SpeedMenuImpl)
