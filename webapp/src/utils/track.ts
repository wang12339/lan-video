import { request, getToken } from '../api/client'

interface TrackData {
  action: string
  target?: string
  page?: string
}

export function track(data: TrackData): Promise<void> {
  if (!getToken()) return Promise.resolve()
  return request('/admin/track', {
    method: 'POST',
    body: data,
    auth: true,
    silent: true,
  }).then(() => {}).catch(() => {})
}

export function trackClick(action: string, target?: string): void {
  track({ action, target, page: window.location.pathname })
}

export function trackPerf(metric: string, value: string): void {
  track({ action: `perf.${metric}`, target: value, page: window.location.pathname })
}

let lastReportedPage: string | null = null
export function trackPage(page: string): void {
  if (lastReportedPage === page) return
  lastReportedPage = page
  track({ action: '页面访问', page })
}

export function trackVideo(action: string, videoId: number | string): void {
  track({ action, target: `视频#${videoId}`, page: window.location.pathname })
}

let routerInitialized = false
export function initTrackRouter(): void {
  if (routerInitialized) return
  routerInitialized = true

  // 首次访问（main.tsx 在应用挂载前调用）
  trackPage(window.location.pathname)

  // SPA 的 pushState/replaceState 导航不触发任何原生事件，也不再猴补丁
  // history.pushState（脆弱且与 React Router 内部实现耦合）——由 Layout 在
  // 路由变化时主动调用 trackPage；此处只负责浏览器后退/前进。
  window.addEventListener('popstate', () => {
    trackPage(window.location.pathname)
  })

  document.addEventListener('click', (e) => {
    const link = (e.target as HTMLElement).closest('a[href]')
    if (link) {
      const href = link.getAttribute('href')
      if (href && href.startsWith('/') && !href.startsWith('/webapp')) {
        trackClick('导航', href)
      }
    }
  })
}
