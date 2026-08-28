import { useState, useEffect, useCallback, useRef, useMemo, lazy, Suspense } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { useQueryClient } from '@tanstack/react-query'
import { useAuth } from '../../context/AuthContext'
import {
  loadPrefs, setPref, uploadAvatar,
  createPlaylist, deletePlaylist, revokeMyShare,
} from '../../api'
import type { ShareListItem } from '../../api'
import type { Playlist } from '../../api/playlists'
import AuthDialog from '../../components/AuthDialog/AuthDialog'
import { ConfirmDialog, AlertDialog } from '../../components/ui'
import { trackClick } from '../../utils/track'
import { useProfileData } from './hooks/useProfileData'
import { formatWatchTime } from './SharedComponents'
const WorksTab = lazy(() => import('./WorksTab'))
const HistoryTab = lazy(() => import('./HistoryTab'))
const FavoritesTab = lazy(() => import('./FavoritesTab'))
const PlaylistsTab = lazy(() => import('./PlaylistsTab'))
const SharesTab = lazy(() => import('./SharesTab'))
const SettingsTab = lazy(() => import('./SettingsTab'))
import './Profile.css'

type TabKey = 'works' | 'history' | 'likes' | 'playlists' | 'shares' | 'settings'

const MAX_AVATAR_SIZE = 5 * 1024 * 1024

interface ConfirmAction {
  title: string
  message: string
  danger?: boolean
  onConfirm: () => void | Promise<void>
}

