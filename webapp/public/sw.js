/*
 * Atmos Web 运行时 Service Worker（手写原生实现，无构建依赖）
 *
 * ── 缓存策略 ──────────────────────────────────────────────────────────
 * | 请求类型                        | 策略                            |
 * |---------------------------------|---------------------------------|
 * | /webapp/assets/*（内容哈希）    | cache-first                     |
 * | 页面导航 mode === 'navigate'    | network-first + 离线回退外壳    |
 * | /webapp/ 下其它同源静态资源     | stale-while-revalidate          |
 *
 * 命中缓存后直接返回，绝不改写响应内容；只有 res.ok 的响应才会写入缓存。
 *
 * ── 版本升级 ──────────────────────────────────────────────────────────
 * 修改缓存策略或需要强制清空旧缓存时，把 SW_VERSION 加一（v1 → v2）。
 * sw.js 由后端 /webapp/ 路由以 no-cache 提供，浏览器每次更新检查都能拿到
 * 新文件内容；install 阶段 skipWaiting 立即接管，activate 阶段删除
 * CACHE_PREFIX 下不属于当前版本的旧缓存并 clients.claim。
 *
 * ── 为什么跳过 API / 媒体请求 ─────────────────────────────────────────
 * 只拦截「同源 + GET + 路径以 /webapp/ 开头」的请求，其余一律直接放行
 * （不调用 respondWith），因为：
 *   - /media /videos /playback：受 media_auth 鉴权、防盗链与限速保护，
 *     且以 Range 分段流式传输，缓存会绕过鉴权并污染播放器分段请求；
 *   - /auth /admin /chat /ws 等 API：响应随登录态与实时数据变化，缓存会
 *     造成越权或串号数据，WebSocket/SSE 也无法经 Cache API 处理；
 *   - 跨域请求：缓存会破坏第三方会话与凭证语义；
 *   - 任何带 Range 头的请求：分段响应（206）不可整体重用。
 */

const SW_VERSION = 'v1'
const CACHE_PREFIX = 'atmos-runtime-'
const CACHE_NAME = CACHE_PREFIX + SW_VERSION
const APP_SHELL_URL = '/webapp/'
const INDEX_URL = '/webapp/index.html'

self.addEventListener('install', (event) => {
  event.waitUntil(self.skipWaiting())
})

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key.startsWith(CACHE_PREFIX) && key !== CACHE_NAME)
            .map((key) => caches.delete(key)),
        ),
      )
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', (event) => {
  const request = event.request

  if (request.method !== 'GET') return

  let url
  try {
    url = new URL(request.url)
  } catch (_) {
    return
  }

  if (url.origin !== self.location.origin) return
  if (!url.pathname.startsWith('/webapp/')) return
  if (request.headers.has('Range')) return

  if (url.pathname.startsWith('/webapp/assets/')) {
    event.respondWith(cacheFirst(request))
    return
  }

  if (request.mode === 'navigate') {
    event.respondWith(networkFirstNavigation(request))
    return
  }

  event.respondWith(staleWhileRevalidate(request))
})

// 内容哈希资源：命中直接返回；未命中取网络，仅 res.ok 才写入缓存。
// 文件名随内容变化，无需后台更新检查。
async function cacheFirst(request) {
  const cache = await caches.open(CACHE_NAME)
  const cached = await cache.match(request)
  if (cached) return cached

  try {
    const response = await fetch(request)
    if (response.ok) {
      await cache.put(request, response.clone())
    }
    return response
  } catch (_) {
    return Response.error()
  }
}

// 导航：network-first；成功后同时以请求 URL 与 /webapp/ 外壳为键缓存两份，
// 任意深链访问过一次后离线都能打开应用外壳。失败依次回退请求自身缓存、
// 应用外壳、index.html，全部缺失时返回 Response.error()。
async function networkFirstNavigation(request) {
  const cache = await caches.open(CACHE_NAME)

  try {
    const response = await fetch(request)
    if (response.ok) {
      const shell = response.clone()
      await cache.put(request, response.clone()).catch(() => {})
      await cache.put(APP_SHELL_URL, shell).catch(() => {})
    }
    return response
  } catch (_) {
    const cached =
      (await caches.match(request)) ||
      (await caches.match(APP_SHELL_URL)) ||
      (await caches.match(INDEX_URL))
    return cached || Response.error()
  }
}

// manifest、favicon、图标等：命中先返回，后台 fetch 静默更新；未命中则
// 等待网络结果，res.ok 才写入缓存。
async function staleWhileRevalidate(request) {
  const cache = await caches.open(CACHE_NAME)
  const cached = await cache.match(request)

  const network = fetch(request)
    .then((response) => {
      if (response.ok) {
        return cache.put(request, response.clone()).then(() => response)
      }
      return response
    })
    .catch(() => undefined)

  if (cached) return cached

  const response = await network
  return response || Response.error()
}
