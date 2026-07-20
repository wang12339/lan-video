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
  if (params?.limit) query.set('limit', String(params.limit))
  if (params?.offset) query.set('offset', String(params.offset))
  const qs = query.toString()
  return request<LogsResponse>(`/admin/logs${qs ? '?' + qs : ''}`)
}

export async function clearLogs(): Promise<void> {
  await request('/admin/logs', { method: 'DELETE' })
}
