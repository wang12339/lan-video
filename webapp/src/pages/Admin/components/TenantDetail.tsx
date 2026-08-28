import React from 'react';
import { useTranslation } from 'react-i18next';
import { type Tenant, type TenantStats, formatBytes } from '../../../api/tenants';

interface TenantDetailProps {
  tenant: Tenant;
  stats?: TenantStats;
  onEdit: () => void;
}

const TenantDetail: React.FC<TenantDetailProps> = ({ tenant, stats, onEdit }) => {
  const { t } = useTranslation();
  const storagePercent = stats?.storage_usage_percent || 0;
  
  return (
    <div className="tenant-detail">
      <div className="detail-header">
        <h3>{t('admin.tenant.detail.title')}</h3>
        <button className="btn-edit" onClick={onEdit}>{t('admin.tenant.detail.editConfig')}</button>
      </div>

      <div className="detail-section">
        <h4>{t('admin.tenant.detail.basicInfo')}</h4>
        <div className="detail-grid">
          <div className="detail-item">
            <label>{t('admin.tenant.detail.tenantId')}</label>
            <span>{tenant.tenant_id}</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.slug')}</label>
            <span>{tenant.slug}</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.name')}</label>
            <span>{tenant.name}</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.domain')}</label>
            <span>{tenant.host || t('admin.tenant.detail.default')}</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.plan')}</label>
            <span className="plan-badge">{tenant.plan}</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.status')}</label>
            <span className={`status-${tenant.status}`}>{tenant.status}</span>
          </div>
        </div>
      </div>

      <div className="detail-section">
        <h4>{t('admin.tenant.detail.usageStats')}</h4>
        {stats ? (
          <>
            <div className="stats-grid">
              <div className="stat-card">
                <div className="stat-value">{stats.user_count}</div>
                <div className="stat-label">{t('admin.tenant.detail.userCount')}</div>
                <div className="stat-limit">/ {tenant.max_users}</div>
              </div>
              <div className="stat-card">
                <div className="stat-value">{stats.video_count}</div>
                <div className="stat-label">{t('admin.tenant.detail.videoCount')}</div>
              </div>
              <div className="stat-card">
                <div className="stat-value">{formatBytes(stats.storage_used_bytes)}</div>
                <div className="stat-label">{t('admin.tenant.detail.storageUsed')}</div>
                <div className="stat-limit">/ {formatBytes(stats.max_storage_bytes)}</div>
              </div>
            </div>

            {/* 存储使用进度条 */}
            <div className="storage-progress">
              <div className="progress-header">
                <span>{t('admin.tenant.detail.storageUsage')}</span>
                <span>{storagePercent.toFixed(1)}%</span>
              </div>
              <div className="tenant-progress-bar">
                <div
                  className="tenant-progress-fill"
                  style={{
                    width: `${Math.min(storagePercent, 100)}%`,
                    backgroundColor: storagePercent > 90 ? '#f44336' : storagePercent > 70 ? '#ff9800' : '#4caf50',
                  }}
                />
              </div>
            </div>
          </>
        ) : (
          <div className="stats-loading">{t('admin.tenant.detail.loadingStats')}</div>
        )}
      </div>

      <div className="detail-section">
        <h4>{t('admin.tenant.detail.config')}</h4>
        <div className="detail-grid">
          <div className="detail-item">
            <label>{t('admin.tenant.detail.maxUploadSize')}</label>
            <span>{tenant.settings.max_upload_size_mb} MB</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.maxVideosPerUser')}</label>
            <span>{tenant.settings.max_videos_per_user}</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.registrationEnabled')}</label>
            <span>{tenant.settings.registration_enabled ? t('admin.tenant.detail.yes') : t('admin.tenant.detail.no')}</span>
          </div>
          <div className="detail-item">
            <label>{t('admin.tenant.detail.storageQuota')}</label>
            <span>{tenant.settings.storage_quota_gb} GB</span>
          </div>
          {tenant.settings.custom_theme && (
            <div className="detail-item">
              <label>{t('admin.tenant.detail.customTheme')}</label>
              <span
                className="theme-preview"
                style={{ backgroundColor: tenant.settings.custom_theme }}
              >
                {tenant.settings.custom_theme}
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default TenantDetail;
