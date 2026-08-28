import { memo, useCallback, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { listVideos, mapVideo } from '../../../api'
import type { MappedVideo } from '../../../api/types'

interface PlayerEndScreenProps {
  currentVideoId: string
  onReplay: () => void
}

const PlayerEndScreen = memo(function PlayerEndScreen({ currentVideoId, onReplay }: PlayerEndScreenProps) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const [loading, setLoading] = useState(false)

  const handleNextVideo = useCallback(async () => {
    if (loading) return
    setLoading(true)
    try {
      const res = await listVideos({ size: 50 })
      const candidates = res.items
        .map(mapVideo)
        .filter((v): v is MappedVideo => !!v && v.id !== currentVideoId)
      if (candidates.length === 0) return
      const next = candidates[Math.floor(Math.random() * candidates.length)]
      if (next) navigate(`/player?id=${next.id}`)
    } catch {
      // ignore
    } finally {
      setLoading(false)
    }
  }, [currentVideoId, navigate, loading])

  return (
    <div className="video-ended-overlay">
      <span className="video-ended-overlay-title">{t('player.videoEnded')}</span>
      <div className="video-ended-actions">
        <button className="video-ended-btn video-ended-btn--replay" onClick={onReplay}>
          ↻ {t('player.replay')}
        </button>
        <button className="video-ended-btn video-ended-btn--next" onClick={handleNextVideo} disabled={loading}>
          ▶ {loading ? t('common.loading') : t('player.nextVideo')}
        </button>
      </div>
    </div>
  )
})

export default PlayerEndScreen
