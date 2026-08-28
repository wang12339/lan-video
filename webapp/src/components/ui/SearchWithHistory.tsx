import { useState, useRef, useEffect, useCallback, memo } from 'react'
import { useTranslation } from 'react-i18next'
import { addToSearchHistory, getRecentSearches } from '../../utils/searchHistory'
import SearchHistory from './SearchHistory'

interface SearchWithHistoryProps {
  placeholder?: string
  onSearch: (query: string) => void
  className?: string
  autoFocus?: boolean
}

function SearchWithHistoryImpl({
  placeholder,
  onSearch,
  className = '',
  autoFocus = false
}: SearchWithHistoryProps) {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [showHistory, setShowHistory] = useState(false)
  const [recentSearches, setRecentSearches] = useState<string[]>([])
  const inputRef = useRef<HTMLInputElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  // 加载最近搜索
  useEffect(() => {
    const recent = getRecentSearches(3)
    setRecentSearches(recent)
  }, [])

  // 点击外部关闭历史记录
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setShowHistory(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  // 处理搜索提交
  const handleSearch = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    const trimmed = query.trim()
    if (trimmed.length >= 2) {
      addToSearchHistory(trimmed)
      onSearch(trimmed)
      setShowHistory(false)
      setRecentSearches(getRecentSearches(3))
    }
  }, [query, onSearch])

  // 处理历史记录选择
  const handleHistorySelect = useCallback((selectedQuery: string) => {
    setQuery(selectedQuery)
    addToSearchHistory(selectedQuery)
    onSearch(selectedQuery)
    setShowHistory(false)
    setRecentSearches(getRecentSearches(3))
  }, [onSearch])

  // 处理输入框焦点
  const handleFocus = useCallback(() => {
    setShowHistory(true)
  }, [])

  // 处理输入变化
  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setQuery(e.target.value)
  }, [])

  // 处理键盘事件
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      setShowHistory(false)
      inputRef.current?.blur()
    }
  }, [])

  return (
    <div ref={containerRef} className={`search-with-history ${className}`}>
      <form onSubmit={handleSearch} className="search-with-history-form">
        <div className="search-with-history-input-wrapper">
          <span className="search-with-history-icon" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
            </svg>
          </span>
          
          <input
            ref={inputRef}
            type="text"
            value={query}
            placeholder={placeholder || t('nav.search')}
            onChange={handleInputChange}
            onFocus={handleFocus}
            onKeyDown={handleKeyDown}
            autoFocus={autoFocus}
            className="search-with-history-input"
            aria-label={t('common.search')}
            autoComplete="off"
          />

          {query && (
            <button
              type="button"
              className="search-with-history-clear"
              onClick={() => {
                setQuery('')
                inputRef.current?.focus()
              }}
              aria-label={t('search.clear')}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          )}
        </div>

        <button type="submit" className="search-with-history-submit">
          {t('common.search')}
        </button>
      </form>

      {/* 最近搜索快捷标签 */}
      {recentSearches.length > 0 && !showHistory && (
        <div className="search-with-history-recent">
          <span className="search-with-history-recent-label">
            {t('search.recentSearches')}:
          </span>
          {recentSearches.map((recent, index) => (
            <button
              key={index}
              className="search-with-history-recent-tag"
              onClick={() => {
                setQuery(recent)
                addToSearchHistory(recent)
                onSearch(recent)
                setRecentSearches(getRecentSearches(3))
              }}
            >
              {recent}
            </button>
          ))}
        </div>
      )}

      {/* 搜索历史下拉 */}
      <SearchHistory
        visible={showHistory}
        onSelect={handleHistorySelect}
        showPrivacyToggle={true}
        showStats={false}
      />
    </div>
  )
}

export default memo(SearchWithHistoryImpl)