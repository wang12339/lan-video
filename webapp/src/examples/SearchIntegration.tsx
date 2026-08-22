/**
 * SearchHistory 集成示例
 * 
 * 本示例展示如何在现有项目中集成优化后的搜索历史功能
 */

import { useState, useCallback } from 'react'
import SearchWithHistory from '../components/ui/SearchWithHistory'
import { addToSearchHistory, getRecentSearches } from '../utils/searchHistory'

// 示例 1: 在搜索页面使用
export function SearchPage() {
  const [searchResults, setSearchResults] = useState<string[]>([])
  const [isSearching, setIsSearching] = useState(false)

  const handleSearch = useCallback(async (query: string) => {
    setIsSearching(true)
    try {
      // 模拟搜索 API 调用
      await new Promise(resolve => setTimeout(resolve, 500))
      setSearchResults([`结果1: ${query}`, `结果2: ${query}`, `结果3: ${query}`])
    } finally {
      setIsSearching(false)
    }
  }, [])

  return (
    <div className="search-page">
      <h1>搜索</h1>
      
      <SearchWithHistory
        placeholder="搜索视频、图片..."
        onSearch={handleSearch}
        autoFocus={true}
      />

      {isSearching && <div className="loading">搜索中...</div>}

      {searchResults.length > 0 && (
        <div className="search-results">
          <h2>搜索结果</h2>
          <ul>
            {searchResults.map((result, index) => (
              <li key={index}>{result}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}

// 示例 2: 在导航栏中集成
export function NavBarSearch() {
  const [showSearch, setShowSearch] = useState(false)

  const handleSearch = useCallback((query: string) => {
    console.log('导航栏搜索:', query)
    setShowSearch(false)
    // 跳转到搜索页面
  }, [])

  return (
    <div className="nav-search-container">
      <button 
        onClick={() => setShowSearch(!showSearch)}
        className="search-toggle"
      >
        🔍
      </button>

      {showSearch && (
        <div className="nav-search-dropdown">
          <SearchWithHistory
            placeholder="快速搜索..."
            onSearch={handleSearch}
            autoFocus={true}
          />
        </div>
      )}
    </div>
  )
}

// 示例 3: 高级用法 - 自定义隐私设置
export function AdvancedSearchComponent() {
  const [privacyEnabled, setPrivacyEnabled] = useState(false)

  const handleSearch = useCallback((query: string) => {
    addToSearchHistory(query)
    console.log('搜索:', query)
  }, [])

  return (
    <div className="advanced-search">
      <div className="privacy-controls">
        <label>
          <input
            type="checkbox"
            checked={privacyEnabled}
            onChange={(e) => setPrivacyEnabled(e.target.checked)}
          />
          隐私模式
        </label>
        <p className="privacy-description">
          {privacyEnabled 
            ? '隐私模式已开启，搜索记录不会被保存'
            : '搜索记录会被保存到本地'}
        </p>
      </div>

      <SearchWithHistory
        placeholder="输入搜索内容..."
        onSearch={handleSearch}
      />

      <div className="search-tips">
        <h3>搜索技巧</h3>
        <ul>
          <li>使用引号进行精确搜索: "完整短语"</li>
          <li>使用减号排除词汇: 关键词 -排除词</li>
          <li>按时间筛选: 添加时间范围</li>
        </ul>
      </div>
    </div>
  )
}

// 示例 4: 批量操作组件
export function HistoryManagement() {
  const [recentSearches, setRecentSearches] = useState<string[]>([])

  const loadRecentSearches = useCallback(() => {
    const recent = getRecentSearches(10)
    setRecentSearches(recent)
  }, [])

  const handleBulkDelete = useCallback((queries: string[]) => {
    queries.forEach(query => {
      removeFromSearchHistory(query)
    })
    loadRecentSearches()
  }, [loadRecentSearches])

  return (
    <div className="history-management">
      <h2>历史记录管理</h2>
      
      <button onClick={loadRecentSearches}>
        加载最近搜索
      </button>

      {recentSearches.length > 0 && (
        <div className="recent-searches">
          <h3>最近搜索 ({recentSearches.length})</h3>
          <ul>
            {recentSearches.map((search, index) => (
              <li key={index} className="search-item">
                <span>{search}</span>
                <button 
                  onClick={() => handleBulkDelete([search])}
                  className="delete-btn"
                >
                  删除
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}

// 辅助函数
function removeFromSearchHistory(query: string): void {
  const history = JSON.parse(localStorage.getItem('atmos_search_history') || '[]')
  const filtered = history.filter((item: any) => item.query !== query)
  localStorage.setItem('atmos_search_history', JSON.stringify(filtered))
}

// 示例 5: 响应式搜索布局
export function ResponsiveSearchLayout() {
  const [isMobile, setIsMobile] = useState(window.innerWidth < 768)

  // 监听窗口大小变化
  useState(() => {
    const handleResize = () => {
      setIsMobile(window.innerWidth < 768)
    }
    window.addEventListener('resize', handleResize)
    return () => window.removeEventListener('resize', handleResize)
  })

  const handleSearch = useCallback((query: string) => {
    console.log('搜索:', query)
  }, [])

  return (
    <div className={`responsive-search ${isMobile ? 'mobile' : 'desktop'}`}>
      {isMobile ? (
        // 移动端布局 - 全屏搜索
        <div className="mobile-search">
          <SearchWithHistory
            placeholder="搜索..."
            onSearch={handleSearch}
            className="full-width"
          />
        </div>
      ) : (
        // 桌面端布局 - 侧边栏搜索
        <div className="desktop-search">
          <SearchWithHistory
            placeholder="搜索视频..."
            onSearch={handleSearch}
          />
        </div>
      )}
    </div>
  )
}

// 样式示例（可以放在 CSS 文件中）
export const styles = `
.search-page {
  max-width: 1200px;
  margin: 0 auto;
  padding: 20px;
}

.nav-search-container {
  position: relative;
}

.nav-search-dropdown {
  position: absolute;
  top: 100%;
  right: 0;
  width: 300px;
  background: white;
  border: 1px solid #ddd;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.1);
  z-index: 1000;
  padding: 16px;
}

.privacy-controls {
  margin-bottom: 20px;
  padding: 16px;
  background: #f5f5f5;
  border-radius: 8px;
}

.privacy-description {
  margin-top: 8px;
  font-size: 0.875rem;
  color: #666;
}

.advanced-search {
  max-width: 600px;
  margin: 0 auto;
}

.search-tips {
  margin-top: 24px;
  padding: 16px;
  background: #f9f9f9;
  border-radius: 8px;
}

.search-tips h3 {
  margin-top: 0;
}

.history-management {
  max-width: 800px;
  margin: 0 auto;
}

.recent-searches {
  margin-top: 16px;
}

.search-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #f5f5f5;
  border-radius: 6px;
  margin-bottom: 8px;
}

.delete-btn {
  background: #ff4444;
  color: white;
  border: none;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
}

.responsive-search.mobile {
  padding: 12px;
}

.responsive-search.desktop {
  padding: 24px;
}

.full-width {
  width: 100%;
}
`

export default SearchPage