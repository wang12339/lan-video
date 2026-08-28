import { useState, useEffect, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import {
  getVideo, mapVideo, listVideos, incrementViews, getSimilarVideos,
  getShareVideo, APIError,
} from '../../../api'
import { request, mediaUrl } from '../../../api/client'
import type { MappedVideo, VideoVariant } from '../../../api/types'
import { useAuth } from '../../../context/AuthContext'
import { MAX_RELATED_VIDEOS } from '../constants'

export type VideoDataErrorType = 'network' | 'auth' | 'not_found' | 'timeout' | 'unknown'

export interface VideoDataError {
  type: VideoDataErrorType
  message: string
}

function isAPIError(e: unknown): e is APIError {
  return typeof e === 'object' && e !== null && 'status' in e && typeof (e as { status: unknown }).status === 'number' && 'message' in e
}

function classifyDataError(e: unknown, fallbackMsg: string): VideoDataError {
  if (isAPIError(e)) {
    const status = (e as { status: number }).status
    const msg = (e as { message?: string }).message || fallbackMsg
    if (status === 0) return { type: 'network', message: msg }
    if (status === 401 || status === 403) return { type: 'auth', message: msg }
    if (status === 404 || status === 410) return { type: 'not_found', message: msg }
    if (status === 408 || msg.includes('timeout')) return { type: 'timeout', message: msg }
    return { type: 'unknown', message: msg }
  }
  if (e instanceof Error) {
    if (e.name === 'AbortError' || e.message?.includes('timeout')) return { type: 'timeout', message: e.message || fallbackMsg }
    return { type: 'network', message: e.message || fallbackMsg }
  }
  return { type: 'unknown', message: fallbackMsg }
}

function isValidMediaUrl(url: string): boolean {
  if (!url) return false
  try {
    const parsed = new URL(url, window.location.origin)
    const protocol = parsed.protocol
    return protocol === 'http:' || protocol === 'https:' || protocol === 'blob:'
  } catch {
    return false
  }
}

export interface UseVideoDataReturn {
  video: MappedVideo | null
  setVideo: React.Dispatch<React.SetStateAction<MappedVideo | null>>
  loading: boolean
  error: string
  setError: (v: string) => void
  related: MappedVideo[]
  variants: VideoVariant[]
  setVariants: React.Dispatch<React.SetStateAction<VideoVariant[]>>
  hlsUrl: string | null
  setHlsUrl: (v: string | null) => void
  loadRelated: (category?: string) => Promise<void>
}

export function useVideoData(
  videoId: string,
  isShared: boolean,
  shareToken: string | null,
  startSession: () => void,
): UseVideoDataReturn {
  const { t } = useTranslation()
  const { user } = useAuth()

  const [video, setVideo] = useState<MappedVideo | null>(() => null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [related, setRelated] = useState<MappedVideo[]>(() => [])
  const [variants, setVariants] = useState<VideoVariant[]>(() => [])
  const [hlsUrl, setHlsUrl] = useState<string | null>(() => null)
  const lastVideoIdRef = useRef(videoId)

  const safeSetVideo = useCallback((updater: React.SetStateAction<MappedVideo | null>) => {
    setVideo(prev => {
      const next = typeof updater === 'function' ? updater(prev) : updater
      if (prev && next && prev.id === next.id
        && prev.title === next.title
        && prev.stream === next.stream
        && prev.progress === next.progress) {
        return prev
      }
      return next
    })
  }, [])

  const loadRelated = useCallback(async (category?: string) => {
    if (isShared) return
    try {
      const recommended = await getSimilarVideos(videoId)
      const items = recommended.filter(v => v.id !== videoId).slice(0, MAX_RELATED_VIDEOS)
      if (items.length > 0) { setRelated(items); return }
    } catch { /* fall back */ }
    try {
      const r = await listVideos({ type: 'local_video', size: 20, category })
      const items = r.items.map(mapVideo).filter((v): v is MappedVideo => !!v && v.id !== videoId).slice(0, MAX_RELATED_VIDEOS)
      setRelated(items)
    } catch { /* ignore */ }
  }, [videoId, isShared])

  useEffect(() => {
    let cancelled = false

    if (isShared && shareToken) {
      const load = async () => {
        setLoading(true)
        try {
          const sv = await getShareVideo(shareToken)
          if (cancelled) return
          const thumbUrl = sv.thumbUrl ? mediaUrl(sv.thumbUrl) : null
          const streamUrl = mediaUrl(sv.streamUrl)
          if (streamUrl && !isValidMediaUrl(streamUrl)) {
            setError(t('player.shareInvalid'))
            setLoading(false)
            return
          }
          const mv: MappedVideo = {
            id: sv.id, title: sv.title, category: sv.category,
            description: sv.description || '', thumb: thumbUrl,
            thumbnail_url: thumbUrl || '', stream: streamUrl,
            cover: null,
            sourceType: (sv.sourceType ?? 'local_video') as 'local_video' | 'external',
            duration: 0, views: 0, date: '', progress: 0,
          }
          safeSetVideo(mv)
          document.title = mv.title + ' · ATMOS'
        } catch (e) {
          if (cancelled) return
          const classified = classifyDataError(e, t('player.shareInvalid'))
          setError(classified.message)
        } finally {
          if (!cancelled) setLoading(false)
        }
      }
      load()
      return () => { cancelled = true }
    }

    if (!videoId) { setError(t('errors.missingVideoId')); setLoading(false); return }
    if (!user && !isShared) {
      setError(t('player.loginRequired'))
      setLoading(false)
      return
    }

    if (lastVideoIdRef.current !== videoId) {
      safeSetVideo(null)
      setRelated([])
      setVariants([])
      setHlsUrl(null)
      setError('')
      lastVideoIdRef.current = videoId
    }

    const load = async () => {
      setLoading(true)
      try {
        startSession()
        const v = await getVideo(videoId)
        const mv = mapVideo(v)
        if (cancelled || !mv) return

        if (mv.stream && !isValidMediaUrl(mv.stream)) {
          setError(t('errors.loadFailed'))
          setLoading(false)
          return
        }

        safeSetVideo(mv)
        const cleanTitle = mv.title.replace(/\.[^.]+$/, '').replace(/_/g, ' ').replace(/\s+/g, ' ').trim() || mv.title
        document.title = cleanTitle + ' · ATMOS'
        incrementViews(videoId).catch(() => {})
        loadRelated(mv.category)

        try {
          const hlsRes = await request<{ status: string; masterUrl?: string }>(`/videos/${videoId}/hls`)
          if (!cancelled && hlsRes.status === 'ready' && hlsRes.masterUrl) {
            if (isValidMediaUrl(hlsRes.masterUrl)) {
              setHlsUrl(hlsRes.masterUrl)
            }
          }
        } catch { /* HLS not available */ }

        if (v.hasVariants) {
          try {
            const res = await request<Array<{ resolution: string; url: string; fileSize?: number; bitrate?: number }>>(`/videos/${videoId}/variants`)
            if (!cancelled && Array.isArray(res)) {
              setVariants(res.filter(r => r.resolution && r.url && isValidMediaUrl(mediaUrl(r.url) || '')).map(r => ({
                resolution: r.resolution, filePath: r.url,
                fileSize: r.fileSize ?? 0, bitrate: r.bitrate,
              })))
            }
          } catch { /* ignore */ }
        }
      } catch (e) {
        if (!cancelled) {
          const classified = classifyDataError(e, t('errors.loadFailed'))
          setError(classified.message)
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    load()
    return () => { cancelled = true }
  }, [videoId, isShared, shareToken, user, startSession, t, loadRelated, safeSetVideo])

  return {
    video, setVideo: safeSetVideo as React.Dispatch<React.SetStateAction<MappedVideo | null>>,
    loading, error, setError,
    related, variants, setVariants, hlsUrl, setHlsUrl,
    loadRelated,
  }
}
