import { request } from './client'

export interface LogEntry {
  timestamp: string
  level: string
  message: string
  method?: string
  path?: string
  status?: number
  duration_ms?: number
  request_id?: string
  user?: string
  video_id?: number
  error?: string
  action?: string
  target?: string
  page?: string
}

export interface LogsResponse {
  entries: LogEntry[]
  total: number
  file: string
}

export async function getLogs(params?: {
  level?: string
  search?: string
  limit?: number
  offset?: number
}): Promise<LogsResponse> {
  const query = new URLSearchParams()
  if (params?.level) query.set('level', params.level)
  if (params?.search) query.set('search', params.search)
  // 后端只做上限截断（limit ≤ 1000），这里同时兜底下限，避免非法值（负数/NaN/浮点）打爆日志解析
  const { limit, offset } = params ?? {}
  if (limit !== undefined && Number.isFinite(limit)) {
    query.set('limit', String(Math.max(1, Math.min(1000, Math.trunc(limit)))))
  }
  if (offset !== undefined && Number.isFinite(offset)) {
    query.set('offset', String(Math.max(0, Math.trunc(offset))))
  }
  const qs = query.toString()
  return request<LogsResponse>(`/admin/logs${qs ? '?' + qs : ''}`)
}

export async function clearLogs(): Promise<void> {
  await request('/admin/logs', { method: 'DELETE' })
}
