import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import './WatchStats.css'

interface WatchStatsProps {
  visible: boolean
  onClose: () => void
}

interface WatchData {
  totalMinutes: number
  totalVideos: number
  dailyMinutes: number[]
  topVideos: Array<{
    title: string
    minutes: number
  }>
}

const STORAGE_KEY = 'atmos_watch_stats'

function getWatchData(): WatchData {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) {
      return {
        totalMinutes: 0,
        totalVideos: 0,
        dailyMinutes: new Array(7).fill(0),
        topVideos: [],
      }
    }
    return JSON.parse(raw) as WatchData
  } catch {
    return {
      totalMinutes: 0,
      totalVideos: 0,
      dailyMinutes: new Array(7).fill(0),
      topVideos: [],
    }
  }
}

export function recordWatchTime(_videoId: string, title: string, seconds: number): void {
  const data = getWatchData()
  const minutes = Math.floor(seconds / 60)

  data.totalMinutes += minutes
  if (minutes > 0) {
    data.totalVideos += 1
  }

  // Update daily stats (last 7 days)
  const today = new Date().getDay()
  data.dailyMinutes[today] = (data.dailyMinutes[today] || 0) + minutes

  // Update top videos
  const existing = data.topVideos.find((v) => v.title === title)
  if (existing) {
    existing.minutes += minutes
  } else {
    data.topVideos.push({ title, minutes })
  }
  data.topVideos.sort((a, b) => b.minutes - a.minutes)
  data.topVideos = data.topVideos.slice(0, 10)

  localStorage.setItem(STORAGE_KEY, JSON.stringify(data))
}

export default function WatchStats({ visible, onClose }: WatchStatsProps) {
  const { t } = useTranslation()
  const [data, setData] = useState<WatchData>(getWatchData())

  useEffect(() => {
    if (visible) {
      setData(getWatchData())
    }
  }, [visible])

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape' && visible) {
      onClose()
    }
  }, [visible, onClose])

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  // Lock body scroll when visible
  useEffect(() => {
    if (visible) {
      document.body.style.overflow = 'hidden'
    } else {
      document.body.style.overflow = ''
    }
    return () => { document.body.style.overflow = '' }
  }, [visible])

  const formatMinutes = (minutes: number) => {
    if (minutes < 60) return `${minutes}分钟`
    const hours = Math.floor(minutes / 60)
    const mins = minutes % 60
    return mins > 0 ? `${hours}小时${mins}分钟` : `${hours}小时`
  }

  const dayNames = ['日', '一', '二', '三', '四', '五', '六']
  const maxDaily = Math.max(...data.dailyMinutes, 1)

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

        <div className="watch-stats-content">
          {data.totalMinutes === 0 ? (
            <div className="watch-stats-empty">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                <path d="M12 8v4l3 3" />
              </svg>
              <p>{t('stats.noData')}</p>
            </div>
          ) : (
            <>
              {/* Summary */}
              <div className="watch-stats-summary">
                <div className="watch-stats-card">
                  <div className="watch-stats-card-value">
                    {formatMinutes(data.totalMinutes)}
                  </div>
                  <div className="watch-stats-card-label">{t('stats.totalWatchTime')}</div>
                </div>
                <div className="watch-stats-card">
                  <div className="watch-stats-card-value">{data.totalVideos}</div>
                  <div className="watch-stats-card-label">{t('stats.videosWatched')}</div>
                </div>
              </div>

              {/* Weekly chart */}
              <div className="watch-stats-chart">
                <div className="watch-stats-chart-title">{t('stats.weeklyActivity')}</div>
                <div className="watch-stats-bars">
                  {data.dailyMinutes.map((minutes, index) => (
                    <div
                      key={index}
                      className="watch-stats-bar"
                      style={{ height: `${(minutes / maxDaily) * 100}%` }}
                      data-label={dayNames[index]}
                      title={`${dayNames[index]}: ${formatMinutes(minutes)}`}
                    />
                  ))}
                </div>
              </div>

              {/* Top videos */}
              {data.topVideos.length > 0 && (
                <div className="watch-stats-top">
                  <div className="watch-stats-top-title">{t('stats.topVideos')}</div>
                  <div className="watch-stats-top-list">
                    {data.topVideos.slice(0, 5).map((video, index) => (
                      <div key={video.title} className="watch-stats-top-item">
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
            </>
          )}
        </div>
      </div>
    </>
  )
}
