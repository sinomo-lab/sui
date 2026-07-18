"use strict";

const CACHE_PREFIX = "sui-demo-assets-";
const CACHEABLE_PATH = /\/sinomo-ui-demo-[^/]+\.(?:js|wasm)$/;

async function cachedResponse(request) {
  const names = await caches.keys();
  for (const name of names.reverse()) {
    if (!name.startsWith(CACHE_PREFIX)) {
      continue;
    }
    const cache = await caches.open(name);
    const response = await cache.match(request, { ignoreVary: true });
    if (response) {
      return response;
    }
  }
  return null;
}

self.addEventListener("install", () => {
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);
  if (
    event.request.method !== "GET" ||
    url.origin !== self.location.origin ||
    !CACHEABLE_PATH.test(url.pathname)
  ) {
    return;
  }

  event.respondWith(
    cachedResponse(event.request).then(
      (response) => response ?? fetch(event.request),
    ),
  );
});

self.addEventListener("message", (event) => {
  if (event.data?.type !== "warm" || !event.data.revision) {
    return;
  }
  const reply = event.ports?.[0];
  const work = (async () => {
    const cache = await caches.open(`${CACHE_PREFIX}${event.data.revision}`);
    for (const asset of event.data.assets ?? []) {
      const url = new URL(asset, self.registration.scope);
      if (!CACHEABLE_PATH.test(url.pathname) || (await cache.match(url.href))) {
        continue;
      }
      const response = await fetch(url.href);
      if (response.ok && response.status === 200) {
        await cache.put(url.href, response);
      }
    }
  })();
  event.waitUntil(
    work.finally(() => {
      reply?.postMessage({ ready: true });
    }),
  );
});
