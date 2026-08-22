import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate, Link } from 'react-router-dom'
import { useInfiniteQuery, useQuery, useQueryClient } from '@tanstack/react-query'
import { useAuth } from '../../context/AuthContext'
import {
  getUserProfile, listVideos, listPlaybackHistory, listFavorites, formatDuration,
  loadPrefs, setPref, uploadAvatar, listMyPlaylists, createPlaylist, deletePlaylist,
  listMyShares, revokeMyShare, sendVerificationEmail, updateEmail, mapVideo, mapHistory,
} from '../../api'
import type { UserProfile, MappedVideo, MappedHistory } from '../../api/types'
import type { Playlist } from '../../api/playlists'
import type { ShareListItem } from '../../api'
import VideoCard, { VideoCardSkeleton } from '../../components/VideoCard/VideoCard'
import AuthDialog from '../../components/AuthDialog/AuthDialog'
import { ConfirmDialog, AlertDialog, SkeletonLoader } from '../../components/ui'
import { trackClick } from '../../utils/track'
import './Profile.css'

type TabKey = 'works' | 'history' | 'likes' | 'playlists' | 'shares' | 'settings'

const WORKS_PAGE_SIZE = 24
const HISTORY_LIMIT = 100
const MAX_AVATAR_SIZE = 5 * 1024 * 1024

interface ConfirmAction {
  title: string
  message: string
  danger?: boolean
  onConfirm: () => void | Promise<void>
}

// 累计时长展示：xx 分钟 / x 小时 y 分
function formatWatchTime(ms: number, t: (k: string, o?: Record<string, unknown>) => string): string {
  const totalMin = Math.floor((ms || 0) / 60000)
  if (totalMin < 60) return t('common.minutes', { n: totalMin })
  const h = Math.floor(totalMin / 60)
  const m = totalMin % 60
  return m > 0 ? t('common.hoursMin', { h, m }) : t('common.hoursOnly', { h })
}

// 播放历史 / 收藏共用的行组件
function HistoryRow({ item, showProgress }: { item: MappedHistory; showProgress?: boolean }) {
  const navigate = useNavigate()
  const { t } = useTranslation()

  const open = useCallback(() => {
    trackClick(showProgress ? '继续观看' : '打开收藏', item.title)
    navigate(`/player?id=${item.id}`)
  }, [navigate, item.id, item.title, showProgress])

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      open()
    }
  }

  const progress = Math.max(0, Math.min(100, item.progress))

  return (
    <div
      className="history-item"
      role="button"
      tabIndex={0}
      onClick={open}
      onKeyDown={onKeyDown}
      aria-label={item.title}
    >
      <div className="history-thumb">
        {item.thumb ? (
          <img
            src={item.thumb}
            alt=""
            loading="lazy"
            onError={(e) => { e.currentTarget.style.display = 'none' }}
          />
        ) : null}
        {item.durationMs > 0 && (
          <span className="history-dur">{formatDuration(Math.floor(item.durationMs / 1000))}</span>
        )}
      </div>
      <div className="history-info">
        <div className="history-title">
          {item.title}
          {showProgress && progress > 0 && (
            <span className="history-continue">{t('common.continue', { progress })}</span>
          )}
        </div>
        <div className="history-meta">
          {[item.category, item.updatedAt ? new Date(item.updatedAt).toLocaleDateString() : ''].filter(Boolean).join(' · ')}
        </div>
        {showProgress && item.durationMs > 0 && (
          <div className="history-progress" aria-hidden="true">
            <div className="hp-fill" style={{ width: `${progress}%` }} />
          </div>
        )}
      </div>
    </div>
  )
}

// 列表骨架屏（历史/收藏用，委托给统一 SkeletonLoader）
function ListSkeleton() {
  return (
    <div className="history-list" aria-hidden="true">
      <SkeletonLoader type="video-card" lines={4} />
    </div>
  )
}

