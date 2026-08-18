// 用户操作追踪 - 记录每一次点击和交互

import { request, getToken } from '../api/client'

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
      // 无内存 token 时走 cookie 认证；未登录用户的 401 不触发全局"会话失效"流程
      auth: getToken() !== null,
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

// 追踪页面访问（页面级去重：同一路径的连续"页面访问"只上报一次，
// 避免 pushState 拦截与 popstate、重复初始化等场景重复上报同一事件）
let lastReportedPage: string | null = null
export function trackPage(page: string) {
  if (lastReportedPage === page) return
  lastReportedPage = page
  track({ action: '页面访问', page })
}

// 追踪视频操作
export function trackVideo(action: string, videoId: number | string) {
  track({ action, target: `视频#${videoId}`, page: window.location.pathname })
}

// 自动追踪路由变化（幂等：重复调用不会挂重复监听器导致事件翻倍）
let routerInitialized = false
export function initTrackRouter() {
  if (routerInitialized) return
  routerInitialized = true

  // 记录首屏页面访问
  trackPage(window.location.pathname)

  // 覆盖 pushState：SPA 前进导航（react-router 的 Link/useNavigate）同样上报
  // 页面访问；与 popstate（后退/前进）互补，重复路径由 trackPage 去重
  const originalPushState = history.pushState.bind(history)
  history.pushState = (data: unknown, unused: string, url?: string | URL | null) => {
    originalPushState(data, unused, url)
    trackPage(window.location.pathname)
  }

  // 监听 popstate
  window.addEventListener('popstate', () => {
    trackPage(window.location.pathname)
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
