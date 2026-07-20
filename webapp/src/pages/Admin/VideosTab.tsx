import { useState, useEffect, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { mediaUrl } from '../../api/client'
import {
  listAdminVideos, updateVideo, deleteVideo, deleteVideos,
  addExternalVideo, uploadCover, batchUpdateCategory,
} from '../../api/admin'
import type { AdminVideo } from '../../api/admin'
import { formatDuration } from '../../api/utils'
import { ConfirmDialog, AlertDialog, SkeletonLoader } from './components'
import EditModal from './components/EditModal'
import AddExternalModal from './components/AddExternalModal'
import BatchCatModal from './components/BatchCatModal'

export default function VideosTab({ sourceType }: { sourceType: string }) {
  const { t } = useTranslation()
  const [videos, setVideos] = useState<AdminVideo[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(0)
  const [query, setQuery] = useState('')
  const [searchInput, setSearchInput] = useState('')
  const [loading, setLoading] = useState(true)
  const [selected, setSelected] = useState<Set<number>>(new Set())

  const [editing, setEditing] = useState<AdminVideo | null>(null)
  const [showAddExternal, setShowAddExternal] = useState(false)
  const [showBatchCat, setShowBatchCat] = useState(false)

  const coverInputRef = useRef<HTMLInputElement>(null)
  const [coverTargetId, setCoverTargetId] = useState<number | null>(null)

  const [confirmDialog, setConfirmDialog] = useState<{
    open: boolean
    title: string
    message: string
    danger?: boolean
    onConfirm: () => void
  }>({ open: false, title: '', message: '', onConfirm: () => {} })

  const [alertMsg, setAlertMsg] = useState('')

  const PAGE_SIZE = 50
  const isImage = sourceType === 'local_image'

  const loadVideos = useCallback(async (p: number, q: string) => {
    setLoading(true)
    try {
      const res = await listAdminVideos({ page: p, size: PAGE_SIZE, query: q || undefined, type: sourceType })
      setVideos(res.items)
      setTotal(res.total)
    } catch { /* ignore */ }
    finally { setLoading(false) }
  }, [sourceType])

  useEffect(() => { loadVideos(page, query) }, [page, query, loadVideos])

  const handleSearch = () => { setPage(0); setQuery(searchInput) }

  const handleSelect = (id: number) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id); else next.add(id)
      return next
    })
  }

  const handleSelectAll = () => {
    setSelected(selected.size === videos.length ? new Set() : new Set(videos.map(v => v.id)))
  }

  const handleBatchDelete = () => {
    setConfirmDialog({
      open: true,
      title: t('admin.media.batchDeleteTitle'),
      message: t('admin.media.batchDeleteConfirm', { count: selected.size }),
      danger: true,
      onConfirm: async () => {
        try { await deleteVideos([...selected]); setSelected(new Set()); loadVideos(page, query) }
        catch { setAlertMsg(t('admin.media.batchDeleteFailed')) }
      },
    })
  }

  const handleDelete = (v: AdminVideo) => {
    setConfirmDialog({
      open: true,
      title: t('admin.media.deleteTitle'),
      message: t('admin.media.deleteConfirm', { title: v.title }),
      danger: true,
      onConfirm: async () => {
        try { await deleteVideo(v.id); loadVideos(page, query) }
        catch { setAlertMsg(t('admin.media.deleteFailed')) }
      },
    })
  }

  const handleSaveEdit = async (title: string, desc: string, cat: string) => {
    if (!editing) return
    try {
      await updateVideo(editing.id, { title, description: desc || undefined, category: cat || undefined })
      setEditing(null); loadVideos(page, query)
    } catch { setAlertMsg(t('admin.media.saveFailed')) }
  }

  const handleAddExternal = async (data: { title: string; description?: string; category?: string; stream_url: string; cover_url?: string }) => {
    try {
      await addExternalVideo(data)
      setShowAddExternal(false); setPage(0); loadVideos(0, query)
    } catch (e: unknown) { setAlertMsg(e instanceof Error ? e.message : t('admin.media.addFailed')) }
  }

  const handleBatchCat = async (cat: string) => {
    if (selected.size === 0) return
    try {
      await batchUpdateCategory([...selected], cat)
      setShowBatchCat(false); setSelected(new Set()); loadVideos(page, query)
    } catch { setAlertMsg(t('admin.media.editFailed')) }
  }

  const handleCoverUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file || coverTargetId === null) return
    try { await uploadCover(coverTargetId, file); loadVideos(page, query) }
    catch { setAlertMsg(t('admin.media.coverUploadFailed')) }
    finally { setCoverTargetId(null); e.target.value = '' }
  }

  const totalPages = Math.ceil(total / PAGE_SIZE)

  return (
    <div className="admin-tab-content">
      <div className="admin-toolbar">
        <div className="admin-search">
          <input type="text" placeholder={isImage ? t('admin.media.searchImage') : t('admin.media.searchVideo')} value={searchInput} onChange={e => setSearchInput(e.target.value)} onKeyDown={e => e.key === 'Enter' && handleSearch()} />
          <button onClick={handleSearch}>{t('admin.media.search')}</button>
        </div>
        <div className="admin-toolbar-actions">
          {selected.size > 0 && (
            <>
              <button className="admin-btn admin-btn-danger" onClick={handleBatchDelete}>{t('admin.media.delete')} ({selected.size})</button>
              <button className="admin-btn" onClick={() => setShowBatchCat(true)}>{t('admin.media.editCategory')} ({selected.size})</button>
            </>
          )}
          {!isImage && <button className="admin-btn admin-btn-primary" onClick={() => setShowAddExternal(true)}>{t('admin.media.addExternal')}</button>}
        </div>
      </div>

      {loading ? <SkeletonLoader type="table" lines={8} /> : videos.length === 0 ? (
        <div className="admin-empty">{t('admin.media.empty', { type: t(isImage ? 'admin.media.subImage' : 'admin.media.subVideo') })}</div>
      ) : (
        <>
          <div className="admin-table-wrap">
            <table className="admin-table">
              <thead>
                <tr>
                  <th className="admin-col-check"><input type="checkbox" checked={selected.size === videos.length && videos.length > 0} onChange={handleSelectAll} /></th>
                  <th className="admin-col-cover">{t('admin.media.thumbnail')}</th>
                  <th>{t('admin.media.title')}</th>
                  <th className="admin-col-cat">{t('admin.media.category')}</th>
                  {!isImage && <th className="admin-col-dur">{t('admin.media.duration')}</th>}
                  <th className="admin-col-views">{t('admin.media.views')}</th>
                  <th className="admin-col-actions">{t('admin.media.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {videos.map(v => (
                  <tr key={v.id} className={selected.has(v.id) ? 'admin-row-selected' : ''}>
                    <td className="admin-col-check"><input type="checkbox" checked={selected.has(v.id)} onChange={() => handleSelect(v.id)} /></td>
                    <td className="admin-col-cover">
                      <div className="admin-cover-thumb" onClick={() => { setCoverTargetId(v.id); coverInputRef.current?.click() }}>
                        {v.thumbUrl ? <img src={mediaUrl(v.thumbUrl) || undefined} alt="" /> : <span className="admin-cover-placeholder">+</span>}
                      </div>
                    </td>
                    <td>
                      <div className="admin-video-title">{v.title}</div>
                      <div className="admin-video-meta">{v.sourceType === 'local_video' ? t('admin.media.typeLocalVideo') : v.sourceType === 'local_image' ? t('admin.media.typeLocalImage') : v.sourceType === 'external' ? t('admin.media.typeExternal') : v.sourceType}</div>
                    </td>
                    <td className="admin-col-cat"><span className="admin-tag">{v.category}</span></td>
                    {!isImage && <td className="admin-col-dur">{formatDuration(v.duration, '--:--')}</td>}
                    <td className="admin-col-views">{v.views}</td>
                    <td className="admin-col-actions">
                      <button className="admin-icon-btn" title={t('admin.media.edit')} onClick={() => setEditing(v)}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                      </button>
                      <button className="admin-icon-btn admin-icon-btn-danger" title={t('admin.media.delete')} onClick={() => handleDelete(v)}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {totalPages > 1 && (
            <div className="admin-pagination">
              <button disabled={page === 0} onClick={() => setPage(p => p - 1)}>{t('admin.media.prevPage')}</button>
              <span>{t('admin.media.pageInfo', { current: page + 1, total: totalPages, count: total })}</span>
              <button disabled={page >= totalPages - 1} onClick={() => setPage(p => p + 1)}>{t('admin.media.nextPage')}</button>
            </div>
          )}
        </>
      )}

      <input ref={coverInputRef} type="file" accept="image/*" hidden onChange={handleCoverUpload} />

      {editing && (
        <EditModal video={editing} onSave={handleSaveEdit} onClose={() => setEditing(null)} />
      )}

      {showAddExternal && (
        <AddExternalModal onSave={handleAddExternal} onClose={() => setShowAddExternal(false)} />
      )}

      {showBatchCat && (
        <BatchCatModal count={selected.size} onSave={handleBatchCat} onClose={() => setShowBatchCat(false)} />
      )}

      <ConfirmDialog
        open={confirmDialog.open}
        title={confirmDialog.title}
        message={confirmDialog.message}
        danger={confirmDialog.danger}
        onConfirm={confirmDialog.onConfirm}
        onCancel={() => setConfirmDialog(prev => ({ ...prev, open: false }))}
      />

      <AlertDialog
        open={!!alertMsg}
        message={alertMsg}
        onClose={() => setAlertMsg('')}
      />
    </div>
  )
}
