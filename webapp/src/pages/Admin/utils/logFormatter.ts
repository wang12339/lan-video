import type { TFunction } from 'i18next'
import type { LogEntry } from '../../../api/logs'

// 路由 → i18n key（路径是数据，文案走 t 翻译）
export const ROUTE_KEYS: Record<string, string> = {
  '/auth/login': 'admin.logs.routes.authLogin',
  '/auth/logout': 'admin.logs.routes.authLogout',
  '/auth/register': 'admin.logs.routes.authRegister',
  '/auth/user': 'admin.logs.routes.authUser',
  '/auth/user/profile': 'admin.logs.routes.authProfile',
  '/auth/refresh': 'admin.logs.routes.authRefresh',
  '/videos': 'admin.logs.routes.videosList',
  '/videos/favorites': 'admin.logs.routes.videosFavorites',
  '/playback/history': 'admin.logs.routes.playbackHistory',
  '/playback/session/start': 'admin.logs.routes.sessionStart',
  '/playback/session/stop': 'admin.logs.routes.sessionStop',
  '/playback/session/heartbeat': 'admin.logs.routes.sessionHeartbeat',
  '/server/info': 'admin.logs.routes.serverInfo',
  '/health': 'admin.logs.routes.health',
  '/admin/logs': 'admin.logs.routes.adminLogs',
  '/admin/stats': 'admin.logs.routes.adminStats',
  '/admin/system': 'admin.logs.routes.adminSystem',
  '/admin/users': 'admin.logs.routes.adminUsers',
  '/admin/config/registration': 'admin.logs.routes.regConfig',
  '/admin/videos/scan': 'admin.logs.routes.scanMedia',
  '/admin/videos/upload': 'admin.logs.routes.uploadVideo',
  '/admin/videos/upload-resume': 'admin.logs.routes.uploadResume',
  '/admin/videos/upload-status': 'admin.logs.routes.uploadStatus',
  '/admin/videos/backfill-thumbnails': 'admin.logs.routes.backfillThumbs',
  '/admin/videos/check-hashes': 'admin.logs.routes.checkHashes',
  '/admin/videos/check-files': 'admin.logs.routes.checkFiles',
  '/admin/videos/batch-category': 'admin.logs.routes.batchCategory',
  '/admin/track': 'admin.logs.routes.track',
}

// 路径模式匹配器：正则 → 格式化函数（仅在 ROUTE_KEYS 未命中时使用）
const PATH_PATTERN_FORMATTERS: { pattern: RegExp; format: (entry: LogEntry, t: TFunction) => { action: string; type?: string } }[] = [
  { pattern: /\/videos\/\d+\/like/, format: (_e, t) => ({ action: t('admin.logs.actions.like'), type: 'like' }) },
  { pattern: /\/videos\/\d+\/favorite/, format: (_e, t) => ({ action: t('admin.logs.actions.favorite'), type: 'fav' }) },
  { pattern: /\/videos\/\d+\/play$/, format: (_e, t) => ({ action: t('admin.logs.actions.play'), type: 'play' }) },
  { pattern: /\/videos\/\d+\/view/, format: (_e, t) => ({ action: t('admin.logs.actions.viewVideo'), type: 'view' }) },
  { pattern: /\/videos\/\d+\/stop/, format: (_e, t) => ({ action: t('admin.logs.actions.stop'), type: 'stop' }) },
  { pattern: /\/videos\/\d+\/heartbeat/, format: (_e, t) => ({ action: t('admin.logs.actions.play'), type: 'play' }) },
  { pattern: /\/videos\/\d+$/, format: (_e, t) => ({ action: t('admin.logs.actions.viewDetail') }) },
  { pattern: /\/playback\/history\/\d+$/, format: (_e, t) => ({ action: t('admin.logs.actions.getHistory') }) },
  { pattern: /\/admin\/videos\/\d+\/cover/, format: (_e, t) => ({ action: t('admin.logs.actions.uploadCover') }) },
  { pattern: /\/admin\/videos\/\d+$/, format: (e, t) => ({
    action: e.method === 'PUT' ? t('admin.logs.actions.editVideo') : t('admin.logs.actions.deleteVideo'),
    type: e.method === 'DELETE' ? 'danger' : 'view',
  }) },
  { pattern: /\/admin\/videos\/batch-category/, format: (_e, t) => ({ action: t('admin.logs.routes.batchCategory') }) },
  { pattern: /\/admin\/users\/\d+\/password/, format: (_e, t) => ({ action: t('admin.logs.actions.resetPassword') }) },
  { pattern: /\/admin\/users\/\d+\/approve/, format: (_e, t) => ({ action: t('admin.logs.actions.approveUser'), type: 'admin' }) },
  { pattern: /\/admin\/users\/\d+\/admin/, format: (_e, t) => ({ action: t('admin.logs.actions.toggleAdmin') }) },
  { pattern: /\/admin\/users\/\d+$/, format: (_e, t) => ({ action: t('admin.logs.actions.deleteUser'), type: 'danger' }) },
]

