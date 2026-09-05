import { useState, useEffect, useRef, memo, useCallback, useMemo, lazy, Suspense, useDeferredValue, useId, Component, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { useAuth } from '../../context/AuthContext'
import { burnVideo } from '../../api'
import { useToast } from '../../components/Toast/Toast'
import VideoCard from '../../components/VideoCard/VideoCard'
import { usePlayerShortcuts } from './usePlayerShortcuts'
import { trackClick } from '../../utils/track'
import PlayerControls from './PlayerControls'
import { ConfirmDialog, AlertDialog } from '../../components/ui'
import { usePlayerState } from './hooks/usePlayerState'
import { useShareHandler } from './useShareHandler'
import { useDeleteHandler } from './useDeleteHandler'
import { useFavoriteHandler } from './useFavoriteHandler'
import { usePlaylistHandler } from './usePlaylistHandler'
import PlayerEndScreen from './components/PlayerEndScreen'
import PlayerHeader from './components/PlayerHeader'
import PlayerLoading from './components/PlayerLoading'
import PlayerError from './components/PlayerError'
import VideoInfo from './components/VideoInfo'
import type { Playlist } from '../../api/playlists'
import './Player.css'

const LazyComments = lazy(() => import('../../components/Comments/Comments'))
const MemoComments = memo(LazyComments)

interface ErrorBoundaryProps { children: ReactNode; fallback?: ReactNode; t?: (k: string) => string }
interface ErrorBoundaryState { hasError: boolean; error: Error | null; errorType: 'network' | 'auth' | 'format' | 'unknown'; retryCount: number }

function classifyBoundaryError(error: Error | null): ErrorBoundaryState['errorType'] {
  if (!error) return 'unknown'
  const msg = error.message.toLowerCase()
  if (msg.includes('network') || msg.includes('fetch') || msg.includes('timeout')) return 'network'
  if (msg.includes('auth') || msg.includes('unauthorized') || msg.includes('token')) return 'auth'
  if (msg.includes('format') || msg.includes('codec') || msg.includes('source')) return 'format'
  return 'unknown'
}

const ERROR_TYPE_KEYS: Record<ErrorBoundaryState['errorType'], string> = {
  network: 'errors.videoNetwork',
  auth: 'errors.unauthorized',
  format: 'errors.videoFormatUnsupported',
  unknown: 'errors.unknownError',
}

const MAX_BOUNDARY_RETRIES = 3

class PlayerErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, error: null, errorType: 'unknown', retryCount: 0 }
  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { hasError: true, error, errorType: classifyBoundaryError(error) }
  }
  render() {
    const t = this.props.t ?? ((k: string) => k)
    if (this.state.hasError) {
      return this.props.fallback || (
        <div className="player-error-boundary">
          <span className="player-error-boundary-icon">⚠️</span>
          <p className="player-error-boundary-msg">{t(ERROR_TYPE_KEYS[this.state.errorType])}</p>
          {this.state.errorType === 'unknown' && this.state.error?.message && (
            <p className="player-error-boundary-detail">{this.state.error.message}</p>
          )}
          {this.state.retryCount < MAX_BOUNDARY_RETRIES ? (
            <button onClick={() => this.setState(prev => ({ hasError: false, error: null, retryCount: prev.retryCount + 1 }))}>{t('common.retry')}</button>
          ) : (
            <p className="player-error-boundary-hint">{t('errors.reloadPage')}</p>
          )}
        </div>
      )
    }
    return this.props.children
  }
}

