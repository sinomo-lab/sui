(() => {
  "use strict";

  if (window.__suiCompressedAssetLoaderInstalled) {
    return;
  }
  window.__suiCompressedAssetLoaderInstalled = true;

  const CACHE_PREFIX = "sui-demo-assets-";
  const nativeFetch = window.fetch.bind(window);
  const baseUrl = new URL(".", document.baseURI);
  const manifestUrl = new URL("compression-manifest.json", baseUrl);
  const state = {
    enabled: false,
    revision: null,
    cacheHits: 0,
    cacheMisses: 0,
    encodings: {},
  };
  const pendingWrites = new Set();
  let resolveWorkerReady;
  const workerReady = new Promise((resolve) => {
    resolveWorkerReady = resolve;
  });

  const manifestPromise = nativeFetch(manifestUrl, {
    cache: "no-store",
    credentials: "same-origin",
  })
    .then((response) => (response.ok ? response.json() : null))
    .then((manifest) => {
      if (!manifest || manifest.version !== 1 || !manifest.assets) {
        return null;
      }
      state.enabled = true;
      state.revision = manifest.revision;
      return manifest;
    })
    .catch(() => null);

  const relativeAssetPath = (url) => {
    if (url.origin !== baseUrl.origin || !url.pathname.startsWith(baseUrl.pathname)) {
      return null;
    }
    return decodeURIComponent(url.pathname.slice(baseUrl.pathname.length));
  };

  const assetCaches = async () => {
    if (!("caches" in window)) {
      return [];
    }
    const names = await caches.keys();
    return Promise.all(
      names
        .filter((name) => name.startsWith(CACHE_PREFIX))
        .reverse()
        .map((name) => caches.open(name)),
    );
  };

  const findCached = async (requestUrl) => {
    for (const cache of await assetCaches()) {
      const response = await cache.match(requestUrl, { ignoreVary: true });
      if (response) {
        state.cacheHits += 1;
        return response;
      }
    }
    state.cacheMisses += 1;
    return null;
  };

  const currentCache = async (manifest) => {
    if (!("caches" in window)) {
      return null;
    }
    return caches.open(`${CACHE_PREFIX}${manifest.revision}`);
  };

  const fetchAsset = async (input, init, requestUrl, path, manifest) => {
    const response = await nativeFetch(input, init);
    if (response.ok && response.status === 200) {
      const cache = await currentCache(manifest);
      if (cache) {
        const write = cache.put(requestUrl, response.clone()).catch(() => {});
        pendingWrites.add(write);
        void write.finally(() => pendingWrites.delete(write));
      }
      state.encodings[path] = response.headers.get("Content-Encoding") || "server";
    }
    return response;
  };

  window.fetch = (input, init) => {
    const method = String(
      init?.method ?? (input instanceof Request ? input.method : "GET"),
    ).toUpperCase();
    if (method !== "GET" || init?.cache === "no-store") {
      return nativeFetch(input, init);
    }

    let requestUrl;
    try {
      requestUrl = new URL(
        input instanceof Request ? input.url : String(input),
        document.baseURI,
      );
    } catch (_error) {
      return nativeFetch(input, init);
    }

    const path = relativeAssetPath(requestUrl);
    if (!path || !/\.(wasm|otf|ttf)$/.test(path)) {
      return nativeFetch(input, init);
    }

    return manifestPromise.then((manifest) => {
      if (!manifest?.assets?.[path]) {
        return nativeFetch(input, init);
      }
      return findCached(requestUrl.href).then((cached) => {
        if (cached) {
          state.encodings[path] = "cache";
          return cached;
        }
        return fetchAsset(input, init, requestUrl.href, path, manifest);
      });
    });
  };

  window.__suiCompressedAssets = {
    manifest: () => manifestPromise,
    snapshot: () => JSON.parse(JSON.stringify(state)),
    whenCached: () => Promise.all([...pendingWrites]),
    whenWorkerReady: () => workerReady,
  };

  window.addEventListener(
    "TrunkApplicationStarted",
    async () => {
      if (!("serviceWorker" in navigator)) {
        resolveWorkerReady(null);
        return;
      }
      try {
        const manifest = await manifestPromise;
        if (!manifest) {
          resolveWorkerReady(null);
          return;
        }
        const workerUrl = new URL("asset-cache-worker.js", baseUrl);
        const registration = await navigator.serviceWorker.register(workerUrl, {
          scope: baseUrl.pathname,
        });
        const readyRegistration = await navigator.serviceWorker.ready;
        const worker =
          readyRegistration.active ?? registration.active ?? registration.waiting;
        if (!worker) {
          resolveWorkerReady(null);
          return;
        }

        const assets = Object.keys(manifest.assets).filter((path) =>
          /^sinomo-ui-demo-[^/]+\.js$/.test(path),
        );
        const channel = new MessageChannel();
        channel.port1.onmessage = () => resolveWorkerReady(registration);
        worker.postMessage(
          { type: "warm", revision: manifest.revision, assets },
          [channel.port2],
        );
      } catch (_error) {
        resolveWorkerReady(null);
      }
    },
    { once: true },
  );
})();
