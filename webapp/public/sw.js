const CACHE_VERSION = 'v5'
const CACHE_NAME = 'atmos-' + CACHE_VERSION
const STATIC_ASSETS = [
  '/webapp/',
  '/webapp/index.html',
  '/webapp/manifest.json'
]

// 安装事件 - 缓存静态资源
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll(STATIC_ASSETS))
      .then(() => self.skipWaiting())
  )
})

// 激活事件 - 清理旧缓存
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys()
      .then(cacheNames => {
        return Promise.all(
          cacheNames
            .filter(name => name !== CACHE_NAME)
            .map(name => caches.delete(name))
        )
      })
      .then(() => self.clients.claim())
  )
})

// 请求拦截 - 网络优先策略
self.addEventListener('fetch', (event) => {
  const { request } = event
  const url = new URL(request.url)

  // 只处理同源请求
  if (url.origin !== location.origin) return

  // manifest.json - 网络优先，确保总是获取最新版本
  if (url.pathname === '/webapp/manifest.json') {
    event.respondWith(
      fetch(request)
        .then(response => {
          // 缓存成功的响应
          if (request.method === 'GET' && response.status === 200) {
            const responseClone = response.clone()
            caches.open(CACHE_NAME)
              .then(cache => cache.put(request, responseClone))
          }
          return response
        })
        .catch(() => {
          // 离线时返回缓存
          return caches.match(request)
        })
    )
    return
  }

  // API请求 - 网络优先
  if (url.pathname.startsWith('/videos') || 
      url.pathname.startsWith('/auth') ||
      url.pathname.startsWith('/admin')) {
    event.respondWith(
      fetch(request)
        .then(response => {
          // 缓存成功的GET请求
          if (request.method === 'GET' && response.status === 200) {
            const responseClone = response.clone()
            caches.open(CACHE_NAME)
              .then(cache => cache.put(request, responseClone))
          }
          return response
        })
        .catch(() => {
          // 离线时返回缓存
          return caches.match(request)
        })
    )
    return
  }

  // 静态资源 - 缓存优先
  if (url.pathname.startsWith('/webapp/assets/')) {
    event.respondWith(
      caches.match(request)
        .then(cached => {
          if (cached) return cached
          return fetch(request)
            .then(response => {
              const responseClone = response.clone()
              caches.open(CACHE_NAME)
                .then(cache => cache.put(request, responseClone))
              return response
            })
        })
    )
    return
  }

  // 其他请求 - 网络优先
  event.respondWith(
    fetch(request)
      .catch(() => caches.match(request))
  )
})
