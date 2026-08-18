import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { useSearchParams, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useInfiniteQuery, useQuery, keepPreviousData } from '@tanstack/react-query'
import { listVideos, mapVideo, getTrendingVideos } from '../../api'
import type { MappedVideo } from '../../api/types'
import VideoCard, { VideoCardSkeleton } from '../../components/VideoCard/VideoCard'
import AuthDialog from '../../components/AuthDialog/AuthDialog'
import { useAuth } from '../../context/AuthContext'
import { trackClick } from '../../utils/track'
import './Home.css'

const CATEGORIES = ['全部', '科技', '设计', '音乐', '教程', '娱乐', '运动', '记录', '外部']
const PAGE_SIZE = 20

// 离开首页时记住滚动位置，返回时恢复（模块级变量，跨组件卸载保留）
let homeScrollY = 0

interface VideoListParams {
  query?: string
  type: string
  category?: string
  page: number
  size: number
}

function buildParams(category: string, query: string, page: number): VideoListParams {
  const params: VideoListParams = {
    type: category === '外部' ? 'external' : 'local_video',
    page,
    size: PAGE_SIZE,
  }
  if (query) params.query = query
  if (category !== '全部' && category !== '外部') params.category = category
  return params
}

export default function Home() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const navigate = useNavigate()
  const { user } = useAuth()
  const [category, setCategory] = useState(() => searchParams.get('cat') || '全部')
  const [emailVerified, setEmailVerified] = useState<boolean | null>(() => {
    const param = searchParams.get('email_verified')
    if (param === 'true') return true
    if (param === 'false') return false
    return null
  })
  const query = searchParams.get('q') || ''

  // URL 分类与 state 同步（支持浏览器前进/后退）
  useEffect(() => {
    const urlCat = searchParams.get('cat') || '全部'
    setCategory((prev) => (prev === urlCat ? prev : urlCat))
  }, [searchParams])

  // Clean up email_verified query parameter after capturing it
  useEffect(() => {
    if (emailVerified === null) return
    const timer = setTimeout(() => {
      setEmailVerified(null)
      const next = new URLSearchParams(searchParams)
      next.delete('email_verified')
      setSearchParams(next, { replace: true })
    }, 5000)
    return () => clearTimeout(timer)
  }, [emailVerified, searchParams, setSearchParams])

  // 视频列表：分页 + 加载更多 + 错误重试（TanStack Query 管理缓存/去重）
  const {
    data,
    isPending,
    isError,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
    refetch,
  } = useInfiniteQuery({
    queryKey: ['home-videos', category, query],
    queryFn: ({ pageParam }) => listVideos(buildParams(category, query, pageParam)),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      const loaded = allPages.reduce((n, p) => n + p.items.length, 0)
      return loaded < lastPage.total ? allPages.length : undefined
    },
    enabled: !!user,
    placeholderData: keepPreviousData,
    staleTime: 30_000,
  })

  // 热门推荐（登录后、无搜索关键词时展示）
  const { data: trendingData } = useQuery({
    queryKey: ['trending-videos'],
    queryFn: getTrendingVideos,
    enabled: !!user && !query,
    staleTime: 60_000,
  })

  const trending = useMemo(
    () => (trendingData ?? []).filter((v) => v.id).slice(0, 6),
    [trendingData]
  )
  const trendingIds = useMemo(() => new Set(trending.map((v) => v.id)), [trending])

  // 展平所有分页，并按 id 去重（热门推荐不重复出现在下方列表）
  const videos = useMemo(() => {
    const pages = data?.pages ?? []
    const seen = new Set<string>()
    const list: MappedVideo[] = []
    for (const page of pages) {
      for (const raw of page.items) {
        const v = mapVideo(raw)
        if (!v || seen.has(v.id) || trendingIds.has(v.id)) continue
        seen.add(v.id)
        list.push(v)
      }
    }
    return list
  }, [data, trendingIds])

  const total = data?.pages[0]?.total ?? 0

  // 返回首页时恢复滚动位置（数据命中缓存时生效）
  const restoredRef = useRef(false)
  useEffect(() => {
    if (restoredRef.current || !data) return
    restoredRef.current = true
    const y = homeScrollY
    if (y > 0) {
      requestAnimationFrame(() => {
        window.scrollTo({ top: y, behavior: 'instant' })
      })
    }
  }, [data])

  // 分类/关键词变化时回到顶部
  useEffect(() => {
    restoredRef.current = false
    homeScrollY = 0
    window.scrollTo({ top: 0, behavior: 'instant' })
  }, [category, query])

  // 记录滚动位置
  useEffect(() => {
    const onScroll = () => {
      homeScrollY = window.scrollY
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  // 底部哨兵：滚动接近末尾时加载下一页
  const sentinelRef = useRef<HTMLDivElement>(null)
  const hasVideosRef = useRef(false)
  hasVideosRef.current = videos.length > 0
  useEffect(() => {
    const el = sentinelRef.current
    if (!el) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (hasVideosRef.current && entries.some((e) => e.isIntersecting)) {
          fetchNextPage()
        }
      },
      { rootMargin: '300px 0px' }
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [fetchNextPage])

  const handleCategoryClick = useCallback((cat: string) => {
    if (cat === category) return
    setCategory(cat)
    const next = new URLSearchParams(searchParams)
    if (cat === '全部') next.delete('cat')
    else next.set('cat', cat)
    navigate(`?${next.toString()}`)
    trackClick('切换分类', cat)
  }, [category, navigate, searchParams])

  const showInitialError = isError && videos.length === 0 && !isPending
  const showEmpty = !isPending && !isError && videos.length === 0

  return (
    <div className="home">
      {emailVerified !== null && (
        <div className={`email-verify-banner ${emailVerified ? 'success' : 'error'}`}>
          {emailVerified ? t('home.emailVerified') : t('home.emailVerifyFailed')}
        </div>
      )}

      {!user && (
        <div className="hero">
          <h1 className="hero-title">{t('home.heroTitle')}</h1>
          <p className="hero-sub">{t('home.heroSub')}</p>
          <div className="hero-decoration" aria-hidden="true">
            <span className="hero-dot hero-dot--1" />
            <span className="hero-dot hero-dot--2" />
            <span className="hero-dot hero-dot--3" />
          </div>
        </div>
      )}

      {user && (
        <>
          <div className="category-bar">
            {CATEGORIES.map((cat) => (
              <button
                key={cat}
                className={`cat-tag ${category === cat ? 'active' : ''}`}
                onClick={() => handleCategoryClick(cat)}
              >
                {cat}
              </button>
            ))}
          </div>

          {total > 0 && <div className="home-count">{t('home.totalCount', { count: total })}</div>}

          {!query && trending.length > 0 && (
            <section className="trending-section" aria-label={t('home.trending')}>
              <h2 className="trending-title">{t('home.trending')}</h2>
              <div className="video-grid">
                {trending.map((video, i) => (
                  <div key={`trend-${video.id}`} style={{ '--card-index': i } as React.CSSProperties}>
                    <VideoCard video={video} />
                  </div>
                ))}
              </div>
            </section>
          )}

          {videos.length > 0 ? (
            <div className="video-grid">
              {videos.map((video, i) => (
                <div key={video.id} style={{ '--card-index': i } as React.CSSProperties}>
                  <VideoCard video={video} />
                </div>
              ))}
            </div>
          ) : isPending ? (
            <div className="video-grid">
              <VideoCardSkeleton count={6} />
            </div>
          ) : showInitialError ? (
            <div className="empty-state">
              <div className="empty-icon">⚠️</div>
              <div className="empty-text">{t('errors.network')}</div>
              <button className="retry-btn" onClick={() => refetch()}>
                {t('common.retry')}
              </button>
            </div>
          ) : showEmpty ? (
            <div className="empty-state">
              <div className="empty-icon">🎬</div>
              <div className="empty-text">
                {query ? t('home.searchEmpty', { query }) : t('home.empty')}
              </div>
            </div>
          ) : null}

          {isError && videos.length > 0 && (
            <div className="load-more-error">
              <span>{t('errors.network')}</span>
              <button className="retry-btn" onClick={() => refetch()}>
                {t('common.retry')}
              </button>
            </div>
          )}

          {!isError && isFetchingNextPage && (
            <div className="loading-more" role="status">{t('common.loading')}</div>
          )}

          {!isError && !isFetchingNextPage && !hasNextPage && videos.length > 0 && (
            <div className="no-more">{t('common.noMore')}</div>
          )}
        </>
      )}

      {/* 哨兵始终渲染，避免登录状态切换后 observer 失效 */}
      <div ref={sentinelRef} className="load-sentinel" aria-hidden="true" />

      {!user && <AuthDialog closable={false} />}
    </div>
  )
}
