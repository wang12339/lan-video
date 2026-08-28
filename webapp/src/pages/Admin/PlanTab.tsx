import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  listAllPlans,
  createPlan,
  updatePlan,
  deletePlan,
  togglePlan,
  type Plan,
  type CreatePlanRequest,
  type UpdatePlanRequest,
} from '../../api/plans'
import { ConfirmDialog } from './components'
import './PlanTab.css'

export default function PlanTab() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [editingPlan, setEditingPlan] = useState<Plan | null>(null)
  const [confirmDelete, setConfirmDelete] = useState<Plan | null>(null)

  const { data: plansData, isLoading, error } = useQuery({
    queryKey: ['plans'],
    queryFn: listAllPlans,
  })

  const createMutation = useMutation({
    mutationFn: (data: CreatePlanRequest) => createPlan(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
      setShowCreateModal(false)
    },
  })

  const updateMutation = useMutation({
    mutationFn: ({ planId, data }: { planId: number; data: UpdatePlanRequest }) =>
      updatePlan(planId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
      setEditingPlan(null)
    },
  })

  const toggleMutation = useMutation({
    mutationFn: ({ planId, active }: { planId: number; active: boolean }) =>
      togglePlan(planId, active),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (planId: number) => deletePlan(planId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plans'] })
      setConfirmDelete(null)
    },
  })

  if (isLoading) {
    return <div className="plan-loading">{t('common.loading')}</div>
  }

  if (error) {
    return <div className="plan-error">{t('admin.plan.loadFailed', { message: (error as Error).message })}</div>
  }

  const plans = plansData?.plans || []

  return (
    <div className="plan-tab">
      <div className="plan-header">
        <h2>{t('admin.plan.title')}</h2>
        <p className="plan-subtitle">{t('admin.plan.subtitle')}</p>
      </div>

      <div className="plan-actions">
        <button
          className="admin-btn admin-btn-primary"
          onClick={() => setShowCreateModal(true)}
        >
          {t('admin.plan.createPlan')}
        </button>
      </div>

      <div className="plan-grid">
        {plans.map((plan) => (
          <div key={plan.plan_id} className={`plan-card ${!plan.sort_order ? 'plan-card-default' : ''}`}>
            <div className="plan-card-header">
              <h3>{plan.name}</h3>
              <span className="plan-slug">{plan.slug}</span>
            </div>
            <div className="plan-card-body">
              {plan.description && (
                <p className="plan-description">{plan.description}</p>
              )}
              <div className="plan-features">
                <div className="plan-feature">
                  <span className="plan-feature-label">{t('admin.plan.maxUsers')}</span>
                  <span className="plan-feature-value">{plan.max_users}</span>
                </div>
                <div className="plan-feature">
                  <span className="plan-feature-label">{t('admin.plan.maxStorage')}</span>
                  <span className="plan-feature-value">{plan.storage_quota_gb} GB</span>
                </div>
                <div className="plan-feature">
                  <span className="plan-feature-label">{t('admin.plan.maxUploadSize')}</span>
                  <span className="plan-feature-value">{plan.max_upload_size_mb} MB</span>
                </div>
                <div className="plan-feature">
                  <span className="plan-feature-label">{t('admin.plan.maxVideosPerUser')}</span>
                  <span className="plan-feature-value">{plan.max_videos_per_user}</span>
                </div>
                <div className="plan-feature">
                  <span className="plan-feature-label">{t('admin.plan.registrationEnabled')}</span>
                  <span className="plan-feature-value">
                    {plan.registration_enabled ? t('common.yes') : t('common.no')}
                  </span>
                </div>
              </div>
            </div>
            <div className="plan-card-actions">
              <button
                className="btn-edit"
                onClick={() => setEditingPlan(plan)}
              >
                {t('common.edit')}
              </button>
              <button
                className={`btn-toggle ${plan.sort_order ? 'btn-disable' : 'btn-enable'}`}
                onClick={() => toggleMutation.mutate({ planId: plan.plan_id, active: !plan.sort_order })}
              >
                {plan.sort_order ? t('common.disable') : t('common.enable')}
              </button>
              <button
                className="btn-delete"
                onClick={() => setConfirmDelete(plan)}
              >
                {t('common.delete')}
              </button>
            </div>
          </div>
        ))}
      </div>

      {showCreateModal && (
        <PlanFormModal
          onClose={() => setShowCreateModal(false)}
          onSubmit={(data) => {
            if ('slug' in data) {
              createMutation.mutate(data as CreatePlanRequest)
            }
          }}
          isLoading={createMutation.isPending}
        />
      )}

      {editingPlan && (
        <PlanFormModal
          plan={editingPlan}
          onClose={() => setEditingPlan(null)}
          onSubmit={(data) => updateMutation.mutate({ planId: editingPlan.plan_id, data: data as UpdatePlanRequest })}
          isLoading={updateMutation.isPending}
        />
      )}

      {confirmDelete && (
        <ConfirmDialog
          open={true}
          title={t('admin.plan.deletePlan')}
          message={t('admin.plan.confirmDelete', { name: confirmDelete.name })}
          danger={true}
          onConfirm={() => deleteMutation.mutate(confirmDelete.plan_id)}
          onCancel={() => setConfirmDelete(null)}
        />
      )}
    </div>
  )
}

