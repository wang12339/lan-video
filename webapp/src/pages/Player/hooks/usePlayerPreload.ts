import { useState, useCallback, useEffect, useRef } from 'react'
import { usePreloadManager, useMemoryManager } from '../usePlayerHooks'
import { listVideos, mapVideo } from '../../../api'
import type { MappedVideo } from '../../../api/types'
import { PRELOAD_THRESHOLD } from '../constants'

export function usePlayerPreload(
  videoId: string,
  _related: MappedVideo[],
  videoRef: React.RefObject<HTMLVideoElement | null>,
) {
  const [preloadingNext, setPreloadingNext] = useState(false)
  const { preloadVideo, cleanup: cleanupPreload } = usePreloadManager()
  const { optimizeMemory } = useMemoryManager()
  const preloadRequestRef = useRef<string | null>(null)
  const optimizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    setPreloadingNext(false)
    preloadRequestRef.current = null
    if (optimizeTimerRef.current) { clearTimeout(optimizeTimerRef.current); optimizeTimerRef.current = null }
    cleanupPreload()
  }, [videoId, cleanupPreload])

  useEffect(() => {
    return () => {
      if (optimizeTimerRef.current) clearTimeout(optimizeTimerRef.current)
    }
  }, [])

  const preloadNextVideo = useCallback(async () => {
    if (preloadingNext) return

    try {
      const res = await listVideos({ size: 50 })
      const candidates = res.items
        .map(mapVideo)
        .filter((v): v is MappedVideo => !!v && v.id !== videoId)
      if (candidates.length === 0) return
      const nextVideo = candidates[Math.floor(Math.random() * candidates.length)]
      if (!nextVideo) return
      if (preloadRequestRef.current === nextVideo.id) return

      if (import.meta.env.DEV) console.log('Preloading next video:', nextVideo.id)
      preloadRequestRef.current = nextVideo.id
      setPreloadingNext(true)
      preloadVideo(nextVideo.id)

      if (optimizeTimerRef.current) clearTimeout(optimizeTimerRef.current)
      optimizeTimerRef.current = setTimeout(optimizeMemory, 1000)
    } catch {
      // ignore
    }
  }, [preloadingNext, videoId, preloadVideo, optimizeMemory])

  const checkPreload = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    if (!isFinite(v.duration) || v.duration <= 0) return
    if (v.currentTime / v.duration >= PRELOAD_THRESHOLD) {
      preloadNextVideo()
    }
  }, [videoRef, preloadNextVideo])

  const resetPreload = useCallback(() => {
    setPreloadingNext(false)
    preloadRequestRef.current = null
  }, [])

  return {
    preloadingNext,
    preloadNextVideo,
    checkPreload,
    resetPreload,
    cleanupPreload,
  }
}
