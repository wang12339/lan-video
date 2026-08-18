import i18n from '../../i18n'

export const COMMENT_MAX_LENGTH = 2000

export function formatDate(s: string): string {
  const d = new Date(s)
  if (Number.isNaN(d.getTime())) return s.slice(0, 10)
  const diff = Date.now() - d.getTime()
  if (diff < 60000) return i18n.t('comments.justNow')
  if (diff < 3600000) return i18n.t('comments.minutesAgo', { n: Math.floor(diff / 60000) })
  if (diff < 86400000) return i18n.t('comments.hoursAgo', { n: Math.floor(diff / 3600000) })
  if (diff < 2592000000) return i18n.t('comments.daysAgo', { n: Math.floor(diff / 86400000) })
  return s.slice(0, 10)
}

export function getInitial(username: string): string {
  return (username.trim() || '?')[0] || '?'
}
