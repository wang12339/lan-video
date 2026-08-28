import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { toggleFavorite, getFavoriteStatus } from '../../api'
import type { MappedVideo } from '../../api/types'
import { trackClick } from '../../utils/track'

export function useFavoriteHandler(user: { id: string } | null | undefined, video: MappedVideo | null, videoId: string, isShared: boolean) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [favorited, setFavorited] = useState(false)

  const { data: favStatus } = useQuery({
    queryKey: ['favorite-status', videoId],
    queryFn: () => getFavoriteStatus(videoId),
    enabled: !!user && !!videoId && !isShared,
  })

  if (favStatus && favStatus.favorited !== favorited) {
    setFavorited(favStatus.favorited)
  }

  const handleFavorite = useCallback(async () => {
    if (!user || !video) { return }
    try {
      const res = await toggleFavorite(video.id)
      setFavorited(res.favorited)
      queryClient.invalidateQueries({ queryKey: ['my-favorites'] })
      trackClick(res.favorited ? 'Favorite' : 'Unfavorite', video.title)
      return res.favorited
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : t('player.favoriteFailed')
      throw new Error(msg)
    }
  }, [user, video, queryClient, t])

  return { favorited, handleFavorite }
}
