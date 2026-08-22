import React, { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { updateTenant, type Tenant, type TenantSettings } from '../../../api/tenants';

interface TenantEditModalProps {
  tenant: Tenant;
  onClose: () => void;
  onSuccess: () => void;
}

const TenantEditModal: React.FC<TenantEditModalProps> = ({ tenant, onClose, onSuccess }) => {
  const [settings, setSettings] = useState<TenantSettings>(tenant.settings);
  const [error, setError] = useState('');

  const updateMutation = useMutation({
    mutationFn: (newSettings: Partial<TenantSettings>) =>
      updateTenant(tenant.tenant_id, newSettings),
    onSuccess: (data) => {
      if (data.ok) {
        onSuccess();
      } else {
        setError(data.message || '更新失败');
      }
    },
    onError: (err: Error) => {
      setError(err.message);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    updateMutation.mutate(settings);
  };

  const handleChange = (field: keyof TenantSettings, value: any) => {
    setSettings((prev) => ({ ...prev, [field]: value }));
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>编辑租户配置</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="modal-body">
            <div className="form-group">
              <label>租户名称</label>
              <input
                type="text"
                value={tenant.name}
                disabled
                className="input-disabled"
              />
              <span className="form-hint">租户名称不可修改</span>
            </div>

            <div className="form-group">
              <label>最大上传大小 (MB)</label>
              <input
                type="number"
                value={settings.max_upload_size_mb}
                onChange={(e) => handleChange('max_upload_size_mb', Number(e.target.value))}
                min="1"
                max="10240"
              />
            </div>

            <div className="form-group">
              <label>每用户最大视频数</label>
              <input
                type="number"
                value={settings.max_videos_per_user}
                onChange={(e) => handleChange('max_videos_per_user', Number(e.target.value))}
                min="1"
                max="100000"
              />
            </div>

            <div className="form-group">
              <label>存储配额 (GB)</label>
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
                允许用户注册
              </label>
            </div>

            <div className="form-group">
              <label>自定义主题色</label>
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
          </div>

          <div className="modal-footer">
            <button type="button" className="btn-cancel" onClick={onClose}>
              取消
            </button>
            <button
              type="submit"
              className="btn-save"
              disabled={updateMutation.isPending}
            >
              {updateMutation.isPending ? '保存中...' : '保存'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default TenantEditModal;
