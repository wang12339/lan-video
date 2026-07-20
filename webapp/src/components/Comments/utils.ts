import i18n from '../../i18n'

export function formatDate(s: string): string {
  const d = new Date(s)
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  if (diff < 60000) return i18n.t('comments.justNow')
  if (diff < 3600000) return i18n.t('comments.minutesAgo', { n: Math.floor(diff / 60000) })
  if (diff < 86400000) return i18n.t('comments.hoursAgo', { n: Math.floor(diff / 3600000) })
  if (diff < 2592000000) return i18n.t('comments.daysAgo', { n: Math.floor(diff / 86400000) })
  return s.slice(0, 10)
}
