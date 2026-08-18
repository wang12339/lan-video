import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useModalEscape } from './useModalEscape'

interface Props {
  count: number
  onSave: (category: string) => Promise<void>
  onClose: () => void
}

export default function BatchCatModal({ count, onSave, onClose }: Props) {
  const { t } = useTranslation()
  const [cat, setCat] = useState('')
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    if (!cat.trim() || saving) return
    setSaving(true)
    try { await onSave(cat.trim()) }
    finally { setSaving(false) }
  }

  useModalEscape(onClose)

  return (
    <div className="admin-modal-overlay" onClick={onClose}>
      <div className="admin-modal" onClick={e => e.stopPropagation()}>
        <h3>{t('admin.media.batchCatTitle', { count })}</h3>
        <label><span>{t('admin.media.newCategory')}</span><input value={cat} onChange={e => setCat(e.target.value)} maxLength={100} placeholder={t('admin.media.categoryPlaceholder')} autoFocus onKeyDown={e => { if (e.key === 'Enter') void handleSave() }} /></label>
        <div className="admin-modal-actions">
          <button type="button" className="admin-btn" onClick={onClose} disabled={saving}>{t('admin.media.cancel')}</button>
          <button type="button" className="admin-btn admin-btn-primary" onClick={handleSave} disabled={saving || !cat.trim()}>{saving ? t('admin.media.modifying') : t('admin.media.confirm')}</button>
        </div>
      </div>
    </div>
  )
}