export default function Profile() {
  const { user, logout, setUser } = useAuth()
  const navigate = useNavigate()
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [showAuth, setShowAuth] = useState(false)
  const [activeTab, setActiveTab] = useState<TabKey>('works')
  const [autoPlay, setAutoPlay] = useState(true)
  const [speedMem, setSpeedMem] = useState(false)
  const [avatarUploading, setAvatarUploading] = useState(false)
  const [avatarPreview, setAvatarPreview] = useState<string | null>(null)
  const avatarPreviewRef = useRef<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const [alertMsg, setAlertMsg] = useState<string | null>(null)
  const [confirmAction, setConfirmAction] = useState<ConfirmAction | null>(null)

  const { profile, worksQuery, worksTotal, works, history, favorites, playlists, shares } =
    useProfileData(user?.id, activeTab)

  useEffect(() => {
    const prefs = loadPrefs()
    setAutoPlay(prefs.autoPlay)
    setSpeedMem(prefs.speedMem)
  }, [])

  const handleAutoPlayChange = useCallback((checked: boolean) => {
    setAutoPlay(checked)
    setPref('autoPlay', checked)
  }, [])

  const handleSpeedMemChange = useCallback((checked: boolean) => {
    setSpeedMem(checked)
    setPref('speedMem', checked)
  }, [])

  const handleLogout = useCallback(async () => {
    await logout()
    navigate('/')
  }, [logout, navigate])

  const clearAvatarPreview = useCallback(() => {
    if (avatarPreviewRef.current) {
      URL.revokeObjectURL(avatarPreviewRef.current)
      avatarPreviewRef.current = null
    }
    setAvatarPreview(null)
  }, [])

  const handleAvatarClick = useCallback(() => fileInputRef.current?.click(), [])

  const handleAvatarUpload = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (fileInputRef.current) fileInputRef.current.value = ''
    if (!file) return
    if (!file.type.startsWith('image/')) {
      setAlertMsg(t('profile.chooseImage'))
      return
    }
    if (file.size > MAX_AVATAR_SIZE) {
      setAlertMsg(t('profile.avatarTooLarge'))
      return
    }
    clearAvatarPreview()
    const previewUrl = URL.createObjectURL(file)
    avatarPreviewRef.current = previewUrl
    setAvatarPreview(previewUrl)
    setAvatarUploading(true)
    try {
      const avatarUrl = await uploadAvatar(file)
      if (setUser && user) {
        setUser({ ...user, avatarUrl })
        queryClient.invalidateQueries({ queryKey: ['user-profile'] })
      }
      setAlertMsg(t('profile.avatarUpdated'))
    } catch (err) {
      setAlertMsg(err instanceof Error ? err.message : t('upload.uploadFailed'))
    } finally {
      setAvatarUploading(false)
      clearAvatarPreview()
    }
  }, [clearAvatarPreview, queryClient, setUser, user, t])

  const handleCreatePlaylist = useCallback(async (name: string) => {
    try {
      await createPlaylist({ name })
      queryClient.invalidateQueries({ queryKey: ['my-playlists', user?.id] })
    } catch (err) {
      setAlertMsg(err instanceof Error ? err.message : t('common.createFailed'))
    }
  }, [queryClient, user?.id, t])

  const handleDeletePlaylist = useCallback((p: Playlist) => {
    setConfirmAction({
      title: t('profile.deletePlaylistTitle'),
      message: t('profile.deletePlaylistConfirm', { name: p.name }),
      danger: true,
      onConfirm: async () => {
        try {
          await deletePlaylist(p.id)
          queryClient.invalidateQueries({ queryKey: ['my-playlists', user?.id] })
        } catch (err) {
          setAlertMsg(err instanceof Error ? err.message : t('common.deleteFailed'))
        }
      },
    })
  }, [queryClient, user?.id, t])

  const handleRevokeShare = useCallback(async (shareId: string) => {
    try {
      await revokeMyShare(shareId)
      queryClient.setQueryData<ShareListItem[]>(['my-shares', user?.id], (prev) => prev?.filter(s => s.id !== shareId))
    } catch (err) {
      console.error('revokeShare failed', err)
      setAlertMsg(err instanceof Error ? err.message : t('profile.revokeShareError'))
      queryClient.invalidateQueries({ queryKey: ['my-shares', user?.id] })
    }
  }, [queryClient, user?.id, t])

  const handleTabChange = useCallback((tab: TabKey, label: string) => {
    setActiveTab(tab)
    trackClick('switchTab', label)
  }, [])

  const tabs = useMemo<Array<{ key: TabKey; icon: string; label: string }>>(() => [
    { key: 'works', icon: '🎬', label: t('profile.works') },
    { key: 'history', icon: '🕐', label: t('profile.history') },
    { key: 'likes', icon: '❤️', label: t('profile.favorites') },
    { key: 'playlists', icon: '📋', label: t('profile.playlists') },
    { key: 'shares', icon: '🔗', label: t('profile.shares') },
    { key: 'settings', icon: '⚙️', label: t('profile.settings') },
  ], [t])

  const handleLogoutClick = useCallback(() => {
    setConfirmAction({
      title: t('profile.logout'),
      message: t('profile.logoutConfirm'),
      onConfirm: handleLogout,
    })
  }, [handleLogout, t])

  const handleAvatarKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      fileInputRef.current?.click()
    }
  }, [])

  const openAuth = useCallback(() => setShowAuth(true), [])
  const closeAuth = useCallback(() => setShowAuth(false), [])

  if (!user) {
    return (
      <div className="profile-page">
        <div className="profile-login-prompt">
          <div className="profile-login-avatar">?</div>
          <h2>{t('profile.notLoggedIn')}</h2>
          <p>{t('profile.loginPrompt')}</p>
          <button className="profile-login-btn" onClick={openAuth}>{t('nav.loginRegister')}</button>
        </div>
        {showAuth && <AuthDialog onClose={closeAuth} />}
      </div>
    )
  }

  return (
    <div className="profile-page">
      <div className="profile-header">
        <div className="profile-cover" />
        <div className="profile-info">
          <div
            className={`profile-avatar-wrap ${avatarUploading ? 'uploading' : ''}`}
            onClick={handleAvatarClick}
            onKeyDown={handleAvatarKeyDown}
            role="button"
            tabIndex={0}
            aria-label={t('profile.changeAvatar')}
          >
            <input
              ref={fileInputRef}
              type="file"
              accept="image/jpeg,image/png,image/webp,image/gif"
              onChange={handleAvatarUpload}
              tabIndex={-1}
              aria-hidden="true"
              style={{ display: 'none' }}
            />
            {avatarPreview ? (
              <img
                className="profile-avatar-img profile-avatar-preview"
                src={avatarPreview}
                alt={t('profile.avatarPreviewAlt')}
              />
            ) : user.avatarUrl ? (
              <img
                className="profile-avatar-img"
                src={user.avatarUrl}
                alt={t('profile.myAvatarAlt')}
              />
            ) : (
              <div className="profile-avatar">
                {avatarUploading ? '...' : (user.username?.[0]?.toUpperCase() || '?')}
              </div>
            )}
            <div className="profile-avatar-overlay">
              {avatarUploading ? t('upload.uploading') : t('profile.changeAvatar')}
            </div>
          </div>
          <div className="profile-data">
            <h1 className="profile-name">{user.username}</h1>
            <p className="profile-bio">{user.isAdmin ? t('profile.admin') : t('profile.user')}</p>
            <div className="profile-stats">
              <div className="stat-item">
                <span className="stat-num">{profile.isLoading ? '—' : (profile.data?.totalVideosWatched ?? 0)}</span>
                <span className="stat-label">{t('profile.stats.watched')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{profile.isLoading ? '—' : formatWatchTime(profile.data?.totalWatchTimeMs ?? 0, t)}</span>
                <span className="stat-label">{t('profile.stats.watchTime')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{worksQuery.isPending ? '—' : worksTotal}</span>
                <span className="stat-label">{t('profile.statWorks')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{profile.isLoading ? '—' : (profile.data?.createdAt ? profile.data.createdAt.slice(0, 4) : '-')}</span>
                <span className="stat-label">{t('profile.statJoined')}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="profile-tabs" role="tablist" aria-label={t('profile.tabsAria')}>
        {tabs.map((tab) => (
          <button
            key={tab.key}
            role="tab"
            aria-selected={activeTab === tab.key}
            className={`ptab ${activeTab === tab.key ? 'active' : ''}`}
            onClick={() => handleTabChange(tab.key, tab.label)}
          >
            <span className="ptab-icon" aria-hidden="true">{tab.icon}</span> {tab.label}
          </button>
        ))}
      </div>

      <Suspense fallback={<div className="tab-loading">{t('common.loading')}</div>}>
        {activeTab === 'works' && (
          <WorksTab
            works={works}
            pending={worksQuery.isPending}
            error={worksQuery.isError}
            isFetchingNextPage={worksQuery.isFetchingNextPage}
            hasNextPage={worksQuery.hasNextPage}
            fetchNextPage={worksQuery.fetchNextPage}
            refetch={worksQuery.refetch}
          />
        )}

        {activeTab === 'history' && (
          <HistoryTab
            history={history.data ?? []}
            pending={history.isPending}
            error={history.isError}
            refetch={history.refetch}
          />
        )}

        {activeTab === 'likes' && (
          <FavoritesTab
            favorites={favorites.data ?? []}
            pending={favorites.isPending}
            error={favorites.isError}
            refetch={favorites.refetch}
          />
        )}

        {activeTab === 'playlists' && (
          <PlaylistsTab
            playlists={playlists.data ?? []}
            pending={playlists.isPending}
            error={playlists.isError}
            refetch={playlists.refetch}
            onCreate={handleCreatePlaylist}
            onDelete={handleDeletePlaylist}
          />
        )}

        {activeTab === 'shares' && (
          <SharesTab
            shares={shares.data ?? []}
            pending={shares.isPending}
            error={shares.isError}
            refetch={shares.refetch}
            onRevoke={handleRevokeShare}
          />
        )}

        {activeTab === 'settings' && (
          <SettingsTab
            autoPlay={autoPlay}
            speedMem={speedMem}
            onAutoPlayChange={handleAutoPlayChange}
            onSpeedMemChange={handleSpeedMemChange}
            onLogout={handleLogoutClick}
            onAlert={setAlertMsg}
          />
        )}
      </Suspense>

      {alertMsg && <AlertDialog open={!!alertMsg} message={alertMsg} onClose={() => setAlertMsg(null)} />}
      {confirmAction && (
        <ConfirmDialog
          open={!!confirmAction}
          title={confirmAction.title}
          message={confirmAction.message}
          danger={confirmAction.danger}
          onConfirm={async () => { await confirmAction.onConfirm(); setConfirmAction(null) }}
          onCancel={() => setConfirmAction(null)}
        />
      )}
    </div>
  )
}
