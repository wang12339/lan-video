// Service Worker for LAN Video Player
// Cache the app shell on install, network-first for API, cache-first for media

const CACHE_NAME = 'lan-video-v1';
const SHELL_URLS = [
  '/',
  '/index.html',
  '/css/style.css',
  '/js/main.js',
  '/js/api.js',
  '/js/config.js',
  '/js/state.js',
  '/js/dom.js',
  '/js/home.js',
  '/js/search.js',
  '/js/player.js',
  '/js/navigation.js',
  '/js/drawer.js',
  '/js/connection.js',
  '/js/icons.js',
  '/js/keyboard.js',
  '/js/settings.js',
  '/js/upload.js',
  '/js/slideshow.js',
  '/js/image-viewer.js',
  '/js/access-gate.js',
  '/js/utils.js',
  '/manifest.json',
  '/icons/icon.svg',
];

// Install: cache the app shell
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      return cache.addAll(SHELL_URLS);
    })
  );
  self.skipWaiting();
});

// Activate: clean old caches
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) => {
      return Promise.all(
        keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k))
      );
    })
  );
  self.clients.claim();
});

// Fetch: network-first for API, cache-first for static assets
self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // API calls: network-first, fallback to cache
  if (url.pathname.startsWith('/videos') ||
      url.pathname.startsWith('/playback') ||
      url.pathname.startsWith('/auth') ||
      url.pathname.startsWith('/admin') ||
      url.pathname.startsWith('/server/')) {
    event.respondWith(networkFirst(event.request));
    return;
  }

  // Media files: cache-first
  if (url.pathname.startsWith('/media/')) {
    event.respondWith(cacheFirst(event.request));
    return;
  }

  // App shell: cache-first (stale-while-revalidate style)
  event.respondWith(
    caches.match(event.request).then((cached) => {
      if (cached) return cached;
      return fetch(event.request).then((response) => {
        if (response && response.status === 200) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
        }
        return response;
      }).catch(() => new Response('Offline', { status: 503 }));
    })
  );
});

async function networkFirst(request) {
  try {
    const response = await fetch(request);
    if (response && response.status === 200) {
      const clone = response.clone();
      caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
    }
    return response;
  } catch (e) {
    const cached = await caches.match(request);
    return cached || new Response(JSON.stringify({ error: 'offline' }), {
      status: 503,
      headers: { 'Content-Type': 'application/json' },
    });
  }
}

async function cacheFirst(request) {
  const cached = await caches.match(request);
  if (cached) return cached;
  try {
    const response = await fetch(request);
    if (response && response.status === 200) {
      const clone = response.clone();
      caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
    }
    return response;
  } catch (e) {
    return new Response('Offline', { status: 503 });
  }
}
