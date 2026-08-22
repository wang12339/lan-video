/**
 * 搜索历史管理（增强版）
 * - 最多保存30条记录
 * - 按时间倒序排列
 * - 支持删除单条和清空
 * - 隐私模式支持
 * - 数据验证和容量限制
 * - 过期记录自动清理
 */

const STORAGE_KEY = 'atmos_search_history'
const PRIVACY_KEY = 'atmos_privacy_mode'
const MAX_HISTORY = 30
const MAX_QUERY_LENGTH = 100
const DEFAULT_EXPIRY_DAYS = 30

export interface SearchHistoryItem {
  query: string
  timestamp: number
  isPrivate?: boolean
}

export interface SearchHistoryOptions {
  maxItems?: number
  expiryDays?: number
  respectPrivacy?: boolean
}

/**
 * 检查是否处于隐私模式
 */
export function isPrivacyMode(): boolean {
  try {
    return localStorage.getItem(PRIVACY_KEY) === 'true'
  } catch {
    return false
  }
}

/**
 * 设置隐私模式
 */
export function setPrivacyMode(enabled: boolean): void {
  try {
    localStorage.setItem(PRIVACY_KEY, String(enabled))
    // 如果开启隐私模式，清除现有历史记录
    if (enabled) {
      clearSearchHistory()
    }
  } catch (error) {
    console.error('Failed to set privacy mode:', error)
  }
}

/**
 * 获取搜索历史（带验证和清理）
 */
export function getSearchHistory(options: SearchHistoryOptions = {}): SearchHistoryItem[] {
  const { expiryDays = DEFAULT_EXPIRY_DAYS, respectPrivacy = true } = options

  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []

    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []

    const now = Date.now()
    const expiryMs = expiryDays * 24 * 60 * 60 * 1000

    // 过滤和验证记录
    const validItems = parsed
      .filter((item): item is SearchHistoryItem => {
        // 基本结构验证
        if (!item || typeof item !== 'object') return false
        if (typeof item.query !== 'string' || typeof item.timestamp !== 'number') return false
        
        // 查询长度验证
        const trimmed = item.query.trim()
        if (trimmed.length < 2 || trimmed.length > MAX_QUERY_LENGTH) return false
        
        // 时间戳验证
        if (item.timestamp <= 0 || item.timestamp > now + 86400000) return false // 不允许未来时间
        
        // 过期检查
        if (now - item.timestamp > expiryMs) return false
        
        // 隐私模式检查
        if (respectPrivacy && isPrivacyMode() && !item.isPrivate) return false
        
        return true
      })
      .map(item => ({
        ...item,
        query: item.query.trim(),
        isPrivate: item.isPrivate || false
      }))

    // 按时间倒序排序
    validItems.sort((a, b) => b.timestamp - a.timestamp)

    // 去重（保留最新的）
    const seen = new Set<string>()
    const uniqueItems = validItems.filter(item => {
      const key = item.query.toLowerCase()
      if (seen.has(key)) return false
      seen.add(key)
      return true
    })

    // 限制数量
    return uniqueItems.slice(0, options.maxItems || MAX_HISTORY)
  } catch (error) {
    console.error('Failed to read search history:', error)
    return []
  }
}

/**
 * 添加搜索记录
 */
export function addToSearchHistory(query: string): boolean {
  const privacyMode = isPrivacyMode()
  
  // 隐私模式下不保存（除非明确标记为私有）
  if (privacyMode) {
    return false
  }

  const trimmed = query.trim()
  if (!trimmed || trimmed.length < 2 || trimmed.length > MAX_QUERY_LENGTH) {
    return false
  }

  try {
    const history = getSearchHistory({ respectPrivacy: false })
    
    // 检查是否已存在（不区分大小写）
    const existingIndex = history.findIndex(
      item => item.query.toLowerCase() === trimmed.toLowerCase()
    )
    
    if (existingIndex !== -1 && history[existingIndex]) {
      // 如果已存在，更新时间戳
      history[existingIndex].timestamp = Date.now()
    } else {
      // 添加新记录
      history.unshift({
        query: trimmed,
        timestamp: Date.now(),
        isPrivate: false
      })
    }

    // 限制数量
    const limitedHistory = history.slice(0, MAX_HISTORY)
    
    localStorage.setItem(STORAGE_KEY, JSON.stringify(limitedHistory))
    return true
  } catch (error) {
    console.error('Failed to add to search history:', error)
    return false
  }
}

