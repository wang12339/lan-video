import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { mediaUrl } from '../../api/client'
import {
  listAdminVideos, updateVideo, deleteVideo, deleteVideos,
  addExternalVideo, uploadCover, batchUpdateCategory, getStats,
} from '../../api/admin'
import type { AdminVideo } from '../../api/admin'
import { getTranscodeStatus, transcodeVideo } from '../../api/videos'
import type { TranscodeStatusResponse } from '../../api/types'
import { formatDuration, formatBytes } from '../../api/utils'
import { useConfirmDialog } from '../../hooks/useConfirmDialog'
import { useAlertDialog } from '../../hooks/useAlertDialog'
import { ConfirmDialog, AlertDialog, SkeletonLoader } from './components'
import EditModal from './components/EditModal'
import AddExternalModal from './components/AddExternalModal'
import BatchCatModal from './components/BatchCatModal'
import AdminModal from './components/AdminModal'
import './VideosTab.css'

const PAGE_SIZE = 50
const TRANSCODE_RESOLUTIONS = ['360p', '480p', '720p', '1080p', '2160p']

type SortKey = 'newest' | 'oldest' | 'views' | 'duration' | 'title'

export default function VideosTab({ sourceType }: { sourceType: string }) {
  const { t } = useTranslation()
  const [videos, setVideos] = useState<AdminVideo[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(0)
  const [query, setQuery] = useState('')
  const [searchInput, setSearchInput] = useState('')
  const [loading, setLoading] = useState(true)
  const [listError, setListError] = useState(false)
  const [selected, setSelected] = useState<Set<string>>(new Set())

  // 排序 / 分类筛选
  const [sortBy, setSortBy] = useState<SortKey>('newest')
  const [category, setCategory] = useState('')

  const [editing, setEditing] = useState<AdminVideo | null>(null)
  const [showAddExternal, setShowAddExternal] = useState(false)
  const [showBatchCat, setShowBatchCat] = useState(false)

  const coverInputRef = useRef<HTMLInputElement>(null)
  const [coverTargetId, setCoverTargetId] = useState<string | null>(null)
  const [coverUploading, setCoverUploading] = useState(false)

  // 转码管理
  const [txTarget, setTxTarget] = useState<AdminVideo | null>(null)
  const [txStatus, setTxStatus] = useState<TranscodeStatusResponse | null>(null)
  const [txLoading, setTxLoading] = useState(false)
  const [txTriggering, setTxTriggering] = useState(false)

  const { confirmDialog, askConfirm, handleCancel } = useConfirmDialog()
  const { alertMsg, showAlert, closeAlert } = useAlertDialog()

  const isImage = sourceType === 'local_image'

  const loadVideos = useCallback(async (p: number, q: string) => {
    setLoading(true)
    setListError(false)
    try {
      const res = await listAdminVideos({ page: p, size: PAGE_SIZE, query: q || undefined, type: sourceType, category: category || undefined })
      setVideos(res.items)
      setTotal(res.total)
    } catch {
      setListError(true)
    } finally { setLoading(false) }
  }, [sourceType, category])

  useEffect(() => { loadVideos(page, query) }, [page, query, loadVideos])

  // 分类变化回到第一页
  useEffect(() => { setPage(0) }, [category])

  // 删除末页最后一条后自动回退一页
  useEffect(() => {
    if (!loading && videos.length === 0 && page > 0 && total > 0) {
      setPage(p => p - 1)
    }
  }, [videos, loading, page, total])

  const refreshList = useCallback(() => {
    loadVideos(page, query)
  }, [loadVideos, page, query])

  const handleSearch = useCallback(() => { setPage(0); setQuery(searchInput.trim()) }, [searchInput])

  const clearSearch = useCallback(() => { setSearchInput(''); setQuery(''); setPage(0) }, [])

  const handleSelect = useCallback((id: string) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id); else next.add(id)
      return next
    })
  }, [])

  const handleSelectAll = useCallback(() => {
    setSelected(selected.size === videos.length ? new Set() : new Set(videos.map(v => v.id)))
  }, [selected.size, videos])

  const handleBatchDelete = () => {
    askConfirm({
      title: t('admin.media.batchDeleteTitle'),
      message: t('admin.media.batchDeleteConfirm', { count: selected.size }),
      danger: true,
      onConfirm: async () => {
        try {
          await deleteVideos([...selected])
          setSelected(new Set())
          refreshList()
        } catch { showAlert(t('admin.media.batchDeleteFailed')) }
      },
    })
  }

  const handleDelete = (v: AdminVideo) => {
    askConfirm({
      title: t('admin.media.deleteTitle'),
      message: t('admin.media.deleteConfirm', { title: v.title }),
      danger: true,
      onConfirm: async () => {
        try { await deleteVideo(v.id); refreshList() }
        catch { showAlert(t('admin.media.deleteFailed')) }
      },
    })
  }

  const handleSaveEdit = async (title: string, desc: string, cat: string) => {
    if (!editing) return
    try {
      await updateVideo(editing.id, { title, description: desc || undefined, category: cat || undefined })
      setEditing(null)
      refreshList()
    } catch { showAlert(t('admin.media.saveFailed')) }
  }

  const handleAddExternal = async (data: { title: string; description?: string; category?: string; stream_url: string; cover_url?: string }) => {
    try {
      await addExternalVideo(data)
      setShowAddExternal(false); setPage(0); loadVideos(0, query)
    } catch (e: unknown) { showAlert(e instanceof Error ? e.message : t('admin.media.addFailed')) }
  }

  const handleBatchCat = async (cat: string) => {
    if (selected.size === 0) return
    try {
      await batchUpdateCategory([...selected], cat)
      setShowBatchCat(false); setSelected(new Set()); refreshList()
    } catch { showAlert(t('admin.media.editFailed')) }
  }

  const handleCoverUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file || coverTargetId === null) return
    setCoverUploading(true)
    try { await uploadCover(coverTargetId, file); refreshList() }
    catch { showAlert(t('admin.media.coverUploadFailed')) }
    finally { setCoverUploading(false); setCoverTargetId(null); e.target.value = '' }
  }

  const openCoverPicker = (v: AdminVideo) => {
    if (coverUploading) return
    setCoverTargetId(v.id)
    coverInputRef.current?.click()
  }

  // ── 转码 ──

  const openTranscode = async (v: AdminVideo) => {
    setTxTarget(v)
    setTxStatus(null)
    setTxLoading(true)
    try { setTxStatus(await getTranscodeStatus(v.id)) }
    catch { showAlert(t('admin.media.transcodeStatusFailed')) }
    finally { setTxLoading(false) }
  }

  const refreshTxStatus = async () => {
    if (!txTarget) return
    setTxLoading(true)
    try { setTxStatus(await getTranscodeStatus(txTarget.id)) }
    catch { showAlert(t('admin.media.transcodeStatusFailed')) }
    finally { setTxLoading(false) }
  }

  const handleTxTrigger = (res: string) => {
    if (!txTarget) return
    askConfirm({
      title: t('admin.media.triggerTranscode'),
      message: t('admin.media.triggerTranscodeConfirm', { title: txTarget.title, resolution: res }),
      onConfirm: async () => {
        setTxTriggering(true)
        try {
          await transcodeVideo(txTarget.id, [res])
          setTxStatus(await getTranscodeStatus(txTarget.id))
        } catch { showAlert(t('admin.media.transcodeTriggerFailed')) }
        finally { setTxTriggering(false) }
      },
    })
  }

  // 分类列表（来自统计接口）
  const { data: stats } = useQuery({ queryKey: ['admin-stats'], queryFn: getStats })
  const categories = useMemo(() => Array.from(new Set((stats?.byCategory ?? []).map(c => c.category))), [stats])

  // 当前页排序（createdAt 可能为 undefined，回退到 id 与 ''）
  const sortedVideos = useMemo(() => {
    const arr = [...videos]
    const byCreated = (a: AdminVideo, b: AdminVideo) =>
      (a.createdAt ?? `id-${a.id}`).localeCompare(b.createdAt ?? `id-${b.id}`)
    switch (sortBy) {
      case 'oldest': return arr.sort(byCreated)
      case 'views': return arr.sort((a, b) => b.views - a.views)
      case 'duration': return arr.sort((a, b) => b.duration - a.duration)
      case 'title': return arr.sort((a, b) => a.title.localeCompare(b.title, 'zh-CN'))
      default: return arr.sort((a, b) => byCreated(b, a))
    }
  }, [videos, sortBy])

  const totalPages = Math.ceil(total / PAGE_SIZE)

  return (
    <div className="admin-tab-content">
      <div className="admin-toolbar">
        <div className="admin-search">
          <input type="text" placeholder={isImage ? t('admin.media.searchImage') : t('admin.media.searchVideo')} value={searchInput} onChange={e => setSearchInput(e.target.value)} onKeyDown={e => e.key === 'Enter' && handleSearch()} aria-label={t('admin.media.search')} />
          <button onClick={handleSearch}>{t('admin.media.search')}</button>
          {searchInput && <button onClick={clearSearch} title={t('admin.media.clearSearch')} aria-label={t('admin.media.clearSearch')}>×</button>}
        </div>
        <select className="admin-btn" value={category} onChange={e => { setCategory(e.target.value); setSelected(new Set()) }} aria-label={t('admin.media.categoryFilter')}>
          <option value="">{t('admin.media.allCategories')}</option>
          {categories.map(c => <option key={c} value={c}>{c}</option>)}
        </select>
        <select className="admin-btn" value={sortBy} onChange={e => setSortBy(e.target.value as SortKey)} aria-label={t('admin.media.sortAria')}>
          <option value="newest">{t('admin.media.sortNewest')}</option>
          <option value="oldest">{t('admin.media.sortOldest')}</option>
          <option value="views">{t('admin.media.sortViews')}</option>
          <option value="duration">{t('admin.media.sortDuration')}</option>
          <option value="title">{t('admin.media.sortTitle')}</option>
        </select>
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

      {loading ? <SkeletonLoader type="table" lines={8} /> : listError ? (
        <div className="admin-error">
          <p>{t('admin.media.loadFailed')}</p>
          <button className="admin-btn" onClick={refreshList}>{t('common.retry')}</button>
        </div>
      ) : videos.length === 0 ? (
        <div className="admin-empty">{t('admin.media.empty', { type: t(isImage ? 'admin.media.subImage' : 'admin.media.subVideo') })}</div>
      ) : (
        <>
          <div className="admin-table-wrap">
            <table className="admin-table">
              <thead>
                <tr>
                  <th className="admin-col-check"><input type="checkbox" checked={selected.size === videos.length && videos.length > 0} onChange={handleSelectAll} aria-label={t('admin.media.selectAll')} /></th>
                  <th className="admin-col-cover">{t('admin.media.thumbnail')}</th>
                  <th>{t('admin.media.title')}</th>
                  <th className="admin-col-cat">{t('admin.media.category')}</th>
                  {!isImage && <th className="admin-col-dur">{t('admin.media.duration')}</th>}
                  <th className="admin-col-views">{t('admin.media.views')}</th>
                  {!isImage && <th>{t('admin.media.transcode')}</th>}
                  <th className="admin-col-actions">{t('admin.media.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {sortedVideos.map(v => (
                  <tr key={v.id} className={selected.has(v.id) ? 'admin-row-selected' : ''}>
                    <td className="admin-col-check"><input type="checkbox" checked={selected.has(v.id)} onChange={() => handleSelect(v.id)} aria-label={t('admin.media.select', { title: v.title })} /></td>
                    <td className="admin-col-cover">
                      <div className={`admin-cover-thumb ${coverUploading && coverTargetId === v.id ? 'admin-videos__cover-uploading' : 'admin-videos__cover-default'}`} role="button" tabIndex={0} title={t('admin.media.uploadCoverTitle')}
                        onClick={() => openCoverPicker(v)}
                        onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openCoverPicker(v) } }}>
                        {v.thumbUrl ? <img src={mediaUrl(v.thumbUrl) || undefined} alt="" /> : <span className="admin-cover-placeholder">{coverUploading && coverTargetId === v.id ? '…' : '+'}</span>}
                      </div>
                    </td>
                    <td>
                      <div className="admin-video-title">{v.title}</div>
                      <div className="admin-video-meta">{v.sourceType === 'local_video' ? t('admin.media.typeLocalVideo') : v.sourceType === 'local_image' ? t('admin.media.typeLocalImage') : v.sourceType === 'external' ? t('admin.media.typeExternal') : v.sourceType}</div>
                    </td>
                    <td className="admin-col-cat"><span className="admin-tag">{v.category}</span></td>
                    {!isImage && <td className="admin-col-dur">{formatDuration(v.duration, '--:--')}</td>}
                    <td className="admin-col-views">{v.views}</td>
                    {!isImage && v.sourceType === 'local_video' && (
                      <td>
                        <button className="admin-icon-btn" title={t('admin.media.transcodeManage')} aria-label={t('admin.media.transcodeManageAria', { title: v.title })} onClick={() => openTranscode(v)}>
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="2" y="2" width="20" height="20" rx="2.18"/><line x1="7" y1="2" x2="7" y2="22"/><line x1="17" y1="2" x2="17" y2="22"/><line x1="2" y1="12" x2="22" y2="12"/><line x1="2" y1="7" x2="7" y2="7"/><line x1="2" y1="17" x2="7" y2="17"/><line x1="17" y1="17" x2="22" y2="17"/><line x1="17" y1="7" x2="22" y2="7"/></svg>
                        </button>
                      </td>
                    )}
                    {!isImage && v.sourceType !== 'local_video' && <td />}
                    <td className="admin-col-actions">
                      <button className="admin-icon-btn" title={t('admin.media.edit')} aria-label={`${t('admin.media.edit')}：${v.title}`} onClick={() => setEditing(v)}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                      </button>
                      <button className="admin-icon-btn admin-icon-btn-danger" title={t('admin.media.delete')} aria-label={`${t('admin.media.delete')}：${v.title}`} onClick={() => handleDelete(v)}>
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

      {txTarget && (
        <AdminModal
          title={t('admin.media.transcodeTitle', { title: txTarget.title })}
          onClose={() => { setTxTarget(null); setTxStatus(null) }}
          actions={
            <>
              <button className="admin-btn" onClick={() => { setTxTarget(null); setTxStatus(null) }}>{t('admin.media.close')}</button>
              <button className="admin-btn admin-btn-primary" onClick={refreshTxStatus} disabled={txLoading}>{t('admin.media.refreshStatus')}</button>
            </>
          }
        >
          {txLoading ? (
            <div className="admin-empty">{t('common.loading')}</div>
          ) : (
            <>
              <div className="admin-videos__section">
                <div className="admin-videos__section-label">{t('admin.media.generatedVersions')}</div>
                {txStatus && txStatus.variants.length > 0 ? (
                  txStatus.variants.map(variant => (
                    <div key={variant.resolution} className="admin-videos__variant-row">
                      <span className="admin-videos__variant-resolution">{variant.resolution}</span>
                      <span className="admin-videos__variant-size">{formatBytes(variant.fileSize)}</span>
                    </div>
                  ))
                ) : (
                  <div className="admin-videos__empty-text">{t('admin.media.noTranscodeYet')}</div>
                )}
              </div>
              <div className="admin-videos__section">
                <div className="admin-videos__section-label">{t('admin.media.pendingJobs')}</div>
                {txStatus && txStatus.pendingJobs.length > 0 ? (
                  txStatus.pendingJobs.map(job => (
                    <div key={job.id} className="admin-videos__variant-row">
                      <span className="admin-videos__variant-resolution">{job.resolution} · {job.status}</span>
                      <span className="admin-videos__variant-size">{job.progress}%</span>
                    </div>
                  ))
                ) : (
                  <div className="admin-videos__empty-text">{t('admin.media.noPendingJobs')}</div>
                )}
              </div>
              <div className="admin-videos__trigger-label">{t('admin.media.triggerTranscode')}</div>
              <div className="admin-pending-actions admin-videos__resolution-actions">
                {TRANSCODE_RESOLUTIONS.map(res => {
                  const done = txStatus?.variants.some(v => v.resolution === res)
                  return (
                    <button key={res} className="admin-btn admin-btn-sm" disabled={txTriggering || !!done} onClick={() => handleTxTrigger(res)}>
                      {done ? `${res} ✓` : res}
                    </button>
                  )
                })}
              </div>
            </>
          )}
        </AdminModal>
      )}

      <ConfirmDialog
        open={confirmDialog.open}
        title={confirmDialog.title}
        message={confirmDialog.message}
        danger={confirmDialog.danger}
        confirmText={t('common.confirm')}
        onConfirm={confirmDialog.onConfirm}
        onCancel={handleCancel}
      />

      <AlertDialog
        open={!!alertMsg}
        message={alertMsg}
        onClose={closeAlert}
      />
    </div>
  )
}
