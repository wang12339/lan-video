import { useState, useEffect, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { useAuth } from '../../context/AuthContext'
import { getUserProfile, listVideos, listPlaybackHistory, listFavorites, formatCount, loadPrefs, setPref, uploadAvatar, listMyPlaylists, createPlaylist, deletePlaylist, listMyShares, revokeMyShare } from '../../api'
import type { UserProfile, MappedVideo, MappedHistory } from '../../api/types'
import type { Playlist } from '../../api/playlists'
import type { ShareListItem } from '../../api'
import { mapVideo, mapHistory } from '../../api'
import VideoCard from '../../components/VideoCard/VideoCard'
import AuthDialog from '../../components/AuthDialog/AuthDialog'
import { ConfirmDialog, AlertDialog } from '../../components/ui'
import './Profile.css'

type TabKey = 'works' | 'history' | 'likes' | 'playlists' | 'shares' | 'settings'

export default function Profile() {
  const { user, logout, setUser } = useAuth()
  const navigate = useNavigate()
  const { t } = useTranslation()
  const [showAuth, setShowAuth] = useState(false)
  const [activeTab, setActiveTab] = useState<TabKey>('works')
  const [profile, setProfile] = useState<UserProfile | null>(null)
  const [videos, setVideos] = useState<MappedVideo[]>([])
  const [history, setHistory] = useState<MappedHistory[]>([])
  const [favorites, setFavorites] = useState<MappedHistory[]>([])
  const [loadingProfile, setLoadingProfile] = useState(true)
  const [autoPlay, setAutoPlay] = useState(true)
  const [speedMem, setSpeedMem] = useState(false)
  const [avatarUploading, setAvatarUploading] = useState(false)
  const [playlists, setPlaylists] = useState<Playlist[]>([])
  const [newPlaylistName, setNewPlaylistName] = useState('')
  const [showNewPlaylist, setShowNewPlaylist] = useState(false)
  const [shares, setShares] = useState<ShareListItem[]>([])
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Dialog state
  const [alertMsg, setAlertMsg] = useState<string | null>(null)
  const [confirmAction, setConfirmAction] = useState<{ name: string; onConfirm: () => void } | null>(null)

  const loadProfile = useCallback(async () => {
    setLoadingProfile(true)
    try {
      const p = await getUserProfile()
      setProfile(p)
    } catch (e) {
      console.error('loadProfile failed', e)
    } finally {
      setLoadingProfile(false)
    }
  }, [])

  const loadVideos = useCallback(async () => {
    try {
      const r = await listVideos({ type: 'local_video', size: 30, uploaderId: user?.id })
      setVideos(r.items.map(mapVideo).filter((v): v is MappedVideo => !!v))
    } catch (e) { console.error('loadVideos failed', e); }
  }, [])

  const loadHistory = useCallback(async () => {
    try {
      const h = await listPlaybackHistory()
      setHistory(h.map(mapHistory).filter((h): h is MappedHistory => !!h))
    } catch (e) { console.error('loadHistory failed', e); }
  }, [])

  const loadFavorites = useCallback(async () => {
    try {
      const f = await listFavorites()
      setFavorites(f.map(mapHistory).filter((h): h is MappedHistory => !!h))
    } catch (e) { console.error('loadFavorites failed', e); }
  }, [])

  const loadPlaylists = useCallback(async () => {
    try {
      const p = await listMyPlaylists()
      setPlaylists(p)
    } catch (e) { console.error('loadPlaylists failed', e); }
  }, [])

  const loadShares = useCallback(async () => {
    try {
      const s = await listMyShares()
      setShares(s)
    } catch (e) { console.error('loadShares failed', e); }
  }, [])

  const [revokingId, setRevokingId] = useState<number | null>(null)

  const handleRevokeShare = useCallback(async (shareId: number) => {
    if (revokingId === shareId) return
    setRevokingId(shareId)
    try {
      await revokeMyShare(shareId)
      setShares(prev => prev.filter(s => s.id !== shareId))
    } catch (e: unknown) {
      const s = shares.find(s => s.id === shareId)
      if (!s) {
        // 分享已不存在（可能已被其他设备删除），直接刷新列表
        loadShares()
        return
      }
      console.error('revokeShare failed', e)
      const msg = e instanceof Error ? e.message : t('profile.revokeShareError')
      setAlertMsg(msg)
    } finally {
      setRevokingId(null)
    }
  }, [shares, revokingId, loadShares])

  useEffect(() => {
    loadProfile()
    loadVideos()
  }, [loadProfile, loadVideos])

  useEffect(() => {
    const prefs = loadPrefs()
    setAutoPlay(prefs.autoPlay)
    setSpeedMem(prefs.speedMem)
  }, [])

  useEffect(() => {
    if (activeTab === 'history') loadHistory()
    if (activeTab === 'likes') loadFavorites()
    if (activeTab === 'playlists') loadPlaylists()
    if (activeTab === 'shares') loadShares()
  }, [activeTab, loadHistory, loadFavorites, loadPlaylists, loadShares])

  const onTabChange = (tab: TabKey) => {
    setActiveTab(tab)
  }

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

  const handleAvatarClick = () => {
    fileInputRef.current?.click()
  }

  const handleAvatarUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    if (file.size > 5 * 1024 * 1024) {
      setAlertMsg('头像文件不能超过 5MB')
      return
    }
    setAvatarUploading(true)
    try {
      const avatarUrl = await uploadAvatar(file)
      // Update local user state
      if (setUser && user) {
        setUser({ ...user, avatarUrl })
      }
    } catch (err) {
      setAlertMsg(err instanceof Error ? err.message : '上传失败')
    } finally {
      setAvatarUploading(false)
      if (fileInputRef.current) fileInputRef.current.value = ''
    }
  }

  const handleCreatePlaylist = async () => {
    const name = newPlaylistName.trim()
    if (!name) return
    try {
      await createPlaylist({ name })
      setNewPlaylistName('')
      setShowNewPlaylist(false)
      loadPlaylists()
    } catch (err) {
      setAlertMsg(err instanceof Error ? err.message : '创建失败')
    }
  }

  const handleDeletePlaylist = (id: number, name: string) => {
    setConfirmAction({
      name,
      onConfirm: async () => {
        try {
          await deletePlaylist(id)
          loadPlaylists()
        } catch (err) {
          setAlertMsg(err instanceof Error ? err.message : '删除失败')
        }
      }
    })
  }

  if (!user) {
    return (
      <div className="profile-page">
        <div className="profile-login-prompt">
          <div className="profile-login-avatar">?</div>
          <h2>未登录</h2>
          <p>请先登录后查看个人中心</p>
          <button className="profile-login-btn" onClick={() => setShowAuth(true)}>登录 / 注册</button>
        </div>
        {showAuth && <AuthDialog onClose={() => setShowAuth(false)} />}
      </div>
    )
  }

  return (
    <div className="profile-page">
      <div className="profile-header">
        <div className="profile-cover" />
        <div className="profile-info">
          <div className="profile-avatar-wrap" onClick={handleAvatarClick} title="点击更换头像">
            <input
              ref={fileInputRef}
              type="file"
              accept="image/jpeg,image/png,image/webp,image/gif"
              onChange={handleAvatarUpload}
              style={{ display: 'none' }}
            />
            {user.avatarUrl ? (
              <img
                className="profile-avatar-img"
                src={user.avatarUrl}
                alt="头像"
              />
            ) : (
              <div className="profile-avatar">
                {avatarUploading ? '...' : (user.username?.[0]?.toUpperCase() || '?')}
              </div>
            )}
            <div className="profile-avatar-overlay">
              {avatarUploading ? '上传中...' : '更换头像'}
            </div>
          </div>
          <div className="profile-data">
            <h1 className="profile-name">{user.username}</h1>
            <p className="profile-bio">{user.isAdmin ? '管理员' : '用户'}</p>
            <div className="profile-stats">
              <div className="stat-item">
                <span className="stat-num">{profile?.totalVideosWatched || 0}</span>
                <span className="stat-label">{t('profile.stats.watched')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{formatCount(profile?.totalWatchTimeMs ? Math.floor(profile.totalWatchTimeMs / 1000) : 0)}</span>
                <span className="stat-label">{t('profile.stats.watchTime')}</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{videos.length}</span>
                <span className="stat-label">作品</span>
              </div>
              <div className="stat-item">
                <span className="stat-num">{profile?.createdAt ? profile.createdAt.slice(0, 4) : '-'}</span>
                <span className="stat-label">加入</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="profile-tabs">
        <button className={`ptab ${activeTab === 'works' ? 'active' : ''}`} onClick={() => onTabChange('works')}>
                          <span className="ptab-icon">🎬</span> {t('profile.works')}
        </button>
        <button className={`ptab ${activeTab === 'history' ? 'active' : ''}`} onClick={() => onTabChange('history')}>
                          <span className="ptab-icon">🕐</span> {t('profile.history')}
        </button>
        <button className={`ptab ${activeTab === 'likes' ? 'active' : ''}`} onClick={() => onTabChange('likes')}>
                          <span className="ptab-icon">❤️</span> {t('profile.favorites')}
        </button>
        <button className={`ptab ${activeTab === 'playlists' ? 'active' : ''}`} onClick={() => onTabChange('playlists')}>
                          <span className="ptab-icon">📋</span> {t('profile.playlists')}
        </button>
        <button className={`ptab ${activeTab === 'shares' ? 'active' : ''}`} onClick={() => onTabChange('shares')}>
                          <span className="ptab-icon">🔗</span> {t('profile.shares')}
        </button>
        <button className={`ptab ${activeTab === 'settings' ? 'active' : ''}`} onClick={() => onTabChange('settings')}>
          <span className="ptab-icon">⚙️</span> 设置
        </button>
      </div>

      {/* Works tab */}
      {activeTab === 'works' && (
        <div className="profile-content active">
          {videos.length > 0 ? (
            <div className="profile-grid">
              {videos.map((v) => (
                <VideoCard key={v.id} video={v} />
              ))}
            </div>
          ) : (
            <div className="profile-empty">
              <div className="empty-icon">🎬</div>
              <div>{t('profile.noWorks')}</div>
            </div>
          )}
        </div>
      )}

      {/* History tab */}
      {activeTab === 'history' && (
        <div className="profile-content active">
          {history.length > 0 ? (
            <div className="history-list">
              {history.map((h) => (
                <div key={h.id} className="history-item" onClick={() => navigate(`/player?id=${h.id}`)}>
                  <div className="history-thumb">
                    {h.thumb && (
                      <img src={h.thumb} alt="" loading="lazy" onError={(e) => { (e.target as HTMLImageElement).style.display = 'none' }} />
                    )}
                  </div>
                  <div className="history-info">
                    <div className="history-title">{h.title}</div>
                    <div className="history-meta">
                      {[h.category, h.updatedAt ? new Date(h.updatedAt).toLocaleDateString() : ''].filter(Boolean).join(' · ')}
                    </div>
                    {h.durationMs > 0 && (
                      <div className="history-progress">
                        <div className="hp-fill" style={{ width: h.progress + '%' }} />
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="profile-empty">
              <div className="empty-icon">🕐</div>
              <div>{t('profile.noHistory')}</div>
            </div>
          )}
        </div>
      )}

      {/* Likes tab */}
      {activeTab === 'likes' && (
        <div className="profile-content active">
          {favorites.length > 0 ? (
            <div className="history-list">
              {favorites.map((h) => (
                <div key={h.id} className="history-item" onClick={() => navigate(`/player?id=${h.id}`)}>
                  <div className="history-thumb">
                    {h.thumb && (
                      <img src={h.thumb} alt="" loading="lazy" onError={(e) => { (e.target as HTMLImageElement).style.display = 'none' }} />
                    )}
                  </div>
                  <div className="history-info">
                    <div className="history-title">{h.title}</div>
                    <div className="history-meta">
                      {[h.category, h.updatedAt ? new Date(h.updatedAt).toLocaleDateString() : ''].filter(Boolean).join(' · ')}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="profile-empty">
              <div className="empty-icon">❤️</div>
              <div>{t('profile.noFavorites')}</div>
            </div>
          )}
        </div>
      )}

      {/* Playlists tab */}
      {activeTab === 'playlists' && (
        <div className="profile-content active">
          <div className="playlists-header">
            <button className="profile-btn" onClick={() => setShowNewPlaylist(true)}>{t('profile.newPlaylist')}</button>
          </div>

          {showNewPlaylist && (
            <div className="new-playlist-form">
              <input
                type="text"
                value={newPlaylistName}
                onChange={e => setNewPlaylistName(e.target.value)}
                placeholder="播放列表名称"
                maxLength={200}
                autoFocus
              />
              <button className="profile-btn" onClick={handleCreatePlaylist} disabled={!newPlaylistName.trim()}>创建</button>
              <button className="profile-btn-secondary" onClick={() => { setShowNewPlaylist(false); setNewPlaylistName('') }}>{t('common.cancel')}</button>
            </div>
          )}

          {playlists.length > 0 ? (
            <div className="playlists-list">
              {playlists.map(p => (
                <div key={p.id} className="playlist-item">
                  <div className="playlist-info">
                    <span className="playlist-name">📋 {p.name}</span>
                    <span className="playlist-meta">{p.item_count} 个视频 · {p.is_public ? '公开' : '私密'}</span>
                  </div>
                  <button className="profile-btn-danger" onClick={() => handleDeletePlaylist(p.id, p.name)}>{t('profile.deletePlaylist')}</button>
                </div>
              ))}
            </div>
          ) : (
            <div className="profile-empty">
              <div className="empty-icon">📋</div>
              <div>{t('profile.noPlaylists')}</div>
            </div>
          )}
        </div>
      )}

      {/* Shares tab */}
      {activeTab === 'shares' && (
        <div className="profile-content active">
          {shares.length > 0 ? (
            <div className="shares-list">
              {shares.map(s => (
                <div key={s.id} className="share-item">
                  <div className="share-info">
                    <span className={`share-status ${s.active ? 'active' : 'expired'}`}>
                      {s.active ? '有效' : '已过期'}
                    </span>
                    <span className="share-meta">
                      创建于 {new Date(s.createdAt).toLocaleDateString()}
                      {s.expiresAt ? ` · 过期于 ${new Date(s.expiresAt).toLocaleDateString()}` : ' · 永不过期'}
                    </span>
                  </div>
                  <button className="profile-btn-danger" onClick={() => handleRevokeShare(s.id)} disabled={revokingId === s.id}>
                    {revokingId === s.id ? t('common.loading') : t('profile.revokeShare')}
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div className="profile-empty">
              <div className="empty-icon">🔗</div>
              <div>{t('profile.noShares')}</div>
            </div>
          )}
        </div>
      )}

      {/* Settings tab */}
      {activeTab === 'settings' && (
        <div className="profile-content active">
          <div className="settings-section">
            <h3 className="settings-title">播放设置</h3>
            <div className="settings-group">
              <div className="settings-row">
                <div>
                  <span className="settings-label">自动播放</span>
                  <span className="settings-desc">进入播放器后自动开始播放</span>
                </div>
                <label className="toggle">
                  <input type="checkbox" checked={autoPlay} onChange={(e) => handleAutoPlayChange(e.target.checked)} />
                  <span className="toggle-track" />
                </label>
              </div>
              <div className="settings-row">
                <div>
                  <span className="settings-label">记忆播放速度</span>
                  <span className="settings-desc">记住每个视频的播放速度</span>
                </div>
                <label className="toggle">
                  <input type="checkbox" checked={speedMem} onChange={(e) => handleSpeedMemChange(e.target.checked)} />
                  <span className="toggle-track" />
                </label>
              </div>
            </div>
          </div>

          <div className="settings-section">
            <h3 className="settings-title">账号</h3>
            <div className="settings-group">
              <div className="settings-row">
                <div>
                  <span className="settings-label">用户名</span>
                  <span className="settings-desc">{user.username}</span>
                </div>
                <span className="settings-value">{user.isAdmin ? '管理员' : '普通用户'}</span>
              </div>
            </div>
          </div>

          <button className="settings-logout" onClick={handleLogout}>{t('profile.logout')}</button>
        </div>
      )}

      {alertMsg && <AlertDialog open={!!alertMsg} message={alertMsg} onClose={() => setAlertMsg(null)} />}
      {confirmAction && (
        <ConfirmDialog
          open={!!confirmAction}
          title="确认删除"
          message={`确定要删除播放列表 ${confirmAction.name} 吗？`}
          danger
          onConfirm={async () => { await confirmAction.onConfirm(); setConfirmAction(null) }}
          onCancel={() => setConfirmAction(null)}
        />
      )}
      {loadingProfile && <div className="profile-loading">{t('common.loading')}</div>}
    </div>
  )
}
