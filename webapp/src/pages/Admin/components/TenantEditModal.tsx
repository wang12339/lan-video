import React, { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useMutation } from '@tanstack/react-query'
import { updateTenant, type Tenant, type TenantSettings } from '../../../api/tenants'
import AdminModal from './AdminModal'

interface TenantEditModalProps {
  tenant: Tenant
  onClose: () => void
  onSuccess: () => void
}

const TenantEditModal: React.FC<TenantEditModalProps> = ({ tenant, onClose, onSuccess }) => {
  const { t } = useTranslation()
  const formRef = useRef<HTMLFormElement>(null)
  const [settings, setSettings] = useState<TenantSettings>(tenant.settings)
  const [error, setError] = useState('')

  const updateMutation = useMutation({
    mutationFn: (newSettings: Partial<TenantSettings>) =>
      updateTenant(tenant.tenant_id, newSettings),
    onSuccess: (data) => {
      if (data.ok) {
        onSuccess()
      } else {
        setError(data.message || t('admin.tenant.edit.updateFailed'))
      }
    },
    onError: (err: Error) => {
      setError(err.message)
    },
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    updateMutation.mutate(settings)
  }

  const handleChange = (field: keyof TenantSettings, value: unknown) => {
    setSettings((prev) => ({ ...prev, [field]: value }))
  }

  return (
    <AdminModal
      title={t('admin.tenant.edit.title')}
      onClose={onClose}
      maxWidth={500}
      actions={
        <>
          <button type="button" className="admin-btn" onClick={onClose}>
            {t('admin.tenant.edit.cancel')}
          </button>
          <button
            type="button"
            className="admin-btn admin-btn-primary"
            disabled={updateMutation.isPending}
            onClick={() => formRef.current?.requestSubmit()}
          >
            {updateMutation.isPending ? t('admin.tenant.edit.saving') : t('admin.tenant.edit.save')}
          </button>
        </>
      }
    >
      <form ref={formRef} id="tenant-edit-form" onSubmit={handleSubmit}>
        <div className="form-group">
          <label>{t('admin.tenant.edit.name')}</label>
          <input
            type="text"
            value={tenant.name}
            disabled
            className="input-disabled"
          />
          <span className="form-hint">{t('admin.tenant.edit.nameImmutable')}</span>
        </div>

        <div className="form-group">
          <label>{t('admin.tenant.edit.maxUploadSize')}</label>
          <input
            type="number"
            value={settings.max_upload_size_mb}
            onChange={(e) => handleChange('max_upload_size_mb', Number(e.target.value))}
            min="1"
            max="10240"
          />
        </div>

        <div className="form-group">
          <label>{t('admin.tenant.edit.maxVideosPerUser')}</label>
          <input
            type="number"
            value={settings.max_videos_per_user}
            onChange={(e) => handleChange('max_videos_per_user', Number(e.target.value))}
            min="1"
            max="100000"
          />
        </div>

        <div className="form-group">
          <label>{t('admin.tenant.edit.storageQuota')}</label>
          <input
            type="number"
            value={settings.storage_quota_gb}
            onChange={(e) => handleChange('storage_quota_gb', Number(e.target.value))}
            min="1"
            max="10000"
          />
        </div>

        <div className="form-group">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={settings.registration_enabled}
              onChange={(e) => handleChange('registration_enabled', e.target.checked)}
            />
            {t('admin.tenant.edit.registrationEnabled')}
          </label>
        </div>

        <div className="form-group">
          <label>{t('admin.tenant.edit.customTheme')}</label>
          <div className="color-input">
            <input
              type="color"
              value={settings.custom_theme || '#1a1a2e'}
              onChange={(e) => handleChange('custom_theme', e.target.value)}
            />
            <input
              type="text"
              value={settings.custom_theme || ''}
              onChange={(e) => handleChange('custom_theme', e.target.value || undefined)}
              placeholder="#1a1a2e"
            />
          </div>
        </div>

        {error && <div className="form-error">{error}</div>}
      </form>
    </AdminModal>
  )
}

export default TenantEditModal