// 区块加载失败 + 重试
function TabError({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation()
  return (
    <div className="tab-error">
      <div className="empty-icon">⚠️</div>
      <div className="tab-error-text">{t('errors.loadFailedNetwork')}</div>
      <button className="profile-retry" onClick={onRetry}>{t('common.retry')}</button>
    </div>
  )
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
  const [newPlaylistName, setNewPlaylistName] = useState('')
  const [showNewPlaylist, setShowNewPlaylist] = useState(false)
  const [sendingVerification, setSendingVerification] = useState(false)
  const [editingEmail, setEditingEmail] = useState(false)
  const [emailValue, setEmailValue] = useState('')
  const [savingEmail, setSavingEmail] = useState(false)
  const [creatingPlaylist, setCreatingPlaylist] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Dialog state
  const [alertMsg, setAlertMsg] = useState<string | null>(null)
  const [confirmAction, setConfirmAction] = useState<ConfirmAction | null>(null)

  // 用户资料与统计
  const { data: profile, isLoading: loadingProfile } = useQuery<UserProfile>({
    queryKey: ['user-profile', user?.id],
    queryFn: getUserProfile,
    enabled: !!user,
    staleTime: 60_000,
  })

  // 我发布的视频（分页加载更多）
  const {
    data: worksData,
    isPending: worksPending,
    isError: worksError,
    isFetchingNextPage,
    hasNextPage,
    fetchNextPage,
    refetch: refetchWorks,
  } = useInfiniteQuery({
    queryKey: ['my-works', user?.id],
    queryFn: ({ pageParam }) => listVideos({
      type: 'local_video',
      size: WORKS_PAGE_SIZE,
      uploaderId: user?.id,
      page: pageParam,
    }),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      const loaded = allPages.reduce((n, p) => n + p.items.length, 0)
      return loaded < lastPage.total ? allPages.length : undefined
    },
    enabled: !!user,
    staleTime: 30_000,
  })

  const worksTotal = worksData?.pages[0]?.total ?? 0

  const works = useMemo(() => {
    const seen = new Set<string>()
    const list: MappedVideo[] = []
    for (const page of worksData?.pages ?? []) {
      for (const raw of page.items) {
        const v = mapVideo(raw)
        if (v && !seen.has(v.id)) {
          seen.add(v.id)
          list.push(v)
        }
      }
    }
    return list
  }, [worksData])

  // 播放历史（进入 Tab 时才加载）
  const { data: history = [], isPending: historyPending, isError: historyError, refetch: refetchHistory } = useQuery({
    queryKey: ['my-history', user?.id],
    queryFn: () => listPlaybackHistory(HISTORY_LIMIT).then(
      (h) => h.map(mapHistory).filter((x): x is MappedHistory => !!x)
    ),
    enabled: activeTab === 'history' && !!user,
  })

  // 收藏（进入 Tab 时才加载）
  const { data: favorites = [], isPending: favoritesPending, isError: favoritesError, refetch: refetchFavorites } = useQuery({
    queryKey: ['my-favorites', user?.id],
    queryFn: () => listFavorites().then(
      (f) => f.map(mapHistory).filter((x): x is MappedHistory => !!x)
    ),
    enabled: activeTab === 'likes' && !!user,
  })

  // 我的播放列表
  const { data: playlists = [], isPending: playlistsPending, isError: playlistsError, refetch: refetchPlaylists } = useQuery({
    queryKey: ['my-playlists', user?.id],
    queryFn: listMyPlaylists,
    enabled: activeTab === 'playlists' && !!user,
  })

  // 我的分享
  const { data: shares = [], isPending: sharesPending, isError: sharesError, refetch: refetchShares } = useQuery({
    queryKey: ['my-shares', user?.id],
    queryFn: listMyShares,
    enabled: activeTab === 'shares' && !!user,
  })

  const [revokingId, setRevokingId] = useState<string | null>(null)

  useEffect(() => {
    const prefs = loadPrefs()
    setAutoPlay(prefs.autoPlay)
    setSpeedMem(prefs.speedMem)
  }, [])

  const handleAutoPlayChange = (checked: boolean) => {
    setAutoPlay(checked)
    setPref('autoPlay', checked)
  }

  const handleSpeedMemChange = (checked: boolean) => {
    setSpeedMem(checked)
    setPref('speedMem', checked)
  }

  const handleLogout = async () => {
    await logout()
    navigate('/')
  }

  const handleLogoutClick = () => {
    setConfirmAction({
      title: t('profile.logout'),
      message: t('profile.logoutConfirm'),
      onConfirm: handleLogout,
    })
  }

  const clearAvatarPreview = useCallback(() => {
    if (avatarPreviewRef.current) {
      URL.revokeObjectURL(avatarPreviewRef.current)
      avatarPreviewRef.current = null
    }
    setAvatarPreview(null)
  }, [])

  const handleAvatarClick = () => {
    fileInputRef.current?.click()
  }

  const handleAvatarUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    // 提前清空 input，保证再次选择同一文件也能触发 onChange
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
    // 上传前先展示本地预览，失败时回滚为原头像
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
  }

  const handleSendVerification = async () => {
    setSendingVerification(true)
    try {
      const res = await sendVerificationEmail()
      setAlertMsg(res.message || t('profile.verifySent'))
    } catch (err) {
      setAlertMsg(err instanceof Error ? err.message : t('profile.sendFailed'))
    } finally {
      setSendingVerification(false)
    }
  }

  const handleSaveEmail = async () => {
    const email = emailValue.trim().toLowerCase()
    if (!email || !email.includes('@')) {
      setAlertMsg(t('auth.validation.emailInvalid'))
      return
    }
    setSavingEmail(true)
    try {
      await updateEmail(email)
      if (setUser && user) {
        setUser({ ...user, email, emailVerified: false })
        queryClient.invalidateQueries({ queryKey: ['user-profile'] })
      }
      setEditingEmail(false)
      setAlertMsg(t('profile.emailUpdated'))
    } catch (err) {
      setAlertMsg(err instanceof Error ? err.message : t('common.saveFailed'))
    } finally {
      setSavingEmail(false)
    }
  }

  const handleCreatePlaylist = async () => {
    const name = newPlaylistName.trim()
    if (!name || creatingPlaylist) return
    setCreatingPlaylist(true)
    try {
      await createPlaylist({ name })
      setNewPlaylistName('')
      setShowNewPlaylist(false)
      queryClient.invalidateQueries({ queryKey: ['my-playlists', user?.id] })
    } catch (err) {
      setAlertMsg(err instanceof Error ? err.message : t('common.createFailed'))
    } finally {
      setCreatingPlaylist(false)
    }
  }

  const handleDeletePlaylist = (p: Playlist) => {
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
  }

  const handleRevokeShare = async (shareId: string) => {
    if (revokingId === shareId) return
    setRevokingId(shareId)
    try {
      await revokeMyShare(shareId)
      // 乐观移除；失败时刷新列表与后端对齐
      queryClient.setQueryData<ShareListItem[]>(['my-shares', user?.id], (prev) => prev?.filter(s => s.id !== shareId))
    } catch (err) {
      console.error('revokeShare failed', err)
      setAlertMsg(err instanceof Error ? err.message : t('profile.revokeShareError'))
      queryClient.invalidateQueries({ queryKey: ['my-shares', user?.id] })
    } finally {
      setRevokingId(null)
    }
  }

  const handleTabChange = (tab: TabKey, label: string) => {
    setActiveTab(tab)
    trackClick('切换Tab', label)
  }

  if (!user) {
    return (
      <div className="profile-page">
        <div className="profile-login-prompt">
          <div className="profile-login-avatar">?</div>
          <h2>{t('profile.notLoggedIn')}</h2>
          <p>{t('profile.loginPrompt')}</p>
          <button className="profile-login-btn" onClick={() => setShowAuth(true)}>{t('nav.loginRegister')}</button>
        </div>
        {showAuth && <AuthDialog onClose={() => setShowAuth(false)} />}
      </div>
    )
  }

  const tabs: Array<{ key: TabKey; icon: string; label: string }> = [
    { key: 'works', icon: '🎬', label: t('profile.works') },
    { key: 'history', icon: '🕐', label: t('profile.history') },
    { key: 'likes', icon: '❤️', label: t('profile.favorites') },
    { key: 'playlists', icon: '📋', label: t('profile.playlists') },
    { key: 'shares', icon: '🔗', label: t('profile.shares') },
    { key: 'settings', icon: '⚙️', label: t('profile.settings') },
  ]

  return (
    <div className="profile-page">
      <div className="profile-header">
        <div className="profile-cover" />
        <div className="profile-info">
          <div
            className={`profile-avatar-wrap ${avatarUploading ? 'uploading' : ''}`}
            onClick={handleAvatarClick}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                handleAvatarClick()
              }
            }}
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
                <span className="stat-num">{loadingProfile ? '—' : (profile?.totalVideosWatched ?? 0)}</span>
                <span className="stat-label">{t('profile.stats.watched')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{loadingProfile ? '—' : formatWatchTime(profile?.totalWatchTimeMs ?? 0, t)}</span>
                <span className="stat-label">{t('profile.stats.watchTime')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{worksPending ? '—' : worksTotal}</span>
                <span className="stat-label">{t('profile.statWorks')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{loadingProfile ? '—' : (profile?.createdAt ? profile.createdAt.slice(0, 4) : '-')}</span>
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

      {/* Works tab */}
      {activeTab === 'works' && (
        <div className="profile-content active" role="tabpanel">
          {worksPending && works.length === 0 ? (
            <div className="profile-grid">
              <VideoCardSkeleton count={8} />
            </div>
          ) : worksError && works.length === 0 ? (
            <TabError onRetry={() => refetchWorks()} />
          ) : works.length > 0 ? (
            <>
              <div className="profile-grid">
                {works.map((v) => (
                  <VideoCard key={v.id} video={v} />
                ))}
              </div>
              {worksError && (
                <div className="load-more-error">
                  <span>{t('errors.loadFailed')}</span>
                  <button className="profile-retry" onClick={() => refetchWorks()}>{t('common.retry')}</button>
                </div>
              )}
              {hasNextPage ? (
                <button className="load-more-btn" onClick={() => fetchNextPage()} disabled={isFetchingNextPage}>
                  {isFetchingNextPage ? t('common.loading') : t('common.loadMore')}
                </button>
              ) : (
                works.length > 0 && <div className="pm-no-more">{t('common.noMore')}</div>
              )}
            </>
          ) : (
            <div className="profile-empty" role="status" aria-live="polite">
              <div className="empty-icon" aria-hidden="true">🎬</div>
              <div>{t('profile.noWorks')}</div>
              <Link to="/upload" className="empty-cta">去上传第一个视频 →</Link>
            </div>
          )}
        </div>
      )}

      {/* History tab */}
      {activeTab === 'history' && (
        <div className="profile-content active" role="tabpanel">
          {historyPending ? (
            <ListSkeleton />
          ) : historyError ? (
            <TabError onRetry={() => refetchHistory()} />
          ) : history.length > 0 ? (
            <div className="history-list">
              {history.map((h) => (
                <HistoryRow key={h.id} item={h} showProgress />
              ))}
            </div>
          ) : (
            <div className="profile-empty" role="status">
              <div className="empty-icon" aria-hidden="true">🕐</div>
              <div>{t('profile.noHistory')}</div>
              <Link to="/gallery" className="empty-cta">去发现精彩 →</Link>
            </div>
          )}
        </div>
      )}

      {/* Likes tab */}
      {activeTab === 'likes' && (
        <div className="profile-content active" role="tabpanel">
          {favoritesPending ? (
            <ListSkeleton />
          ) : favoritesError ? (
            <TabError onRetry={() => refetchFavorites()} />
          ) : favorites.length > 0 ? (
            <div className="history-list">
              {favorites.map((h) => (
                <HistoryRow key={h.id} item={h} />
              ))}
            </div>
          ) : (
            <div className="profile-empty">
              <div className="empty-icon">❤️</div>
              <div>{t('profile.noFavorites')}</div>
              <Link to="/gallery" className="empty-cta">去逛逛 →</Link>
            </div>
          )}
        </div>
      )}

      {/* Playlists tab */}
      {activeTab === 'playlists' && (
        <div className="profile-content active" role="tabpanel">
          <div className="playlists-header">
            <button className="profile-btn" onClick={() => setShowNewPlaylist(true)}>{t('profile.newPlaylist')}</button>
          </div>

          {showNewPlaylist && (
            <div className="new-playlist-form">
              <input
                type="text"
                value={newPlaylistName}
                onChange={e => setNewPlaylistName(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleCreatePlaylist() }}
                placeholder={t('profile.playlistNamePlaceholder')}
                maxLength={200}
                autoFocus
              />
              <button className="profile-btn" onClick={handleCreatePlaylist} disabled={!newPlaylistName.trim()}>{t('common.create')}</button>
              <button className="profile-btn-secondary" onClick={() => { setShowNewPlaylist(false); setNewPlaylistName('') }}>{t('common.cancel')}</button>
            </div>
          )}

          {playlistsPending ? (
            <div className="tab-loading">{t('common.loading')}</div>
          ) : playlistsError ? (
            <TabError onRetry={() => refetchPlaylists()} />
          ) : playlists.length > 0 ? (
            <div className="playlists-list">
              {playlists.map(p => (
                <div key={p.id} className="playlist-item">
                  <div className="playlist-info">
                    <span className="playlist-name">📋 {p.name}</span>
                    <span className="playlist-meta">
                      {t('profile.itemCount', { count: p.item_count })} · {p.is_public ? t('profile.public') : t('profile.private')} · {t('profile.createdAt', { date: new Date(p.created_at).toLocaleDateString() })}
                    </span>
                  </div>
                  <button className="profile-btn-danger" onClick={() => handleDeletePlaylist(p)}>{t('profile.deletePlaylist')}</button>
                </div>
              ))}
            </div>
          ) : (
            <div className="profile-empty">
              <div className="empty-icon">📋</div>
              <div>{t('profile.noPlaylists')}</div>
              <Link to="" onClick={e => { e.preventDefault(); setShowNewPlaylist(true) }} className="empty-cta" role="button">
                {t('profile.newPlaylist')} +
              </Link>
            </div>
          )}
        </div>
      )}

      {/* Shares tab */}
      {activeTab === 'shares' && (
        <div className="profile-content active" role="tabpanel">
          {sharesPending ? (
            <div className="tab-loading">{t('common.loading')}</div>
          ) : sharesError ? (
            <TabError onRetry={() => refetchShares()} />
          ) : shares.length > 0 ? (
            <div className="shares-list">
              {shares.map(s => (
                <div key={s.id} className="share-item">
                  <div className="share-info">
                    <span className={`share-status ${s.active ? 'active' : 'expired'}`}>
                      {s.active ? t('profile.shareActive') : t('profile.shareExpired')}
                    </span>
                    <span className="share-meta">
                      {t('profile.createdAt', { date: new Date(s.createdAt).toLocaleDateString() })}
                      {s.expiresAt ? ` · ${t('profile.expiresAt', { date: new Date(s.expiresAt).toLocaleDateString() })}` : ` · ${t('profile.neverExpires')}`}
                    </span>
                  </div>
                  <button className="profile-btn-danger" onClick={() => handleRevokeShare(s.id)} disabled={revokingId === s.id}>
                    {revokingId === s.id ? t('common.loading') : t('profile.revokeShare')}
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div className="profile-empty" role="status">
              <div className="empty-icon" aria-hidden="true">🔗</div>
              <div>{t('profile.noShares')}</div>
              <p style={{fontSize:'13px', color:'var(--text3)', marginTop:'8px'}}>观看视频后可在播放页创建分享链接</p>
              <Link to="/" className="empty-cta">去首页观看 →</Link>
            </div>
          )}
        </div>
      )}

      {/* Settings tab */}
      {activeTab === 'settings' && (
        <div className="profile-content active" role="tabpanel">
          <div className="settings-section">
            <h3 className="settings-title">{t('profile.settingsPlayback')}</h3>
            <div className="settings-group">
              <div className="settings-row">
                <div>
                  <span className="settings-label">{t('profile.autoPlay')}</span>
                  <span className="settings-desc">{t('profile.autoPlayDesc')}</span>
                </div>
                <label className="toggle">
                  <input type="checkbox" checked={autoPlay} onChange={(e) => handleAutoPlayChange(e.target.checked)} />
                  <span className="toggle-track" aria-hidden="true" />
                </label>
              </div>
              <div className="settings-row">
                <div>
                  <span className="settings-label">{t('profile.speedMem')}</span>
                  <span className="settings-desc">{t('profile.speedMemDesc')}</span>
                </div>
                <label className="toggle">
                  <input type="checkbox" checked={speedMem} onChange={(e) => handleSpeedMemChange(e.target.checked)} />
                  <span className="toggle-track" aria-hidden="true" />
                </label>
              </div>
            </div>
          </div>

          <div className="settings-section">
            <h3 className="settings-title">{t('profile.account')}</h3>
            <div className="settings-group">
              <div className="settings-row">
                <div>
                  <span className="settings-label">{t('auth.username')}</span>
                  <span className="settings-desc">{user.username}</span>
                </div>
                <span className="settings-value">{user.isAdmin ? t('profile.admin') : t('profile.normalUser')}</span>
              </div>
              <div className="settings-row">
                <div>
                  <span className="settings-label">{t('auth.email')}</span>
                  {editingEmail ? (
                    <div className="email-edit">
                      <input
                        type="email"
                        className="email-input"
                        placeholder={t('auth.email')}
                        value={emailValue}
                        onChange={(e) => setEmailValue(e.target.value)}
                        onKeyDown={(e) => { if (e.key === 'Enter') handleSaveEmail() }}
                        autoFocus
                      />
                      <div className="email-actions">
                        <button className="profile-btn" onClick={handleSaveEmail} disabled={savingEmail}>
                          {savingEmail ? t('common.saving') : t('common.save')}
                        </button>
                        <button className="profile-btn-secondary" onClick={() => setEditingEmail(false)}>{t('common.cancel')}</button>
                      </div>
                    </div>
                  ) : (
                    <span className="settings-desc">{user.email || t('profile.notSet')}</span>
                  )}
                </div>
                {!editingEmail && (
                  <div className="email-actions">
                    {user.email ? (
                      <>
                        {user.emailVerified ? (
                          <span className="settings-value" style={{ color: 'var(--green)' }}>{t('profile.emailVerified')}</span>
                        ) : (
                          <button
                            className="profile-btn"
                            onClick={handleSendVerification}
                            disabled={sendingVerification}
                          >
                            {sendingVerification ? t('common.sending') : t('profile.verify')}
                          </button>
                        )}
                        <button className="profile-btn-secondary" onClick={() => { setEmailValue(user.email || ''); setEditingEmail(true) }}>
                          {t('profile.modify')}
                        </button>
                      </>
                    ) : (
                      <button className="profile-btn" onClick={() => { setEmailValue(''); setEditingEmail(true) }}>
                        {t('profile.bindEmail')}
                      </button>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>

          <button className="settings-logout" onClick={handleLogoutClick}>{t('profile.logout')}</button>
        </div>
      )}

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
