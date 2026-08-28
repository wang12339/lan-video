import { memo } from 'react'
import { useTranslation } from 'react-i18next'

interface PlayerLoadingProps {
  className: string
  preloadingNext: boolean
}

const PlayerLoading = memo(function PlayerLoading({ className, preloadingNext }: PlayerLoadingProps) {
  const { t } = useTranslation()

  return (
    <div className={className} aria-label={t('common.loading')} aria-busy="true">
      <div className="loading-ring" />
      <span>{t('common.loading')}</span>
      {preloadingNext && (
        <div className="preload-indicator">
          {t('player.preloadingNext')}
        </div>
      )}
    </div>
  )
})

export default PlayerLoading
