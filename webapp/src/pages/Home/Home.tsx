import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { useSearchParams, Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useInfiniteQuery, useQuery } from '@tanstack/react-query'
import { listVideos } from '../../api/videos'
import { mapVideo } from '../../api/utils'
import { getTrendingVideos } from '../../api/recommendations'
import type { MappedVideo } from '../../api/types'
import VideoCard, { VideoCardSkeleton } from '../../components/VideoCard/VideoCard'
import { useAuth } from '../../context/AuthContext'
import { trackClick } from '../../utils/track'
import { useInfiniteScroll } from '../../hooks/useInfiniteScroll'
import './Home.css'

const CATEGORY_KEYS = [
  { key: 'all', value: '全部' },
  { key: 'tech', value: '科技' },
  { key: 'design', value: '设计' },
  { key: 'music', value: '音乐' },
  { key: 'tutorial', value: '教程' },
  { key: 'entertainment', value: '娱乐' },
  { key: 'sports', value: '运动' },
  { key: 'record', value: '记录' },
  { key: 'external', value: '外部' },
]
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

// 使用 React.memo 优化视频卡片组件，避免不必要的重渲染
const VideoCardMemo = React.memo(VideoCard)

export default function Home() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const { user } = useAuth()
  const [category, setCategory] = useState(() => searchParams.get('cat') || '全部')
  const [emailVerified, setEmailVerified] = useState<boolean | null>(() => {
    const param = searchParams.get('email_verified')
    if (param === 'true') return true
    if (param === 'false') return false
    return null
  })
  const query = (searchParams.get('q') || '').trim()

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
    placeholderData: (prev) => prev,
    staleTime: 30_000,
  })

  // 热门推荐（无搜索时展示，游客可看 3 条作试看）
  const { data: trendingData } = useQuery({
    queryKey: ['trending-videos', query ? 'q' : 'all'],
    queryFn: getTrendingVideos,
    enabled: !query,
    staleTime: 60_000,
  })

  const trending = useMemo(
    () => (trendingData ?? []).filter((v) => v.id).slice(0, 6),
    [trendingData]
  )
  const trendingIds = useMemo(() => new Set(trending.map((v) => v.id)), [trending])

  // 增量映射：缓存已映射结果，只对新增页面执行 mapVideo
  const mappedCacheRef = useRef<Map<string, MappedVideo>>(new Map())
  const lastPageCountRef = useRef(0)

  const videos = useMemo(() => {
    const pages = data?.pages ?? []
    const cache = mappedCacheRef.current

    // 如果页数减少（切换分类/搜索），清空缓存
    if (pages.length < lastPageCountRef.current) {
      cache.clear()
    }
    lastPageCountRef.current = pages.length

    // 只处理新增的页面
    const startPage = cache.size === 0 ? 0 : Math.max(0, pages.length - 1)
    for (let pi = startPage; pi < pages.length; pi++) {
      const page = pages[pi]
      if (!page) continue
      for (const raw of page.items) {
        const v = mapVideo(raw)
        if (v && !cache.has(v.id)) {
          cache.set(v.id, v)
        }
      }
    }

    // 过滤 trending 并保持顺序
    const seen = new Set<string>()
    const list: MappedVideo[] = []
    for (const page of pages) {
      for (const raw of page.items) {
        const id = raw.id
        if (!id || seen.has(id) || trendingIds.has(id)) continue
        const v = cache.get(id)
        if (v) {
          seen.add(id)
          list.push(v)
        }
      }
    }
    return list
  }, [data, trendingIds])

  // 后端已按 query 过滤，无需前端二次过滤（避免分页 total 错位）
  const filteredVideos = videos

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

  // 底部哨兵：统一 useInfiniteScroll
  const sentinelRef = useRef<HTMLDivElement>(null)
  useInfiniteScroll(sentinelRef, {
    hasMore: !!hasNextPage && filteredVideos.length > 0,
    loading: isFetchingNextPage,
    onLoadMore: fetchNextPage,
  })

  // 使用 useCallback 缓存事件处理函数
  const handleCategoryClick = useCallback((cat: string) => {
    if (cat === category) return
    setCategory(cat)
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      if (cat === '全部') next.delete('cat')
      else next.set('cat', cat)
      return next
    })
    trackClick('切换分类', cat)
  }, [category, setSearchParams])

  const showInitialError = isError && filteredVideos.length === 0 && !isPending
  const showEmpty = !isPending && !isError && filteredVideos.length === 0

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
          <p className="hero-desc">{t('home.heroDesc')}</p>
          <div className="hero-features">
            <div className="hero-feature">
              <span className="hero-feature-icon">⚡</span>
              <div className="hero-feature-text">
                <span className="hero-feature-name">{t('home.featureHls')}</span>
                <span className="hero-feature-desc">{t('home.featureHlsDesc')}</span>
              </div>
            </div>
            <div className="hero-feature">
              <span className="hero-feature-icon">📦</span>
              <div className="hero-feature-text">
                <span className="hero-feature-name">{t('home.featureUpload')}</span>
                <span className="hero-feature-desc">{t('home.featureUploadDesc')}</span>
              </div>
            </div>
            <div className="hero-feature">
              <span className="hero-feature-icon">🔒</span>
              <div className="hero-feature-text">
                <span className="hero-feature-name">{t('home.featureShare')}</span>
                <span className="hero-feature-desc">{t('home.featureShareDesc')}</span>
              </div>
            </div>
            <div className="hero-feature">
              <span className="hero-feature-icon">🏠</span>
              <div className="hero-feature-text">
                <span className="hero-feature-name">{t('home.featurePrivate')}</span>
                <span className="hero-feature-desc">{t('home.featurePrivateDesc')}</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {!user && trending.length > 0 && (
        <section className="trending-section guest-preview" aria-label={t('home.trending')}>
          <h2 className="trending-title">{t('home.trending')} · {t('home.guestPreview')}</h2>
          <div className="video-grid">
            {trending.slice(0, 3).map((video, i) => (
              <div key={`guest-${video.id}`} style={{ '--card-index': i } as React.CSSProperties}>
                <VideoCardMemo video={video} eager={i < 2} />
              </div>
            ))}
          </div>
          <div className="guest-cta-wrap">
            <Link to="/profile" className="empty-cta">{t('home.guestCta')} →</Link>
          </div>
        </section>
      )}

      {user && (
        <>
          <div className="category-bar">
            {CATEGORY_KEYS.map((cat) => (
              <button
                key={cat.key}
                className={`cat-tag ${category === cat.value ? 'active' : ''}`}
                onClick={() => handleCategoryClick(cat.value)}
              >
                {t('home.categories.' + cat.key)}
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
                    <VideoCardMemo video={video} eager={i < 4} />
                  </div>
                ))}
              </div>
            </section>
          )}

          {filteredVideos.length > 0 ? (
            <div className="video-grid">
              {filteredVideos.map((video, i) => (
                <div key={video.id} style={{ '--card-index': i } as React.CSSProperties}>
                  <VideoCardMemo video={video} eager={i < 4} />
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
            <div className="empty-state" role="status" aria-live="polite">
              <div className="empty-icon" aria-hidden="true">🎬</div>
              <div className="empty-text">
                {query ? t('home.searchEmpty', { query }) : t('home.empty')}
              </div>
              {query ? (
                <button
                  className="empty-cta"
                  onClick={() => {
                    const next = new URLSearchParams(searchParams)
                    next.delete('q')
                    setSearchParams(next, { replace: true })
                  }}
                >
                  {t('common.clearSearch') !== 'common.clearSearch' ? t('common.clearSearch') : '清空搜索'}
                </button>
              ) : (
                <Link to="/upload" className="empty-cta">
                  {t('common.goUpload') !== 'common.goUpload' ? t('common.goUpload') : '去上传第一个视频'} →
                </Link>
              )}
            </div>
          ) : null}

          {isError && filteredVideos.length > 0 && (
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

          {!isError && !isFetchingNextPage && !hasNextPage && filteredVideos.length > 0 && (
            <div className="no-more">{t('common.noMore')}</div>
          )}
        </>
      )}

      {/* 哨兵始终渲染，避免登录状态切换后 observer 失效 */}
      <div ref={sentinelRef} className="load-sentinel" aria-hidden="true" />

      {/* 游客不强制弹窗，通过 CTA 按钮引导登录 */}
    </div>
  )
}
