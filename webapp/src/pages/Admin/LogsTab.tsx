import { useState, useEffect, useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { getLogs, clearLogs, type LogEntry } from '../../api/logs'
import { ConfirmDialog } from './components'
import './LogsTab.css'

// 中文日志翻译
function formatLog(entry: LogEntry): { desc: string; type: string } {
  const msg = entry.message || ''
  const vid = entry.video_id

  if (entry.method && entry.path) {
    const s = entry.status || 0
    const ms = entry.duration_ms || 0

    const routes: Record<string, string> = {
      '/auth/login': '登录', '/auth/logout': '退出', '/auth/register': '注册',
      '/auth/user': '获取用户信息', '/auth/user/profile': '获取用户资料',
      '/auth/refresh': '刷新令牌',
      '/videos': '浏览列表', '/videos/favorites': '查看收藏',
      '/playback/history': '播放历史',
      '/playback/session/start': '开始播放', '/playback/session/stop': '停止播放',
      '/playback/session/heartbeat': '播放心跳',
      '/server/info': '服务器信息', '/health': '健康检查',
      '/admin/logs': '查看日志', '/admin/stats': '查看统计',
      '/admin/system': '系统状态', '/admin/users': '用户列表',
      '/admin/config/registration': '修改注册配置',
      '/admin/videos/scan': '扫描媒体', '/admin/videos/upload': '上传视频',
      '/admin/videos/upload-resume': '续传视频', '/admin/videos/upload-status': '上传状态',
      '/admin/videos/backfill-thumbnails': '补全缩略图',
      '/admin/videos/check-hashes': '检查哈希', '/admin/videos/check-files': '检查文件',
      '/admin/videos/batch-category': '批量分类',
      '/admin/track': '追踪操作',
    }

    let action = routes[entry.path] || ''
    let type = 'view'

    if (!action) {
      if (entry.path.match(/\/videos\/\d+\/like/)) { action = '点赞'; type = 'like' }
      else if (entry.path.match(/\/videos\/\d+\/favorite/)) { action = '收藏'; type = 'fav' }
      else if (entry.path.match(/\/videos\/\d+\/play$/)) { action = '播放'; type = 'play' }
      else if (entry.path.match(/\/videos\/\d+\/view/)) { action = '查看视频'; type = 'view' }
      else if (entry.path.match(/\/videos\/\d+\/stop/)) { action = '停止'; type = 'stop' }
      else if (entry.path.match(/\/videos\/\d+\/heartbeat/)) { action = '播放心跳'; type = 'play' }
      else if (entry.path.match(/\/videos\/\d+$/)) { action = '查看视频详情' }
      else if (entry.path.match(/\/playback\/history\/\d+$/)) { action = '获取播放历史' }
      else if (entry.path.match(/\/admin\/videos\/\d+\/cover/)) { action = '上传封面' }
      else if (entry.path.match(/\/admin\/videos\/\d+$/)) {
        action = entry.method === 'PUT' ? '编辑视频' : '删除视频'
        type = entry.method === 'DELETE' ? 'danger' : 'view'
      }
      else if (entry.path === '/admin/videos/external') { action = '添加外部视频' }
      else if (entry.path === '/admin/videos/batch') { action = '批量删除'; type = 'danger' }
      else if (entry.path.match(/\/admin\/videos\/batch-category/)) { action = '批量分类' }
      else if (entry.path.match(/\/admin\/users\/\d+\/password/)) { action = '重置密码' }
      else if (entry.path.match(/\/admin\/users\/\d+\/approve/)) {
        action = '审批用户'; type = 'admin'
      }
      else if (entry.path.match(/\/admin\/users\/\d+\/admin/)) { action = '切换管理员' }
      else if (entry.path.match(/\/admin\/users\/\d+$/)) {
        action = '删除用户'; type = 'danger'
      }
      else { action = entry.path }
    }

    if (s >= 500) type = 'error'
    else if (s >= 400) type = 'warn'

    const timeStr = ms > 0 ? ` ${ms}毫秒` : ''
    return { desc: `${action}${timeStr}`, type }
  }

  if (msg.includes('server starting')) return { desc: '服务启动', type: 'system' }
  if (msg.includes('shutdown')) return { desc: '服务关闭', type: 'system' }
  if (msg.includes('Database connection')) return { desc: '数据库连接', type: 'system' }
  if (msg.includes('expired tokens')) return { desc: '清理令牌', type: 'system' }
  if (msg.includes('开始播放')) return { desc: `播放视频 #${vid}`, type: 'play' }
  if (msg.includes('停止播放')) return { desc: `停止播放 #${vid}`, type: 'stop' }
  if (msg.includes('toggle like')) return { desc: msg.includes('liked: true') ? '点赞' : '取消点赞', type: 'like' }
  if (msg.includes('toggle favorite')) return { desc: msg.includes('favorited: true') ? '收藏' : '取消收藏', type: 'fav' }
  if (msg.includes('user logged in')) return { desc: '登录成功', type: 'login' }
  if (msg.includes('failed login')) return { desc: '登录失败', type: 'error' }
  if (msg.includes('rate limit')) return { desc: '请求限制', type: 'danger' }
  if (msg.includes('Path traversal')) return { desc: '路径遍历警告', type: 'danger' }
  if (msg.includes('invalid file')) return { desc: '无效文件警告', type: 'danger' }
  if (msg.includes('admin deleted')) return { desc: '管理员删除操作', type: 'danger' }
  if (msg.includes('admin approved')) return { desc: '管理员审批操作', type: 'admin' }
  if (msg.includes('registration toggle')) return { desc: '修改注册配置', type: 'system' }
  if (msg.includes('media scan')) return { desc: '扫描媒体文件', type: 'system' }
  if (msg.includes('thumbnail backfill')) return { desc: '补全缩略图', type: 'system' }
  if (msg.includes('rate limiter cleanup')) return { desc: '清理限流记录', type: 'system' }

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

const TYPE_STYLES: Record<string, { color: string; label: string }> = {
  play: { color: '#8b5cf6', label: '播放' },
  stop: { color: '#6b7280', label: '停止' },
  like: { color: '#ec4899', label: '点赞' },
  fav: { color: '#f59e0b', label: '收藏' },
  login: { color: '#3b82f6', label: '登录' },
  view: { color: '#10b981', label: '浏览' },
  admin: { color: '#f97316', label: '管理' },
  danger: { color: '#ef4444', label: '危险' },
  error: { color: '#ef4444', label: '错误' },
  warn: { color: '#f59e0b', label: '警告' },
  system: { color: '#6b7280', label: '系统' },
  default: { color: '#9ca3af', label: '其他' },
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
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [selectedUser, setSelectedUser] = useState<string | null>(null)
  const [expandedIndex, setExpandedIndex] = useState<string | null>(null)
  const [showClearConfirm, setShowClearConfirm] = useState(false)

  const fetchLogs = useCallback(async () => {
    try {
      const res = await getLogs({ limit: 500 })
      setEntries(res.entries)
      setTotal(res.total)
    } catch (e) {
      console.error('Failed to fetch logs:', e)
    }
  }, [])

  useEffect(() => { fetchLogs() }, [fetchLogs])

  useEffect(() => {
    if (!autoRefresh) return
    let timer: ReturnType<typeof setInterval>
    const startPolling = () => { timer = setInterval(fetchLogs, 2000) }
    const stopPolling = () => { clearInterval(timer) }
    
    const handleVisibility = () => {
      if (document.hidden) stopPolling()
      else startPolling()
    }
    
    startPolling()
    document.addEventListener('visibilitychange', handleVisibility)
    return () => { stopPolling(); document.removeEventListener('visibilitychange', handleVisibility) }
  }, [autoRefresh, fetchLogs])

  const handleClear = async () => {
    try { await clearLogs(); fetchLogs() } catch { /* noop */ }
  }

  const fmtTime = (ts: string) => {
    if (!ts) return ''
    try { return new Date(ts).toLocaleTimeString('zh-CN', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }) }
    catch { return ts }
  }

  const fmtTimeFull = (ts: string) => {
    if (!ts) return ''
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

    entries.forEach(e => {
      if (!e.user) return
      if (!map[e.user]) map[e.user] = { logs: [], count: 0, types: {}, lastActive: '', firstActive: '', videos: new Set() }
      const u = map[e.user]!
      u.logs.push(e)
      u.count++
      const { type } = formatLog(e)
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
  }, [entries])

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
      const t = ts.slice(0, 16) // YYYY-MM-DDTHH:MM
      if (t !== lastGroup) {
        lastGroup = t
        groups.push({ time: ts, entries: [] })
      }
      groups[groups.length - 1]!.entries.push({ entry, formatted: formatLog(entry) })
    })

    return { ...u, groups }
  }, [userData, selectedUser])

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
            <span className="a-stat-num">{entries.filter(e => e.level === 'ERROR').length}</span>
            <span className="a-stat-label">{t('admin.logs.errors')}</span>
          </div>
        </div>
        <div className="a-actions">
          <label className="a-toggle">
            <input type="checkbox" checked={autoRefresh} onChange={e => setAutoRefresh(e.target.checked)} />
            <span className="a-toggle-track"></span>
          </label>
          <span className="a-toggle-text">{autoRefresh ? t('admin.logs.autoRefresh') : t('admin.logs.paused')}</span>
          <button className="a-clear" onClick={() => setShowClearConfirm(true)}>{t('admin.logs.clear')}</button>
        </div>
      </div>

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
                      {TYPE_ICONS[type] || '·'} {TYPE_STYLES[type]?.label || type} {n}
                    </span>
                  ))}
                </div>
                <div className="a-user-time">
                  <span>{t('admin.logs.firstActive', { time: fmtTimeFull(firstActive) })}</span>
                  <span>{t('admin.logs.lastActive', { time: fmtTimeFull(lastActive) })}</span>
                </div>
              </div>
            ))}
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
                    title={`${TYPE_STYLES[type]?.label || type}: ${n} 次`}
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
                        const style = TYPE_STYLES[formatted.type] || { color: '#6b7280', label: '其他' }
                        const nodeKey = entry.timestamp + entry.method + entry.path
                        const isExpanded = expandedIndex === nodeKey
                        return (
                          <div
                            key={entry.timestamp + entry.method + entry.path}
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
                                {entry.path && <div className="a-node-detail"><span className="a-detail-key">路径</span><span className="a-detail-value">{entry.method} {entry.path}</span></div>}
                                {entry.status && <div className="a-node-detail"><span className="a-detail-key">状态</span><span className="a-detail-value">{entry.status}</span></div>}
                                {entry.duration_ms && <div className="a-node-detail"><span className="a-detail-key">耗时</span><span className="a-detail-value">{entry.duration_ms}ms</span></div>}
                                {entry.request_id && <div className="a-node-detail"><span className="a-detail-key">请求ID</span><span className="a-detail-value">{entry.request_id.slice(0, 8)}</span></div>}
                                {entry.error && <div className="a-node-detail a-detail-error"><span className="a-detail-key">错误</span><span className="a-detail-value">{entry.error}</span></div>}
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
