import { useState, useEffect, useCallback, useRef, useMemo, memo, lazy, Suspense, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import { createPortal } from 'react-dom'
import { useSearchParams, Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { listVideos, mapImage, burnVideo } from '../../api'
import {
  getGalleryCacheKey,
  getGalleryCachedData,
  setGalleryCacheData,
  clearGalleryCache,
} from '../../api/galleryCache'
import { useAuth } from '../../context/AuthContext'
import { useToast } from '../../components/Toast/Toast'
import ConfirmDialog from '../../components/ui/ConfirmDialog'
import type { MappedImage } from '../../api/types'
import { useVirtualGrid, type RowHeightContext } from '../../hooks/useVirtualGrid'
import { useSlideshow } from './hooks/useSlideshow'
import ExifPanel, { type GalleryExif } from './components/ExifPanel'
import './Gallery.css'

// 足迹地图依赖 leaflet（JS+CSS 约 150KB），仅在用户打开时按需加载
const FootprintMap = lazy(() => import('./components/FootprintMap'))

const PAGE_SIZE = 40
const SEARCH_DEBOUNCE_MS = 300
/** IntersectionObserver rootMargin：提前 300px 触发加载 */
const INTERSECTION_ROOT_MARGIN = '0px 0px 300px 0px'
/** 灯箱预加载前后各 2 张图片 */
const LIGHTBOX_PRELOAD_RANGE = 2
/** 画廊网格列间距（与 Gallery.css 的 gap 默认值一致） */
const GALLERY_GAP = 14
/** 时间线分组每页加载量：40/页，分组视图不做虚拟化，渲染量可接受 */
const TIMELINE_SORT = 'taken_desc'

// ── 幻灯片偏好持久化 ─────────────────────────────────────────────────────────
// api/prefs.ts 的白名单只有 autoPlay/speedMem，未知键会被忽略且没有通用 setter，
// 故这里直接用命名空间化的 localStorage 键持久化；读写全程 try/catch，
// localStorage 不可用（隐私模式/沙箱）时降级为默认值。
const SLIDESHOW_PREFS_KEY = 'atmos_gallery_slideshow'
const SLIDESHOW_INTERVALS = [3000, 5000, 10000] as const
type SlideshowInterval = (typeof SLIDESHOW_INTERVALS)[number]

interface SlideshowPrefs {
  intervalMs: SlideshowInterval
  shuffle: boolean
  loop: boolean
}

const DEFAULT_SLIDESHOW_PREFS: SlideshowPrefs = { intervalMs: 5000, shuffle: false, loop: false }

function loadSlideshowPrefs(): SlideshowPrefs {
  try {
    const raw = localStorage.getItem(SLIDESHOW_PREFS_KEY)
    if (!raw) return DEFAULT_SLIDESHOW_PREFS
    const parsed = JSON.parse(raw) as Partial<SlideshowPrefs> | null
    if (!parsed || typeof parsed !== 'object') return DEFAULT_SLIDESHOW_PREFS
    return {
      intervalMs: (SLIDESHOW_INTERVALS as readonly number[]).includes(parsed.intervalMs as number)
        ? (parsed.intervalMs as SlideshowInterval)
        : DEFAULT_SLIDESHOW_PREFS.intervalMs,
      shuffle: typeof parsed.shuffle === 'boolean' ? parsed.shuffle : DEFAULT_SLIDESHOW_PREFS.shuffle,
      loop: typeof parsed.loop === 'boolean' ? parsed.loop : DEFAULT_SLIDESHOW_PREFS.loop,
    }
  } catch {
    return DEFAULT_SLIDESHOW_PREFS
  }
}

function saveSlideshowPrefs(prefs: SlideshowPrefs): void {
  try {
    localStorage.setItem(SLIDESHOW_PREFS_KEY, JSON.stringify(prefs))
  } catch {
    // 写盘失败（隐私模式/配额满）：仅本次会话生效，不影响播放
  }
}

/**
 * 读取图片 exif：api/types.ts 的 MappedImage 已含 `exif?: ImageExif`，
 * 字段结构与本组件使用的 GalleryExif 一致。
 */
function getImageExif(img: MappedImage | null | undefined): GalleryExif | undefined {
  return img?.exif
}

/** 窄屏判断（用于 EXIF 面板底部抽屉样式；每次渲染读取，灯箱内重渲染频繁足够） */
function isMobileViewport(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false
  return window.matchMedia('(max-width: 640px)').matches
}

/** 时间线分组（takenAt 缺失归入 unknownDate，保持接口返回的先后顺序） */
interface TimelineGroup {
  key: string
  label: string
  entries: { img: MappedImage; index: number }[]
}

/**
 * 画廊行高估算：卡片固定宽高比（grid 4:3 / wide 16:9），
 * 按列宽换算后即为准确的卡片高度，保证滚动条与总高度稳定。
 */
function estimateGalleryRowHeight(layout: 'grid' | 'wide') {
  return ({ containerWidth, columns, gap }: RowHeightContext): number => {
    const safeColumns = Math.max(1, columns)
    const columnWidth = (containerWidth - Math.max(0, safeColumns - 1) * gap) / safeColumns
    return layout === 'wide' ? columnWidth * (9 / 16) : columnWidth * (3 / 4)
  }
}

// ── 图片卡片组件（memo 优化） ─────────────────────────────────────────────────
interface GalleryCardProps {
  img: MappedImage
  index: number
  onClick: (idx: number) => void
  onKeyDown: (e: ReactKeyboardEvent, idx: number) => void
  t: (key: string, opts?: Record<string, unknown>) => string
}

const GalleryCard = memo(function GalleryCard({ img, index, onClick, onKeyDown, t }: GalleryCardProps) {
  const [loaded, setLoaded] = useState(false)
  const [error, setError] = useState(false)

  return (
    <div
      className="gallery-card"
      role="button"
      tabIndex={0}
      aria-label={t('gallery.viewLarge', { title: img.title })}
      onClick={() => onClick(index)}
      onKeyDown={(e) => onKeyDown(e, index)}
    >
      {!error && img.thumb && (
        <>
          {!loaded && <div className="gallery-card-placeholder" aria-hidden="true" />}
          <img
            src={img.thumb}
            alt={img.title}
            loading="lazy"
            decoding="async"
            onLoad={() => setLoaded(true)}
            onError={() => {
              setError(true)
              setLoaded(true)
            }}
            style={loaded ? undefined : { opacity: 0 }}
          />
        </>
      )}
    </div>
  )
})

// ── 主组件 ────────────────────────────────────────────────────────────────────
export default function Gallery() {
  const { t, i18n } = useTranslation()
  const { user } = useAuth()
  const { toast } = useToast()
  const [searchParams, setSearchParams] = useSearchParams()

  // 筛选/排序状态以 URL 为准：刷新后保留，浏览器前进/后退可同步
  const query = searchParams.get('q')?.trim() ?? ''
  const layout: 'grid' | 'wide' = searchParams.get('view') === 'wide' ? 'wide' : 'grid'
  // 按拍摄时间分组：?group=date（与 ?view=/?q= 一样以 URL 为准）
  const groupByDate = searchParams.get('group') === 'date'
  // EXIF 拍摄日期筛选：?after=YYYY-MM-DD&before=YYYY-MM-DD（含当天）
  const dateAfter = searchParams.get('after') ?? ''
  const dateBefore = searchParams.get('before') ?? ''

  const [images, setImages] = useState<MappedImage[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(false)
  const [appendError, setAppendError] = useState(false)
  const [lbOpen, setLbOpen] = useState(false)
  const [lbIndex, setLbIndex] = useState(-1)
  const [searchInput, setSearchInput] = useState(query)
  // 灯箱照片信息面板开关
  const [showExif, setShowExif] = useState(false)
  // 幻灯片设置（localStorage 持久化，见 loadSlideshowPrefs 注释）
  const [slideshowPrefs, setSlideshowPrefs] = useState<SlideshowPrefs>(loadSlideshowPrefs)
  // 足迹地图开关
  const [showFootprint, setShowFootprint] = useState(false)
  // 阅后即焚确认门：待确认的图片下标（null = 无）
  const [burnConfirmIdx, setBurnConfirmIdx] = useState<number | null>(null)
  // 本次灯箱会话中已查看的图片 id（确认进入 + 灯箱内切换浏览），关闭时统一焚毁
  const viewedIdsRef = useRef<string[]>([])

  const pageRef = useRef(0)
  const loadGenRef = useRef(0)
  const loadingRef = useRef(true)
  const hasMoreRef = useRef(true)
  const fillLenRef = useRef(0)
  const lightboxRef = useRef<HTMLDivElement>(null)
  const prevFocusRef = useRef<HTMLElement | null>(null)
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const sentinelRef = useRef<HTMLDivElement>(null)

  // ── 幻灯片 ──────────────────────────────────────────────────────────────
  const slideshow = useSlideshow({
    total: images.length,
    index: lbIndex,
    onIndexChange: setLbIndex,
    intervalMs: slideshowPrefs.intervalMs,
    shuffle: slideshowPrefs.shuffle,
    loop: slideshowPrefs.loop,
  })

  const updateSlideshowPrefs = useCallback((patch: Partial<SlideshowPrefs>) => {
    setSlideshowPrefs((prev) => {
      const next = { ...prev, ...patch }
      saveSlideshowPrefs(next)
      return next
    })
  }, [])

  // 解构出稳定引用，供 useCallback/useEffect 依赖（对象本身每次渲染都是新引用）
  const { pause: pauseSlideshow, toggle: toggleSlideshow } = slideshow

  const loadImages = useCallback(async (pageNum: number, append: boolean) => {
    // 未登录时不调用 API
    if (!user) {
      setLoading(false)
      return
    }
    // 每次调用递增 generation：作废在途请求，旧响应到达时直接丢弃
    const gen = ++loadGenRef.current
    loadingRef.current = true
    setLoading(true)
    setError(false)
    setAppendError(false)
    if (!append) setTotal(0)
    try {
      // 检查缓存：分组/日期筛选会改变请求参数 → 用带筛选标识的伪 type 区分键，避免互相污染
      const cacheType = [
        'local_image',
        groupByDate ? TIMELINE_SORT : '',
        dateAfter ? `after:${dateAfter}` : '',
        dateBefore ? `before:${dateBefore}` : '',
      ]
        .filter(Boolean)
        .join('|')
      const cacheKey = getGalleryCacheKey(cacheType, query, pageNum, PAGE_SIZE)
      const cached = getGalleryCachedData(cacheKey)

      let mapped: MappedImage[]
      let newTotal: number

      if (cached) {
        mapped = cached.items
        newTotal = cached.total
      } else {
        const res = await listVideos({
          type: 'local_image',
          query,
          page: pageNum,
          size: PAGE_SIZE,
          // 分组模式请求按拍摄时间倒序；关闭时保持原有 sort 行为（不传 sort）
          ...(groupByDate ? { sort: TIMELINE_SORT } : {}),
          ...(dateAfter ? { takenAfter: dateAfter } : {}),
          ...(dateBefore ? { takenBefore: dateBefore } : {}),
        })
        if (gen !== loadGenRef.current) return
        mapped = res.items.map(mapImage).filter((v): v is MappedImage => !!v)
        newTotal = res.total
        setGalleryCacheData(cacheKey, mapped, newTotal)
      }

      if (append) {
        // 追加去重，防止重试/翻页异常产生重复卡片
        setImages((prev) => {
          const seen = new Set(prev.map((i) => i.id))
          return [...prev, ...mapped.filter((i) => !seen.has(i.id))]
        })
      } else {
        setImages(mapped)
      }
      setTotal(newTotal)
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
  }, [query, user, groupByDate, dateAfter, dateBefore])

  // 窗口滚动的响应式网格虚拟化：只渲染可视区域 + overscan 行
  const { containerRef, windowed, virtualItems, paddingTop, paddingBottom } = useVirtualGrid({
    itemCount: images.length,
    minItemWidth: layout === 'wide' ? 340 : 220,
    rowHeight: estimateGalleryRowHeight(layout),
    gap: GALLERY_GAP,
    overscan: 2,
    layoutKey: layout,
    onReachEnd: () => {
      // 窗口化后哨兵可能不在视口内，由“可见范围触达已加载末尾”兜底触发
      if (hasMoreRef.current && !loadingRef.current) loadImages(pageRef.current, true)
    },
  })

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

  // ── IntersectionObserver 替代 scroll 事件（虚拟滚动触发器） ───────────────
  useEffect(() => {
    const sentinel = sentinelRef.current
    if (!sentinel) return
    // 哨兵不可见时（无更多数据或正在加载）不创建 observer
    if (!hasMoreRef.current || loadingRef.current) return

    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[0]
        if (!entry) return
        if (entry.isIntersecting && !loadingRef.current && hasMoreRef.current) {
          loadImages(pageRef.current, true)
        }
      },
      { rootMargin: INTERSECTION_ROOT_MARGIN }
    )

    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [loadImages, images.length, loading])

  // 首屏内容不足一屏时自动补页，避免"无限追加"停摆
  // 用 ref 判断加载中：与虚拟化窗口的触底加载共享同一同步标记，避免重复请求
  useEffect(() => {
    if (loadingRef.current || images.length === 0 || !hasMoreRef.current) return
    if (images.length === fillLenRef.current) return
    const scrollable = document.documentElement.scrollHeight > window.innerHeight
    if (!scrollable) {
      fillLenRef.current = images.length
      loadImages(pageRef.current, true)
    }
  }, [images, loading, loadImages])

  // 阅后即焚：关闭查看器时焚毁本次查看过的全部图片（平台全局行为）。
  // 404 视为已焚毁（其他端/重复请求）同样成功；失败弹 toast 并保留在列表中。
  const burnViewedImages = useCallback(() => {
    const ids = viewedIdsRef.current
    viewedIdsRef.current = []
    if (ids.length === 0) return
    void Promise.all(
      ids.map(async (id) => {
        try {
          await burnVideo(id)
          return id
        } catch (e) {
          const status = (e as { status?: number } | null)?.status
          if (status === 404) return id
          const msg = (e as { message?: string } | null)?.message
          toast(msg || t('gallery.burnFailed'), 'error')
          return null
        }
      })
    ).then((results) => {
      const burned = results.filter((x): x is string => x !== null)
      if (burned.length === 0) return
      const burnedSet = new Set(burned)
      setImages((prev) => prev.filter((i) => !burnedSet.has(i.id)))
      setTotal((n) => Math.max(0, n - burned.length))
      clearGalleryCache()
      toast(t('gallery.burnedCount', { count: burned.length }), 'success')
    })
  }, [toast, t])

  const openLightbox = useCallback((idx: number) => {
    const img = images[idx]
    if (!img) return
    // 阅后即焚确认门：与播放器一致，未确认前不打开查看器
    if (user) {
      setBurnConfirmIdx(idx)
      return
    }
    setLbIndex(idx)
    setLbOpen(true)
    document.documentElement.classList.add('overflow-hidden')
  }, [images, user])

  const handleBurnConfirm = useCallback(() => {
    const idx = burnConfirmIdx
    if (idx === null) return
    const img = images[idx]
    if (img && !viewedIdsRef.current.includes(img.id)) viewedIdsRef.current.push(img.id)
    setBurnConfirmIdx(null)
    setLbIndex(idx)
    setLbOpen(true)
    document.documentElement.classList.add('overflow-hidden')
  }, [burnConfirmIdx, images])

  const handleBurnCancel = useCallback(() => {
    setBurnConfirmIdx(null)
  }, [])

  const closeLightbox = useCallback(() => {
    setLbOpen(false)
    setLbIndex(-1)
    setShowExif(false)
    pauseSlideshow()
    document.documentElement.classList.remove('overflow-hidden')
    burnViewedImages()
  }, [burnViewedImages, pauseSlideshow])

  // 灯箱内切换浏览的图片同样属于"已查看"，关闭时一并焚毁
  useEffect(() => {
    if (!lbOpen || lbIndex < 0) return
    const img = images[lbIndex]
    if (img && !viewedIdsRef.current.includes(img.id)) viewedIdsRef.current.push(img.id)
  }, [lbOpen, lbIndex, images])

  const lbPrev = useCallback(() => {
    setLbIndex((i) => Math.max(0, i - 1))
  }, [])

  const lbNext = useCallback(() => {
    setLbIndex((i) => Math.min(images.length - 1, i + 1))
  }, [images.length])

  // ── 灯箱预加载相邻图片 ──────────────────────────────────────────────────
  useEffect(() => {
    if (!lbOpen || lbIndex < 0) return

    const preloadIndices: number[] = []
    for (let offset = -LIGHTBOX_PRELOAD_RANGE; offset <= LIGHTBOX_PRELOAD_RANGE; offset++) {
      const targetIdx = lbIndex + offset
      if (targetIdx >= 0 && targetIdx < images.length && targetIdx !== lbIndex) {
        preloadIndices.push(targetIdx)
      }
    }

    // 幻灯片播放时额外预加载序列中的下一张（随机模式下与相邻张不同）
    if (slideshow.playing) {
      const nextIdx = slideshow.nextIndex
      if (nextIdx >= 0 && nextIdx < images.length && nextIdx !== lbIndex && !preloadIndices.includes(nextIdx)) {
        preloadIndices.push(nextIdx)
      }
    }

    const preloaded: HTMLImageElement[] = []
    for (const idx of preloadIndices) {
      const img = images[idx]
      if (img?.thumb) {
        const imgEl = new Image()
        imgEl.src = img.thumb
        preloaded.push(imgEl)
      }
    }

    return () => {
      // 清理预加载的 Image 对象引用
      for (const el of preloaded) {
        el.src = ''
      }
    }
  }, [lbOpen, lbIndex, images, slideshow.playing, slideshow.nextIndex])

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
      } else if (e.key === ' ' || e.key === 'Spacebar') {
        // 焦点在可交互元素上时交给原生行为（按钮点击/输入），避免双重触发
        const tag = (document.activeElement?.tagName ?? '').toLowerCase()
        if (tag === 'button' || tag === 'a' || tag === 'input' || tag === 'select' || tag === 'textarea') return
        e.preventDefault()
        toggleSlideshow()
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
  }, [lbOpen, closeLightbox, lbPrev, lbNext, toggleSlideshow])

  // 触屏左右滑动翻页（移动端没有 ←/→ 键）
  const lbTouchStartX = useRef<number | null>(null)
  const onLbTouchStart = (e: React.TouchEvent) => {
    lbTouchStartX.current = e.touches[0]?.clientX ?? null
  }
  const onLbTouchEnd = (e: React.TouchEvent) => {
    const startX = lbTouchStartX.current
    lbTouchStartX.current = null
    if (startX == null) return
    const endX = e.changedTouches[0]?.clientX ?? null
    if (endX == null) return
    const dx = endX - startX
    // 阈值 48px 且接近水平，避免与纵向滚动/点按冲突
    if (Math.abs(dx) < 48 || Math.abs(dx) < Math.abs((e.changedTouches[0]?.clientY ?? 0))) return
    if (dx < 0) lbNext()
    else lbPrev()
  }

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

  // 按拍摄时间分组开关（URL 持久化，与 ?view=/?q= 一致）
  const toggleGroupByDate = () => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      if (groupByDate) next.delete('group')
      else next.set('group', 'date')
      return next
    }, { replace: true })
    window.scrollTo({ top: 0 })
  }

  // 拍摄日期筛选（URL 持久化，?after=YYYY-MM-DD&before=YYYY-MM-DD）
  const setDateParam = (key: 'after' | 'before', value: string) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      if (value) next.set(key, value)
      else next.delete(key)
      return next
    }, { replace: true })
    window.scrollTo({ top: 0 })
  }

  const clearDates = () => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      next.delete('after')
      next.delete('before')
      return next
    }, { replace: true })
    window.scrollTo({ top: 0 })
  }

  const onCardKeyDown = useCallback((e: ReactKeyboardEvent, idx: number) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      openLightbox(idx)
    }
  }, [openLightbox])

  // ── Memoized values ──────────────────────────────────────────────────────
  const currentImage = useMemo(
    () => (lbIndex >= 0 && lbIndex < images.length ? images[lbIndex] : null),
    [lbIndex, images]
  )

  // 时间线分组：按 exif.takenAt 的日期归组，缺失/非法归入"未知时间"；
  // 组内 index 保留完整列表下标，灯箱切换与焚毁逻辑依赖它。
  const timelineGroups = useMemo<TimelineGroup[]>(() => {
    if (!groupByDate) return []
    const groups = new Map<string, TimelineGroup>()
    images.forEach((img, index) => {
      const taken = img.exif?.takenAt
      let key = 'unknown'
      let label = t('gallery.unknownDate')
      if (taken) {
        const date = new Date(taken)
        if (!isNaN(date.getTime())) {
          key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
          label = date.toLocaleDateString(i18n.language, { year: 'numeric', month: 'long', day: 'numeric' })
        }
      }
      const group = groups.get(key)
      if (group) group.entries.push({ img, index })
      else groups.set(key, { key, label, entries: [{ img, index }] })
    })
    return [...groups.values()]
  }, [groupByDate, images, i18n.language, t])

  // 足迹点：已加载图片中带 GPS 的（标题/缩略图用于地图弹窗）
  const footprintPoints = useMemo(
    () =>
      images.flatMap((img) => {
        const { lat, lon } = img.exif ?? {}
        if (typeof lat !== 'number' || typeof lon !== 'number') return []
        return [{ id: img.id, lat, lon, title: img.title, thumb: img.thumb ?? undefined }]
      }),
    [images]
  )

  const showInitialError = error && images.length === 0 && !loading
  const showEmpty = !loading && !error && images.length === 0
  const showEnd = images.length > 0 && !loading && images.length >= total

  // 骨架屏数量：根据布局响应式调整
  const skeletonCount = layout === 'wide' ? 8 : PAGE_SIZE

  return (
    <div className="gallery-page">
      {!user ? (
        <div className="gallery-auth-required">
          <div className="gallery-auth-icon">🔐</div>
          <h2>{t('gallery.authRequired', { defaultValue: '请先登录' })}</h2>
          <p>{t('gallery.authHint', { defaultValue: '登录后即可浏览图片库' })}</p>
          <Link to="/profile" className="empty-cta">{t('nav.login', { defaultValue: '登录' })}</Link>
        </div>
      ) : (
      <>
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
          <button
            className={`gv-btn ${groupByDate ? 'active' : ''}`}
            onClick={toggleGroupByDate}
            aria-label={t('gallery.groupByDate')}
            aria-pressed={groupByDate}
            title={t('gallery.groupByDate')}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M7 2v2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-2V2h-2v2H9V2H7zm12 8v10H5V10h14zM7 12v2h2v-2H7zm4 0v2h2v-2h-2zm4 0v2h2v-2h-2zm-8 4v2h2v-2H7zm4 0v2h2v-2h-2zm4 0v2h2v-2h-2z"/></svg>
          </button>
          <button
            className="gv-btn"
            onClick={() => setShowFootprint(true)}
            disabled={footprintPoints.length === 0}
            aria-label={t('gallery.footprint')}
            title={t('gallery.footprint')}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2a7 7 0 0 0-7 7c0 5.25 7 13 7 13s7-7.75 7-13a7 7 0 0 0-7-7zm0 9.5A2.5 2.5 0 1 1 12 6a2.5 2.5 0 0 1 0 5.5z"/></svg>
          </button>
        </div>
        <div className="gallery-date-filter" role="group" aria-label={t('gallery.dateFilter')}>
          <label className="gallery-date-field">
            <span>{t('gallery.dateFrom')}</span>
            <input
              type="date"
              value={dateAfter}
              max={dateBefore || undefined}
              onChange={(e) => setDateParam('after', e.target.value)}
              aria-label={t('gallery.dateFrom')}
            />
          </label>
          <label className="gallery-date-field">
            <span>{t('gallery.dateTo')}</span>
            <input
              type="date"
              value={dateBefore}
              min={dateAfter || undefined}
              onChange={(e) => setDateParam('before', e.target.value)}
              aria-label={t('gallery.dateTo')}
            />
          </label>
          {(dateAfter || dateBefore) && (
            <button
              type="button"
              className="gallery-date-clear"
              onClick={clearDates}
              aria-label={t('gallery.dateClear')}
              title={t('gallery.dateClear')}
            >
              ✕
            </button>
          )}
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
        groupByDate ? (
          <div className="gallery-timeline">
            {timelineGroups.map((group) => (
              <section key={group.key} className="timeline-group">
                <h2 className="timeline-date">{group.label}</h2>
                <div className={`gallery-grid ${layout === 'wide' ? 'wide' : ''}`}>
                  {group.entries.map(({ img, index }) => (
                    <GalleryCard
                      key={img.id}
                      img={img}
                      // index 必须是完整列表中的下标：灯箱切换/焚毁依赖它
                      index={index}
                      onClick={openLightbox}
                      onKeyDown={onCardKeyDown}
                      t={t}
                    />
                  ))}
                </div>
              </section>
            ))}
          </div>
        ) : (
        <div
          ref={containerRef}
          className={`gallery-grid ${layout === 'wide' ? 'wide' : ''}`}
          style={windowed ? { paddingTop, paddingBottom } : undefined}
        >
          {virtualItems.map((idx) => {
            const img = images[idx]
            if (!img) return null
            return (
              <GalleryCard
                key={img.id}
                img={img}
                // index 必须是完整列表中的下标：灯箱切换/焚毁依赖它
                index={idx}
                onClick={openLightbox}
                onKeyDown={onCardKeyDown}
                t={t}
              />
            )
          })}
        </div>
        )
      ) : showEmpty ? (
        <div className="gallery-empty" role="status" aria-live="polite">
          <div className="gallery-empty-icon" aria-hidden="true">📷</div>
          <div className="gallery-empty-text">{query ? t('home.searchEmpty', { query }) : t('gallery.empty')}</div>
          {query ? (
            <button
              className="empty-cta gallery-empty-cta"
              onClick={() => {
                const next = new URLSearchParams(searchParams)
                next.delete('q')
                setSearchParams(next, { replace: true })
              }}
            >
              {t('common.clearSearch') !== 'common.clearSearch' ? t('common.clearSearch') : '清空搜索'}
            </button>
          ) : (
            <Link to="/upload" className="empty-cta gallery-empty-cta">
              {t('common.goUpload') !== 'common.goUpload' ? t('common.goUpload') : '去上传图片'} →
            </Link>
          )}
        </div>
      ) : null}

      {loading && images.length === 0 && (
        <div className="gallery-skeleton-grid" aria-hidden="true">
          {Array.from({ length: skeletonCount }).map((_, i) => (
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

      {/* IntersectionObserver 哨兵元素：替代 scroll 事件监听 */}
      {hasMoreRef.current && !loading && images.length > 0 && (
        <div ref={sentinelRef} className="gallery-sentinel" aria-hidden="true" />
      )}

      {/* Lightbox */}
      {lbOpen && currentImage && createPortal(
        <>
        <div
          className="lightbox"
          ref={lightboxRef}
          role="dialog"
          aria-modal="true"
          aria-label={t('gallery.preview', { title: currentImage.title })}
          tabIndex={-1}
          onClick={closeLightbox}
        >
          <div className="lightbox-topbar">
            <span className="lightbox-title">{currentImage.title}</span>
            <div className="lightbox-actions">
              <button
                className={`lightbox-icon-btn ${showExif ? 'active' : ''}`}
                onClick={(e) => { e.stopPropagation(); setShowExif((v) => !v) }}
                aria-label={t('gallery.exif')}
                aria-pressed={showExif}
                title={t('gallery.exif')}
              >
                ⓘ
              </button>
              <button className="lightbox-close" onClick={closeLightbox} aria-label={t('gallery.close')}>✕</button>
            </div>
          </div>

          <div
            className="lightbox-img-container"
            onClick={(e) => e.stopPropagation()}
            onTouchStart={onLbTouchStart}
            onTouchEnd={onLbTouchEnd}
          >
            <img
              className="lightbox-img"
              src={currentImage.original || currentImage.thumb || ''}
              alt={currentImage.title}
              decoding="async"
              draggable={false}
            />
          </div>

          {lbIndex > 0 && (
            <button className="lightbox-nav lightbox-prev" onClick={(e) => { e.stopPropagation(); lbPrev() }} aria-label={t('gallery.prev')}>‹</button>
          )}
          {lbIndex < images.length - 1 && (
            <button className="lightbox-nav lightbox-next" onClick={(e) => { e.stopPropagation(); lbNext() }} aria-label={t('gallery.next')}>›</button>
          )}

          <div className="lightbox-bottombar" onClick={(e) => e.stopPropagation()}>
            <span className="lightbox-counter">{lbIndex + 1} / {images.length}</span>
            <div className="lightbox-slideshow" role="group" aria-label={t('gallery.slideshow')}>
              <button
                type="button"
                className="slideshow-toggle"
                onClick={slideshow.toggle}
                aria-label={slideshow.playing ? t('gallery.slideshowPause') : t('gallery.slideshowStart')}
                aria-pressed={slideshow.playing}
              >
                <span className="slideshow-toggle-icon" aria-hidden="true">{slideshow.playing ? '❚❚' : '▶'}</span>
                {slideshow.playing && (
                  <span
                    className="slideshow-progress"
                    style={{ width: `${Math.round(slideshow.progress * 100)}%` }}
                    aria-hidden="true"
                  />
                )}
              </button>
              <select
                className="slideshow-interval"
                value={slideshowPrefs.intervalMs}
                onChange={(e) => updateSlideshowPrefs({ intervalMs: Number(e.target.value) as SlideshowInterval })}
                aria-label={t('gallery.slideshowInterval')}
              >
                {SLIDESHOW_INTERVALS.map((ms) => (
                  <option key={ms} value={ms}>{ms / 1000}s</option>
                ))}
              </select>
              <label className="slideshow-option">
                <input
                  type="checkbox"
                  checked={slideshowPrefs.shuffle}
                  onChange={(e) => updateSlideshowPrefs({ shuffle: e.target.checked })}
                />
                {t('gallery.slideshowShuffle')}
              </label>
              <label className="slideshow-option">
                <input
                  type="checkbox"
                  checked={slideshowPrefs.loop}
                  onChange={(e) => updateSlideshowPrefs({ loop: e.target.checked })}
                />
                {t('gallery.slideshowLoop')}
              </label>
            </div>
          </div>
        </div>
        <ExifPanel
          exif={getImageExif(currentImage)}
          open={showExif}
          onClose={() => setShowExif(false)}
          isMobile={isMobileViewport()}
        />
        </>,
        document.body
      )}

      {/* GPS 足迹地图（leaflet 懒加载） */}
      {showFootprint && (
        <Suspense fallback={null}>
          <FootprintMap
            points={footprintPoints}
            open={showFootprint}
            onClose={() => setShowFootprint(false)}
          />
        </Suspense>
      )}

      {/* 阅后即焚确认门（与播放器一致，确认后才会打开查看器） */}
      <ConfirmDialog
        open={burnConfirmIdx !== null}
        title={t('gallery.burnConfirmTitle')}
        message={t('gallery.burnConfirmMessage')}
        danger
        confirmVariant="danger"
        confirmText={t('gallery.burnView')}
        closeOnOverlay={false}
        onConfirm={handleBurnConfirm}
        onCancel={handleBurnCancel}
      />
      </>
      )}
    </div>
  )
}