// 精确路径匹配器（非正则）
const EXACT_PATH_FORMATTERS: Record<string, (t: TFunction) => { action: string; type?: string }> = {
  '/admin/videos/external': (t) => ({ action: t('admin.logs.actions.addExternal') }),
  '/admin/videos/batch': (t) => ({ action: t('admin.logs.actions.batchDelete'), type: 'danger' }),
}

// 系统消息模式匹配器：关键词 → 格式化函数
const MSG_FORMATTERS: { keyword: string; format: (entry: LogEntry, t: TFunction) => { desc: string; type: string } }[] = [
  { keyword: 'server starting', format: (_e, t) => ({ desc: t('admin.logs.system.serverStart'), type: 'system' }) },
  { keyword: 'shutdown', format: (_e, t) => ({ desc: t('admin.logs.system.serverShutdown'), type: 'system' }) },
  { keyword: 'Database connection', format: (_e, t) => ({ desc: t('admin.logs.system.dbConnection'), type: 'system' }) },
  { keyword: 'expired tokens', format: (_e, t) => ({ desc: t('admin.logs.system.clearTokens'), type: 'system' }) },
  { keyword: '开始播放', format: (e, t) => ({ desc: t('admin.logs.system.playVideo', { id: e.video_id }), type: 'play' }) },
  { keyword: '停止播放', format: (e, t) => ({ desc: t('admin.logs.system.stopVideo', { id: e.video_id }), type: 'stop' }) },
  { keyword: 'toggle like', format: (e, t) => ({ desc: (e.message || '').includes('liked: true') ? t('admin.logs.system.liked') : t('admin.logs.system.unliked'), type: 'like' }) },
  { keyword: 'toggle favorite', format: (e, t) => ({ desc: (e.message || '').includes('favorited: true') ? t('admin.logs.system.favorited') : t('admin.logs.system.unfavorited'), type: 'fav' }) },
  { keyword: 'user logged in', format: (_e, t) => ({ desc: t('admin.logs.system.loginSuccess'), type: 'login' }) },
  { keyword: 'failed login', format: (_e, t) => ({ desc: t('admin.logs.system.loginFailed'), type: 'error' }) },
  { keyword: 'rate limit', format: (_e, t) => ({ desc: t('admin.logs.system.rateLimit'), type: 'danger' }) },
  { keyword: 'Path traversal', format: (_e, t) => ({ desc: t('admin.logs.system.pathTraversal'), type: 'danger' }) },
  { keyword: 'invalid file', format: (_e, t) => ({ desc: t('admin.logs.system.invalidFile'), type: 'danger' }) },
  { keyword: 'admin deleted', format: (_e, t) => ({ desc: t('admin.logs.system.adminDeleted'), type: 'danger' }) },
  { keyword: 'admin approved', format: (_e, t) => ({ desc: t('admin.logs.system.adminApproved'), type: 'admin' }) },
  { keyword: 'registration toggle', format: (_e, t) => ({ desc: t('admin.logs.system.regToggle'), type: 'system' }) },
  { keyword: 'media scan', format: (_e, t) => ({ desc: t('admin.logs.system.mediaScan'), type: 'system' }) },
  { keyword: 'thumbnail backfill', format: (_e, t) => ({ desc: t('admin.logs.system.thumbBackfill'), type: 'system' }) },
  { keyword: 'rate limiter cleanup', format: (_e, t) => ({ desc: t('admin.logs.system.rateLimitCleanup'), type: 'system' }) },
]

