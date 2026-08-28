import { useEffect, useRef, useCallback } from 'react'
import type Hls from 'hls.js'
import { getToken } from '../api/client'

/**
 * HLS 播放器 Hook 的配置选项。
 *
 * @remarks
 * - `videoRef` 必须指向一个已挂载到 DOM 的 `<video>` 元素。
 * - `src` 为 `null` 时，Hook 会销毁已有的 HLS 实例并释放资源。
 * - 当 `src` 为 `.m3u8` 文件或包含 `/hls/` 路径时，自动走 HLS 播放流程；
 *   否则视为普通视频，直接赋值给 `video.src` 由浏览器原生播放。
 */
interface HlsPlayerOptions {
  /** 指向目标 `<video>` 元素的 React ref。 */
  videoRef: React.RefObject<HTMLVideoElement | null>
  /**
   * 视频资源的 URL。
   *
   * - 以 `.m3u8` 结尾或包含 `/hls/` 路径 → 视为 HLS 流，使用 hls.js 解码。
   * - 否则 → 直接赋值给 `<video>.src`，由浏览器原生播放。
   * - 传入 `null` → 销毁现有播放器实例，停止播放。
   */
  src: string | null
  /**
   * 是否在 manifest 加载完成后自动播放。
   *
   * @defaultValue `false`
   */
  autoPlay?: boolean
}

/**
 * 动态加载 hls.js 库。
 *
 * 从 jsDelivr CDN 注入 `<script>` 标签。如果 `window.Hls` 已存在
 * （即脚本已加载过），则直接返回缓存的引用，避免重复加载。
 *
 * @returns hls.js 的构造函数（`Hls` 类）。
 * @throws 如果脚本加载失败（网络错误等），Promise 会被 reject。
 */
let hlsLoadPromise: Promise<typeof Hls> | null = null

async function loadHls(): Promise<typeof Hls> {
  const win = window as unknown as { Hls?: typeof Hls }
  if (win.Hls) {
    return win.Hls
  }
  if (hlsLoadPromise) return hlsLoadPromise

  hlsLoadPromise = new Promise<typeof Hls>((resolve, reject) => {
    const script = document.createElement('script')
    // Pinned version to avoid supply-chain drift; update intentionally via PR
    script.src = 'https://cdn.jsdelivr.net/npm/hls.js@1.5.7/dist/hls.min.js'
    script.crossOrigin = 'anonymous'
    const timer = window.setTimeout(() => reject(new Error('hls.js load timeout')), 8000)
    script.onload = () => {
      window.clearTimeout(timer)
      resolve(win.Hls!)
    }
    script.onerror = () => {
      window.clearTimeout(timer)
      reject(new Error('hls.js CDN load failed'))
    }
    document.head.appendChild(script)
  }).catch(async (err) => {
    // CDN failed — caller will handle error; reset promise so next attempt can retry
    hlsLoadPromise = null
    throw err
  })

  return hlsLoadPromise!
}

/**
 * React Hook —— 为 `<video>` 元素提供 HLS 自适应码率播放能力。
 *
 * ## 功能概述
 *
 * 1. **HLS 自动识别**：当 `src` 是 `.m3u8` 文件或包含 `/hls/` 路径时，
 *    Hook 自动走 HLS 播放流程；否则直接赋值给 `<video>.src`，由浏览器原生播放。
 *
 * 2. **原生 HLS 优先**：在 Safari / iOS 等原生支持 HLS 的环境中，
 *    直接使用 `<video>` 的原生 `application/vnd.apple.mpegurl` 解码，不加载 hls.js。
 *
 * 3. **hls.js 动态加载**：在不支持原生 HLS 的浏览器中，
 *    从 jsDelivr CDN 动态注入 hls.js，首次加载后缓存在 `window.Hls`。
 *
 * 4. **认证支持**：通过 `xhrSetup` 将当前用户的 Bearer Token 注入到
 *    所有 HLS 分片请求的 `Authorization` 头中，确保受保护资源可正常加载。
 *
 * 5. **自动错误恢复**：
 *    - **网络错误** → 指数退避后重试（最多 3 次，间隔 `1s × 第 N 次`）。
 *    - **媒体错误** → 调用 `hls.recoverMediaError()` 自动恢复。
 *    - **未知致命错误** 或超过最大重试次数 → 销毁实例。
 *
 * 6. **自动清理**：当 `src` 变化或组件卸载时，自动销毁旧的 HLS 实例，释放资源。
 *
 * ## 使用方法
 *
 * ```tsx
 * import { useRef } from 'react'
 * import { useHlsPlayer } from '../hooks/useHlsPlayer'
 *
 * function VideoPlayer({ videoUrl }: { videoUrl: string }) {
 *   const videoRef = useRef<HTMLVideoElement>(null)
 *   const { destroy } = useHlsPlayer({
 *     videoRef,
 *     src: videoUrl,
 *     autoPlay: true,
 *   })
 *
 *   return (
 *     <div>
 *       <video ref={videoRef} controls style={{ width: '100%' }} />
 *       <button onClick={destroy}>停止播放</button>
 *     </div>
 *   )
 * }
 * ```
 *
 * ## 参数
 *
 * @param options - HLS 播放器配置选项。
 * @param options.videoRef - 指向目标 `<video>` 元素的 React ref（必须已挂载到 DOM）。
 * @param options.src - 视频资源 URL；`null` 表示停止播放并清理资源。
 * @param options.autoPlay - 是否在 manifest 解析完成后自动播放（默认 `false`）。
 *
 * @returns 包含以下字段的对象：
 *
 * | 字段 | 类型 | 说明 |
 * |------|------|------|
 * | `destroy` | `() => void` | 手动销毁 HLS 实例、释放资源的回调函数。 |
 * | `hls` | `Hls \| null` | 当前 hls.js 实例的引用；非 HLS 播放或已销毁时为 `null`。 |
 *
 * @example
 * ```tsx
 * // 基本用法：HLS 流播放
 * const videoRef = useRef<HTMLVideoElement>(null)
 * const { destroy } = useHlsPlayer({
 *   videoRef,
 *   src: 'https://example.com/stream/index.m3u8',
 * })
 *
 * // 普通 MP4 文件也兼容，会走原生播放
 * const { destroy } = useHlsPlayer({
 *   videoRef,
 *   src: 'https://example.com/video.mp4',
 * })
 *
 * // src 为 null 时自动清理
 * useHlsPlayer({ videoRef, src: null })
 * ```
 */