const MemoPlaylistPickerDialog = memo(function PlaylistPickerDialog({
  playlists,
  onSelect,
  onClose,
  t,
}: {
  playlists: Playlist[]
  onSelect: (id: string) => void
  onClose: () => void
  t: (key: string) => string
}) {
  return (
    <div className="cd-overlay" onClick={onClose}>
      <div className="cd-dialog" role="dialog" aria-modal="true" aria-label={t('player.selectPlaylist')} onClick={e => e.stopPropagation()}>
        <h3 className="cd-title">{t('player.selectPlaylist')}</h3>
        <div className="player__playlist-list">
          {playlists.length === 0 ? (
            <p className="player__playlist-empty">{t('player.noPlaylistsHint')}</p>
          ) : (
            playlists.map((pl) => (
              <button key={pl.id} className="cd-btn cd-btn-outline player__playlist-btn" onClick={() => onSelect(pl.id)}>
                {pl.name} ({pl.item_count})
              </button>
            ))
          )}
        </div>
        <div className="cd-actions">
          <button className="cd-btn cd-btn-cancel" onClick={onClose}>{t('common.cancel')}</button>
        </div>
      </div>
    </div>
  )
})

const MemoKeyboardShortcutsHelp = memo(function KeyboardShortcutsHelp({ onClose, t }: { onClose: () => void; t: (key: string, opts?: Record<string, unknown>) => string }) {
  return (
    <div className="cd-overlay" onClick={onClose} role="dialog" aria-modal="true" aria-label={t('player.keyboardShortcuts')}>
      <div className="cd-dialog player-shortcuts-help" onClick={e => e.stopPropagation()}>
        <h3 className="cd-title">{t('player.keyboardShortcuts')}</h3>
        <div className="player-shortcuts-list">
          <div className="player-shortcut-item"><kbd>Space</kbd> / <kbd>K</kbd><span>{t('player.shortcutPlay')}</span></div>
          <div className="player-shortcut-item"><kbd>←</kbd> / <kbd>J</kbd><span>{t('player.shortcutSeekBack')}</span></div>
          <div className="player-shortcut-item"><kbd>→</kbd> / <kbd>L</kbd><span>{t('player.shortcutSeekForward')}</span></div>
          <div className="player-shortcut-item"><kbd>↑</kbd><span>{t('player.shortcutVolumeUp')}</span></div>
          <div className="player-shortcut-item"><kbd>↓</kbd><span>{t('player.shortcutVolumeDown')}</span></div>
          <div className="player-shortcut-item"><kbd>F</kbd><span>{t('player.fullscreen')}</span></div>
          <div className="player-shortcut-item"><kbd>M</kbd><span>{t('player.mute')}</span></div>
          <div className="player-shortcut-item"><kbd>P</kbd><span>{t('player.pictureInPicture')}</span></div>
          <div className="player-shortcut-item"><kbd>,</kbd><span>{t('player.shortcutSpeedSlower', { val: '' })}</span></div>
          <div className="player-shortcut-item"><kbd>.</kbd><span>{t('player.shortcutSpeedFaster', { val: '' })}</span></div>
          <div className="player-shortcut-item"><kbd>0</kbd>-<kbd>9</kbd><span>{t('player.shortcutPercent', { val: '' })}</span></div>
          <div className="player-shortcut-item"><kbd>?</kbd><span>{t('player.shortcutHelp')}</span></div>
        </div>
        <div className="cd-actions">
          <button className="cd-btn cd-btn-cancel" onClick={onClose}>{t('common.close')}</button>
        </div>
      </div>
    </div>
  )
})

const MemoAlertDialog = memo(AlertDialog)
const MemoConfirmDialog = memo(ConfirmDialog)

