import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { listTags, createTag, updateTag, deleteTag, type Tag } from '../../api'
import { ConfirmDialog, AlertDialog, SkeletonLoader } from '../../components/ui'
import AdminModal from './components/AdminModal'

const PRESET_COLORS = [
  '#3b82f6', '#ec4899', '#8b5cf6', '#10b981', '#f59e0b',
  '#ef4444', '#06b6d4', '#f97316', '#84cc16', '#6366f1',
]

const TAGS_QUERY_KEY = ['admin-tags']

export default function TagsTab() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [showForm, setShowForm] = useState(false)
  const [editingTag, setEditingTag] = useState<Tag | null>(null)
  const [formName, setFormName] = useState('')
  const [formColor, setFormColor] = useState('#3b82f6')
  const [confirmDelete, setConfirmDelete] = useState<Tag | null>(null)
  const [alertMsg, setAlertMsg] = useState('')

  const { data: tags, isLoading, error } = useQuery({
    queryKey: TAGS_QUERY_KEY,
    queryFn: listTags,
  })

  const invalidate = () => queryClient.invalidateQueries({ queryKey: TAGS_QUERY_KEY })

  const saveMut = useMutation({
    mutationFn: (input: { name: string; color: string }) =>
      editingTag ? updateTag(editingTag.id, input) : createTag(input),
    onSuccess: () => {
      setShowForm(false)
      setEditingTag(null)
      setAlertMsg('')
      void invalidate()
    },
    onError: (e: unknown) => {
      setAlertMsg(e instanceof Error ? e.message : t('admin.tags.saveFailed'))
    },
  })

  const deleteMut = useMutation({
    mutationFn: (id: number) => deleteTag(id),
    onSuccess: () => {
      setConfirmDelete(null)
      void invalidate()
    },
    onError: (e: unknown) => {
      setAlertMsg(e instanceof Error ? e.message : t('admin.tags.deleteFailed'))
    },
  })

  const openCreate = useCallback(() => {
    setEditingTag(null)
    setFormName('')
    setFormColor('#3b82f6')
    setShowForm(true)
  }, [])

  const openEdit = useCallback((tag: Tag) => {
    setEditingTag(tag)
    setFormName(tag.name)
    setFormColor(tag.color || '#3b82f6')
    setShowForm(true)
  }, [])

  const handleSave = () => {
    const name = formName.trim()
    if (!name || saveMut.isPending) return
    saveMut.mutate({ name, color: formColor })
  }

  if (isLoading) return <SkeletonLoader type="card" lines={4} />
  if (error) {
    return (
      <div className="admin-tab-content">
        <div className="admin-error">
          <p>{t('admin.tags.loadFailed')}</p>
          <button type="button" className="admin-btn" onClick={() => void queryClient.invalidateQueries({ queryKey: TAGS_QUERY_KEY })}>
            {t('common.retry')}
          </button>
        </div>
      </div>
    )
  }

  const tagList = tags ?? []

  return (
    <div className="tags-tab">
      <div className="tags-header">
        <h2>{t('admin.tags.title')}</h2>
        <button type="button" className="admin-btn admin-btn-primary" onClick={openCreate}>{t('admin.tags.newTag')}</button>
      </div>

      {tagList.length === 0 ? (
        <div className="admin-empty">{t('admin.tags.empty')}</div>
      ) : (
        <div className="tags-grid">
          {tagList.map(tag => (
            <div key={tag.id} className="tag-card">
              <div className="tag-card-header">
                <span className="tag-badge" style={{ background: tag.color || '#666' }}>{tag.name}</span>
                <span className="tag-count">{t('admin.tags.videos', { count: tag.usageCount })}</span>
              </div>
              <div className="tag-card-actions">
                <button type="button" className="admin-btn-sm" onClick={() => openEdit(tag)}>{t('admin.tags.edit')}</button>
                <button type="button" className="admin-btn-sm admin-btn-danger" onClick={() => setConfirmDelete(tag)}>{t('admin.tags.delete')}</button>
              </div>
            </div>
          ))}
        </div>
      )}

      {showForm && (
        <AdminModal
          title={editingTag ? t('admin.tags.editTag') : t('admin.tags.createTag')}
          onClose={() => setShowForm(false)}
          actions={
            <>
              <button type="button" className="admin-btn" onClick={() => setShowForm(false)} disabled={saveMut.isPending}>{t('common.cancel')}</button>
              <button type="button" className="admin-btn admin-btn-primary" onClick={handleSave} disabled={saveMut.isPending || !formName.trim()}>
                {saveMut.isPending ? t('common.loading') : editingTag ? t('common.save') : t('common.create')}
              </button>
            </>
          }
        >
          <div className="form-group">
            <label htmlFor="tag-name">{t('admin.tags.tagName')}</label>
            <input
              id="tag-name"
              type="text"
              value={formName}
              onChange={e => setFormName(e.target.value)}
              placeholder={t('admin.tags.tagNamePlaceholder')}
              maxLength={50}
              autoFocus
              onKeyDown={e => { if (e.key === 'Enter') handleSave() }}
            />
          </div>
          <div className="form-group">
            <label>{t('admin.tags.tagColor')}</label>
            <div className="color-picker">
              {PRESET_COLORS.map(c => (
                <button
                  key={c}
                  type="button"
                  className={`color-swatch ${formColor === c ? 'selected' : ''}`}
                  style={{ background: c }}
                  aria-label={c}
                  aria-pressed={formColor === c}
                  onClick={() => setFormColor(c)}
                />
              ))}
              <label className="color-custom">
                <input
                  type="color"
                  value={formColor}
                  onChange={e => setFormColor(e.target.value)}
                  aria-label={t('admin.tags.tagColor')}
                />
                <span className="color-custom-hex">{formColor}</span>
              </label>
            </div>
          </div>
          <div className="form-group">
            <label>{t('admin.tags.preview')}</label>
            <span className="tag-badge" style={{ background: formColor }}>{formName || t('admin.tags.preview')}</span>
          </div>
        </AdminModal>
      )}

      {confirmDelete && (
        <ConfirmDialog
          open={true}
          title={t('admin.tags.deleteTitle')}
          message={t('admin.tags.deleteConfirm', { name: confirmDelete.name })}
          danger
          onConfirm={() => deleteMut.mutate(confirmDelete.id)}
          onCancel={() => setConfirmDelete(null)}
        />
      )}

      <AlertDialog open={!!alertMsg} message={alertMsg} onClose={() => setAlertMsg('')} />
    </div>
  )
}
