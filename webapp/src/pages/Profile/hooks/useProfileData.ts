import { useMemo } from 'react'
import { useInfiniteQuery, useQuery } from '@tanstack/react-query'
import {
  getUserProfile, listVideos, listPlaybackHistory, listFavorites,
  listMyPlaylists, listMyShares, mapVideo, mapHistory,
} from '../../../api'
import type { UserProfile, MappedVideo, MappedHistory } from '../../../api/types'
import type { Playlist } from '../../../api/playlists'
import type { ShareListItem } from '../../../api'

const WORKS_PAGE_SIZE = 24
const HISTORY_LIMIT = 100

export function useProfileData(userId: string | undefined, activeTab: string) {
  const isWorks = activeTab === 'works'
  const isHistory = activeTab === 'history'
  const isLikes = activeTab === 'likes'
  const isPlaylists = activeTab === 'playlists'
  const isShares = activeTab === 'shares'
  const isEnabled = !!userId

  const profile = useQuery<UserProfile>({
    queryKey: ['user-profile', userId],
    queryFn: getUserProfile,
    enabled: isEnabled,
    staleTime: 60_000,
  })

  const worksQuery = useInfiniteQuery({
    queryKey: ['my-works', userId],
    queryFn: ({ pageParam }) => listVideos({
      type: 'local_video',
      size: WORKS_PAGE_SIZE,
      uploaderId: userId,
      page: pageParam,
    }),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      const loaded = allPages.reduce((n, p) => n + p.items.length, 0)
      return loaded < lastPage.total ? allPages.length : undefined
    },
    enabled: isWorks && isEnabled,
    staleTime: 30_000,
  })

  const worksTotal = worksQuery.data?.pages[0]?.total ?? 0

  const works = useMemo(() => {
    const seen = new Set<string>()
    const list: MappedVideo[] = []
    for (const page of worksQuery.data?.pages ?? []) {
      for (const raw of page.items) {
        const v = mapVideo(raw)
        if (v && !seen.has(v.id)) {
          seen.add(v.id)
          list.push(v)
        }
      }
    }
    return list
  }, [worksQuery.data])

  const history = useQuery<MappedHistory[]>({
    queryKey: ['my-history', userId],
    queryFn: () => listPlaybackHistory(HISTORY_LIMIT).then(
      (h) => h.map(mapHistory).filter((x): x is MappedHistory => !!x)
    ),
    enabled: isHistory && isEnabled,
    staleTime: 30_000,
  })

  const favorites = useQuery<MappedHistory[]>({
    queryKey: ['my-favorites', userId],
    queryFn: () => listFavorites().then(
      (f) => f.map(mapHistory).filter((x): x is MappedHistory => !!x)
    ),
    enabled: isLikes && isEnabled,
    staleTime: 30_000,
  })

  const playlists = useQuery<Playlist[]>({
    queryKey: ['my-playlists', userId],
    queryFn: listMyPlaylists,
    enabled: isPlaylists && isEnabled,
    staleTime: 30_000,
  })

  const shares = useQuery<ShareListItem[]>({
    queryKey: ['my-shares', userId],
    queryFn: listMyShares,
    enabled: isShares && isEnabled,
    staleTime: 30_000,
  })

  return { profile, worksQuery, worksTotal, works, history, favorites, playlists, shares }
}
