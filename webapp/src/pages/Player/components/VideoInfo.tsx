import { memo, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { getCatColor } from '../../../api'
import type { MappedVideo } from '../../../api/types'
import type { ShareErrorType } from '../useShareHandler'

interface VideoInfoProps {
  video: MappedVideo
  cleanedTitle: string
  favorited: boolean
  onFavorite: () => void
  onAddToPlaylist: () => void
  onShare: () => void
  shareTooltipId: string
  playlistDialogId: string
  showShareTooltip: boolean
  shareTooltipMsg: string
  shareErrorType?: ShareErrorType
  showPlaylistPicker: boolean
}

const VideoInfo = memo(function VideoInfo({
  video, cleanedTitle, favorited,
  onFavorite, onAddToPlaylist, onShare,
  shareTooltipId, playlistDialogId,
  showShareTooltip, shareTooltipMsg, shareErrorType,
}: VideoInfoProps) {
  const { t } = useTranslation()

  const categoryColor = useMemo(() => {
    const color = getCatColor(video.category)
    return { background: color + '1a', color } as React.CSSProperties
  }, [video.category])

  return (
    <div className="player-detail" id="player-detail-section">
      <div className="pd-meta">
        <span style={categoryColor}>{video.category || t('common.other')}</span>
        <button className={`pd-action-btn ${favorited ? 'favorited' : ''}`} onClick={onFavorite} aria-label={favorited ? t('player.unfavorited') : t('player.favorite')}>
          {favorited ? '❤️' : '♡'} {favorited ? t('player.favorited') : t('player.favorite')}
        </button>
        <button className="pd-action-btn" onClick={onAddToPlaylist} aria-controls={playlistDialogId}>
          ➕ {t('player.addToPlaylist')}
        </button>
        <button className="pd-share-btn" onClick={onShare} aria-describedby={shareTooltipId}>
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7c.05-.23.09-.46.09-.7s-.04-.47-.09-.7l7.05-4.11c.54.5 1.25.81 2.04.81 1.66 0 3-1.34 3-3s-1.34-3-3-3-3 1.34-3 3c0 .24.04.47.09.7L8.04 9.81C7.5 9.31 6.79 9 6 9c-1.66 0-3 1.34-3 3s1.34 3 3 3c.79 0 1.5-.31 2.04-.81l7.12 4.16c-.05.21-.08.43-.08.65 0 1.61 1.31 2.92 2.92 2.92 1.61 0 2.92-1.31 2.92-2.92s-1.31-2.92-2.92-2.92z"/></svg>
          {t('player.share')}
        </button>
        {showShareTooltip && (
          <span className={`pd-share-tooltip ${shareErrorType ? 'pd-share-tooltip--error' : ''}`} id={shareTooltipId} key={shareTooltipMsg + Date.now()}>{shareTooltipMsg}</span>
        )}
      </div>
      <h1 className="pd-title">{cleanedTitle}</h1>
      {video.description && <p className="pd-desc" id="video-description">{video.description}</p>}
    </div>
  )
})

export default VideoInfo