const Player = memo(function Player() {
  const { t } = useTranslation()
  const { user } = useAuth()
  const { toast } = useToast()
  const navigate = useNavigate()
  const shareTooltipId = useId()
  const playlistDialogId = useId()

  const videoRef = useRef<HTMLVideoElement>(null)
  const playerRef = useRef<HTMLDivElement>(null)

  const {
    videoId, isShared,
    video, loading, error,
    showLoading, videoError,
    paused, duration, speed,
    showSpeedMenu, setShowSpeedMenu,
    controlsVisible, shortcutText,
    related, variants,
    currentQuality,
    showQualityMenu, setShowQualityMenu,
    preloadingNext,
    hideTimerRef,
    playerWrapClassName, playerTopClassName, loadingClassName,
    resetHideTimer,
    togglePlay, toggleFullscreen, toggleMute, togglePiP,
    setSpeedValue, setVolumeValue,
    showShortcut, seekBy,
    switchQuality, retryLoad,
    onTimeUpdate, onPlay, onPause,
    onLoadedMetadata, onWaiting, onCanPlay, onPlaying, onError,
    onVolumeChange, onRateChange, onEnded,
    onMouseMove,
  } = usePlayerState(videoRef, playerRef)

  const [videoEnded, setVideoEnded] = useState(false)

  // ── 阅后即焚（平台全局行为）──
  // 所有视频、所有用户（含上传者）看完即焚毁。分享链接（cookie 会话）无
  // bearer 令牌无法调用焚毁接口，因此分享模式不启用确认门与自动焚毁。
  const burnGateNeeded = !isShared && !!user
  const [burnConfirmOpen, setBurnConfirmOpen] = useState(false)
  const [burnArmed, setBurnArmed] = useState(false)
  const [burned, setBurned] = useState(false)

  useEffect(() => {
    setBurnConfirmOpen(false)
    setBurnArmed(false)
    setBurned(false)
  }, [videoId])

  // 视频就绪后弹出确认门；确认前不允许起播（源加载完成会自动 play，这里压住）
  useEffect(() => {
    if (burnGateNeeded && !burnArmed && !burned && video) setBurnConfirmOpen(true)
  }, [burnGateNeeded, burnArmed, burned, video])

  useEffect(() => {
    if (burnGateNeeded && !burnArmed && !burned) videoRef.current?.pause()
  }, [burnGateNeeded, burnArmed, burned, videoId])

  const { handleShare, showShareTooltip, shareTooltipMsg, shareErrorType } = useShareHandler(user, video)
  const { handleDelete, handleDeleteConfirm, showDeleteDialog, setShowDeleteDialog, deleteAlertMsg, setDeleteAlertMsg } = useDeleteHandler(videoId)
  const { favorited, handleFavorite } = useFavoriteHandler(user, video, videoId, isShared)
  const { showPlaylistPicker, setShowPlaylistPicker, myPlaylists, handleAddToPlaylist } = usePlaylistHandler(user, video)

  const deferredShortcutText = useDeferredValue(shortcutText)

  useEffect(() => { setVideoEnded(false) }, [videoId])

  const handleFavoriteClick = useCallback(() => {
    handleFavorite().catch((e: Error) => {
      toast(e.message || t('player.favoriteFailed'), 'error')
    })
  }, [handleFavorite, toast, t])

  const handleAddToPlaylistClick = useCallback((playlistId: string) => {
    handleAddToPlaylist(playlistId).catch((e: Error) => {
      toast(e.message || t('player.addToPlaylistFailed'), 'error')
    })
  }, [handleAddToPlaylist, toast, t])

  const { showShortcutHelp, toggleShortcutHelp } = usePlayerShortcuts(videoRef, {
    togglePlay, toggleFullscreen, toggleMute, togglePiP,
    setVolumeValue, setSpeedValue, showShortcut, resetHideTimer,
    t,
  })

  const handleVideoEnded = useCallback(() => {
    setVideoEnded(true)
    onEnded()
    if (burnGateNeeded && videoId) {
      burnVideo(videoId)
        .then(() => setBurned(true))
        .catch((e: Error) => {
          toast(e.message || t('player.burnFailed'), 'error')
        })
    }
  }, [onEnded, burnGateNeeded, videoId, toast, t])

  const handleBurnConfirm = useCallback(() => {
    setBurnConfirmOpen(false)
    setBurnArmed(true)
    const v = videoRef.current
    if (v) v.play().catch(() => {})
  }, [])

  const handleBurnCancel = useCallback(() => {
    setBurnConfirmOpen(false)
    navigate(-1)
  }, [navigate])

  const handleBurnedAlertClose = useCallback(() => {
    setBurned(false)
    navigate('/')
  }, [navigate])

  const handleReplay = useCallback(() => {
    const v = videoRef.current
    if (v) { v.currentTime = 0; v.play().catch(() => {}) }
    setVideoEnded(false)
  }, [videoRef])

  const handleMouseLeave = useCallback(() => {
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
    hideTimerRef.current = setTimeout(() => {
      if (videoRef.current && !videoRef.current.paused) { /* hide handled by hook */ }
    }, 1000)
  }, [hideTimerRef, videoRef])

  const handleOpenPlaylistPicker = useCallback(() => setShowPlaylistPicker(true), [setShowPlaylistPicker])
  const handleClosePlaylistPicker = useCallback(() => setShowPlaylistPicker(false), [setShowPlaylistPicker])
  const handleCloseDeleteAlert = useCallback(() => setDeleteAlertMsg(''), [setDeleteAlertMsg])
  const handleCloseDeleteDialog = useCallback(() => setShowDeleteDialog(false), [setShowDeleteDialog])

  const cleanedTitle = useMemo(() => {
    if (!video) return ''
    return video.title.replace(/\.[^.]+$/, '').replace(/_/g, ' ').replace(/\s+/g, ' ').trim() || video.title
  }, [video?.title])

  const relatedGrid = useMemo(() => {
    if (related.length === 0) return null
    return (
      <section className="player-related">
        <h2 className="prs-title">{t('player.related')}</h2>
        <div className="prs-grid" onClick={(e) => { if ((e.target as HTMLElement).closest('.video-card')) trackClick('推荐视频点击') }}>
          {related.map((v) => (
            <VideoCard key={v.id} video={v} compact />
          ))}
        </div>
      </section>
    )
  }, [related, t])

  const loadingSkeleton = useMemo(() => {
    if (!loading) return null
    return (
      <div className="skeleton-detail">
        <div className="skeleton-meta">
          <div className="skeleton-tag" />
          <div className="skeleton-tag" />
        </div>
        <div className="skeleton-title" />
        <div className="skeleton-desc" />
        <div className="skeleton-desc" style={{ width: '55%' }} />
      </div>
    )
  }, [loading])

  const handlerRef = useRef({
    onTimeUpdate, onPlay, onPause, onLoadedMetadata, onWaiting,
    onCanPlay, onPlaying, onError, onVolumeChange, onRateChange,
  })
  handlerRef.current = {
    onTimeUpdate, onPlay, onPause, onLoadedMetadata, onWaiting,
    onCanPlay, onPlaying, onError, onVolumeChange, onRateChange,
  }

  const videoCallbacks = useMemo(() => {
    const r = handlerRef
    return {
      onTimeUpdate: () => r.current.onTimeUpdate(),
      onPlay: () => r.current.onPlay(),
      onPause: () => r.current.onPause(),
      onLoadedMetadata: () => r.current.onLoadedMetadata(),
      onWaiting: () => r.current.onWaiting(),
      onCanPlay: () => r.current.onCanPlay(),
      onPlaying: () => r.current.onPlaying(),
      onError: () => r.current.onError(),
      onVolumeChange: () => r.current.onVolumeChange(),
      onRateChange: () => r.current.onRateChange(),
    }
  }, [])

  if (error) {
    return <PlayerError message={error} />
  }

  return (
    <PlayerErrorBoundary t={t}>
    <div className="player-page" role="main">
      <div
        className={playerWrapClassName}
        ref={playerRef}
        onMouseMove={onMouseMove}
        onDoubleClick={toggleFullscreen}
        onTouchStart={resetHideTimer}
        onMouseLeave={handleMouseLeave}
      >
        <video
          ref={videoRef}
          className="player-video"
          playsInline
          preload="auto"
          aria-label={video?.title || t('player.videoPlayer')}
          aria-describedby={video?.description ? 'video-description' : undefined}
          {...videoCallbacks}
          onEnded={handleVideoEnded}
          onClick={togglePlay}
        />

        <PlayerHeader
          title={video?.title || ''}
          className={playerTopClassName}
          isAdmin={!!user?.isAdmin}
          onDelete={handleDelete}
        />

        <PlayerLoading className={loadingClassName} preloadingNext={preloadingNext} />

        {videoError && (
          <div className="player-center" role="alert">
            <p className="player-center__error-msg">{videoError}</p>
            <button className="center-play" onClick={retryLoad} aria-label={t('common.retry')}>↻</button>
          </div>
        )}

        {!showLoading && paused && !videoError && !videoEnded && (
          <div className="player-center">
            <button className="center-play" onClick={togglePlay}>▶</button>
          </div>
        )}

        {videoEnded && (
          <PlayerEndScreen currentVideoId={videoId} onReplay={handleReplay} />
        )}

        {deferredShortcutText && (
          <div className="shortcut-indicator" aria-live="polite">{deferredShortcutText}</div>
        )}

        <PlayerControls
            videoRef={videoRef}
            controlsVisible={controlsVisible}
            paused={paused}
            duration={duration}
            speed={speed}
            showQualityMenu={showQualityMenu}
            showSpeedMenu={showSpeedMenu}
            currentQuality={currentQuality}
            variants={variants}
            togglePlay={togglePlay}
            toggleMute={toggleMute}
            toggleFullscreen={toggleFullscreen}
            togglePiP={togglePiP}
            setSpeedValue={setSpeedValue}
            setVolumeValue={setVolumeValue}
            switchQuality={switchQuality}
            seekBy={seekBy}
            resetHideTimer={resetHideTimer}
            setShowQualityMenu={setShowQualityMenu}
            setShowSpeedMenu={setShowSpeedMenu}
            t={t}
          />
      </div>

      {video && (
        <>
          <VideoInfo
            video={video}
            cleanedTitle={cleanedTitle}
            favorited={favorited}
            onFavorite={handleFavoriteClick}
            onAddToPlaylist={handleOpenPlaylistPicker}
            onShare={handleShare}
            shareTooltipId={shareTooltipId}
            playlistDialogId={playlistDialogId}
            showShareTooltip={showShareTooltip}
            shareTooltipMsg={shareTooltipMsg}
            shareErrorType={shareErrorType}
            showPlaylistPicker={showPlaylistPicker}
          />
          {showPlaylistPicker && (
            <div id={playlistDialogId}>
              <MemoPlaylistPickerDialog
                playlists={myPlaylists}
                onSelect={handleAddToPlaylistClick}
                onClose={handleClosePlaylistPicker}
                t={t}
              />
            </div>
          )}
        </>
      )}

      {videoId && (
        <Suspense fallback={null}>
          <MemoComments videoId={videoId} />
        </Suspense>
      )}

      {relatedGrid}

      {showShortcutHelp && (
        <MemoKeyboardShortcutsHelp onClose={toggleShortcutHelp} t={t} />
      )}

      {deleteAlertMsg && (
        <MemoAlertDialog
          open={!!deleteAlertMsg}
          message={deleteAlertMsg}
          onClose={handleCloseDeleteAlert}
        />
      )}
      <MemoConfirmDialog
        open={showDeleteDialog}
        title={t('player.deleteConfirm')}
        message={t('player.deleteConfirmMessage')}
        danger
        onConfirm={handleDeleteConfirm}
        onCancel={handleCloseDeleteDialog}
      />
      <MemoConfirmDialog
        open={burnConfirmOpen}
        title={t('player.burnConfirmTitle')}
        message={t('player.burnConfirmMessage')}
        danger
        confirmVariant="danger"
        confirmText={t('player.burnWatch')}
        closeOnOverlay={false}
        onConfirm={handleBurnConfirm}
        onCancel={handleBurnCancel}
      />
      <MemoAlertDialog
        open={burned}
        title={t('player.burnedTitle')}
        message={t('player.burnedMessage')}
        onClose={handleBurnedAlertClose}
      />
      {loadingSkeleton}
    </div>
    </PlayerErrorBoundary>
  )
})

export default Player
