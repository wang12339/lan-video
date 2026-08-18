import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { clearLogs, type LogEntry, type LogsResponse } from '../../api/logs'
import { request } from '../../api/client'
import { ConfirmDialog } from './components'
import './LogsTab.css'

const PAGE_SIZE = 200
// 2 秒轮询：/admin/logs 无后端限流，且页签隐藏/失焦时已暂停，频率合理
const POLL_INTERVAL_MS = 2000
const LEVELS = ['INFO', 'WARN', 'ERROR', 'DEBUG']

// 路由 → i18n key（路径是数据，文案走 t 翻译）
const ROUTE_KEYS: Record<string, string> = {
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

// 中文日志翻译
function formatLog(entry: LogEntry, t: TFunction): { desc: string; type: string } {
  const msg = entry.message || ''
  const vid = entry.video_id

  if (entry.method && entry.path) {
    const s = entry.status || 0
    const ms = entry.duration_ms || 0

    const routeKey = ROUTE_KEYS[entry.path]
    let action = routeKey ? t(routeKey) : ''
    let type = 'view'

    if (!action) {
      if (entry.path.match(/\/videos\/\d+\/like/)) { action = t('admin.logs.actions.like'); type = 'like' }
      else if (entry.path.match(/\/videos\/\d+\/favorite/)) { action = t('admin.logs.actions.favorite'); type = 'fav' }
      else if (entry.path.match(/\/videos\/\d+\/play$/)) { action = t('admin.logs.actions.play'); type = 'play' }
      else if (entry.path.match(/\/videos\/\d+\/view/)) { action = t('admin.logs.actions.viewVideo'); type = 'view' }
      else if (entry.path.match(/\/videos\/\d+\/stop/)) { action = t('admin.logs.actions.stop'); type = 'stop' }
      else if (entry.path.match(/\/videos\/\d+\/heartbeat/)) { action = t('admin.logs.actions.play'); type = 'play' }
      else if (entry.path.match(/\/videos\/\d+$/)) { action = t('admin.logs.actions.viewDetail') }
      else if (entry.path.match(/\/playback\/history\/\d+$/)) { action = t('admin.logs.actions.getHistory') }
      else if (entry.path.match(/\/admin\/videos\/\d+\/cover/)) { action = t('admin.logs.actions.uploadCover') }
      else if (entry.path.match(/\/admin\/videos\/\d+$/)) {
        action = entry.method === 'PUT' ? t('admin.logs.actions.editVideo') : t('admin.logs.actions.deleteVideo')
        type = entry.method === 'DELETE' ? 'danger' : 'view'
      }
      else if (entry.path === '/admin/videos/external') { action = t('admin.logs.actions.addExternal') }
      else if (entry.path === '/admin/videos/batch') { action = t('admin.logs.actions.batchDelete'); type = 'danger' }
      else if (entry.path.match(/\/admin\/videos\/batch-category/)) { action = t('admin.logs.routes.batchCategory') }
      else if (entry.path.match(/\/admin\/users\/\d+\/password/)) { action = t('admin.logs.actions.resetPassword') }
      else if (entry.path.match(/\/admin\/users\/\d+\/approve/)) {
        action = t('admin.logs.actions.approveUser'); type = 'admin'
      }
      else if (entry.path.match(/\/admin\/users\/\d+\/admin/)) { action = t('admin.logs.actions.toggleAdmin') }
      else if (entry.path.match(/\/admin\/users\/\d+$/)) {
        action = t('admin.logs.actions.deleteUser'); type = 'danger'
      }
      else { action = entry.path }
    }

    if (s >= 500) type = 'error'
    else if (s >= 400) type = 'warn'

    const timeStr = ms > 0 ? t('admin.logs.ms', { ms }) : ''
    return { desc: `${action}${timeStr}`, type }
  }

  if (msg.includes('server starting')) return { desc: t('admin.logs.system.serverStart'), type: 'system' }
  if (msg.includes('shutdown')) return { desc: t('admin.logs.system.serverShutdown'), type: 'system' }
  if (msg.includes('Database connection')) return { desc: t('admin.logs.system.dbConnection'), type: 'system' }
  if (msg.includes('expired tokens')) return { desc: t('admin.logs.system.clearTokens'), type: 'system' }
  if (msg.includes('开始播放')) return { desc: t('admin.logs.system.playVideo', { id: vid }), type: 'play' }
  if (msg.includes('停止播放')) return { desc: t('admin.logs.system.stopVideo', { id: vid }), type: 'stop' }
  if (msg.includes('toggle like')) return { desc: msg.includes('liked: true') ? t('admin.logs.system.liked') : t('admin.logs.system.unliked'), type: 'like' }
  if (msg.includes('toggle favorite')) return { desc: msg.includes('favorited: true') ? t('admin.logs.system.favorited') : t('admin.logs.system.unfavorited'), type: 'fav' }
  if (msg.includes('user logged in')) return { desc: t('admin.logs.system.loginSuccess'), type: 'login' }
  if (msg.includes('failed login')) return { desc: t('admin.logs.system.loginFailed'), type: 'error' }
  if (msg.includes('rate limit')) return { desc: t('admin.logs.system.rateLimit'), type: 'danger' }
  if (msg.includes('Path traversal')) return { desc: t('admin.logs.system.pathTraversal'), type: 'danger' }
  if (msg.includes('invalid file')) return { desc: t('admin.logs.system.invalidFile'), type: 'danger' }
  if (msg.includes('admin deleted')) return { desc: t('admin.logs.system.adminDeleted'), type: 'danger' }
  if (msg.includes('admin approved')) return { desc: t('admin.logs.system.adminApproved'), type: 'admin' }
  if (msg.includes('registration toggle')) return { desc: t('admin.logs.system.regToggle'), type: 'system' }
  if (msg.includes('media scan')) return { desc: t('admin.logs.system.mediaScan'), type: 'system' }
  if (msg.includes('thumbnail backfill')) return { desc: t('admin.logs.system.thumbBackfill'), type: 'system' }
  if (msg.includes('rate limiter cleanup')) return { desc: t('admin.logs.system.rateLimitCleanup'), type: 'system' }

  // 用户操作追踪
  if (msg.includes('用户操作') && entry.action) {
    let desc = entry.action
    if (entry.target) desc += ` ${entry.target}`
    if (entry.page) desc += ` (${entry.page})`
    return { desc, type: 'view' }
  }

  const displayMsg = msg.length > 30 ? msg.slice(0, 30) + '...' : msg
  return { desc: displayMsg, type: 'default' }
}

const TYPE_STYLES: Record<string, { color: string; labelKey: string }> = {
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

const TYPE_ICONS: Record<string, string> = {
  play: '▶', stop: '■', like: '♥', fav: '★',
  login: '→', view: '○', admin: '⚡', danger: '⚠',
  error: '✗', warn: '!', system: '⚙', default: '·',
}

export default function LogsTab() {
  const { t } = useTranslation()
  const [entries, setEntries] = useState<LogEntry[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [pollError, setPollError] = useState(false)
  // 请求序号：翻页/改筛选时丢弃过期响应，避免旧页数据覆盖当前页
  const reqSeqRef = useRef(0)

  // 级别 / 搜索 / 时间范围 / 分页
  const [level, setLevel] = useState('')
  const [searchInput, setSearchInput] = useState('')
  const [search, setSearch] = useState('')
  const [timeFrom, setTimeFrom] = useState('')
  const [timeTo, setTimeTo] = useState('')
  const [page, setPage] = useState(0)

  const [selectedUser, setSelectedUser] = useState<string | null>(null)
  const [expandedIndex, setExpandedIndex] = useState<string | null>(null)
  const [showClearConfirm, setShowClearConfirm] = useState(false)

  // silent：轮询静默请求——失败只显示页内小提示，不触发全局 toast（request 的 silent 跳过 onErrorCb）
  const loadLogs = useCallback(async (p: number, opts?: { silent?: boolean }) => {
    const silent = !!opts?.silent
    const seq = ++reqSeqRef.current
    if (!silent) {
      setLoading(true)
      setError('')
    }
    try {
      const query = new URLSearchParams()
      if (level) query.set('level', level)
      if (search) query.set('search', search)
      query.set('limit', String(PAGE_SIZE))
      query.set('offset', String(p * PAGE_SIZE))
      const res = await request<LogsResponse>(`/admin/logs?${query}`, { silent })
      if (seq !== reqSeqRef.current) return // 过期响应（页码/筛选已变），丢弃
      setEntries(res.entries)
      setTotal(res.total)
      setError('')
      setPollError(false)
    } catch {
      if (seq !== reqSeqRef.current) return
      if (silent) setPollError(true)
      else setError(t('admin.logs.readFailed'))
    } finally {
      if (!silent && seq === reqSeqRef.current) setLoading(false)
    }
  }, [level, search, t])

  const fetchLogs = useCallback((opts?: { silent?: boolean }) => loadLogs(page, opts), [loadLogs, page])

  useEffect(() => { fetchLogs() }, [fetchLogs])

  // 筛选条件变化时回到第一页
  useEffect(() => { setPage(0) }, [level, search])

  // 自动刷新轮询：页签隐藏或窗口失焦时暂停，恢复后立即刷新一次再继续
  useEffect(() => {
    if (!autoRefresh) return
    let timer: ReturnType<typeof setInterval> | undefined
    const isInactive = () => document.hidden || !document.hasFocus()
    const tick = () => loadLogs(page, { silent: true })
    const stopPolling = () => {
      if (timer) { clearInterval(timer); timer = undefined }
    }
    const startPolling = () => {
      stopPolling()
      if (!isInactive()) timer = setInterval(tick, POLL_INTERVAL_MS)
    }
    const handleVisibility = () => {
      if (document.hidden) stopPolling()
      else { tick(); startPolling() }
    }
    const handleFocus = () => {
      if (document.hasFocus()) { tick(); startPolling() }
      else stopPolling()
    }

    startPolling()
    document.addEventListener('visibilitychange', handleVisibility)
    window.addEventListener('focus', handleFocus)
    window.addEventListener('blur', handleFocus)
    return () => {
      stopPolling()
      document.removeEventListener('visibilitychange', handleVisibility)
      window.removeEventListener('focus', handleFocus)
      window.removeEventListener('blur', handleFocus)
    }
  }, [autoRefresh, loadLogs, page])

  const handleClear = async () => {
    try {
      await clearLogs()
      setPage(0)
      setEntries([])
      setTotal(0)
      setSelectedUser(null)
      loadLogs(0)
    } catch { setError(t('admin.logs.clearFailed')) }
  }

  // 时间范围过滤（客户端）
  const visibleEntries = useMemo(() => {
    if (!timeFrom && !timeTo) return entries
    return entries.filter(e => {
      const k = (e.timestamp || '').slice(0, 16)
      if (timeFrom && k < timeFrom) return false
      if (timeTo && k > timeTo) return false
      return true
    })
  }, [entries, timeFrom, timeTo])

  const fmtTime = (ts: string) => {
    if (!ts) return ''
    try { return new Date(ts).toLocaleTimeString('zh-CN', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }) }
    catch { return ts }
  }

  const fmtTimeFull = (ts: string) => {
    if (!ts) return '--'
    try { return new Date(ts).toLocaleString('zh-CN', { hour12: false, month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }
    catch { return ts }
  }

  // 用户数据
  const userData = useMemo(() => {
    const map: Record<string, {
      logs: LogEntry[]
      count: number
      types: Record<string, number>
      lastActive: string
      firstActive: string
      videos: Set<number>
    }> = {}

    visibleEntries.forEach(e => {
      if (!e.user) return
      if (!map[e.user]) map[e.user] = { logs: [], count: 0, types: {}, lastActive: '', firstActive: '', videos: new Set() }
      const u = map[e.user]!
      u.logs.push(e)
      u.count++
      const { type } = formatLog(e, t)
      u.types[type] = (u.types[type] || 0) + 1
      if (e.video_id) u.videos.add(e.video_id)
      if (!u.lastActive || e.timestamp > u.lastActive) u.lastActive = e.timestamp
      if (!u.firstActive || e.timestamp < u.firstActive) u.firstActive = e.timestamp
    })

    return Object.entries(map)
      .map(([user, data]) => ({
        user,
        ...data,
        videos: data.videos.size,
        logs: data.logs.slice().reverse(),
      }))
      .sort((a, b) => b.count - a.count)
  }, [visibleEntries, t])

  // 选中用户的路线数据
  const userRoute = useMemo(() => {
    if (!selectedUser) return null
    const u = userData.find(d => d.user === selectedUser)
    if (!u) return null

    // 按时间分组（每5分钟一组）
    const groups: { time: string; entries: { entry: LogEntry; formatted: ReturnType<typeof formatLog> }[] }[] = []
    let lastGroup = ''

    u.logs.forEach(entry => {
      const ts = entry.timestamp || ''
      const tk = ts.slice(0, 16) // YYYY-MM-DDTHH:MM
      if (tk !== lastGroup) {
        lastGroup = tk
        groups.push({ time: ts, entries: [] })
      }
      groups[groups.length - 1]!.entries.push({ entry, formatted: formatLog(entry, t) })
    })

    return { ...u, groups }
  }, [userData, selectedUser, t])

  const errorCount = visibleEntries.filter(e => e.level === 'ERROR').length
  const hasMore = entries.length >= PAGE_SIZE

  return (
    <div className="analysis">
      {/* 头部 */}
      <div className="a-header">
        <div className="a-stats">
          <div className="a-stat">
            <span className="a-stat-num">{total}</span>
            <span className="a-stat-label">{t('admin.logs.totalLogs')}</span>
          </div>
          <div className="a-stat">
            <span className="a-stat-num">{userData.length}</span>
            <span className="a-stat-label">{t('admin.logs.activeUsers')}</span>
          </div>
          <div className="a-stat">
            <span className="a-stat-num">{errorCount}</span>
            <span className="a-stat-label">{t('admin.logs.errors')}</span>
          </div>
        </div>
        <div className="a-actions">
          <label className="a-toggle">
            <input type="checkbox" checked={autoRefresh} onChange={e => setAutoRefresh(e.target.checked)} aria-label={t('admin.logs.autoRefreshAria')} />
            <span className="a-toggle-track"></span>
          </label>
          <span className="a-toggle-text">{autoRefresh ? t('admin.logs.autoRefresh') : t('admin.logs.paused')}</span>
          <button className="a-refresh" onClick={() => fetchLogs()} disabled={loading}>{loading ? t('common.loading') : t('admin.logs.refresh')}</button>
          <button className="a-clear" onClick={() => setShowClearConfirm(true)}>{t('admin.logs.clear')}</button>
        </div>
      </div>

      {/* 过滤栏 */}
      <div className="a-filter-bar">
        <span className="a-filter-label">{t('admin.logs.level')}</span>
        <select className="a-filter" value={level} onChange={e => setLevel(e.target.value)} aria-label={t('admin.logs.levelAria')}>
          <option value="">{t('admin.logs.all')}</option>
          {LEVELS.map(l => <option key={l} value={l}>{l}</option>)}
        </select>
        <div className="a-search">
          <input type="search" value={searchInput} onChange={e => setSearchInput(e.target.value)} onKeyDown={e => e.key === 'Enter' && setSearch(searchInput.trim())} placeholder={t('admin.logs.searchPlaceholder')} aria-label={t('admin.logs.searchAria')} />
          <button onClick={() => setSearch(searchInput.trim())}>{t('common.search')}</button>
          {search && <button onClick={() => { setSearch(''); setSearchInput('') }} title={t('admin.media.clearSearch')}>×</button>}
        </div>
        <span className="a-filter-label">{t('admin.logs.from')}</span>
        <input type="datetime-local" className="a-filter" value={timeFrom} onChange={e => setTimeFrom(e.target.value)} aria-label={t('admin.logs.startTimeAria')} />
        <span className="a-filter-label">{t('admin.logs.to')}</span>
        <input type="datetime-local" className="a-filter" value={timeTo} onChange={e => setTimeTo(e.target.value)} aria-label={t('admin.logs.endTimeAria')} />
      </div>

      {error && (
        <div className="a-error">
          <span>{error}</span>
          <button onClick={() => fetchLogs()}>{t('common.retry')}</button>
        </div>
      )}

      {pollError && !error && (
        <div className="a-poll-error" role="status">
          <span>{t('admin.logs.pollFailed')}</span>
          <button onClick={() => fetchLogs()}>{t('admin.logs.retryNow')}</button>
        </div>
      )}

      {/* 账号路线视图 */}
      <div className="a-content">
        {/* 左侧：账号列表 */}
        <div className="a-sidebar">
          <h3 className="a-sidebar-title">{t('admin.logs.userList')}</h3>
          <div className="a-user-list">
            {userData.map(({ user, count, types, videos, firstActive, lastActive }) => (
              <div
                key={user}
                className={`a-user-card ${selectedUser === user ? 'selected' : ''}`}
                onClick={() => setSelectedUser(selectedUser === user ? null : user)}
              >
                <div className="a-user-header">
                  <div className="a-user-avatar" style={{ background: TYPE_STYLES[Object.entries(types).sort((a, b) => b[1] - a[1])[0]?.[0] || 'default']?.color || '#6b7280' }}>
                    {user[0]?.toUpperCase()}
                  </div>
                  <div className="a-user-info">
                    <span className="a-user-name">{user}</span>
                    <span className="a-user-meta">{t('admin.logs.operations', { count })} · {t('admin.logs.videos', { count: videos })}</span>
                  </div>
                </div>
                <div className="a-user-tags">
                  {Object.entries(types).sort((a, b) => b[1] - a[1]).slice(0, 4).map(([type, n]) => (
                    <span key={type} className="a-tag" style={{ color: TYPE_STYLES[type]?.color || '#6b7280' }}>
                      {TYPE_ICONS[type] || '·'} {TYPE_STYLES[type] ? t(TYPE_STYLES[type].labelKey) : type} {n}
                    </span>
                  ))}
                </div>
                <div className="a-user-time">
                  <span>{t('admin.logs.firstActive', { time: fmtTimeFull(firstActive) })}</span>
                  <span>{t('admin.logs.lastActive', { time: fmtTimeFull(lastActive) })}</span>
                </div>
              </div>
            ))}
            {userData.length === 0 && !loading && (
              <div className="a-empty-text" style={{ padding: 12 }}>{t('admin.logs.noLogs')}</div>
            )}
          </div>
        </div>

        {/* 右侧：路线详情 */}
        <div className="a-main">
          {userRoute ? (
            <>
              {/* 用户概况 */}
              <div className="a-profile">
                <div className="a-profile-avatar" style={{ background: TYPE_STYLES[Object.entries(userRoute.types).sort((a, b) => b[1] - a[1])[0]?.[0] || 'default']?.color || '#6b7280' }}>
                  {selectedUser?.[0]?.toUpperCase()}
                </div>
                <div className="a-profile-info">
                  <h2 className="a-profile-name">{selectedUser}</h2>
                  <div className="a-profile-stats">
                    <span>{t('admin.logs.operations', { count: userRoute.count })}</span>
                    <span>·</span>
                    <span>{t('admin.logs.videos', { count: userRoute.videos })}</span>
                    <span>·</span>
                    <span>{t('admin.logs.lastActiveShort', { time: fmtTime(userRoute.lastActive) })}</span>
                  </div>
                </div>
              </div>

              {/* 操作分布条 */}
              <div className="a-type-bar">
                {Object.entries(userRoute.types).sort((a, b) => b[1] - a[1]).map(([type, n]) => (
                  <div
                    key={type}
                    className="a-type-seg"
                    style={{
                      width: `${(n / userRoute.count) * 100}%`,
                      background: TYPE_STYLES[type]?.color || '#6b7280',
                    }}
                    title={t('admin.logs.typeCount', { label: TYPE_STYLES[type] ? t(TYPE_STYLES[type].labelKey) : type, count: n })}
                  />
                ))}
              </div>

              {/* 操作路线时间线 */}
              <div className="a-route">
                <h3 className="a-route-title">{t('admin.logs.route')}</h3>
                {userRoute.groups.map((group) => (
                  <div key={group.time} className="a-route-group">
                    <div className="a-route-time">
                      <span className="a-route-date">{new Date(group.time).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })}</span>
                      <span className="a-route-hour">{fmtTime(group.time)}</span>
                    </div>
                    <div className="a-route-line">
                      {group.entries.map(({ entry, formatted }) => {
                        const style = TYPE_STYLES[formatted.type] || { color: '#6b7280', labelKey: 'admin.logs.types.default' }
                        const nodeKey = entry.timestamp + entry.method + entry.path + String(entry.video_id ?? '') + (entry.user ?? '')
                        const isExpanded = expandedIndex === nodeKey
                        return (
                          <div
                            key={nodeKey}
                            className={`a-route-node ${isExpanded ? 'expanded' : ''}`}
                            style={{ '--node-color': style.color } as React.CSSProperties}
                            onClick={() => setExpandedIndex(isExpanded ? null : nodeKey)}
                          >
                            <div className="a-node-dot">{TYPE_ICONS[formatted.type] || '·'}</div>
                            <div className="a-node-content">
                              <span className="a-node-desc">{formatted.desc}</span>
                              {entry.video_id && <span className="a-node-vid">{t('admin.logs.videoId', { id: entry.video_id })}</span>}
                            </div>
                            {isExpanded && (
                              <div className="a-node-details">
                                {entry.path && <div className="a-node-detail"><span className="a-detail-key">{t('admin.logs.path')}</span><span className="a-detail-value">{entry.method} {entry.path}</span></div>}
                                {entry.status && <div className="a-node-detail"><span className="a-detail-key">{t('admin.logs.status')}</span><span className="a-detail-value">{entry.status}</span></div>}
                                {entry.duration_ms && <div className="a-node-detail"><span className="a-detail-key">{t('admin.logs.duration')}</span><span className="a-detail-value">{entry.duration_ms}ms</span></div>}
                                {entry.request_id && <div className="a-node-detail"><span className="a-detail-key">{t('admin.logs.requestId')}</span><span className="a-detail-value">{entry.request_id.slice(0, 8)}</span></div>}
                                {entry.error && <div className="a-node-detail a-detail-error"><span className="a-detail-key">{t('admin.logs.error')}</span><span className="a-detail-value">{entry.error}</span></div>}
                              </div>
                            )}
                            <svg className="a-node-arrow" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ transform: isExpanded ? 'rotate(180deg)' : 'none' }}>
                              <polyline points="6 9 12 15 18 9"/>
                            </svg>
                          </div>
                        )
                      })}
                    </div>
                  </div>
                ))}
              </div>
            </>
          ) : (
            <div className="a-empty">
              <div className="a-empty-icon">👤</div>
              <div className="a-empty-text">{t('admin.logs.selectUser')}</div>
            </div>
          )}
        </div>
      </div>

      {/* 分页 */}
      <div className="a-pagination">
        <button disabled={page === 0} onClick={() => setPage(p => p - 1)}>{t('admin.media.prevPage')}</button>
        <span>{t('admin.logs.pageInfo', { page: page + 1, size: PAGE_SIZE })}</span>
        <button disabled={!hasMore} onClick={() => setPage(p => p + 1)}>{t('admin.media.nextPage')}</button>
      </div>

      <ConfirmDialog
        open={showClearConfirm}
        title={t('admin.logs.clearTitle')}
        message={t('admin.logs.clearConfirm')}
        danger
        confirmText={t('admin.logs.clearBtn')}
        onConfirm={handleClear}
        onCancel={() => setShowClearConfirm(false)}
      />
    </div>
  )
}
