// 用户操作追踪 - 记录每一次点击和交互

import { request } from '../api/client'

interface TrackData {
  action: string
  target?: string
  page?: string
}

// 发送追踪事件
export async function track(data: TrackData) {
  try {
    await request('/admin/track', {
      method: 'POST',
      body: data,
      auth: true,
    })
  } catch {
    // 静默失败，不影响用户体验
  }
}

// 追踪按钮点击
export function trackClick(action: string, target?: string) {
  const page = window.location.pathname
  track({ action, target, page })
}

// 追踪页面访问
export function trackPage(page: string) {
  track({ action: '页面访问', page })
}

// 追踪视频操作
export function trackVideo(action: string, videoId: number | string) {
  track({ action, target: `视频#${videoId}`, page: window.location.pathname })
}

// 追踪用户操作
export function trackUser(action: string, target?: string) {
  track({ action, target, page: window.location.pathname })
}

// 自动追踪路由变化
export function initTrackRouter() {
  let lastPath = window.location.pathname

  // 监听 popstate
  window.addEventListener('popstate', () => {
    const newPath = window.location.pathname
    if (newPath !== lastPath) {
      trackPage(newPath)
      lastPath = newPath
    }
  })

  // 监听所有链接点击
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
