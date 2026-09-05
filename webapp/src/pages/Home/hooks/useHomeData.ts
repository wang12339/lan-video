import { useState, useEffect, useMemo, useRef } from 'react'
import { useSearchParams } from 'react-router-dom'
import { useInfiniteQuery, useQuery } from '@tanstack/react-query'
import { listVideos } from '../../../api/videos'
import { mapVideo } from '../../../api/utils'
import { getTrendingVideos } from '../../../api/recommendations'
import { listPlaybackHistory } from '../../../api/playback'
import type { PlaybackHistory, MappedVideo } from '../../../api/types'
import { useAuth } from '../../../context/AuthContext'
import { CATEGORY_API_MAP } from '../../../config/categories'

const PAGE_SIZE = 20

function buildParams(category: string, query: string, page: number) {
  const apiCategory = CATEGORY_API_MAP[category] ?? category
  const params: { query?: string; type: string; category?: string; page: number; size: number } = {
    type: category === 'external' ? 'external' : 'local_video',
    page,
    size: PAGE_SIZE,
  }
  if (query) params.query = query
  if (category !== 'all' && category !== 'external') params.category = apiCategory
  return params
}

export function useHomeData() {
  const { user } = useAuth()
  const [searchParams, setSearchParams] = useSearchParams()
  const [category, setCategory] = useState(() => searchParams.get('cat') || 'all')
  const [emailVerified, setEmailVerified] = useState<boolean | null>(() => {
    const param = searchParams.get('email_verified')
    if (param === 'true') return true
    if (param === 'false') return false
    return null
  })
  const query = (searchParams.get('q') || '').trim()

  useEffect(() => {
    const urlCat = searchParams.get('cat') || 'all'
    setCategory((prev) => (prev === urlCat ? prev : urlCat))
  }, [searchParams])

  useEffect(() => {
    if (emailVerified === null) return
    const timer = setTimeout(() => {
      setEmailVerified(null)
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev)
        next.delete('email_verified')
        return next
      }, { replace: true })
    }, 5000)
    return () => clearTimeout(timer)
  }, [emailVerified, setSearchParams])

  const { data, isPending, isError, hasNextPage, isFetchingNextPage, fetchNextPage, refetch } =
    useInfiniteQuery({
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

  const { data: trendingData } = useQuery({
    queryKey: ['trending-videos', query ? 'q' : 'all'],
    queryFn: getTrendingVideos,
    enabled: !!user && !query,
    staleTime: 60_000,
  })

  const { data: recentData } = useQuery({
    queryKey: ['recent-videos'],
    queryFn: () => listPlaybackHistory(8),
    enabled: !!user && !query,
    staleTime: 30_000,
  })

  const recentVideos = useMemo(
    () =>
      (Array.isArray(recentData) ? recentData : [])
        .filter((h) => h.videoId && h.title)
        .slice(0, 4)
        .map((h: PlaybackHistory) => ({
          id: h.videoId,
          title: h.title,
          thumbnail_url: h.coverUrl ?? undefined,
          thumb: h.coverUrl ?? undefined,
          views: 0,
          category: h.category,
          duration: h.durationMs > 0 ? Math.floor(h.durationMs / 1000) : undefined,
          date: h.updatedAt,
        })),
    [recentData],
  )

  const trending = useMemo(
    () => (Array.isArray(trendingData) ? trendingData : []).filter((v) => v.id).slice(0, 6),
    [trendingData],
  )
  const trendingIds = useMemo(() => new Set(trending.map((v) => v.id)), [trending])

  const mappedCacheRef = useRef<Map<string, MappedVideo>>(new Map())
  const lastPageCountRef = useRef(0)

  const videos = useMemo(() => {
    const pages = data?.pages ?? []
    const cache = mappedCacheRef.current
    if (pages.length < lastPageCountRef.current) cache.clear()
    lastPageCountRef.current = pages.length

    const startPage = cache.size === 0 ? 0 : Math.max(0, pages.length - 1)
    for (let pi = startPage; pi < pages.length; pi++) {
      const page = pages[pi]
      if (!page) continue
      for (const raw of page.items) {
        const v = mapVideo(raw)
        if (v && !cache.has(v.id)) cache.set(v.id, v)
      }
    }

    const trendingHas = trendingIds.size > 0
    const seen = new Set<string>()
    const list: MappedVideo[] = []
    for (const page of pages) {
      for (const raw of page.items) {
        const id = raw.id
        if (!id || seen.has(id)) continue
        if (trendingHas && trendingIds.has(id)) continue
        const v = cache.get(id)
        if (v) { seen.add(id); list.push(v) }
      }
    }
    return list
  }, [data, trendingIds])

  const total = data?.pages[0]?.total ?? 0

  return {
    user, category, setCategory, query, emailVerified,
    searchParams, setSearchParams,
    data, isPending, isError, hasNextPage, isFetchingNextPage, fetchNextPage, refetch,
    trending, recentVideos, videos, total,
  }
}
