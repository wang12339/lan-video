import { useState, useEffect, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { listVideos, mapImage } from '../../api'
import type { MappedImage } from '../../api/types'
import './Gallery.css'

const PAGE_SIZE = 40

export default function Gallery() {
  const { t } = useTranslation()
  const [images, setImages] = useState<MappedImage[]>([])
  const [loading, setLoading] = useState(true)
  const [total, setTotal] = useState(0)
  const [query, setQuery] = useState('')
  const [layout, setLayout] = useState<'grid' | 'wide'>('grid')
  const [lbOpen, setLbOpen] = useState(false)
  const [lbIndex, setLbIndex] = useState(-1)

  const pageRef = useRef(0)
  const loadGenRef = useRef(0)
  const loadingRef = useRef(true)
  const hasMoreRef = useRef(true)
  const queryRef = useRef(query)
  queryRef.current = query
  const lightboxRef = useRef<HTMLDivElement>(null)

  const loadImages = useCallback(async (pageNum: number, q: string, append: boolean) => {
    loadingRef.current = true
    setLoading(true)
    const gen = ++loadGenRef.current
    try {
      const res = await listVideos({ type: 'local_image', query: q, page: pageNum, size: PAGE_SIZE })
      if (gen !== loadGenRef.current) return
      const mapped = res.items.map(mapImage).filter((v): v is MappedImage => !!v)
      if (append) {
        setImages((prev) => [...prev, ...mapped])
      } else {
        setImages(mapped)
      }
      setTotal(res.total)
      const more = mapped.length >= PAGE_SIZE
      hasMoreRef.current = more
      pageRef.current = pageNum + 1
    } catch {
      if (gen !== loadGenRef.current) return
    } finally {
      loadingRef.current = false
      if (gen === loadGenRef.current) setLoading(false)
    }
  }, [])

  useEffect(() => {
    pageRef.current = 0
    loadImages(0, query, false)
  }, [query, loadImages])

  useEffect(() => {
    const onScroll = () => {
      if (loadingRef.current || !hasMoreRef.current) return
      const doc = document.documentElement
      const body = document.body
      const scrollTop = window.pageYOffset || doc.scrollTop || body.scrollTop || 0
      const scrollHeight = Math.max(doc.scrollHeight, body.scrollHeight)
      const clientHeight = window.innerHeight || doc.clientHeight
      if (scrollTop + clientHeight >= scrollHeight - 300) {
        loadImages(pageRef.current, queryRef.current, true)
      }
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [loadImages])

  const openLightbox = (idx: number) => {
    setLbIndex(idx)
    setLbOpen(true)
    document.documentElement.classList.add('overflow-hidden')
  }

  const closeLightbox = () => {
    setLbOpen(false)
    setLbIndex(-1)
    document.documentElement.classList.remove('overflow-hidden')
  }

  const lbPrev = useCallback(() => {
    if (lbIndex > 0) { setLbIndex((i) => i - 1) }
  }, [lbIndex])

  const lbNext = useCallback(() => {
    if (lbIndex < images.length - 1) { setLbIndex((i) => i + 1) }
  }, [lbIndex, images.length])

  useEffect(() => {
    if (!lbOpen || !lightboxRef.current) return
    const prevFocus = document.activeElement as HTMLElement | null
    lightboxRef.current.focus()

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeLightbox()
      if (e.key === 'ArrowLeft' && lbIndex > 0) setLbIndex((i) => i - 1)
      if (e.key === 'ArrowRight' && lbIndex < images.length - 1) setLbIndex((i) => i + 1)

      if (e.key === 'Tab' && lightboxRef.current) {
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
      prevFocus?.focus()
    }
  }, [lbOpen, lbIndex, images.length])

  useEffect(() => {
    if (lbOpen && (lbIndex < 0 || lbIndex >= images.length)) {
      setLbOpen(false)
    }
  }, [lbOpen, lbIndex, images.length])

  useEffect(() => {
    return () => {
      document.documentElement.classList.remove('overflow-hidden')
    }
  }, [])

  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const onSearchInput = (val: string) => {
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current)
    searchTimerRef.current = setTimeout(() => {
      setQuery(val.trim())
    }, 150)
  }

  const currentImage = (lbIndex >= 0 && lbIndex < images.length) ? images[lbIndex] : null

  return (
    <div className="gallery-page">
      <div className="gallery-header">
        <span className="gallery-label">GALLERY</span>
        <h1 className="gallery-title">图片浏览</h1>
        <p className="gallery-desc">{t('gallery.totalCount', { count: total })}</p>
      </div>

      <div className="gallery-toolbar">
        <div className="gallery-search">
          <span className="gallery-search-icon">🔍</span>
          <input
            type="text"
            placeholder={t('gallery.search')}
            onChange={(e) => onSearchInput(e.target.value)}
          />
        </div>
        <div className="gallery-view-switch">
          <button
            className={`gv-btn ${layout === 'grid' ? 'active' : ''}`}
            onClick={() => setLayout('grid')}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
          </button>
          <button
            className={`gv-btn ${layout === 'wide' ? 'active' : ''}`}
            onClick={() => setLayout('wide')}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="3" y="3" width="18" height="7" rx="1"/><rect x="3" y="14" width="18" height="7" rx="1"/></svg>
          </button>
        </div>
      </div>

      {images.length > 0 ? (
        <div className={`gallery-grid ${layout === 'wide' ? 'wide' : ''}`}>
          {images.map((img, idx) => (
            <div
              key={img.id}
              className="gallery-card"
              onClick={() => openLightbox(idx)}
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
      ) : !loading ? (
        <div className="gallery-empty">
          <div className="gallery-empty-icon">📷</div>
          <div>{t('gallery.empty')}</div>
        </div>
      ) : null}

      {loading && <div className="gallery-loading">{t('common.loading')}</div>}

      {/* Lightbox */}
      {lbOpen && currentImage && (
        <div className="lightbox" ref={lightboxRef} tabIndex={-1} onClick={closeLightbox}>
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
            {lbIndex >= 0 && <span> ({lbIndex + 1} / {images.length})</span>}
          </div>
        </div>
      )}
    </div>
  )
}
