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

  trackPage(window.location.pathname)

  const originalPushState = history.pushState.bind(history)
  history.pushState = (data: unknown, unused: string, url?: string | URL | null) => {
    originalPushState(data, unused, url)
    trackPage(window.location.pathname)
  }

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
