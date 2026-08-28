import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  getSearchHistory,
  addToSearchHistory,
  removeFromSearchHistory,
  clearSearchHistory,
  isPrivacyMode,
  setPrivacyMode,
  cleanExpiredHistory,
  getHistoryStats,
  exportHistory,
  importHistory,
  getRecentSearches,
} from '../utils/searchHistory'

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {}
  return {
    getItem: vi.fn((key: string) => store[key] || null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key]
    }),
    clear: vi.fn(() => {
      store = {}
    }),
  }
})()

Object.defineProperty(window, 'localStorage', {
  value: localStorageMock,
})

describe('SearchHistory', () => {
  beforeEach(() => {
    localStorageMock.clear()
    vi.clearAllMocks()
  })

  describe('Basic Operations', () => {
    it('should return empty array when no history', () => {
      expect(getSearchHistory()).toEqual([])
    })

    it('should add search to history', () => {
      addToSearchHistory('test query')
      const history = getSearchHistory()
      expect(history).toHaveLength(1)
      expect(history[0]?.query).toBe('test query')
    })

    it('should not add short queries', () => {
      addToSearchHistory('a')
      const history = getSearchHistory()
      expect(history).toHaveLength(0)
    })

    it('should remove duplicate queries', () => {
      addToSearchHistory('test')
      addToSearchHistory('test')
      const history = getSearchHistory()
      expect(history).toHaveLength(1)
    })

    it('should remove from history', () => {
      addToSearchHistory('test')
      removeFromSearchHistory('test')
      const history = getSearchHistory()
      expect(history).toHaveLength(0)
    })

    it('should clear all history', () => {
      addToSearchHistory('test1')
      addToSearchHistory('test2')
      clearSearchHistory()
      const history = getSearchHistory()
      expect(history).toHaveLength(0)
    })
  })
})

describe('Privacy Mode', () => {
  beforeEach(() => {
    localStorageMock.clear()
  })

  it('should check privacy mode', () => {
    expect(isPrivacyMode()).toBe(false)
  })

  it('should set privacy mode', () => {
    setPrivacyMode(true)
    expect(isPrivacyMode()).toBe(true)
  })

  it('should clear history when enabling privacy mode', () => {
    addToSearchHistory('test')
    setPrivacyMode(true)
    const history = getSearchHistory()
    expect(history).toHaveLength(0)
  })
})

describe('History Management', () => {
  beforeEach(() => {
    localStorageMock.clear()
    setPrivacyMode(false)
  })

  it('should clean expired history', () => {
    // 添加一个过期的记录
    const expiredItem = {
      query: 'expired',
      timestamp: Date.now() - 31 * 24 * 60 * 60 * 1000, // 31 天前
      isPrivate: false,
    }
    localStorageMock.setItem('atmos_search_history', JSON.stringify([expiredItem]))

    const removedCount = cleanExpiredHistory(30)
    expect(removedCount).toBe(1)
  })

  it('should get history stats', () => {
    addToSearchHistory('test')
    const stats = getHistoryStats()
    expect(stats.total).toBe(1)
    expect(stats.privacyMode).toBe(false)
  })

  it('should export history', () => {
    addToSearchHistory('test')
    const exported = exportHistory()
    const parsed = JSON.parse(exported)
    expect(parsed).toHaveLength(1)
    expect(parsed[0].query).toBe('test')
  })

  it('should import history', () => {
    const history = [
      { query: 'imported', timestamp: Date.now(), isPrivate: false },
    ]
    const result = importHistory(JSON.stringify(history))
    expect(result).toBe(true)

    const currentHistory = getSearchHistory()
    expect(currentHistory).toHaveLength(1)
    expect(currentHistory[0]?.query).toBe('imported')
  })
})

describe('Data Validation', () => {
  beforeEach(() => {
    localStorageMock.clear()
    setPrivacyMode(false)
  })

  it('should handle invalid JSON', () => {
    localStorageMock.setItem('atmos_search_history', 'invalid')
    const history = getSearchHistory()
    expect(history).toHaveLength(0)
  })

  it('should handle empty array', () => {
    localStorageMock.setItem('atmos_search_history', '[]')
    const history = getSearchHistory()
    expect(history).toHaveLength(0)
  })

  it('should filter invalid items', () => {
    const mixedData = [
      { query: 'valid', timestamp: Date.now(), isPrivate: false },
      { invalid: 'data' },
      null,
      { query: '', timestamp: Date.now(), isPrivate: false },
      { query: 'test', timestamp: -1, isPrivate: false },
    ]
    localStorageMock.setItem('atmos_search_history', JSON.stringify(mixedData))
    const history = getSearchHistory()
    expect(history).toHaveLength(1)
    expect(history[0]?.query).toBe('valid')
  })
})

