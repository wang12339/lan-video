const STORAGE_KEY = 'atmos_search_history'
const PRIVACY_KEY = 'atmos_privacy_mode'
const MAX_HISTORY = 30
const MAX_QUERY_LENGTH = 100
const DEFAULT_EXPIRY_DAYS = 30
const FUTURE_TOLERANCE_MS = 86_400_000

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

let cachedPrivacyMode: boolean | null = null

export function isPrivacyMode(): boolean {
  if (cachedPrivacyMode !== null) return cachedPrivacyMode
  try {
    cachedPrivacyMode = localStorage.getItem(PRIVACY_KEY) === 'true'
  } catch {
    cachedPrivacyMode = false
  }
  return cachedPrivacyMode
}

export function setPrivacyMode(enabled: boolean): void {
  cachedPrivacyMode = enabled
  try {
    localStorage.setItem(PRIVACY_KEY, String(enabled))
    if (enabled) {
      clearSearchHistory()
    }
  } catch (error) {
    console.error('Failed to set privacy mode:', error)
  }
}

function isValidItem(item: unknown, now: number, expiryMs: number): item is SearchHistoryItem {
  if (!item || typeof item !== 'object') return false
  const obj = item as Record<string, unknown>
  if (typeof obj.query !== 'string' || typeof obj.timestamp !== 'number') return false
  const trimmed = (obj.query as string).trim()
  if (trimmed.length < 2 || trimmed.length > MAX_QUERY_LENGTH) return false
  const ts = obj.timestamp as number
  if (ts <= 0 || ts > now + FUTURE_TOLERANCE_MS) return false
  if (now - ts > expiryMs) return false
  return true
}

export function getSearchHistory(options: SearchHistoryOptions = {}): SearchHistoryItem[] {
  const { expiryDays = DEFAULT_EXPIRY_DAYS, respectPrivacy = true } = options

  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []

    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []

    const now = Date.now()
    const expiryMs = expiryDays * 86_400_000
    const privacyOn = respectPrivacy && isPrivacyMode()

    const seen = new Set<string>()
    const result: SearchHistoryItem[] = []
    const limit = options.maxItems ?? MAX_HISTORY

    for (const item of parsed) {
      if (result.length >= limit) break
      if (!isValidItem(item, now, expiryMs)) continue
      if (privacyOn && !item.isPrivate) continue
      const key = item.query.toLowerCase()
      if (seen.has(key)) continue
      seen.add(key)
      result.push({ query: item.query.trim(), timestamp: item.timestamp, isPrivate: item.isPrivate || false })
    }

    return result
  } catch (error) {
    console.error('Failed to read search history:', error)
    return []
  }
}

export function addToSearchHistory(query: string): boolean {
  if (isPrivacyMode()) return false

  const trimmed = query.trim()
  if (!trimmed || trimmed.length < 2 || trimmed.length > MAX_QUERY_LENGTH) return false

  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    let history: SearchHistoryItem[] = []
    if (raw) {
      const parsed: unknown = JSON.parse(raw)
      if (Array.isArray(parsed)) {
        const now = Date.now()
        const expiryMs = DEFAULT_EXPIRY_DAYS * 86_400_000
        history = parsed.filter((item): item is SearchHistoryItem => isValidItem(item, now, expiryMs))
      }
    }

    const lowerQuery = trimmed.toLowerCase()
    const now = Date.now()
    const existingIndex = history.findIndex(item => item.query.toLowerCase() === lowerQuery)

    if (existingIndex !== -1 && history[existingIndex]) {
      history[existingIndex].timestamp = now
    } else {
      history.unshift({ query: trimmed, timestamp: now, isPrivate: false })
    }

    if (history.length > MAX_HISTORY) history.length = MAX_HISTORY

    localStorage.setItem(STORAGE_KEY, JSON.stringify(history))
    return true
  } catch (error) {
    console.error('Failed to add to search history:', error)
    return false
  }
}

export function removeFromSearchHistory(query: string): boolean {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return false

    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return false

    const lowerQuery = query.toLowerCase()
    const now = Date.now()
    const expiryMs = DEFAULT_EXPIRY_DAYS * 86_400_000
    const valid = parsed.filter((item): item is SearchHistoryItem => isValidItem(item, now, expiryMs))
    const filtered = valid.filter(item => item.query.toLowerCase() !== lowerQuery)

    if (filtered.length === valid.length) return false

    localStorage.setItem(STORAGE_KEY, JSON.stringify(filtered))
    return true
  } catch (error) {
    console.error('Failed to remove from search history:', error)
    return false
  }
}

export function clearSearchHistory(): boolean {
  try {
    localStorage.removeItem(STORAGE_KEY)
    return true
  } catch (error) {
    console.error('Failed to clear search history:', error)
    return false
  }
}

export function cleanExpiredHistory(expiryDays: number = DEFAULT_EXPIRY_DAYS): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return 0

    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return 0

    const now = Date.now()
    const expiryMs = expiryDays * 86_400_000
    const valid: SearchHistoryItem[] = []
    let removed = 0

    for (const item of parsed) {
      if (isValidItem(item, now, expiryMs)) {
        valid.push({ query: item.query.trim(), timestamp: item.timestamp, isPrivate: item.isPrivate || false })
      } else {
        removed++
      }
    }

    if (removed > 0) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(valid))
    }

    return removed
  } catch (error) {
    console.error('Failed to clean expired history:', error)
    return 0
  }
}

export function getRecentSearches(limit: number = 5): string[] {
  return getSearchHistory()
    .slice(0, limit)
    .map(item => item.query)
}

export function getHistoryStats(): {
  total: number
  oldest: number | null
  newest: number | null
  privacyMode: boolean
} {
  const history = getSearchHistory({ respectPrivacy: false })
  const len = history.length

  if (len === 0) {
    return { total: 0, oldest: null, newest: null, privacyMode: isPrivacyMode() }
  }

  let oldest = Infinity
  let newest = -Infinity
  for (let i = 0; i < len; i++) {
    const ts = history[i]!.timestamp
    if (ts < oldest) oldest = ts
    if (ts > newest) newest = ts
  }

  return { total: len, oldest, newest, privacyMode: isPrivacyMode() }
}

export function exportHistory(): string {
  const history = getSearchHistory({ respectPrivacy: false })
  return JSON.stringify(history, null, 2)
}

export function importHistory(json: string): boolean {
  try {
    const imported: unknown = JSON.parse(json)
    if (!Array.isArray(imported)) return false

    const existing = getSearchHistory({ respectPrivacy: false })
    const seen = new Set<string>()
    const result: SearchHistoryItem[] = []

    for (const item of existing) {
      const key = item.query.toLowerCase()
      seen.add(key)
      result.push(item)
    }

    for (const item of imported) {
      if (result.length >= MAX_HISTORY) break
      if (!isValidItem(item, Infinity, Infinity)) continue
      const key = item.query.toLowerCase()
      if (seen.has(key)) continue
      seen.add(key)
      result.push({ query: item.query.trim(), timestamp: item.timestamp, isPrivate: item.isPrivate || false })
    }

    result.sort((a, b) => b.timestamp - a.timestamp)
    if (result.length > MAX_HISTORY) result.length = MAX_HISTORY

    localStorage.setItem(STORAGE_KEY, JSON.stringify(result))
    return result.length > 0
  } catch (error) {
    console.error('Failed to import history:', error)
    return false
  }
}
