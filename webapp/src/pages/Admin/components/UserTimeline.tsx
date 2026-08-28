import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { LogEntry as LogEntryType } from '../../../api/logs'
import { formatLog, TYPE_STYLES, fmtTime } from '../utils/logFormatter'
import LogEntry from './LogEntry'

interface UserData {
  user: string
  logs: LogEntryType[]
  count: number
  types: Record<string, number>
  lastActive: string
  firstActive: string
  videos: number
}

interface UserTimelineProps {
  selectedUser: string | null
  userData: UserData[]
}

export default function UserTimeline({ selectedUser, userData }: UserTimelineProps) {
  const { t } = useTranslation()
  const [expandedIndex, setExpandedIndex] = useState<string | null>(null)

  const userRoute = useMemo(() => {
    if (!selectedUser) return null
    const u = userData.find(d => d.user === selectedUser)
    if (!u) return null

    const groups: { time: string; entries: { entry: LogEntryType; formatted: ReturnType<typeof formatLog> }[] }[] = []
    let lastGroup = ''

    u.logs.forEach(entry => {
      const ts = entry.timestamp || ''
      const tk = ts.slice(0, 16)
      if (tk !== lastGroup) {
        lastGroup = tk
        groups.push({ time: ts, entries: [] })
      }
      groups[groups.length - 1]!.entries.push({ entry, formatted: formatLog(entry, t) })
    })

    return { ...u, groups }
  }, [userData, selectedUser, t])

  const handleToggle = (key: string) => {
    setExpandedIndex(prev => prev === key ? null : key)
  }

  const dominantType = userRoute
    ? Object.entries(userRoute.types).sort((a, b) => b[1] - a[1])[0]?.[0] || 'default'
    : 'default'

  return (
    <div className="a-main">
      {userRoute ? (
        <>
          <div className="a-profile">
            <div className="a-profile-avatar" style={{ background: TYPE_STYLES[dominantType]?.color || '#6b7280' }}>
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
                    const nodeKey = entry.timestamp + entry.method + entry.path + String(entry.video_id ?? '') + (entry.user ?? '')
                    return (
                      <LogEntry
                        key={nodeKey}
                        entry={entry}
                        formatted={formatted}
                        nodeKey={nodeKey}
                        isExpanded={expandedIndex === nodeKey}
                        onToggle={handleToggle}
                      />
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
  )
}

export type { UserData }
