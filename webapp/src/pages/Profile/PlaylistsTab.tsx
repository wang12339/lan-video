import { useState, useCallback } from 'react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { Playlist } from '../../api/playlists'
import { TabError } from './SharedComponents'

interface Props {
  playlists: Playlist[]
  pending: boolean
  error: boolean
  refetch: () => void
  onCreate: (name: string) => Promise<void>
  onDelete: (p: Playlist) => void
}

export default function PlaylistsTab({ playlists, pending, error, refetch, onCreate, onDelete }: Props) {
  const { t } = useTranslation()
  const [showNewPlaylist, setShowNewPlaylist] = useState(false)
  const [newPlaylistName, setNewPlaylistName] = useState('')
  const [creating, setCreating] = useState(false)

  const handleCreate = useCallback(async () => {
    const name = newPlaylistName.trim()
    if (!name || creating) return
    setCreating(true)
    try {
      await onCreate(name)
      setNewPlaylistName('')
      setShowNewPlaylist(false)
    } finally {
      setCreating(false)
    }
  }, [newPlaylistName, creating, onCreate])

  return (
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
            onKeyDown={(e) => { if (e.key === 'Enter') handleCreate() }}
            placeholder={t('profile.playlistNamePlaceholder')}
            maxLength={200}
            autoFocus
          />
          <button className="profile-btn" onClick={handleCreate} disabled={!newPlaylistName.trim()}>{t('common.create')}</button>
          <button className="profile-btn-secondary" onClick={() => { setShowNewPlaylist(false); setNewPlaylistName('') }}>{t('common.cancel')}</button>
        </div>
      )}

      {pending ? (
        <div className="tab-loading">{t('common.loading')}</div>
      ) : error ? (
        <TabError onRetry={() => refetch()} />
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
              <button className="profile-btn-danger" onClick={() => onDelete(p)}>{t('profile.deletePlaylist')}</button>
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
  )
}
