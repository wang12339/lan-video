import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { toggleFavorite, getFavoriteStatus } from '../../api'
import type { MappedVideo } from '../../api/types'
import { trackClick } from '../../utils/track'

export function useFavoriteHandler(user: { id: string } | null | undefined, video: MappedVideo | null, videoId: string, isShared: boolean) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()

  const { data: favStatus } = useQuery({
    queryKey: ['favorite-status', videoId],
    queryFn: () => getFavoriteStatus(videoId),
    enabled: !!user && !!videoId && !isShared,
  })

  // 以 query 数据为唯一来源，渲染期不 setState：旧的本地 state 会被
  // 在途的过期响应覆盖用户刚点击的切换结果
  const favorited = favStatus?.favorited ?? false

  const handleFavorite = useCallback(async () => {
    if (!user || !video) { return }
    try {
      // 取消在途的状态查询，避免其过期响应在切换后覆盖 setQueryData 的结果
      await queryClient.cancelQueries({ queryKey: ['favorite-status', videoId] })
      const res = await toggleFavorite(video.id)
      queryClient.setQueryData(['favorite-status', videoId], { favorited: res.favorited })
      queryClient.invalidateQueries({ queryKey: ['my-favorites'] })
      trackClick(res.favorited ? 'Favorite' : 'Unfavorite', video.title)
      return res.favorited
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : t('player.favoriteFailed')
      throw new Error(msg)
    }
  }, [user, video, videoId, queryClient, t])

  return { favorited, handleFavorite }
}
