import { useEffect, useRef, useCallback } from 'react'
import { getToken } from '../api/client'

interface HlsPlayerOptions {
  videoRef: React.RefObject<HTMLVideoElement | null>
  src: string | null
  autoPlay?: boolean
}

// Load hls.js from CDN dynamically
async function loadHls(): Promise<any> {
  // Check if already loaded
  if ((window as any).Hls) {
    return (window as any).Hls
  }

  return new Promise((resolve, reject) => {
    const script = document.createElement('script')
    script.src = 'https://cdn.jsdelivr.net/npm/hls.js@latest'
    script.onload = () => {
      resolve((window as any).Hls)
    }
    script.onerror = reject
    document.head.appendChild(script)
  })
}

export function useHlsPlayer({ videoRef, src, autoPlay = false }: HlsPlayerOptions) {
  const hlsRef = useRef<any>(null)

  const destroyHls = useCallback(() => {
    if (hlsRef.current) {
      hlsRef.current.destroy()
      hlsRef.current = null
    }
  }, [])

  useEffect(() => {
    const video = videoRef.current
    if (!video || !src) {
      destroyHls()
      return
    }

    // Check if the source is an HLS stream
    const isHls = src.endsWith('.m3u8') || src.includes('/hls/')

    if (!isHls) {
      // Not HLS, use native playback
      destroyHls()
      video.src = src
      return
    }

    // Check native HLS support (Safari, iOS)
    if (video.canPlayType('application/vnd.apple.mpegurl')) {
      video.src = src
      if (autoPlay) {
        video.play().catch(() => {})
      }
      return
    }

    // Load hls.js for browsers without native support
    let cancelled = false

    loadHls().then((Hls) => {
      if (cancelled || !Hls || !video) return

      if (Hls.isSupported()) {
        const token = getToken()
        const hls = new Hls({
          enableWorker: true,
          maxBufferLength: 30,
          maxMaxBufferLength: 600,
          startLevel: -1, // Auto select quality
          xhrSetup: (xhr: XMLHttpRequest) => {
            // Add auth token to all HLS requests
            if (token) {
              xhr.setRequestHeader('Authorization', `Bearer ${token}`)
            }
          },
        } as any)

        hlsRef.current = hls
        hls.loadSource(src)
        hls.attachMedia(video)

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          if (autoPlay) {
            video.play().catch(() => {})
          }
        })

        hls.on(Hls.Events.ERROR, (_event: any, data: any) => {
          if (data.fatal) {
            switch (data.type) {
              case Hls.ErrorTypes.NETWORK_ERROR:
                console.error('HLS network error, attempting recovery...')
                hls.startLoad()
                break
              case Hls.ErrorTypes.MEDIA_ERROR:
                console.error('HLS media error, attempting recovery...')
                hls.recoverMediaError()
                break
              default:
                console.error('HLS fatal error, destroying...')
                destroyHls()
                break
            }
          }
        })
      }
    })

    return () => {
      cancelled = true
      destroyHls()
    }
  }, [videoRef, src, autoPlay, destroyHls])

  return {
    destroy: destroyHls,
    hls: hlsRef.current,
  }
}
