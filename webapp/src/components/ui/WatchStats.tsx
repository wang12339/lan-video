import { useState, useEffect, useCallback, useMemo, memo } from 'react'
import { useTranslation } from 'react-i18next'
import './WatchStats.css'

// ─── Types ────────────────────────────────────────────────────────────────────

interface WatchStatsProps {
  visible: boolean
  onClose: () => void
}

interface WatchRecord {
  videoId: string
  title: string
  minutes: number
  timestamp: number // ISO timestamp
}

interface WatchData {
  records: WatchRecord[]
  totalMinutes: number
  totalVideos: number
}

type TimeFilter = '7d' | '30d' | '90d' | 'all'

// ─── Constants ────────────────────────────────────────────────────────────────

const STORAGE_KEY = 'atmos_watch_stats_v2'

const FILTER_LABELS: Record<TimeFilter, string> = {
  '7d': '7天',
  '30d': '30天',
  '90d': '90天',
  'all': '全部',
}

// ─── Storage Helpers ──────────────────────────────────────────────────────────

const WATCH_STATS_MAX_AGE_MS = 90 * 24 * 60 * 60 * 1000

function isPrivacyEnabled(): boolean {
  try {
    return localStorage.getItem('atmos_privacy_mode') === 'true'
  } catch {
    return false
  }
}

function getWatchData(): WatchData {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return { records: [], totalMinutes: 0, totalVideos: 0 }
    const parsed = JSON.parse(raw) as Partial<WatchData>
    let records = Array.isArray(parsed.records) ? parsed.records : []
    // 90天过期滚动清理
    const cutoff = Date.now() - WATCH_STATS_MAX_AGE_MS
    records = records.filter((r) => typeof r.timestamp === 'number' && r.timestamp >= cutoff)
    return {
      records,
      totalMinutes: parsed.totalMinutes ?? 0,
      totalVideos: parsed.totalVideos ?? 0,
    }
  } catch {
    return { records: [], totalMinutes: 0, totalVideos: 0 }
  }
}

export function recordWatchTime(videoId: string, title: string, seconds: number): void {
  try {
    if (isPrivacyEnabled()) return
    const data = getWatchData()
    const minutes = Math.floor(seconds / 60)
    if (minutes <= 0) return

    data.records.push({ videoId, title, minutes, timestamp: Date.now() })
    data.totalMinutes += minutes
    data.totalVideos += 1

    // Keep last 1000 records
    if (data.records.length > 1000) {
      data.records = data.records.slice(-1000)
    }

    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data))
    } catch (e) {
      // QuotaExceededError: 丢弃最旧 200 条重试
      if (e instanceof DOMException && e.name === 'QuotaExceededError' && data.records.length > 200) {
        data.records = data.records.slice(-800)
        try { localStorage.setItem(STORAGE_KEY, JSON.stringify(data)) } catch {}
      }
    }
  } catch {}
}

// ─── Aggregation Utilities ────────────────────────────────────────────────────

function filterRecordsByTime(records: WatchRecord[], filter: TimeFilter): WatchRecord[] {
  if (filter === 'all') return records
  const daysMap: Record<Exclude<TimeFilter, 'all'>, number> = { '7d': 7, '30d': 30, '90d': 90 }
  const days = daysMap[filter as Exclude<TimeFilter, 'all'>]
  const cutoff = Date.now() - days * 24 * 60 * 60 * 1000
  return records.filter((r) => r.timestamp >= cutoff)
}

function buildDailyData(records: WatchRecord[], days: number): Array<{ date: string; value: number }> {
  const now = new Date()
  const map = new Map<string, number>()

  for (const r of records) {
    const d = new Date(r.timestamp)
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    map.set(key, (map.get(key) ?? 0) + r.minutes)
  }

  const result: Array<{ date: string; value: number }> = []
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(now)
    d.setDate(d.getDate() - i)
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    result.push({ date: key, value: map.get(key) ?? 0 })
  }
  return result
}

function buildTopVideos(records: WatchRecord[], limit: number): Array<{ title: string; minutes: number; videoId: string }> {
  const map = new Map<string, { title: string; minutes: number; videoId: string }>()
  for (const r of records) {
    const existing = map.get(r.videoId)
    if (existing) {
      existing.minutes += r.minutes
    } else {
      map.set(r.videoId, { title: r.title, minutes: r.minutes, videoId: r.videoId })
    }
  }
  return [...map.values()].sort((a, b) => b.minutes - a.minutes).slice(0, limit)
}

