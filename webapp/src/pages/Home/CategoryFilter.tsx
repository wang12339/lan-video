import React, { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { CATEGORIES } from '../../config/categories'
import { trackClick } from '../../utils/track'

interface CategoryFilterProps {
  category: string
  total: number
  viewMode: 'grid' | 'list'
  onCategoryChange: (cat: string) => void
  onViewModeChange: (mode: 'grid' | 'list') => void
  searchParams: URLSearchParams
  setSearchParams: (fn: (prev: URLSearchParams) => URLSearchParams) => void
}

function CategoryFilter({
  category,
  total,
  viewMode,
  onCategoryChange,
  onViewModeChange,
  setSearchParams,
}: CategoryFilterProps) {
  const { t } = useTranslation()

  const handleCategoryClick = useCallback((cat: string) => {
    if (cat === category) return
    onCategoryChange(cat)
    requestAnimationFrame(() => {
      const active = document.querySelector('.cat-tag.active')
      active?.scrollIntoView?.({ behavior: 'smooth', block: 'nearest', inline: 'center' })
    })
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      if (cat === 'all') next.delete('cat')
      else next.set('cat', cat)
      return next
    })
    trackClick('切换分类', cat)
  }, [category, onCategoryChange, setSearchParams])

  const switchToGrid = useCallback(() => {
    onViewModeChange('grid')
    localStorage.setItem('home-view-mode', 'grid')
  }, [onViewModeChange])

  const switchToList = useCallback(() => {
    onViewModeChange('list')
    localStorage.setItem('home-view-mode', 'list')
  }, [onViewModeChange])

  return (
    <>
      <div className="category-bar">
        {CATEGORIES.map((cat) => (
          <button
            key={cat.key}
            className={`cat-tag ${category === cat.key ? 'active' : ''}`}
            onClick={() => handleCategoryClick(cat.key)}
            aria-label={t(cat.i18nKey)}
          >
            <span className="cat-icon" aria-hidden="true">{cat.icon}</span>
            {t(cat.i18nKey)}
            {cat.key === 'all' && total > 0 && (
              <span className="cat-count">{total}</span>
            )}
          </button>
        ))}
      </div>

      {total > 0 && <div className="home-count">{t('home.totalCount', { count: total })}</div>}

      <div className="view-toggle">
        <button
          className={`view-btn ${viewMode === 'grid' ? 'active' : ''}`}
          onClick={switchToGrid}
          aria-label={t('home.gridView')}
        >
          <svg viewBox="0 0 16 16" fill="currentColor" width="16" height="16">
            <rect x="1" y="1" width="6" height="6" rx="1"/><rect x="9" y="1" width="6" height="6" rx="1"/>
            <rect x="1" y="9" width="6" height="6" rx="1"/><rect x="9" y="9" width="6" height="6" rx="1"/>
          </svg>
        </button>
        <button
          className={`view-btn ${viewMode === 'list' ? 'active' : ''}`}
          onClick={switchToList}
          aria-label={t('home.listView')}
        >
          <svg viewBox="0 0 16 16" fill="currentColor" width="16" height="16">
            <rect x="1" y="1" width="14" height="3" rx="1"/><rect x="1" y="6" width="14" height="3" rx="1"/>
            <rect x="1" y="11" width="14" height="3" rx="1"/>
          </svg>
        </button>
      </div>
    </>
  )
}

export default React.memo(CategoryFilter)
