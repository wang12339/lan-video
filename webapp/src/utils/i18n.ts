// 国际化增强工具

// 日期格式化
export function formatDate(
  date: Date | string | number,
  locale: string = 'zh-CN',
  options?: Intl.DateTimeFormatOptions
): string {
  const d = new Date(date)
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
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  
  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  
  if (seconds < 60) return '刚刚'
  if (minutes < 60) return `${minutes}分钟前`
  if (hours < 24) return `${hours}小时前`
  if (days < 7) return `${days}天前`
  
  return formatDate(d, locale)
}

// 数字格式化
export function formatNumber(
  num: number,
  locale: string = 'zh-CN',
  options?: Intl.NumberFormatOptions
): string {
  return num.toLocaleString(locale, options)
}

// 文件大小格式化
export function formatFileSize(bytes: number, locale: string = 'zh-CN'): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = bytes
  let unitIndex = 0
  
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024
    unitIndex++
  }
  
  return `${formatNumber(size, locale, { maximumFractionDigits: 1 })} ${units[unitIndex]}`
}

// 百分比格式化
export function formatPercent(
  value: number,
  locale: string = 'zh-CN',
  decimals: number = 0
): string {
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
  return new Date(d.toLocaleString('en-US', { timeZone: timezone }))
}

// 获取用户时区
export function getUserTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone
}