// ─── Chart Component ─────────────────────────────────────────────────────────

function MiniChart({
  data,
  height = 100,
  color = 'var(--accent)',
  showLabels = true,
}: {
  data: Array<{ date: string; value: number }>
  height?: number
  color?: string
  showLabels?: boolean
}) {
  const maxVal = Math.max(...data.map((d) => d.value), 1)
  const barWidth = Math.max(2, Math.min(16, (100 / data.length) * 0.7))

  return (
    <div className="watch-stats-chart-container" style={{ height }}>
      <div className="watch-stats-chart-bars">
        {data.map((d, i) => (
          <div
            key={i}
            className="watch-stats-chart-col"
            style={{ width: `${100 / data.length}%` }}
          >
            <div
              className="watch-stats-chart-bar"
              style={{
                height: `${(d.value / maxVal) * 100}%`,
                width: `${barWidth}px`,
                background: color,
              }}
              title={`${d.date}: ${d.value}分钟`}
            />
            {showLabels && data.length <= 14 && (
              <div className="watch-stats-chart-label">
                {d.date.slice(5)}
              </div>
            )}
          </div>
        ))}
      </div>
      <div className="watch-stats-chart-grid">
        {[0.25, 0.5, 0.75, 1].map((pct) => (
          <div
            key={pct}
            className="watch-stats-chart-gridline"
            style={{ bottom: `${pct * 100}%` }}
          >
            <span>{Math.round(maxVal * pct)}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

export default memo(WatchStatsImpl)

// ─── Export Utility ───────────────────────────────────────────────────────────

function exportToCSV(records: WatchRecord[]) {
  const header = '视频ID,标题,观看分钟数,时间戳\n'
  const rows = records.map((r) =>
    `"${r.videoId}","${r.title.replace(/"/g, '""')}",${r.minutes},${new Date(r.timestamp).toISOString()}`
  ).join('\n')

  const bom = '\uFEFF'
  const blob = new Blob([bom + header + rows], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `watch_stats_${new Date().toISOString().slice(0, 10)}.csv`
  a.click()
  URL.revokeObjectURL(url)
}

// ─── Main Component ───────────────────────────────────────────────────────────

function WatchStatsImpl({ visible, onClose }: WatchStatsProps) {
  const { t } = useTranslation()
  const [filter, setFilter] = useState<TimeFilter>('7d')
  const [data, setData] = useState<WatchData>(getWatchData())

  useEffect(() => {
    if (visible) setData(getWatchData())
  }, [visible])

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape' && visible) onClose()
    },
    [visible, onClose]
  )

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  useEffect(() => {
    if (visible) {
      document.body.style.overflow = 'hidden'
    } else {
      document.body.style.overflow = ''
    }
    return () => { document.body.style.overflow = '' }
  }, [visible])

  // ─── Derived Data ───────────────────────────────────────────────────────────

  const filteredRecords = useMemo(
    () => filterRecordsByTime(data.records, filter),
    [data.records, filter]
  )

  const chartDays = filter === '7d' ? 7 : filter === '30d' ? 30 : filter === '90d' ? 90 : 90
  const dailyData = useMemo(
    () => buildDailyData(filteredRecords, chartDays),
    [filteredRecords, chartDays]
  )

  const topVideos = useMemo(
    () => buildTopVideos(filteredRecords, 5),
    [filteredRecords]
  )

  const filteredTotalMinutes = useMemo(
    () => filteredRecords.reduce((sum, r) => sum + r.minutes, 0),
    [filteredRecords]
  )

  const filteredVideoCount = useMemo(
    () => new Set(filteredRecords.map((r) => r.videoId)).size,
    [filteredRecords]
  )

  const avgDailyMinutes = useMemo(() => {
    const days = Math.max(1, chartDays)
    return Math.round(filteredTotalMinutes / days)
  }, [filteredTotalMinutes, chartDays])

  const formatMinutes = useMemo(() => {
    return (minutes: number) => {
      if (minutes < 60) return `${minutes}分钟`
      const hours = Math.floor(minutes / 60)
      const mins = minutes % 60
      return mins > 0 ? `${hours}小时${mins}分钟` : `${hours}小时`
    }
  }, [])

  const isEmpty = data.records.length === 0

  // ─── Render ─────────────────────────────────────────────────────────────────

  return (
    <>
      <div
        className={`watch-stats-overlay ${visible ? 'visible' : ''}`}
        onClick={onClose}
      />
      <div
        className={`watch-stats ${visible ? 'visible' : ''}`}
        role="dialog"
        aria-modal="true"
        aria-label={t('stats.title')}
      >
        {/* Header */}
        <div className="watch-stats-header">
          <h2 className="watch-stats-title">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              <path d="M9 12l2 2 4-4" />
            </svg>
            {t('stats.title')}
          </h2>
          <button className="watch-stats-close" onClick={onClose} aria-label={t('common.close')}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="watch-stats-content">
          {isEmpty ? (
            <div className="watch-stats-empty">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                <path d="M12 8v4l3 3" />
              </svg>
              <p>{t('stats.noData')}</p>
            </div>
          ) : (
            <>
              {/* Time Filter */}
              <div className="watch-stats-filter">
                {(['7d', '30d', '90d', 'all'] as TimeFilter[]).map((f) => (
                  <button
                    key={f}
                    type="button"
                    className={`watch-stats-filter-btn ${filter === f ? 'active' : ''}`}
                    aria-pressed={filter === f}
                    onClick={() => setFilter(f)}
                  >
                    {FILTER_LABELS[f]}
                  </button>
                ))}
              </div>

              {/* Summary Cards */}
              <div className="watch-stats-summary">
                <div className="watch-stats-card">
                  <div className="watch-stats-card-icon">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <circle cx="12" cy="12" r="10" />
                      <path d="M12 6v6l4 2" />
                    </svg>
                  </div>
                  <div className="watch-stats-card-value">
                    {formatMinutes(filteredTotalMinutes)}
                  </div>
                  <div className="watch-stats-card-label">{t('stats.totalWatchTime')}</div>
                </div>
                <div className="watch-stats-card">
                  <div className="watch-stats-card-icon">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M4 6h16M4 12h16M4 18h7" />
                    </svg>
                  </div>
                  <div className="watch-stats-card-value">{filteredVideoCount}</div>
                  <div className="watch-stats-card-label">{t('stats.videosWatched')}</div>
                </div>
                <div className="watch-stats-card">
                  <div className="watch-stats-card-icon">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M3 3v18h18" />
                      <path d="M7 16l4-8 4 4 4-6" />
                    </svg>
                  </div>
                  <div className="watch-stats-card-value">
                    {avgDailyMinutes}分钟
                  </div>
                  <div className="watch-stats-card-label">日均观看</div>
                </div>
              </div>

              {/* Chart */}
              <div className="watch-stats-chart">
                <div className="watch-stats-chart-header">
                  <span className="watch-stats-chart-title">观看趋势</span>
                  <span className="watch-stats-chart-subtitle">
                    {dailyData.length > 0
                      ? `${dailyData[0]!.date} ~ ${dailyData[dailyData.length - 1]!.date}`
                      : ''}
                  </span>
                </div>
                <MiniChart
                  data={dailyData}
                  height={120}
                  showLabels={chartDays <= 14}
                />
              </div>

              {/* Top Videos */}
              {topVideos.length > 0 && (
                <div className="watch-stats-top">
                  <div className="watch-stats-top-title">{t('stats.topVideos')}</div>
                  <div className="watch-stats-top-list">
                    {topVideos.map((video, index) => (
                      <div key={video.videoId} className="watch-stats-top-item">
                        <span
                          className={`watch-stats-top-rank ${
                            index === 0 ? 'gold' : index === 1 ? 'silver' : index === 2 ? 'bronze' : ''
                          }`}
                        >
                          {index + 1}
                        </span>
                        <div className="watch-stats-top-info">
                          <div className="watch-stats-top-name">{video.title}</div>
                          <div className="watch-stats-top-time">
                            {formatMinutes(video.minutes)}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Export */}
              <button
                type="button"
                className="watch-stats-export-btn"
                onClick={() => exportToCSV(filteredRecords)}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                  <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
                  <polyline points="7 10 12 15 17 10" />
                  <line x1="12" y1="15" x2="12" y2="3" />
                </svg>
                导出 CSV
              </button>
            </>
          )}
        </div>
      </div>
    </>
  )
}
