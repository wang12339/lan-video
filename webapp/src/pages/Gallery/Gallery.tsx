import { useState, useEffect, useCallback, useRef, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import { useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { listVideos, mapImage } from '../../api'
import type { MappedImage } from '../../api/types'
import './Gallery.css'

const PAGE_SIZE = 40
const SEARCH_DEBOUNCE_MS = 300

export default function Gallery() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()

  // 筛选/排序状态以 URL 为准：刷新后保留，浏览器前进/后退可同步
  const query = searchParams.get('q')?.trim() ?? ''
  const layout: 'grid' | 'wide' = searchParams.get('view') === 'wide' ? 'wide' : 'grid'

  const [images, setImages] = useState<MappedImage[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(false)
  const [appendError, setAppendError] = useState(false)
  const [lbOpen, setLbOpen] = useState(false)
  const [lbIndex, setLbIndex] = useState(-1)
  const [searchInput, setSearchInput] = useState(query)

  const pageRef = useRef(0)
  const loadGenRef = useRef(0)
  const loadingRef = useRef(true)
  const hasMoreRef = useRef(true)
  const fillLenRef = useRef(0)
  const lightboxRef = useRef<HTMLDivElement>(null)
  const prevFocusRef = useRef<HTMLElement | null>(null)
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const loadImages = useCallback(async (pageNum: number, append: boolean) => {
    // 每次调用递增 generation：作废在途请求，旧响应到达时直接丢弃
    const gen = ++loadGenRef.current
    loadingRef.current = true
    setLoading(true)
    setError(false)
    setAppendError(false)
    if (!append) setTotal(0)
    try {
      const res = await listVideos({ type: 'local_image', query, page: pageNum, size: PAGE_SIZE })
      if (gen !== loadGenRef.current) return
      const mapped = res.items.map(mapImage).filter((v): v is MappedImage => !!v)
      if (append) {
        // 追加去重，防止重试/翻页异常产生重复卡片
        setImages((prev) => {
          const seen = new Set(prev.map((i) => i.id))
          return [...prev, ...mapped.filter((i) => !seen.has(i.id))]
        })
      } else {
        setImages(mapped)
      }
      setTotal(res.total)
      // 本页数量不足一页即视为最后一页，之后不再发起追加请求
      hasMoreRef.current = mapped.length >= PAGE_SIZE
      pageRef.current = pageNum + 1
    } catch {
      if (gen !== loadGenRef.current) return
      if (append) setAppendError(true)
      else setError(true)
    } finally {
      loadingRef.current = false
      if (gen === loadGenRef.current) setLoading(false)
    }
  }, [query])

  // 切换搜索条件：作废在途请求、重置页码、清空旧列表、回到顶部
  useEffect(() => {
    loadGenRef.current++
    pageRef.current = 0
    hasMoreRef.current = true
    fillLenRef.current = 0
    setImages([])
    loadImages(0, false)
    window.scrollTo({ top: 0 })
  }, [query, loadImages])

  // 滚动接近底部时追加下一页（loadingRef 防止重复触发）
  useEffect(() => {
    const onScroll = () => {
      if (loadingRef.current || !hasMoreRef.current) return
      const doc = document.documentElement
      const scrollTop = window.pageYOffset || doc.scrollTop || document.body.scrollTop || 0
      const scrollHeight = Math.max(doc.scrollHeight, document.body.scrollHeight)
      const clientHeight = window.innerHeight || doc.clientHeight
      if (scrollTop + clientHeight >= scrollHeight - 300) {
        loadImages(pageRef.current, true)
      }
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [loadImages])

  // 首屏内容不足一屏时自动补页，避免"无限追加"停摆
  useEffect(() => {
    if (loading || images.length === 0 || !hasMoreRef.current) return
    if (images.length === fillLenRef.current) return
    const scrollable = document.documentElement.scrollHeight > window.innerHeight
    if (!scrollable) {
      fillLenRef.current = images.length
      loadImages(pageRef.current, true)
    }
  }, [images, loading, loadImages])

  const openLightbox = useCallback((idx: number) => {
    setLbIndex(idx)
    setLbOpen(true)
    document.documentElement.classList.add('overflow-hidden')
  }, [])

  const closeLightbox = useCallback(() => {
    setLbOpen(false)
    setLbIndex(-1)
    document.documentElement.classList.remove('overflow-hidden')
  }, [])

  const lbPrev = useCallback(() => {
    setLbIndex((i) => Math.max(0, i - 1))
  }, [])

  const lbNext = useCallback(() => {
    setLbIndex((i) => Math.min(images.length - 1, i + 1))
  }, [images.length])

  // 灯箱：焦点进入、Esc/方向键、Tab 循环，关闭后焦点归还触发元素
  useEffect(() => {
    if (!lbOpen) return
    prevFocusRef.current = document.activeElement as HTMLElement | null
    lightboxRef.current?.focus()

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        closeLightbox()
      } else if (e.key === 'ArrowLeft') {
        lbPrev()
      } else if (e.key === 'ArrowRight') {
        lbNext()
      } else if (e.key === 'Tab' && lightboxRef.current) {
        const focusable = lightboxRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        )
        if (focusable.length === 0) return
        const first = focusable[0]!
        const last = focusable[focusable.length - 1]!
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault()
          last.focus()
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault()
          first.focus()
        }
      }
    }
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('keydown', onKey)
      prevFocusRef.current?.focus()
      prevFocusRef.current = null
    }
  }, [lbOpen, closeLightbox, lbPrev, lbNext])

  // 索引越界（如图片列表变化）时自动关闭灯箱
  useEffect(() => {
    if (lbOpen && (lbIndex < 0 || lbIndex >= images.length)) {
      closeLightbox()
    }
  }, [lbOpen, lbIndex, images.length, closeLightbox])

  // 卸载时清理滚动锁定
  useEffect(() => {
    return () => {
      document.documentElement.classList.remove('overflow-hidden')
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current)
    }
  }, [])

  // 输入框本地即时响应，防抖后写入 URL
  const onSearchInput = (val: string) => {
    setSearchInput(val)
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current)
    searchTimerRef.current = setTimeout(() => {
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev)
        const trimmed = val.trim()
        if (trimmed) next.set('q', trimmed)
        else next.delete('q')
        return next
      }, { replace: true })
    }, SEARCH_DEBOUNCE_MS)
  }

  // URL 变化（浏览器前进/后退）时同步输入框
  useEffect(() => {
    setSearchInput(query)
  }, [query])

  const switchLayout = (v: 'grid' | 'wide') => {
    if (v === layout) return
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      if (v === 'wide') next.set('view', 'wide')
      else next.delete('view')
      return next
    })
    window.scrollTo({ top: 0 })
  }

  const onCardKeyDown = (e: ReactKeyboardEvent, idx: number) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      openLightbox(idx)
    }
  }

  const currentImage = lbIndex >= 0 && lbIndex < images.length ? images[lbIndex] : null
  const showInitialError = error && images.length === 0 && !loading
  const showEmpty = !loading && !error && images.length === 0
  const showEnd = images.length > 0 && !loading && images.length >= total

  return (
    <div className="gallery-page">
      <div className="gallery-header">
        <span className="gallery-label">GALLERY</span>
        <h1 className="gallery-title">{t('gallery.title')}</h1>
        <p className="gallery-desc">
          {loading && images.length === 0
            ? t('common.loading')
            : t('gallery.totalCount', { count: total })}
        </p>
      </div>

      <div className="gallery-toolbar">
        <div className="gallery-search">
          <span className="gallery-search-icon">🔍</span>
          <input
            type="text"
            placeholder={t('gallery.search')}
            value={searchInput}
            onChange={(e) => onSearchInput(e.target.value)}
          />
        </div>
        <div className="gallery-view-switch">
          <button
            className={`gv-btn ${layout === 'grid' ? 'active' : ''}`}
            onClick={() => switchLayout('grid')}
            aria-label={t('gallery.gridView')}
            aria-pressed={layout === 'grid'}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
          </button>
          <button
            className={`gv-btn ${layout === 'wide' ? 'active' : ''}`}
            onClick={() => switchLayout('wide')}
            aria-label={t('gallery.wideView')}
            aria-pressed={layout === 'wide'}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="3" y="3" width="18" height="7" rx="1"/><rect x="3" y="14" width="18" height="7" rx="1"/></svg>
          </button>
        </div>
      </div>

      {showInitialError ? (
        <div className="gallery-error">
          <div className="gallery-empty-icon">⚠️</div>
          <div>{t('errors.loadFailedNetwork')}</div>
          <button className="gallery-retry" onClick={() => loadImages(0, false)}>
            {t('common.retry')}
          </button>
        </div>
      ) : images.length > 0 ? (
        <div className={`gallery-grid ${layout === 'wide' ? 'wide' : ''}`}>
          {images.map((img, idx) => (
            <div
              key={img.id}
              className="gallery-card"
              role="button"
              tabIndex={0}
              aria-label={t('gallery.viewLarge', { title: img.title })}
              onClick={() => openLightbox(idx)}
              onKeyDown={(e) => onCardKeyDown(e, idx)}
            >
              {img.thumb && (
                <img
                  src={img.thumb}
                  alt={img.title}
                  loading="lazy"
                  onError={(e) => { (e.target as HTMLImageElement).style.display = 'none' }}
                />
              )}
            </div>
          ))}
        </div>
      ) : showEmpty ? (
        <div className="gallery-empty">
          <div className="gallery-empty-icon">📷</div>
          <div className="gallery-empty-text">{t('gallery.empty')}</div>
        </div>
      ) : null}

      {loading && images.length === 0 && (
        <div className="gallery-skeleton-grid" aria-hidden="true">
          {Array.from({ length: PAGE_SIZE }).map((_, i) => (
            <div key={i} className="gallery-skeleton-card" />
          ))}
        </div>
      )}

      {loading && images.length > 0 && (
        <div className="gallery-loading">{t('common.loading')}</div>
      )}

      {appendError && (
        <div className="gallery-append-error">
          <span>{t('gallery.loadMoreFailed')}</span>
          <button className="gallery-retry" onClick={() => loadImages(pageRef.current, true)}>
            {t('common.retry')}
          </button>
        </div>
      )}

      {showEnd && <div className="gallery-end">{t('common.noMore')}</div>}

      {/* Lightbox */}
      {lbOpen && currentImage && (
        <div
          className="lightbox"
          ref={lightboxRef}
          role="dialog"
          aria-modal="true"
          aria-label={t('gallery.preview', { title: currentImage.title })}
          tabIndex={-1}
          onClick={closeLightbox}
        >
          <img
            className="lightbox-img"
            src={currentImage.thumb || ''}
            alt={currentImage.title}
            onClick={(e) => e.stopPropagation()}
          />
          <button className="lightbox-close" onClick={closeLightbox} aria-label={t('gallery.close')}>✕</button>
          {lbIndex > 0 && (
            <button className="lightbox-nav lightbox-prev" onClick={(e) => { e.stopPropagation(); lbPrev() }} aria-label={t('gallery.prev')}>‹</button>
          )}
          {lbIndex < images.length - 1 && (
            <button className="lightbox-nav lightbox-next" onClick={(e) => { e.stopPropagation(); lbNext() }} aria-label={t('gallery.next')}>›</button>
          )}
          <div className="lightbox-info">
            {currentImage.title}
            <span>（{lbIndex + 1} / {images.length}）</span>
          </div>
        </div>
      )}
    </div>
  )
}
