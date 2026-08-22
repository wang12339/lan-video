import React, { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listTenants,
  getTenantStats,
  toggleTenant,
  type Tenant,
  formatBytes,
  getStatusColor,
  getStatusText,
} from '../../api/tenants';
import TenantDetail from './components/TenantDetail';
import TenantEditModal from './components/TenantEditModal';
import './TenantTab.css';

const TenantTab: React.FC = () => {
  const [selectedTenant, setSelectedTenant] = useState<Tenant | null>(null);
  const [showEditModal, setShowEditModal] = useState(false);
  const queryClient = useQueryClient();

  // 获取租户列表
  const { data: tenantsData, isLoading, error } = useQuery({
    queryKey: ['tenants'],
    queryFn: listTenants,
  });

  // 获取选中租户的统计
  const { data: stats } = useQuery({
    queryKey: ['tenantStats', selectedTenant?.tenant_id],
    queryFn: () => {
      if (!selectedTenant) throw new Error('No tenant selected')
      return getTenantStats(selectedTenant.tenant_id)
    },
    enabled: !!selectedTenant,
  });

  // 切换租户状态
  const toggleMutation = useMutation({
    mutationFn: ({ tenantId, status }: { tenantId: number; status: string }) =>
      toggleTenant(tenantId, status as any),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tenants'] });
      queryClient.invalidateQueries({ queryKey: ['tenantStats'] });
      // 强制重新获取租户列表
      setTimeout(() => {
        queryClient.refetchQueries({ queryKey: ['tenants'] });
      }, 100);
    },
  });

  const handleToggleStatus = (tenant: Tenant) => {
    const newStatus = tenant.status === 'active' ? 'disabled' : 'active';
    if (window.confirm(`确定要${newStatus === 'active' ? '启用' : '禁用'}租户 "${tenant.name}" 吗？`)) {
      toggleMutation.mutate({ tenantId: tenant.tenant_id, status: newStatus });
    }
  };

  if (isLoading) {
    return <div className="tenant-loading">加载中...</div>;
  }

  if (error) {
    return <div className="tenant-error">加载失败: {(error as Error).message}</div>;
  }

  const tenants = tenantsData?.tenants || [];

  return (
    <div className="tenant-tab">
      <div className="tenant-header">
        <h2>租户管理</h2>
        <p className="tenant-subtitle">管理多租户配置和状态</p>
      </div>

      <div className="tenant-content">
        {/* 租户列表 */}
        <div className="tenant-list">
          <div className="tenant-list-header">
            <h3>租户列表</h3>
            <span className="tenant-count">共 {tenants.length} 个租户</span>
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
                    {getStatusText(tenant.status)}
                  </span>
                </div>
                
                <div className="tenant-card-body">
                  <div className="tenant-meta">
                    <span className="tenant-plan">{tenant.plan}</span>
                    <span className="tenant-host">{tenant.host || '默认域名'}</span>
                  </div>
                  <div className="tenant-stats-preview">
                    <span>最大用户: {tenant.max_users}</span>
                    <span>存储配额: {formatBytes(tenant.max_storage_bytes)}</span>
                  </div>
                </div>

                <div className="tenant-card-actions">
                  <button
                    className="btn-edit"
                    onClick={(e) => {
                      e.stopPropagation();
                      setSelectedTenant(tenant);
                      setShowEditModal(true);
                    }}
                  >
                    编辑
                  </button>
                  <button
                    className={`btn-toggle ${tenant.status === 'active' ? 'btn-disable' : 'btn-enable'}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleToggleStatus(tenant);
                    }}
                  >
                    {tenant.status === 'active' ? '禁用' : '启用'}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* 租户详情 */}
        {selectedTenant && (
          <TenantDetail
            tenant={selectedTenant}
            stats={stats}
            onEdit={() => setShowEditModal(true)}
          />
        )}
      </div>

      {/* 编辑弹窗 */}
      {showEditModal && selectedTenant && (
        <TenantEditModal
          tenant={selectedTenant}
          onClose={() => setShowEditModal(false)}
          onSuccess={() => {
            setShowEditModal(false);
            queryClient.invalidateQueries({ queryKey: ['tenants'] });
          }}
        />
      )}
    </div>
  );
};

export default TenantTab;
