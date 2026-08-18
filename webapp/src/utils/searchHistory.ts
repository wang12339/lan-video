/**
 * 搜索历史管理
 * - 最多保存20条记录
 * - 按时间倒序排列
 * - 支持删除单条和清空
 */

const STORAGE_KEY = 'atmos_search_history'
const MAX_HISTORY = 20

export interface SearchHistoryItem {
  query: string
  timestamp: number
}

export function getSearchHistory(): SearchHistoryItem[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    return JSON.parse(raw) as SearchHistoryItem[]
  } catch {
    return []
  }
}

export function addToSearchHistory(query: string): void {
  const trimmed = query.trim()
  if (!trimmed || trimmed.length < 2) return

  const history = getSearchHistory().filter(item => item.query !== trimmed)
  history.unshift({ query: trimmed, timestamp: Date.now() })

  if (history.length > MAX_HISTORY) {
    history.pop()
  }

  localStorage.setItem(STORAGE_KEY, JSON.stringify(history))
}

export function removeFromSearchHistory(query: string): void {
  const history = getSearchHistory().filter(item => item.query !== query)
  localStorage.setItem(STORAGE_KEY, JSON.stringify(history))
}

export function clearSearchHistory(): void {
  localStorage.removeItem(STORAGE_KEY)
}

export function getRecentSearches(limit: number = 5): string[] {
  return getSearchHistory()
    .slice(0, limit)
    .map(item => item.query)
}
