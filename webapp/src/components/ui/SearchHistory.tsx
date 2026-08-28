import { useState, useEffect, useCallback, useMemo, memo } from 'react'
import { useTranslation } from 'react-i18next'
import {
  getSearchHistory,
  removeFromSearchHistory,
  clearSearchHistory,
  isPrivacyMode,
  setPrivacyMode,
  cleanExpiredHistory,
  getHistoryStats,
  type SearchHistoryItem,
} from '../../utils/searchHistory'
import './SearchHistory.css'

interface SearchHistoryProps {
  onSelect: (query: string) => void
  visible: boolean
  showPrivacyToggle?: boolean
  showStats?: boolean
}

function SearchHistoryImpl({
  onSelect,
  visible,
  showPrivacyToggle = true,
  showStats = false
}: SearchHistoryProps) {
  const { t } = useTranslation()
  const [history, setHistory] = useState<SearchHistoryItem[]>([])
  const [privacyMode, setPrivacyModeState] = useState(isPrivacyMode())
  const [showConfirmClear, setShowConfirmClear] = useState(false)
  const [stats, setStats] = useState(getHistoryStats())

  // 加载历史记录
  const loadHistory = useCallback(() => {
    if (visible) {
      const newHistory = getSearchHistory()
      setHistory(newHistory)
      setStats(getHistoryStats())
    }
  }, [visible])

  useEffect(() => {
    loadHistory()
  }, [loadHistory])

  // 清理过期记录
  useEffect(() => {
    const cleaned = cleanExpiredHistory()
    if (cleaned > 0) {
      loadHistory()
    }
  }, [loadHistory])

  // 切换隐私模式
  const handlePrivacyToggle = useCallback(() => {
    const newPrivacyMode = !privacyMode
    setPrivacyMode(newPrivacyMode)
    setPrivacyModeState(newPrivacyMode)
    
    if (newPrivacyMode) {
      // 开启隐私模式时清除历史
      clearSearchHistory()
      setHistory([])
      setStats(getHistoryStats())
    } else {
      // 关闭隐私模式时重新加载
      loadHistory()
    }
  }, [privacyMode, loadHistory])

  // 删除单条记录
  const handleDelete = useCallback((e: React.MouseEvent, query: string) => {
    e.stopPropagation()
    if (removeFromSearchHistory(query)) {
      setHistory(prev => prev.filter(item => item.query !== query))
      setStats(getHistoryStats())
    }
  }, [])

  // 清除所有记录
  const handleClear = useCallback(() => {
    if (clearSearchHistory()) {
      setHistory([])
      setStats(getHistoryStats())
      setShowConfirmClear(false)
    }
  }, [])

  // 取消清除确认
  const handleCancelClear = useCallback(() => {
    setShowConfirmClear(false)
  }, [])

  // 格式化时间戳
  const formatTimestamp = useMemo(() => {
    return (timestamp: number) => {
      const now = Date.now()
      const diff = now - timestamp
      
      if (diff < 60000) return t('search.justNow')
      if (diff < 3600000) return t('search.minutesAgo', { count: Math.floor(diff / 60000) })
      if (diff < 86400000) return t('search.hoursAgo', { count: Math.floor(diff / 3600000) })
      
      const date = new Date(timestamp)
      return date.toLocaleDateString()
    }
  }, [t])

  // 如果不可见或隐私模式下没有私有记录，不渲染
  if (!visible || (privacyMode && history.length === 0)) {
    return null
  }

  return (
    <div className="search-history" role="listbox" aria-label={t('search.history')}>
      {/* 头部控制区 */}
      <div className="search-history-header">
        <div className="search-history-header-left">
          <span className="search-history-title">{t('search.recentSearches')}</span>
          {showStats && stats.total > 0 && (
            <span className="search-history-count">
              ({stats.total})
            </span>
          )}
        </div>
        
        <div className="search-history-header-right">
          {/* 隐私模式切换 */}
          {showPrivacyToggle && (
            <button
              className={`search-history-privacy ${privacyMode ? 'active' : ''}`}
              onClick={handlePrivacyToggle}
              aria-label={privacyMode ? t('search.disablePrivacy') : t('search.enablePrivacy')}
              title={privacyMode ? t('search.privacyModeOn') : t('search.privacyModeOff')}
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                {privacyMode ? (
                  <>
                    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                    <path d="M12 8v4" />
                    <path d="M12 16h.01" />
                  </>
                ) : (
                  <>
                    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                    <path d="M12 8v4" />
                    <path d="M12 16h.01" />
                  </>
                )}
              </svg>
            </button>
          )}
          
          {/* 清除按钮 */}
          {history.length > 0 && (
            <button
              className="search-history-clear"
              onClick={() => setShowConfirmClear(true)}
              aria-label={t('search.clearHistory')}
            >
              {t('search.clear')}
            </button>
          )}
        </div>
      </div>

      {/* 隐私模式提示 */}
      {privacyMode && (
        <div className="search-history-privacy-notice">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
          </svg>
          <span>{t('search.privacyModeActive')}</span>
        </div>
      )}

      {/* 历史记录列表 */}
      <div className="search-history-list">
        {history.map((item) => (
          <div
            key={item.query}
            className={`search-history-item ${item.isPrivate ? 'private' : ''}`}
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
            <div className="search-history-item-icon">
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
            </div>
            
            <div className="search-history-item-content">
              <span className="search-history-text">{item.query}</span>
              <span className="search-history-time">
                {formatTimestamp(item.timestamp)}
              </span>
            </div>
            
            <button
              type="button"
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
                aria-hidden="true"
              >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        ))}
      </div>

      {/* 空状态提示 */}
      {history.length === 0 && !privacyMode && (
        <div className="search-history-empty">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="10" />
            <line x1="8" y1="12" x2="16" y2="12" />
          </svg>
          <span>{t('search.noHistory')}</span>
        </div>
      )}

      {/* 清除确认对话框 */}
      {showConfirmClear && (
        <div className="search-history-confirm-overlay">
          <div className="search-history-confirm-dialog">
            <div className="search-history-confirm-header">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
              </svg>
              <span>{t('search.confirmClearTitle')}</span>
            </div>
            
            <div className="search-history-confirm-content">
              <p>{t('search.confirmClearMessage')}</p>
              {stats.total > 0 && (
                <p className="search-history-confirm-count">
                  {t('search.recordsWillBeDeleted', { count: stats.total })}
                </p>
              )}
            </div>
            
            <div className="search-history-confirm-actions">
              <button
                type="button"
                className="search-history-confirm-cancel"
                onClick={handleCancelClear}
              >
                {t('common.cancel')}
              </button>
              <button
                type="button"
                className="search-history-confirm-delete"
                onClick={handleClear}
              >
                {t('search.clearAll')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default memo(SearchHistoryImpl)