function formatHttpLog(entry: LogEntry, t: TFunction): { desc: string; type: string } {
  const ms = entry.duration_ms || 0
  const s = entry.status || 0
  const routeKey = ROUTE_KEYS[entry.path!]
  let action = routeKey ? t(routeKey) : ''
  let type = 'view'

  if (!action) {
    const exact = EXACT_PATH_FORMATTERS[entry.path!]
    if (exact) {
      const r = exact(t)
      action = r.action
      if (r.type) type = r.type
    } else {
      const matched = PATH_PATTERN_FORMATTERS.find(m => m.pattern.test(entry.path!))
      if (matched) {
        const r = matched.format(entry, t)
        action = r.action
        if (r.type) type = r.type
      } else {
        action = entry.path!
      }
    }
  }

  if (s >= 500) type = 'error'
  else if (s >= 400) type = 'warn'

  const timeStr = ms > 0 ? t('admin.logs.ms', { ms }) : ''
  return { desc: `${action}${timeStr}`, type }
}

function formatMsgLog(entry: LogEntry, t: TFunction): { desc: string; type: string } {
  const msg = entry.message || ''

  const matched = MSG_FORMATTERS.find(m => msg.includes(m.keyword))
  if (matched) return matched.format(entry, t)

  if (msg.includes('用户操作') && entry.action) {
    let desc = entry.action
    if (entry.target) desc += ` ${entry.target}`
    if (entry.page) desc += ` (${entry.page})`
    return { desc, type: 'view' }
  }

  const displayMsg = msg.length > 30 ? msg.slice(0, 30) + '...' : msg
  return { desc: displayMsg, type: 'default' }
}

export function formatLog(entry: LogEntry, t: TFunction): { desc: string; type: string } {
  if (entry.method && entry.path) return formatHttpLog(entry, t)
  return formatMsgLog(entry, t)
}

export const TYPE_STYLES: Record<string, { color: string; labelKey: string }> = {
  play: { color: '#8b5cf6', labelKey: 'admin.logs.types.play' },
  stop: { color: '#6b7280', labelKey: 'admin.logs.types.stop' },
  like: { color: '#ec4899', labelKey: 'admin.logs.types.like' },
  fav: { color: '#f59e0b', labelKey: 'admin.logs.types.fav' },
  login: { color: '#3b82f6', labelKey: 'admin.logs.types.login' },
  view: { color: '#10b981', labelKey: 'admin.logs.types.view' },
  admin: { color: '#f97316', labelKey: 'admin.logs.types.admin' },
  danger: { color: '#ef4444', labelKey: 'admin.logs.types.danger' },
  error: { color: '#ef4444', labelKey: 'admin.logs.types.error' },
  warn: { color: '#f59e0b', labelKey: 'admin.logs.types.warn' },
  system: { color: '#6b7280', labelKey: 'admin.logs.types.system' },
  default: { color: '#9ca3af', labelKey: 'admin.logs.types.default' },
}

export const TYPE_ICONS: Record<string, string> = {
  play: '▶', stop: '■', like: '♥', fav: '★',
  login: '→', view: '○', admin: '⚡', danger: '⚠',
  error: '✗', warn: '!', system: '⚙', default: '·',
}

export const LEVELS = ['INFO', 'WARN', 'ERROR', 'DEBUG']

export function fmtTime(ts: string): string {
  if (!ts) return ''
  try { return new Date(ts).toLocaleTimeString('zh-CN', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }) }
  catch { return ts }
}

export function fmtTimeFull(ts: string): string {
  if (!ts) return '--'
  try { return new Date(ts).toLocaleString('zh-CN', { hour12: false, month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }
  catch { return ts }
}
