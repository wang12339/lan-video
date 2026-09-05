// 国际化增强工具
import i18n from '../i18n'

// 日期格式化
export function formatDate(
  date: Date | string | number,
  locale: string = 'zh-CN',
  options?: Intl.DateTimeFormatOptions
): string {
  const d = new Date(date)
  if (isNaN(d.getTime())) return ''
  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    ...options
  }
  return d.toLocaleDateString(locale, defaultOptions)
}

// 时间格式化
export function formatTime(
  date: Date | string | number,
  locale: string = 'zh-CN',
  options?: Intl.DateTimeFormatOptions
): string {
  const d = new Date(date)
  if (isNaN(d.getTime())) return ''
  const defaultOptions: Intl.DateTimeFormatOptions = {
    hour: '2-digit',
    minute: '2-digit',
    ...options
  }
  return d.toLocaleTimeString(locale, defaultOptions)
}

// 相对时间格式化
export function formatRelativeTime(
  date: Date | string | number,
  locale: string = 'zh-CN'
): string {
  const d = new Date(date)
  if (isNaN(d.getTime())) return formatDate(new Date(), locale)
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  
  if (diff < 0) return formatDate(d, locale)
  
  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)

  if (seconds < 60) return i18n.t('time.justNow')
  if (minutes < 60) return i18n.t('time.minutesAgo', { n: minutes })
  if (hours < 24) return i18n.t('time.hoursAgo', { n: hours })
  if (days < 7) return i18n.t('time.daysAgo', { n: days })

  return formatDate(d, locale)
}

// 数字格式化
export function formatNumber(
  num: number,
  locale: string = 'zh-CN',
  options?: Intl.NumberFormatOptions
): string {
  if (!Number.isFinite(num)) return '0'
  return num.toLocaleString(locale, options)
}

// 文件大小格式化
export function formatFileSize(bytes: number, _locale: string = 'zh-CN'): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const k = 1024
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), units.length - 1)
  const size = bytes / Math.pow(k, i)
  // 使用 toFixed 确保一致的格式，然后根据 locale 添加千位分隔符
  const formatted = i === 0 ? size.toString() : size.toFixed(1)
  return `${formatted} ${units[i]}`
}

// 百分比格式化
export function formatPercent(
  value: number,
  locale: string = 'zh-CN',
  decimals: number = 0
): string {
  if (!Number.isFinite(value)) return '0%'
  return formatNumber(value / 100, locale, {
    style: 'percent',
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals
  })
}

// 货币格式化
export function formatCurrency(
  amount: number,
  currency: string = 'CNY',
  locale: string = 'zh-CN'
): string {
  if (!Number.isFinite(amount)) return '0'
  return formatNumber(amount, locale, {
    style: 'currency',
    currency
  })
}

// 复数处理
export function pluralize(
  count: number,
  singular: string,
  plural?: string,
  locale: string = 'zh-CN'
): string {
  // 中文没有复数形式
  if (locale.startsWith('zh')) {
    return `${count} ${singular}`
  }
  
  // 英文复数规则
  if (count === 1) {
    return `1 ${singular}`
  }
  
  return `${count} ${plural || singular + 's'}`
}

// 列表格式化
export function formatList(
  items: string[],
  locale: string = 'zh-CN',
  style: 'long' | 'short' | 'narrow' = 'long'
): string {
  if (items.length === 0) return ''
  if (items.length === 1) return items[0] || ''
  
  // 使用Intl.ListFormat（如果可用）
  try {
    const ListFormat = (Intl as unknown as { ListFormat: new (locale: string, options: { style: string }) => { format: (items: string[]) => string } }).ListFormat
    return new ListFormat(locale, { style }).format(items)
  } catch {
    // 回退到简单拼接
    return items.join(style === 'narrow' ? '/' : '、')
  }
}

// 时区转换
export function toTimezone(
  date: Date | string | number,
  timezone: string
): Date {
  const d = new Date(date)
  const parts = d.toLocaleString('en-US', {
    timeZone: timezone,
    hour12: false,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
  const segments = parts.split(', ')
  const datePart = segments[0] ?? ''
  const timePart = segments[1] ?? ''
  const [monthStr, dayStr, yearStr] = datePart.split('/')
  const [hourStr, minuteStr, secondStr] = timePart.split(':')
  const year = parseInt(yearStr ?? '0', 10)
  const month = parseInt(monthStr ?? '1', 10) - 1
  const day = parseInt(dayStr ?? '1', 10)
  const hour = parseInt(hourStr ?? '0', 10)
  const minute = parseInt(minuteStr ?? '0', 10)
  const second = parseInt(secondStr ?? '0', 10)
  if (isNaN(year) || isNaN(month) || isNaN(day) || isNaN(hour) || isNaN(minute) || isNaN(second)) {
    return d
  }
  return new Date(year, month, day, hour, minute, second)
}

// 获取用户时区
export function getUserTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone
}
