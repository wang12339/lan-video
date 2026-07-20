import { useState, useEffect, useCallback, useRef } from 'react'
import { useSearchParams, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { listVideos, mapVideo, getTrendingVideos } from '../../api'
import type { MappedVideo } from '../../api/types'
import VideoCard from '../../components/VideoCard/VideoCard'
import { SkeletonLoader } from '../../components/ui'
import AuthDialog from '../../components/AuthDialog/AuthDialog'
import { useAuth } from '../../context/AuthContext'
import { trackClick } from '../../utils/track'
import './Home.css'

const CATEGORIES = ['全部', '科技', '设计', '音乐', '教程', '娱乐', '运动', '记录', '外部']
const PAGE_SIZE = 20

interface HomeCache {
  videos: MappedVideo[]
  total: number
  page: number
  category: string
  hasMore: boolean
}

export default function Home() {
  const { t } = useTranslation()
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const { user } = useAuth()
  const [videos, setVideos] = useState<MappedVideo[]>([])
  const [trending, setTrending] = useState<MappedVideo[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(false)
  const [hasMore, setHasMore] = useState(true)
  const [category, setCategory] = useState(() => searchParams.get('cat') || '全部')
  const [showAuth, setShowAuth] = useState(false)

  const pageRef = useRef(0)
  const loadingRef = useRef(false)
  const hasMoreRef = useRef(true)
  const categoryRef = useRef(category)
  categoryRef.current = category
  const cachedDataRef = useRef<HomeCache | null>(null)
  const cachedScrollRef = useRef(0)
  const query = searchParams.get('q') || ''
  const queryRef = useRef(query)
  queryRef.current = query
  const videosRef = useRef<MappedVideo[]>([])
  const totalRef = useRef(0)
  const scrollYRef = useRef(0)

  const loadInitial = useCallback(async (cat: string, restore?: boolean) => {
    if (restore && cachedDataRef.current) {
      if (cachedDataRef.current.category !== cat) {
        cachedDataRef.current = null
        cachedScrollRef.current = 0
        restore = false
      }
    }
    if (restore && cachedDataRef.current) {
      videosRef.current = cachedDataRef.current.videos
      totalRef.current = cachedDataRef.current.total
      setVideos(cachedDataRef.current.videos)
      setTotal(cachedDataRef.current.total)
      pageRef.current = cachedDataRef.current.page
      hasMoreRef.current = cachedDataRef.current.hasMore
      setHasMore(cachedDataRef.current.hasMore)
      const scrollY = cachedScrollRef.current
      cachedDataRef.current = null
      cachedScrollRef.current = 0
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          window.scrollTo({ top: scrollY, behavior: 'instant' })
        })
      })
      return
    }

    loadingRef.current = true
    setLoading(true)
    pageRef.current = 0
    try {
      const params: Record<string, unknown> = {
        page: 0,
        size: PAGE_SIZE,
      }
      if (queryRef.current) params.query = queryRef.current
      if (cat === '外部') {
        params.type = 'external'
      } else {
        params.type = 'local_video'
        if (cat !== '全部') params.category = cat
      }

      const res = await listVideos(params)
      const mapped = res.items.map(mapVideo).filter(Boolean) as MappedVideo[]
      const more = mapped.length >= PAGE_SIZE

      videosRef.current = mapped
      totalRef.current = res.total
      setVideos(mapped)
      setTotal(res.total)
      hasMoreRef.current = more
      setHasMore(more)
      pageRef.current = 1
    } catch (err) {
      console.error('加载视频失败:', err)
    } finally {
      loadingRef.current = false
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!user) return
    const restore = !!cachedDataRef.current
    loadInitial(category, restore)
  }, [user, category, loadInitial])

  // Load trending videos when no search query (TanStack Query handles caching/dedup)
  const { data: trendingData } = useQuery({
    queryKey: ['trending-videos'],
    queryFn: getTrendingVideos,
    enabled: !!user && !query,
    staleTime: 60_000,
  })
  useEffect(() => {
    if (trendingData) {
      setTrending(trendingData.filter(v => v.id !== 0).slice(0, 6))
    }
  }, [trendingData])

  useEffect(() => {
    const onScroll = () => {
      scrollYRef.current = window.scrollY

      if (loadingRef.current || !hasMoreRef.current) return
      const doc = document.documentElement
      const body = document.body
      const scrollTop = window.pageYOffset || doc.scrollTop || body.scrollTop || 0
      const scrollHeight = Math.max(doc.scrollHeight, body.scrollHeight)
      const clientHeight = window.innerHeight || doc.clientHeight
      if (scrollTop + clientHeight >= scrollHeight - 300) {
        loadingRef.current = true
        const params: Record<string, unknown> = {
          page: pageRef.current,
          size: PAGE_SIZE,
        }
        if (queryRef.current) params.query = queryRef.current
        if (categoryRef.current === '外部') {
          params.type = 'external'
        } else {
          params.type = 'local_video'
          if (categoryRef.current !== '全部') params.category = categoryRef.current
        }

        listVideos(params).then((res) => {
          const mapped = res.items.map(mapVideo).filter(Boolean) as MappedVideo[]
          const more = mapped.length >= PAGE_SIZE
          setVideos((prev) => {
            const next = [...prev, ...mapped]
            videosRef.current = next
            return next
          })
          totalRef.current = res.total
          setTotal(res.total)
          hasMoreRef.current = more
          setHasMore(more)
          pageRef.current += 1
        }).catch(() => {}).finally(() => {
          loadingRef.current = false
        })
      }
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  useEffect(() => {
    return () => {
      if (videosRef.current.length > 0) {
        cachedDataRef.current = {
          videos: videosRef.current,
          total: totalRef.current,
          page: pageRef.current,
          category: categoryRef.current,
          hasMore: hasMoreRef.current,
        }
        cachedScrollRef.current = scrollYRef.current
      }
    }
  }, [])

  return (
    <div className="home">
      {!user && (
        <div className="hero">
          <h1 className="hero-title">{t('home.heroTitle')}</h1>
          <p className="hero-sub">{t('home.heroSub')}</p>
        </div>
      )}

      {user && (
        <>
          <div className="category-bar">
            {CATEGORIES.map((cat) => (
              <button
                key={cat}
                className={`cat-tag ${category === cat ? 'active' : ''}`}
                onClick={() => {
                  setCategory(cat)
                  const next = new URLSearchParams(searchParams)
                  if (cat === '全部') next.delete('cat')
                  else next.set('cat', cat)
                  navigate(`?${next.toString()}`)
                  trackClick('切换分类', cat)
                }}
              >
                {cat}
              </button>
            ))}
          </div>

          {total > 0 && <div className="home-count">{t('home.totalCount', { count: total })}</div>}

          {!query && trending.length > 0 && (
            <div className="trending-section">
              <h2 className="trending-title">{t('home.trending')}</h2>
              <div className="video-grid">
                {trending.map((video) => (
                  <VideoCard key={`trend-${video.id}`} video={video} />
                ))}
              </div>
            </div>
          )}

          {videos.length > 0 ? (
            <div className="video-grid">
              {videos.map((video) => (
                <VideoCard key={video.id} video={video} />
              ))}
            </div>
          ) : !loading ? (
            <div className="empty-state">
              <div className="empty-icon">🎬</div>
              <div className="empty-text">{t('home.empty')}</div>
            </div>
          ) : null}

          {loading && (
            <div className="video-grid" style={{ marginTop: 16 }}>
              {Array.from({ length: 6 }).map((_, i) => (
                <SkeletonLoader key={i} type="card" lines={1} />
              ))}
            </div>
          )}

          {!loading && !hasMore && videos.length > 0 && (
            <div className="no-more">{t('common.noMore')}</div>
          )}
        </>
      )}

      {showAuth && <AuthDialog onClose={() => setShowAuth(false)} />}

      {!user && !loading && <AuthDialog closable={false} />}
    </div>
  )
}
