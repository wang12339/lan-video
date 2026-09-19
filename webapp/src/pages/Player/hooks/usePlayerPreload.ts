import { useState, useCallback, useEffect, useRef } from 'react'
import { usePreloadManager, useMemoryManager } from '../usePlayerHooks'
import { listVideos, mapVideo } from '../../../api'
import type { MappedVideo } from '../../../api/types'
import { PRELOAD_THRESHOLD } from '../constants'

export function usePlayerPreload(
  videoId: string,
  related: MappedVideo[],
  videoRef: React.RefObject<HTMLVideoElement | null>,
) {
  const [preloadingNext, setPreloadingNext] = useState(false)
  const { preloadVideo, cleanup: cleanupPreload } = usePreloadManager()
  const { optimizeMemory } = useMemoryManager()
  const preloadRequestRef = useRef<string | null>(null)
  const fallbackAttemptedRef = useRef(false)
  const optimizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    setPreloadingNext(false)
    preloadRequestRef.current = null
    fallbackAttemptedRef.current = false
    if (optimizeTimerRef.current) { clearTimeout(optimizeTimerRef.current); optimizeTimerRef.current = null }
    cleanupPreload()
  }, [videoId, cleanupPreload])

  useEffect(() => {
    return () => {
      if (optimizeTimerRef.current) clearTimeout(optimizeTimerRef.current)
    }
  }, [])

  const commitPreload = useCallback((nextVideo: MappedVideo) => {
    preloadRequestRef.current = nextVideo.id
    setPreloadingNext(true)
    preloadVideo(nextVideo.id)
    if (optimizeTimerRef.current) clearTimeout(optimizeTimerRef.current)
    optimizeTimerRef.current = setTimeout(optimizeMemory, 1000)
  }, [preloadVideo, optimizeMemory])

  const preloadNextVideo = useCallback(async () => {
    // 优先预取 related 中第一个与当前视频不同的视频（即真正可能接着播放的视频）
    const relatedNext = related.find((v) => v.id !== videoId)
    if (relatedNext) {
      if (preloadRequestRef.current === relatedNext.id) return
      if (import.meta.env.DEV) console.log('Preloading related video:', relatedNext.id)
      commitPreload(relatedNext)
      return
    }

    // related 为空时回退到列表接口，仅尝试一次，避免播放中反复触发额外请求
    if (fallbackAttemptedRef.current) return
    fallbackAttemptedRef.current = true

    try {
      const res = await listVideos({ size: 50 })
      const nextVideo = res.items
        .map(mapVideo)
        .find((v): v is MappedVideo => !!v && v.id !== videoId)
      if (!nextVideo) return
      if (preloadRequestRef.current === nextVideo.id) return

      if (import.meta.env.DEV) console.log('Preloading next video:', nextVideo.id)
      commitPreload(nextVideo)
    } catch {
      // ignore
    }
  }, [videoId, related, commitPreload])

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
    fallbackAttemptedRef.current = false
  }, [])

  return {
    preloadingNext,
    preloadNextVideo,
    checkPreload,
    resetPreload,
    cleanupPreload,
  }
}
