import { request } from './client';

export interface Tenant {
  tenant_id: number;
  slug: string;
  name: string;
  host: string;
  status: 'active' | 'disabled' | 'maintenance';
  plan: string;
  max_users: number;
  max_storage_bytes: number;
  settings: TenantSettings;
  created_at?: string;
  updated_at?: string;
}

export interface TenantSettings {
  max_upload_size_mb: number;
  max_videos_per_user: number;
  registration_enabled: boolean;
  custom_theme?: string;
  storage_quota_gb: number;
}

export interface TenantStats {
  tenant_id: number;
  slug: string;
  name: string;
  user_count: number;
  video_count: number;
  storage_used_bytes: number;
  max_storage_bytes: number;
  storage_usage_percent: number;
}

export interface TenantListResponse {
  tenants: Tenant[];
  total: number;
}

/**
 * 获取所有租户列表
 */
export async function listTenants(): Promise<TenantListResponse> {
  return request<TenantListResponse>('/admin/tenants');
}

/**
 * 获取单个租户详情
 */
export async function getTenant(tenantId: number): Promise<Tenant> {
  return request<Tenant>(`/admin/tenants/${tenantId}`);
}

/**
 * 更新租户配置
 */
export async function updateTenant(
  tenantId: number,
  settings: Partial<TenantSettings>
): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>(`/admin/tenants/${tenantId}`, {
    method: 'PUT',
    body: settings,
  });
}

/**
 * 获取租户使用统计
 */
export async function getTenantStats(tenantId: number): Promise<TenantStats> {
  return request<TenantStats>(`/admin/tenants/${tenantId}/stats`);
}

/**
 * 切换租户状态（启用/禁用）
 */
export async function toggleTenant(
  tenantId: number,
  status: 'active' | 'disabled' | 'maintenance'
): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>(`/admin/tenants/${tenantId}/toggle`, {
    method: 'POST',
    body: { status },
  });
}

/**
 * 格式化字节大小
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

/**
 * 获取状态颜色
 */
export function getStatusColor(status: string): string {
  switch (status) {
    case 'active': return '#4caf50';
    case 'disabled': return '#f44336';
    case 'maintenance': return '#ff9800';
    default: return '#9e9e9e';
  }
}

/**
 * 获取状态文本
 */
export function getStatusText(status: string): string {
  switch (status) {
    case 'active': return '运行中';
    case 'disabled': return '已禁用';
    case 'maintenance': return '维护中';
    default: return '未知';
  }
}
