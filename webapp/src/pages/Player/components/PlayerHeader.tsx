import { memo, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'

interface PlayerHeaderProps {
  title: string
  className: string
  isAdmin: boolean
  onDelete: () => void
}

const PlayerHeader = memo(function PlayerHeader({ title, className, isAdmin, onDelete }: PlayerHeaderProps) {
  const { t } = useTranslation()
  const navigate = useNavigate()

  const handleBack = useCallback(() => navigate('/'), [navigate])

  return (
    <header className={className}>
      <button className="player-back" onClick={handleBack} aria-label={t('player.backToHome')}>←</button>
      <span className="player-title">{title}</span>
      {isAdmin && (
        <button className="player-delete" onClick={onDelete} title={t('player.delete')} aria-label={t('player.delete')}>🗑</button>
      )}
    </header>
  )
})

export default PlayerHeader