/**
 * 删除单条记录
 */
export function removeFromSearchHistory(query: string): boolean {
  try {
    const history = getSearchHistory({ respectPrivacy: false })
    const filteredHistory = history.filter(
      item => item.query.toLowerCase() !== query.toLowerCase()
    )
    
    if (filteredHistory.length === history.length) {
      return false // 没有找到要删除的记录
    }

    localStorage.setItem(STORAGE_KEY, JSON.stringify(filteredHistory))
    return true
  } catch (error) {
    console.error('Failed to remove from search history:', error)
    return false
  }
}

/**
 * 清除所有历史记录
 */
export function clearSearchHistory(): boolean {
  try {
    localStorage.removeItem(STORAGE_KEY)
    return true
  } catch (error) {
    console.error('Failed to clear search history:', error)
    return false
  }
}

/**
 * 清除过期记录
 */
export function cleanExpiredHistory(expiryDays: number = DEFAULT_EXPIRY_DAYS): number {
  try {
    const history = getSearchHistory({ 
      expiryDays, 
      respectPrivacy: false 
    })
    
    const removedCount = getSearchHistory({ 
      expiryDays: 365 * 10, // 获取所有记录
      respectPrivacy: false 
    }).length - history.length

    if (removedCount > 0) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(history))
    }

    return removedCount
  } catch (error) {
    console.error('Failed to clean expired history:', error)
    return 0
  }
}

/**
 * 获取最近搜索建议
 */
export function getRecentSearches(limit: number = 5): string[] {
  return getSearchHistory()
    .slice(0, limit)
    .map(item => item.query)
}

/**
 * 获取历史记录统计信息
 */
export function getHistoryStats(): {
  total: number
  oldest: number | null
  newest: number | null
  privacyMode: boolean
} {
  const history = getSearchHistory({ respectPrivacy: false })
  
  return {
    total: history.length,
    oldest: history.length > 0 ? Math.min(...history.map(i => i.timestamp)) : null,
    newest: history.length > 0 ? Math.max(...history.map(i => i.timestamp)) : null,
    privacyMode: isPrivacyMode()
  }
}

/**
 * 导出历史记录（用于备份）
 */
export function exportHistory(): string {
  const history = getSearchHistory({ respectPrivacy: false })
  return JSON.stringify(history, null, 2)
}

/**
 * 导入历史记录
 */
export function importHistory(json: string): boolean {
  try {
    const imported: unknown = JSON.parse(json)
    if (!Array.isArray(imported)) return false

    const validItems = imported.filter((item): item is SearchHistoryItem => {
      if (!item || typeof item !== 'object') return false
      return typeof item.query === 'string' && typeof item.timestamp === 'number'
    })

    if (validItems.length === 0) return false

    // 合并现有历史
    const existing = getSearchHistory({ respectPrivacy: false })
    const merged = [...validItems, ...existing]
    
    // 去重（保留最新的）
    const seen = new Set<string>()
    const unique = merged.filter(item => {
      const key = item.query.toLowerCase()
      if (seen.has(key)) return false
      seen.add(key)
      return true
    })

    // 排序并限制数量
    const sorted = unique
      .sort((a, b) => b.timestamp - a.timestamp)
      .slice(0, MAX_HISTORY)

    localStorage.setItem(STORAGE_KEY, JSON.stringify(sorted))
    return true
  } catch (error) {
    console.error('Failed to import history:', error)
    return false
  }
}
