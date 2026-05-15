// Service Worker for LAN Video Player
// Network-first for API, passthrough for everything else

const CACHE_NAME = 'lan-video-v1';

self.addEventListener('install', (event) => {
  self.skipWaiting();
});

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

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // API calls: network-first, fallback to cached response
  if (url.pathname.startsWith('/videos') ||
      url.pathname.startsWith('/playback') ||
      url.pathname.startsWith('/auth') ||
      url.pathname.startsWith('/admin') ||
      url.pathname.startsWith('/server/')) {
    event.respondWith(networkFirst(event.request));
    return;
  }

  // Everything else (media, webapp static files): network only
  // Media files need Range headers which don't work with cache.
  // Static files are served by the backend with proper cache headers.
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