interface PlanFormModalProps {
  plan?: Plan
  onClose: () => void
  onSubmit: (data: CreatePlanRequest | UpdatePlanRequest) => void
  isLoading: boolean
}

function PlanFormModal({ plan, onClose, onSubmit, isLoading }: PlanFormModalProps) {
  const { t } = useTranslation()
  const [name, setName] = useState(plan?.name || '')
  const [slug, setSlug] = useState(plan?.slug || '')
  const [description, setDescription] = useState(plan?.description || '')
  const [maxUsers, setMaxUsers] = useState(plan?.max_users || 10)
  const [maxStorageGb, setMaxStorageGb] = useState(plan?.storage_quota_gb || 50)
  const [maxUploadSizeMb, setMaxUploadSizeMb] = useState(plan?.max_upload_size_mb || 500)
  const [maxVideosPerUser, setMaxVideosPerUser] = useState(plan?.max_videos_per_user || 1000)
  const [registrationEnabled, setRegistrationEnabled] = useState(plan?.registration_enabled || false)
  const [sortOrder, setSortOrder] = useState(plan?.sort_order || 0)

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    if (plan) {
      onSubmit({
        name,
        description,
        max_users: maxUsers,
        max_storage_bytes: maxStorageGb * 1024 * 1024 * 1024,
        max_upload_size_mb: maxUploadSizeMb,
        max_videos_per_user: maxVideosPerUser,
        registration_enabled: registrationEnabled,
        sort_order: sortOrder,
      })
    } else {
      onSubmit({
        name,
        slug,
        description,
        max_users: maxUsers,
        max_storage_bytes: maxStorageGb * 1024 * 1024 * 1024,
        max_upload_size_mb: maxUploadSizeMb,
        max_videos_per_user: maxVideosPerUser,
        registration_enabled: registrationEnabled,
        sort_order: sortOrder,
      })
    }
  }, [plan, name, slug, description, maxUsers, maxStorageGb, maxUploadSizeMb, maxVideosPerUser, registrationEnabled, sortOrder, onSubmit])

  return (
    <div className="plan-modal-overlay" onClick={onClose}>
      <div className="plan-modal" onClick={(e) => e.stopPropagation()}>
        <div className="plan-modal-header">
          <h3>{plan ? t('admin.plan.editPlan') : t('admin.plan.createPlan')}</h3>
          <button className="plan-modal-close" onClick={onClose}>×</button>
        </div>
        <form onSubmit={handleSubmit} className="plan-modal-body">
          <div className="form-group">
            <label>{t('admin.plan.name')}</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
            />
          </div>

          {!plan && (
            <div className="form-group">
              <label>{t('admin.plan.slug')}</label>
              <input
                type="text"
                value={slug}
                onChange={(e) => setSlug(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '-'))}
                required
              />
              <span className="form-hint">{t('admin.plan.slugHint')}</span>
            </div>
          )}

          <div className="form-group">
            <label>{t('admin.plan.description')}</label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>

          <div className="form-row">
            <div className="form-group">
              <label>{t('admin.plan.maxUsers')}</label>
              <input
                type="number"
                value={maxUsers}
                onChange={(e) => setMaxUsers(Number(e.target.value))}
                min="1"
              />
            </div>

            <div className="form-group">
              <label>{t('admin.plan.maxStorage')}</label>
              <input
                type="number"
                value={maxStorageGb}
                onChange={(e) => setMaxStorageGb(Number(e.target.value))}
                min="1"
              />
              <span className="form-hint">GB</span>
            </div>
          </div>

          <div className="form-row">
            <div className="form-group">
              <label>{t('admin.plan.maxUploadSize')}</label>
              <input
                type="number"
                value={maxUploadSizeMb}
                onChange={(e) => setMaxUploadSizeMb(Number(e.target.value))}
                min="1"
                max="10240"
              />
              <span className="form-hint">MB</span>
            </div>

            <div className="form-group">
              <label>{t('admin.plan.maxVideosPerUser')}</label>
              <input
                type="number"
                value={maxVideosPerUser}
                onChange={(e) => setMaxVideosPerUser(Number(e.target.value))}
                min="1"
              />
            </div>
          </div>

          <div className="form-row">
            <div className="form-group">
              <label>{t('admin.plan.sortOrder')}</label>
              <input
                type="number"
                value={sortOrder}
                onChange={(e) => setSortOrder(Number(e.target.value))}
                min="0"
              />
            </div>

            <div className="form-group">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={registrationEnabled}
                  onChange={(e) => setRegistrationEnabled(e.target.checked)}
                />
                {t('admin.plan.registrationEnabled')}
              </label>
            </div>
          </div>

          <div className="plan-modal-actions">
            <button type="button" className="admin-btn" onClick={onClose}>
              {t('common.cancel')}
            </button>
            <button type="submit" className="admin-btn admin-btn-primary" disabled={isLoading}>
              {isLoading ? t('common.saving') : t('common.save')}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
