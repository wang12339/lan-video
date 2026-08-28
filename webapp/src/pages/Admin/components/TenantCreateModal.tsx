import React, { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useMutation, useQuery } from '@tanstack/react-query'
import { createTenant, type CreateTenantRequest, type TenantSettings } from '../../../api/tenants'
import { listPlans } from '../../../api/plans'
import AdminModal from './AdminModal'

interface TenantCreateModalProps {
  onClose: () => void
  onSuccess: () => void
}

const defaultSettings: TenantSettings = {
  max_upload_size_mb: 500,
  max_videos_per_user: 1000,
  registration_enabled: false,
  storage_quota_gb: 50,
}

const TenantCreateModal: React.FC<TenantCreateModalProps> = ({ onClose, onSuccess }) => {
  const { t } = useTranslation()
  const formRef = useRef<HTMLFormElement>(null)
  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')
  const [customDomain, setCustomDomain] = useState('')
  const [selectedPlanId, setSelectedPlanId] = useState<number | null>(null)
  const [maxUsers, setMaxUsers] = useState(10)
  const [maxStorageGb, setMaxStorageGb] = useState(50)
  const [settings, setSettings] = useState<TenantSettings>(defaultSettings)
  const [error, setError] = useState('')

  const { data: plansData } = useQuery({
    queryKey: ['plans'],
    queryFn: listPlans,
  })

  const plans = plansData?.plans || []

  const handlePlanChange = (planId: number | null) => {
    setSelectedPlanId(planId)
    if (planId) {
      const plan = plans.find(p => p.plan_id === planId)
      if (plan) {
        setMaxUsers(plan.max_users)
        setMaxStorageGb(plan.storage_quota_gb)
        setSettings({
          max_upload_size_mb: plan.max_upload_size_mb,
          max_videos_per_user: plan.max_videos_per_user,
          registration_enabled: plan.registration_enabled,
          storage_quota_gb: plan.storage_quota_gb,
        })
      }
    }
  }

  const createMutation = useMutation({
    mutationFn: (data: CreateTenantRequest) => createTenant(data),
    onSuccess: (data) => {
      if (data.ok) {
        onSuccess()
      } else {
        setError(t('admin.tenant.create.createFailed'))
      }
    },
    onError: (err: Error) => {
      setError(err.message)
    },
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    createMutation.mutate({
      name: name.trim(),
      slug: slug.trim().toLowerCase(),
      custom_domain: customDomain.trim() || undefined,
      plan: selectedPlanId ? plans.find(p => p.plan_id === selectedPlanId)?.slug || 'free' : 'free',
      max_users: maxUsers,
      max_storage_bytes: maxStorageGb * 1024 * 1024 * 1024,
      settings,
    })
  }

  const handleSettingsChange = (field: keyof TenantSettings, value: unknown) => {
    setSettings((prev) => ({ ...prev, [field]: value }))
  }

  const handleSlugChange = (value: string) => {
    const sanitized = value.toLowerCase().replace(/[^a-z0-9-]/g, '-').replace(/-+/g, '-').replace(/^-|-$/g, '')
    setSlug(sanitized)
  }

  return (
    <AdminModal
      title={t('admin.tenant.create.title')}
      onClose={onClose}
      maxWidth={500}
      actions={
        <>
          <button type="button" className="admin-btn" onClick={onClose}>
            {t('admin.tenant.create.cancel')}
          </button>
          <button
            type="button"
            className="admin-btn admin-btn-primary"
            disabled={createMutation.isPending}
            onClick={() => formRef.current?.requestSubmit()}
          >
            {createMutation.isPending ? t('admin.tenant.create.creating') : t('admin.tenant.create.create')}
          </button>
        </>
      }
    >
      <form ref={formRef} onSubmit={handleSubmit}>
        <div className="form-group">
          <label>{t('admin.tenant.create.name')}</label>
          <input
            type="text"
            value={name}
            onChange={(e) => {
              setName(e.target.value)
              if (!slug) handleSlugChange(e.target.value)
            }}
            placeholder={t('admin.tenant.create.namePlaceholder')}
            required
          />
        </div>

        <div className="form-group">
          <label>{t('admin.tenant.create.slug')}</label>
          <input
            type="text"
            value={slug}
            onChange={(e) => handleSlugChange(e.target.value)}
            placeholder={t('admin.tenant.create.slugPlaceholder')}
            required
          />
          <span className="form-hint">{t('admin.tenant.create.slugHint')}</span>
        </div>

        <div className="form-group">
          <label>{t('admin.tenant.create.domain')}</label>
          <input
            type="text"
            value={customDomain}
            onChange={(e) => setCustomDomain(e.target.value)}
            placeholder={t('admin.tenant.create.domainPlaceholder')}
          />
          <span className="form-hint">{t('admin.tenant.create.domainHint')}</span>
        </div>

        <div className="form-group">
          <label>{t('admin.tenant.create.plan')}</label>
          <select 
            value={selectedPlanId || ''} 
            onChange={(e) => handlePlanChange(e.target.value ? Number(e.target.value) : null)}
          >
            <option value="">{t('admin.tenant.create.customPlan')}</option>
            {plans.map((plan) => (
              <option key={plan.plan_id} value={plan.plan_id}>
                {plan.name} - {plan.description}
              </option>
            ))}
          </select>
        </div>

        {!selectedPlanId && (
          <>
            <div className="form-group">
              <label>{t('admin.tenant.create.maxUsers')}</label>
              <input
                type="number"
                value={maxUsers}
                onChange={(e) => setMaxUsers(Number(e.target.value))}
                min="1"
                max="100000"
              />
            </div>

            <div className="form-group">
              <label>{t('admin.tenant.create.maxStorage')}</label>
              <input
                type="number"
                value={maxStorageGb}
                onChange={(e) => setMaxStorageGb(Number(e.target.value))}
                min="1"
                max="10000"
              />
              <span className="form-hint">GB</span>
            </div>

            <div className="form-group">
              <label>{t('admin.tenant.create.maxUploadSize')}</label>
              <input
                type="number"
                value={settings.max_upload_size_mb}
                onChange={(e) => handleSettingsChange('max_upload_size_mb', Number(e.target.value))}
                min="1"
                max="10240"
              />
              <span className="form-hint">MB</span>
            </div>

            <div className="form-group">
              <label>{t('admin.tenant.create.maxVideosPerUser')}</label>
              <input
                type="number"
                value={settings.max_videos_per_user}
                onChange={(e) => handleSettingsChange('max_videos_per_user', Number(e.target.value))}
                min="1"
                max="100000"
              />
            </div>

            <div className="form-group">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={settings.registration_enabled}
                  onChange={(e) => handleSettingsChange('registration_enabled', e.target.checked)}
                />
                {t('admin.tenant.create.registrationEnabled')}
              </label>
            </div>
          </>
        )}

        {error && <div className="form-error">{error}</div>}
      </form>
    </AdminModal>
  )
}

export default TenantCreateModal
