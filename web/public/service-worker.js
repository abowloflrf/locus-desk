const SHELL_CACHE = 'locus-shell-v1';
const APP_SHELL = '/';

self.addEventListener('install', (event) => {
  event.waitUntil(
    fetch(APP_SHELL, { cache: 'no-cache' })
      .then((response) => {
        if (!response.ok) throw new Error('Unable to cache the application shell');
        return caches.open(SHELL_CACHE).then((cache) => cache.put(APP_SHELL, response));
      })
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((key) => key !== SHELL_CACHE).map((key) => caches.delete(key))),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const requestUrl = new URL(event.request.url);
  if (event.request.mode !== 'navigate' || requestUrl.origin !== self.location.origin) return;

  event.respondWith(
    fetch(event.request)
      .then((response) => {
        if (response.ok) {
          const copy = response.clone();
          event.waitUntil(caches.open(SHELL_CACHE).then((cache) => cache.put(APP_SHELL, copy)));
        }
        return response;
      })
      .catch(() => caches.match(APP_SHELL)),
  );
});
