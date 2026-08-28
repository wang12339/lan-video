import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  listTenants,
  getTenantStats,
  toggleTenant,
  type Tenant,
  formatBytes,
  getStatusColor,
} from '../../api/tenants'
import { ConfirmDialog } from './components'
import TenantDetail from './components/TenantDetail'
import TenantEditModal from './components/TenantEditModal'
import TenantCreateModal from './components/TenantCreateModal'
import './TenantTab.css'

const STATUS_TEXT_KEY: Record<string, string> = {
  active: 'admin.tenant.statusActive',
  disabled: 'admin.tenant.statusDisabled',
  maintenance: 'admin.tenant.statusMaintenance',
}

export default function TenantTab() {
  const { t } = useTranslation()
  const [selectedTenant, setSelectedTenant] = useState<Tenant | null>(null)
  const [showEditModal, setShowEditModal] = useState(false)
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [confirmToggle, setConfirmToggle] = useState<Tenant | null>(null)
  const queryClient = useQueryClient()

  const { data: tenantsData, isLoading, error } = useQuery({
    queryKey: ['tenants'],
    queryFn: listTenants,
  })

  const { data: stats } = useQuery({
    queryKey: ['tenantStats', selectedTenant?.tenant_id],
    queryFn: () => {
      if (!selectedTenant) throw new Error('No tenant selected')
      return getTenantStats(selectedTenant.tenant_id)
    },
    enabled: !!selectedTenant,
  })

  const toggleMutation = useMutation({
    mutationFn: ({ tenantId, status }: { tenantId: number; status: 'active' | 'disabled' }) =>
      toggleTenant(tenantId, status),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tenants'] })
      queryClient.invalidateQueries({ queryKey: ['tenantStats'] })
      setTimeout(() => {
        queryClient.refetchQueries({ queryKey: ['tenants'] })
      }, 100)
    },
  })

  const handleToggleStatus = (tenant: Tenant) => {
    setConfirmToggle(tenant)
  }

  const doToggle = useCallback(() => {
    if (!confirmToggle) return
    const newStatus: 'active' | 'disabled' = confirmToggle.status === 'active' ? 'disabled' : 'active'
    toggleMutation.mutate({ tenantId: confirmToggle.tenant_id, status: newStatus })
    setConfirmToggle(null)
  }, [confirmToggle, toggleMutation])

  if (isLoading) {
    return <div className="tenant-loading">{t('common.loading')}</div>
  }

  if (error) {
    return <div className="tenant-error">{t('admin.tenant.loadFailed', { message: (error as Error).message })}</div>
  }

  const tenants = tenantsData?.tenants || []

  return (
    <div className="tenant-tab">
      <div className="tenant-header">
        <h2>{t('admin.tenant.title')}</h2>
        <p className="tenant-subtitle">{t('admin.tenant.subtitle')}</p>
      </div>

      <div className="tenant-content">
        <div className="tenant-list">
          <div className="tenant-list-header">
            <h3>{t('admin.tenant.tenantList')}</h3>
            <div className="tenant-list-actions">
              <span className="tenant-count">{t('admin.tenant.tenantCount', { count: tenants.length })}</span>
              <button
                className="admin-btn admin-btn-primary"
                onClick={() => setShowCreateModal(true)}
              >
                {t('admin.tenant.createTenant')}
              </button>
            </div>
          </div>
          
          <div className="tenant-grid">
            {tenants.map((tenant) => (
              <div
                key={tenant.tenant_id}
                className={`tenant-card ${selectedTenant?.tenant_id === tenant.tenant_id ? 'selected' : ''} `}
                onClick={() => setSelectedTenant(tenant)}
              >
                <div className="tenant-card-header">
                  <div className="tenant-info">
                    <h4>{tenant.name}</h4>
                    <span className="tenant-slug">{tenant.slug}</span>
                  </div>
                  <span
                    className="tenant-status"
                    style={{ backgroundColor: getStatusColor(tenant.status) }}
                  >
                    {t(STATUS_TEXT_KEY[tenant.status] ?? 'admin.tenant.statusUnknown')}
                  </span>
                </div>
                
                <div className="tenant-card-body">
                  <div className="tenant-meta">
                    <span className="tenant-plan">{tenant.plan}</span>
                    <span className="tenant-host">{tenant.host || t('admin.tenant.defaultDomain')}</span>
                  </div>
                  <div className="tenant-stats-preview">
                    <span>{t('admin.tenant.maxUsers', { count: tenant.max_users })}</span>
                    <span>{t('admin.tenant.storageQuota', { size: formatBytes(tenant.max_storage_bytes) })}</span>
                  </div>
                </div>

                <div className="tenant-card-actions">
                  <button
                    className="btn-edit"
                    onClick={(e) => {
                      e.stopPropagation()
                      setSelectedTenant(tenant)
                      setShowEditModal(true)
                    }}
                  >
                    {t('admin.tenant.edit')}
                  </button>
                  <button
                    className={`btn-toggle ${tenant.status === 'active' ? 'btn-disable' : 'btn-enable'}`}
                    onClick={(e) => {
                      e.stopPropagation()
                      handleToggleStatus(tenant)
                    }}
                  >
                    {tenant.status === 'active' ? t('admin.tenant.disable') : t('admin.tenant.enable')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {selectedTenant && (
          <TenantDetail
            tenant={selectedTenant}
            stats={stats}
            onEdit={() => setShowEditModal(true)}
          />
        )}
      </div>

      {showEditModal && selectedTenant && (
        <TenantEditModal
          tenant={selectedTenant}
          onClose={() => setShowEditModal(false)}
          onSuccess={() => {
            setShowEditModal(false)
            queryClient.invalidateQueries({ queryKey: ['tenants'] })
          }}
        />
      )}

      {showCreateModal && (
        <TenantCreateModal
          onClose={() => setShowCreateModal(false)}
          onSuccess={() => {
            setShowCreateModal(false)
            queryClient.invalidateQueries({ queryKey: ['tenants'] })
          }}
        />
      )}

      {confirmToggle && (
        <ConfirmDialog
          open={true}
          title={confirmToggle.status === 'active' ? t('admin.tenant.disable') : t('admin.tenant.enable')}
          message={
            confirmToggle.status === 'active'
              ? t('admin.tenant.confirmDisable', { name: confirmToggle.name })
              : t('admin.tenant.confirmEnable', { name: confirmToggle.name })
          }
          danger={confirmToggle.status === 'active'}
          onConfirm={doToggle}
          onCancel={() => setConfirmToggle(null)}
        />
      )}
    </div>
  )
}
