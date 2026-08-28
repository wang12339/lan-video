import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { clearLogs, type LogEntry, type LogsResponse } from '../../api/logs'
import { request } from '../../api/client'
import { ConfirmDialog } from './components'
import { formatLog, TYPE_STYLES, TYPE_ICONS, fmtTimeFull } from './utils/logFormatter'
import LogFilters from './components/LogFilters'
import UserTimeline from './components/UserTimeline'
import './LogsTab.css'

const PAGE_SIZE = 200
// 2 秒轮询：/admin/logs 无后端限流，且页签隐藏/失焦时已暂停，频率合理
const POLL_INTERVAL_MS = 2000

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

  const errorCount = useMemo(() => visibleEntries.filter(e => e.level === 'ERROR').length, [visibleEntries])
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

      <LogFilters
        level={level}
        onLevelChange={setLevel}
        searchInput={searchInput}
        onSearchInputChange={setSearchInput}
        search={search}
        onSearchChange={setSearch}
        timeFrom={timeFrom}
        onTimeFromChange={setTimeFrom}
        timeTo={timeTo}
        onTimeToChange={setTimeTo}
      />

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
        <UserTimeline
          selectedUser={selectedUser}
          userData={userData}
        />
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
