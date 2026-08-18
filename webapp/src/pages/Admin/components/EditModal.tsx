import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { AdminVideo } from '../../../api/admin'
import { useModalEscape } from './useModalEscape'

interface Props {
  video: AdminVideo
  onSave: (title: string, desc: string, cat: string) => Promise<void>
  onClose: () => void
}

export default function EditModal({ video, onSave, onClose }: Props) {
  const { t } = useTranslation()
  const [title, setTitle] = useState(video.title)
  const [desc, setDesc] = useState(video.description)
  const [cat, setCat] = useState(video.category)
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    if (!title.trim() || saving) return
    setSaving(true)
    try { await onSave(title.trim(), desc.trim(), cat.trim()) }
    finally { setSaving(false) }
  }

  useModalEscape(onClose)

  return (
    <div className="admin-modal-overlay" onClick={onClose}>
      <div className="admin-modal" onClick={e => e.stopPropagation()}>
        <h3>{video.sourceType === 'local_image' ? t('admin.media.editImage') : t('admin.media.editVideo')}</h3>
        <label><span>{t('admin.media.titleField')}</span><input value={title} onChange={e => setTitle(e.target.value)} maxLength={500} autoFocus /></label>
        <label><span>{t('admin.media.description')}</span><textarea value={desc} onChange={e => setDesc(e.target.value)} rows={3} /></label>
        <label><span>{t('admin.media.categoryField')}</span><input value={cat} onChange={e => setCat(e.target.value)} maxLength={100} /></label>
        <div className="admin-modal-actions">
          <button type="button" className="admin-btn" onClick={onClose} disabled={saving}>{t('admin.media.cancel')}</button>
          <button type="button" className="admin-btn admin-btn-primary" onClick={handleSave} disabled={saving || !title.trim()}>{saving ? t('admin.media.saving') : t('admin.media.save')}</button>
        </div>
      </div>
    </div>
  )
}
