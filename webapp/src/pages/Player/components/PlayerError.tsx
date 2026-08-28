import { memo, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'

interface PlayerErrorProps {
  message: string
  onRetry?: () => void
}

const PlayerError = memo(function PlayerError({ message, onRetry }: PlayerErrorProps) {
  const { t } = useTranslation()
  const navigate = useNavigate()

  const handleBack = useCallback(() => navigate('/'), [navigate])

  if (!message) return null

  return (
    <div className="player-error">
      <span className="player-error-icon">⚠️</span>
      <p>{message}</p>
      {onRetry ? (
        <button onClick={onRetry}>{t('common.retry')}</button>
      ) : (
        <button onClick={handleBack}>{t('player.backToHome')}</button>
      )}
    </div>
  )
})

export default PlayerError
