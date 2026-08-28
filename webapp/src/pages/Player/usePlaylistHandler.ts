import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { listMyPlaylists, addVideoToPlaylist } from '../../api/playlists'
import type { MappedVideo } from '../../api/types'
import { trackClick } from '../../utils/track'

export function usePlaylistHandler(user: { id: string } | null | undefined, video: MappedVideo | null) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [showPlaylistPicker, setShowPlaylistPicker] = useState(false)

  const { data: myPlaylists = [] } = useQuery({
    queryKey: ['my-playlists', user?.id],
    queryFn: listMyPlaylists,
    enabled: !!user && showPlaylistPicker,
  })

  const handleAddToPlaylist = useCallback(async (playlistId: string) => {
    if (!video) return
    try {
      await addVideoToPlaylist(playlistId, video.id)
      setShowPlaylistPicker(false)
      queryClient.invalidateQueries({ queryKey: ['my-playlists'] })
      trackClick('Add to playlist', video.title)
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : t('player.addToPlaylistFailed')
      throw new Error(msg)
    }
  }, [video, queryClient, t])

  return { showPlaylistPicker, setShowPlaylistPicker, myPlaylists, handleAddToPlaylist }
}
