import React from 'react';
import { type Tenant, type TenantStats, formatBytes } from '../../../api/tenants';

interface TenantDetailProps {
  tenant: Tenant;
  stats?: TenantStats;
  onEdit: () => void;
}

const TenantDetail: React.FC<TenantDetailProps> = ({ tenant, stats, onEdit }) => {
  const storagePercent = stats?.storage_usage_percent || 0;
  
  return (
    <div className="tenant-detail">
      <div className="detail-header">
        <h3>租户详情</h3>
        <button className="btn-edit" onClick={onEdit}>编辑配置</button>
      </div>

      <div className="detail-section">
        <h4>基本信息</h4>
        <div className="detail-grid">
          <div className="detail-item">
            <label>租户 ID</label>
            <span>{tenant.tenant_id}</span>
          </div>
          <div className="detail-item">
            <label>标识符</label>
            <span>{tenant.slug}</span>
          </div>
          <div className="detail-item">
            <label>名称</label>
            <span>{tenant.name}</span>
          </div>
          <div className="detail-item">
            <label>域名</label>
            <span>{tenant.host || '默认'}</span>
          </div>
          <div className="detail-item">
            <label>套餐</label>
            <span className="plan-badge">{tenant.plan}</span>
          </div>
          <div className="detail-item">
            <label>状态</label>
            <span className={`status-${tenant.status}`}>{tenant.status}</span>
          </div>
        </div>
      </div>

      <div className="detail-section">
        <h4>使用统计</h4>
        {stats ? (
          <>
            <div className="stats-grid">
              <div className="stat-card">
                <div className="stat-value">{stats.user_count}</div>
                <div className="stat-label">用户数</div>
                <div className="stat-limit">/ {tenant.max_users}</div>
              </div>
              <div className="stat-card">
                <div className="stat-value">{stats.video_count}</div>
                <div className="stat-label">视频数</div>
              </div>
              <div className="stat-card">
                <div className="stat-value">{formatBytes(stats.storage_used_bytes)}</div>
                <div className="stat-label">已用存储</div>
                <div className="stat-limit">/ {formatBytes(stats.max_storage_bytes)}</div>
              </div>
            </div>

            {/* 存储使用进度条 */}
            <div className="storage-progress">
              <div className="progress-header">
                <span>存储使用率</span>
                <span>{storagePercent.toFixed(1)}%</span>
              </div>
              <div className="progress-bar">
                <div
                  className="progress-fill"
                  style={{
                    width: `${Math.min(storagePercent, 100)}%`,
                    backgroundColor: storagePercent > 90 ? '#f44336' : storagePercent > 70 ? '#ff9800' : '#4caf50',
                  }}
                />
              </div>
            </div>
          </>
        ) : (
          <div className="stats-loading">加载统计数据...</div>
        )}
      </div>

      <div className="detail-section">
        <h4>配置参数</h4>
        <div className="detail-grid">
          <div className="detail-item">
            <label>最大上传大小</label>
            <span>{tenant.settings.max_upload_size_mb} MB</span>
          </div>
          <div className="detail-item">
            <label>每用户最大视频数</label>
            <span>{tenant.settings.max_videos_per_user}</span>
          </div>
          <div className="detail-item">
            <label>允许注册</label>
            <span>{tenant.settings.registration_enabled ? '是' : '否'}</span>
          </div>
          <div className="detail-item">
            <label>存储配额</label>
            <span>{tenant.settings.storage_quota_gb} GB</span>
          </div>
          {tenant.settings.custom_theme && (
            <div className="detail-item">
              <label>自定义主题</label>
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