export function useHlsPlayer({ videoRef, src, autoPlay = false }: HlsPlayerOptions) {
  const hlsRef = useRef<Hls | null>(null)
  const retryCountRef = useRef<number>(0)
  const maxRetries = 3

  /**
   * 销毁当前 HLS 实例并重置重试计数器。
   *
   * @remarks
   * 调用后 `hlsRef.current` 变为 `null`，后续不会再触发任何
   * hls.js 事件监听。组件卸载和 `src` 变化时会自动调用此函数。
   */
  const destroyHls = useCallback(() => {
    if (hlsRef.current) {
      hlsRef.current.destroy()
      hlsRef.current = null
    }
    retryCountRef.current = 0
  }, [])

  useEffect(() => {
    const video = videoRef.current
    if (!video || !src) {
      destroyHls()
      return
    }

    const isHls = /\.m3u8(\?|$)/.test(src) || src.includes('/hls/')

    if (!isHls) {
      destroyHls()
      video.src = src
      return
    }

    if (video.canPlayType('application/vnd.apple.mpegurl') !== '') {
      video.src = src
      if (autoPlay) {
        video.play().catch(() => {})
      }
      return
    }

    let cancelled = false
    let retryTimer: ReturnType<typeof setTimeout> | null = null

    const clearRetryTimer = () => {
      if (retryTimer !== null) {
        clearTimeout(retryTimer)
        retryTimer = null
      }
    }

    loadHls().then((Hls) => {
      if (cancelled || !Hls || !video) return

      if (Hls.isSupported()) {
        const hlsConfig: Partial<Hls.Config> & { lowLatencyMode?: boolean } = {
          maxBufferLength: 15,
          maxMaxBufferLength: 60,
          startLevel: -1,
          startFragPrefetch: true,
          enableWorker: true,
          lowLatencyMode: false,
          xhrSetup: (xhr: XMLHttpRequest, _url: string) => {
            const t = getToken()
            if (t) {
              xhr.setRequestHeader('Authorization', `Bearer ${t}`)
            }
          },
        }
        const hls = new Hls(hlsConfig)

        hlsRef.current = hls
        hls.loadSource(src)
        hls.attachMedia(video)

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          if (autoPlay) {
            video.play().catch(() => {})
          }
        })

        hls.on(Hls.Events.ERROR, (_event: string, data: Hls.errorData) => {
          if (data.fatal) {
            retryCountRef.current++

            if (retryCountRef.current > maxRetries) {
              console.error(`HLS error: exceeded max retries (${maxRetries}), destroying...`)
              destroyHls()
              return
            }

            switch (data.type) {
              case Hls.ErrorTypes.NETWORK_ERROR:
                console.warn(`HLS network error (attempt ${retryCountRef.current}/${maxRetries}), recovering...`)
                clearRetryTimer()
                retryTimer = setTimeout(() => {
                  retryTimer = null
                  if (hlsRef.current) {
                    hls.startLoad()
                  }
                }, 1000 * retryCountRef.current)
                break
              case Hls.ErrorTypes.MEDIA_ERROR:
                console.warn(`HLS media error (attempt ${retryCountRef.current}/${maxRetries}), recovering...`)
                hls.recoverMediaError()
                break
              default:
                console.error('HLS fatal error, destroying...')
                destroyHls()
                break
            }
          }
        })
      } else {
        console.warn('HLS not supported in this browser, falling back to native playback')
        video.src = src
      }
    }).catch((err) => {
      console.error('Failed to load hls.js', err)
      if (!cancelled) video.src = src
    })

    return () => {
      cancelled = true
      clearRetryTimer()
      destroyHls()
    }
  }, [videoRef, src, autoPlay, destroyHls])

  return {
    destroy: destroyHls,
  }
}
