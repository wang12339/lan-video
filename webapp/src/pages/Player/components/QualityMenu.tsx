import { memo } from 'react'
import { TFunction } from 'i18next'
import type { VideoVariant } from '../../../api/types'

interface Props {
  currentQuality: string
  variants: VideoVariant[]
  showQualityMenu: boolean
  setShowQualityMenu: (v: boolean | ((p: boolean) => boolean)) => void
  switchQuality: (quality: string) => void
  t: TFunction
}

function QualityMenuImpl({ currentQuality, variants, showQualityMenu, setShowQualityMenu, switchQuality, t }: Props) {
  if (variants.length === 0) return null

  return (
    <div className="quality-wrap">
      <button className="ctrl-btn quality-btn" aria-label={t('player.quality')} aria-haspopup="menu" aria-expanded={showQualityMenu} onClick={(e) => { e.stopPropagation(); setShowQualityMenu(!showQualityMenu) }}>
        {currentQuality === 'original' ? t('player.original') : currentQuality}
      </button>
      {showQualityMenu && (
        <div className="quality-menu" role="menu" onClick={(e) => e.stopPropagation()}>
          <button
            className={`quality-opt ${currentQuality === 'original' ? 'active' : ''}`}
            aria-current={currentQuality === 'original' ? 'true' : undefined}
            onClick={() => switchQuality('original')}
          >
            {t('player.original')}
          </button>
          {variants.map((variant) => (
            <button
              key={variant.resolution}
              className={`quality-opt ${currentQuality === variant.resolution ? 'active' : ''}`}
              aria-current={currentQuality === variant.resolution ? 'true' : undefined}
              onClick={() => switchQuality(variant.resolution)}
            >
              {variant.resolution}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

export default memo(QualityMenuImpl)
