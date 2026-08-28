import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import AdminModal from './AdminModal'

interface Props {
  onSave: (data: { title: string; description?: string; category?: string; stream_url: string; cover_url?: string }) => Promise<void>
  onClose: () => void
}

export default function AddExternalModal({ onSave, onClose }: Props) {
  const { t } = useTranslation()
  const [title, setTitle] = useState('')
  const [desc, setDesc] = useState('')
  const [url, setUrl] = useState('')
  const [cover, setCover] = useState('')
  const [cat, setCat] = useState('local')
  const [saving, setSaving] = useState(false)

  const handleSave = useCallback(async () => {
    if (!title.trim() || !url.trim() || saving) return
    setSaving(true)
    try {
      await onSave({
        title: title.trim(),
        description: desc.trim() || undefined,
        category: cat.trim() || undefined,
        stream_url: url.trim(),
        cover_url: cover.trim() || undefined,
      })
    } finally { setSaving(false) }
  }, [title, desc, url, cover, cat, saving, onSave])

  return (
    <AdminModal
      title={t('admin.media.addExternalTitle')}
      onClose={onClose}
      actions={
        <>
          <button type="button" className="admin-btn" onClick={onClose} disabled={saving}>{t('admin.media.cancel')}</button>
          <button type="button" className="admin-btn admin-btn-primary" onClick={handleSave} disabled={saving || !title.trim() || !url.trim()}>{saving ? t('admin.media.adding') : t('admin.media.add')}</button>
        </>
      }
    >
      <label><span>{t('admin.media.titleField')} *</span><input value={title} onChange={e => setTitle(e.target.value)} maxLength={500} placeholder={t('admin.media.titleField')} autoFocus /></label>
      <label><span>{t('admin.media.videoLink')} *</span><input type="url" value={url} onChange={e => setUrl(e.target.value)} placeholder="https://..." /></label>
      <label><span>{t('admin.media.coverLink')}</span><input type="url" value={cover} onChange={e => setCover(e.target.value)} placeholder={t('admin.media.coverOptional')} /></label>
      <label><span>{t('admin.media.categoryField')}</span><input value={cat} onChange={e => setCat(e.target.value)} maxLength={100} placeholder="local" /></label>
      <label><span>{t('admin.media.description')}</span><textarea value={desc} onChange={e => setDesc(e.target.value)} rows={3} placeholder={t('admin.media.description')} /></label>
    </AdminModal>
  )
}