describe('Capacity Limits', () => {
  beforeEach(() => {
    localStorageMock.clear()
    setPrivacyMode(false)
  })

  it('should limit to 30 items', () => {
    for (let i = 0; i < 35; i++) {
      addToSearchHistory(`query${i}`)
    }
    const history = getSearchHistory()
    expect(history).toHaveLength(30)
  })

  it('should keep most recent items', () => {
    for (let i = 0; i < 35; i++) {
      addToSearchHistory(`query${i}`)
    }
    const history = getSearchHistory()
    expect(history[0]?.query).toBe('query34')
  })
})

describe('Edge Cases', () => {
  beforeEach(() => {
    localStorageMock.clear()
    setPrivacyMode(false)
  })

  it('should not add queries longer than 100 characters', () => {
    const longQuery = 'a'.repeat(101)
    const result = addToSearchHistory(longQuery)
    expect(result).toBe(false)
    expect(getSearchHistory()).toHaveLength(0)
  })

  it('should add queries at exactly 100 characters', () => {
    const query = 'a'.repeat(100)
    addToSearchHistory(query)
    expect(getSearchHistory()).toHaveLength(1)
  })

  it('should handle case-insensitive deduplication', () => {
    addToSearchHistory('Test')
    addToSearchHistory('test')
    const history = getSearchHistory()
    expect(history).toHaveLength(1)
  })

  it('should update timestamp when duplicate is added', () => {
    addToSearchHistory('test')
    const history1 = getSearchHistory()
    const ts1 = history1[0]?.timestamp
    addToSearchHistory('test')
    const history2 = getSearchHistory()
    expect(history2[0]?.timestamp).toBeGreaterThanOrEqual(ts1!)
  })

  it('addToSearchHistory returns false in privacy mode', () => {
    setPrivacyMode(true)
    expect(addToSearchHistory('test')).toBe(false)
  })

  it('should handle corrupt localStorage data', () => {
    localStorageMock.setItem('atmos_search_history', '{corrupt')
    expect(getSearchHistory()).toEqual([])
  })

  it('should handle non-array localStorage data', () => {
    localStorageMock.setItem('atmos_search_history', '{"key": "value"}')
    expect(getSearchHistory()).toEqual([])
  })

  it('should filter items with future timestamps beyond tolerance', () => {
    const futureItem = {
      query: 'future',
      timestamp: Date.now() + 2 * 24 * 60 * 60 * 1000,
      isPrivate: false,
    }
    localStorageMock.setItem('atmos_search_history', JSON.stringify([futureItem]))
    expect(getSearchHistory()).toHaveLength(0)
  })

  it('getRecentSearches returns correct limit', () => {
    for (let i = 0; i < 10; i++) {
      addToSearchHistory(`q${i}`)
    }
    const recent = getRecentSearches(3)
    expect(recent).toHaveLength(3)
    expect(recent[0]).toBe('q9')
  })

  it('importHistory returns false for invalid JSON', () => {
    expect(importHistory('not json')).toBe(false)
  })

  it('importHistory returns false for non-array', () => {
    expect(importHistory('{"key": "value"}')).toBe(false)
  })

  it('importHistory deduplicates with existing entries', () => {
    addToSearchHistory('existing')
    const imported = [{ query: 'existing', timestamp: Date.now(), isPrivate: false }]
    importHistory(JSON.stringify(imported))
    const history = getSearchHistory()
    expect(history).toHaveLength(1)
  })

  it('importHistory sorts by timestamp descending', () => {
    const imported = [
      { query: 'older', timestamp: Date.now() - 100000, isPrivate: false },
      { query: 'newer', timestamp: Date.now(), isPrivate: false },
    ]
    importHistory(JSON.stringify(imported))
    const history = getSearchHistory()
    expect(history[0]?.query).toBe('newer')
    expect(history[1]?.query).toBe('older')
  })

  it('cleanExpiredHistory returns 0 when nothing to clean', () => {
    addToSearchHistory('recent')
    expect(cleanExpiredHistory(30)).toBe(0)
  })

  it('getHistoryStats returns correct data with entries', () => {
    addToSearchHistory('test1')
    const stats = getHistoryStats()
    expect(stats.total).toBe(1)
    expect(stats.oldest).not.toBeNull()
    expect(stats.newest).not.toBeNull()
  })

  it('removeFromSearchHistory returns false when query not found', () => {
    addToSearchHistory('test')
    expect(removeFromSearchHistory('nonexistent')).toBe(false)
  })
})