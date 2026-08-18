import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import {
  getSearchHistory,
  removeFromSearchHistory,
  clearSearchHistory,
  type SearchHistoryItem,
} from '../../utils/searchHistory'
import './SearchHistory.css'

interface SearchHistoryProps {
  onSelect: (query: string) => void
  visible: boolean
}

export default function SearchHistory({ onSelect, visible }: SearchHistoryProps) {
  const { t } = useTranslation()
  const [history, setHistory] = useState<SearchHistoryItem[]>([])

  useEffect(() => {
    if (visible) {
      setHistory(getSearchHistory())
    }
  }, [visible])

  if (!visible || history.length === 0) {
    return null
  }

  const handleDelete = (e: React.MouseEvent, query: string) => {
    e.stopPropagation()
    removeFromSearchHistory(query)
    setHistory(getSearchHistory())
  }

  const handleClear = () => {
    clearSearchHistory()
    setHistory([])
  }

  return (
    <div className="search-history" role="listbox" aria-label={t('search.history')}>
      <div className="search-history-header">
        <span className="search-history-title">{t('search.recentSearches')}</span>
        <button
          className="search-history-clear"
          onClick={handleClear}
          aria-label={t('search.clearHistory')}
        >
          {t('search.clear')}
        </button>
      </div>
      <div className="search-history-list">
        {history.map((item) => (
          <div
            key={item.query}
            className="search-history-item"
            role="option"
            tabIndex={0}
            onClick={() => onSelect(item.query)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                onSelect(item.query)
              }
            }}
          >
            <svg
              className="search-history-icon"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <circle cx="12" cy="12" r="10" />
              <polyline points="12,6 12,12 16,14" />
            </svg>
            <span className="search-history-text">{item.query}</span>
            <button
              className="search-history-delete"
              onClick={(e) => handleDelete(e, item.query)}
              aria-label={`${t('search.delete')} ${item.query}`}
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}
