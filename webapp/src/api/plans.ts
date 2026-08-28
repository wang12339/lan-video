import { request } from './client';

export { formatBytes } from './utils';

export interface Plan {
  plan_id: number;
  name: string;
  slug: string;
  description?: string;
  max_users: number;
  max_storage_bytes: number;
  max_upload_size_mb: number;
  max_videos_per_user: number;
  storage_quota_gb: number;
  registration_enabled: boolean;
  sort_order: number;
}

export interface CreatePlanRequest {
  name: string;
  slug: string;
  description?: string;
  max_users: number;
  max_storage_bytes: number;
  max_upload_size_mb: number;
  max_videos_per_user: number;
  storage_quota_gb: number;
  registration_enabled: boolean;
  sort_order?: number;
}

export interface UpdatePlanRequest {
  name?: string;
  description?: string;
  max_users?: number;
  max_storage_bytes?: number;
  max_upload_size_mb?: number;
  max_videos_per_user?: number;
  storage_quota_gb?: number;
  registration_enabled?: boolean;
  sort_order?: number;
}

export interface PlanListResponse {
  plans: Plan[];
}

/**
 * 获取所有活跃套餐
 */
export async function listPlans(): Promise<PlanListResponse> {
  return request<PlanListResponse>('/admin/plans');
}

/**
 * 获取所有套餐（包括禁用的）
 */
export async function listAllPlans(): Promise<PlanListResponse> {
  return request<PlanListResponse>('/admin/plans/all');
}

/**
 * 获取单个套餐详情
 */
export async function getPlan(planId: number): Promise<Plan> {
  return request<Plan>(`/admin/plans/${planId}`);
}

/**
 * 创建套餐
 */
export async function createPlan(
  data: CreatePlanRequest
): Promise<{ ok: boolean; plan: Plan }> {
  return request<{ ok: boolean; plan: Plan }>('/admin/plans', {
    method: 'POST',
    body: data,
  });
}

/**
 * 更新套餐
 */
export async function updatePlan(
  planId: number,
  data: UpdatePlanRequest
): Promise<{ ok: boolean; plan: Plan }> {
  return request<{ ok: boolean; plan: Plan }>(`/admin/plans/${planId}`, {
    method: 'PUT',
    body: data,
  });
}

/**
 * 切换套餐状态（启用/禁用）
 */
export async function togglePlan(
  planId: number,
  active: boolean
): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>(`/admin/plans/${planId}/toggle`, {
    method: 'POST',
    body: { active },
  });
}

/**
 * 删除套餐
 */
export async function deletePlan(planId: number): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>(`/admin/plans/${planId}`, {
    method: 'DELETE',
  });
}
