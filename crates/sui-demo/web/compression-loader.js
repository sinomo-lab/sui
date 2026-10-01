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

  // Startup waits on the Wasm module and the web fonts, so the loading screen
  // reports their combined download progress. Totals come from the manifest's
  // decoded sizes, which stay correct when the server or cache applies an
  // encoding; without a manifest they fall back to an unencoded Content-Length.
  const progress = {
    assets: new Map(),
    done: false,
  };

  const progressSnapshot = () => {
    let loaded = 0;
    let total = 0;
    let known = true;
    for (const asset of progress.assets.values()) {
      loaded += asset.loaded;
      total += asset.total;
      known &&= asset.total > 0;
    }
    return { loaded, total: known ? total : 0, done: progress.done };
  };

  const publishProgress = () => {
    window.dispatchEvent(
      new CustomEvent("sui-load-progress", { detail: progressSnapshot() }),
    );
  };

  const expectAsset = (path, total) => {
    const asset = progress.assets.get(path);
    if (asset) {
      asset.total ||= total;
    } else {
      progress.assets.set(path, { loaded: 0, total });
    }
  };

  const trackDownload = (path, response, expectedBytes) => {
    if (!response.ok || !response.body) {
      return response;
    }
    const length = Number(response.headers.get("Content-Length"));
    const encoded = Boolean(response.headers.get("Content-Encoding"));
    expectAsset(path, expectedBytes || (!encoded && length > 0 ? length : 0));
    const asset = progress.assets.get(path);
    asset.loaded = 0;
    publishProgress();

    const reader = response.body.getReader();
    const body = new ReadableStream({
      async pull(controller) {
        try {
          const { done, value } = await reader.read();
          if (done) {
            asset.total = Math.max(asset.total, asset.loaded);
            publishProgress();
            controller.close();
            return;
          }
          asset.loaded += value.byteLength;
          publishProgress();
          controller.enqueue(value);
        } catch (error) {
          controller.error(error);
        }
      },
      cancel(reason) {
        return reader.cancel(reason);
      },
    });
    return new Response(body, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });
  };

  void manifestPromise.then((manifest) => {
    for (const [path, entry] of Object.entries(manifest?.assets ?? {})) {
      if (/\.(wasm|otf|ttf)$/.test(path)) {
        expectAsset(path, entry.bytes);
      }
    }
    publishProgress();
  });

  window.addEventListener("sui-app-ready", () => {
    progress.done = true;
    publishProgress();
  }, { once: true });

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
      const entry = manifest?.assets?.[path];
      if (!entry) {
        return nativeFetch(input, init).then((response) =>
          trackDownload(path, response, 0),
        );
      }
      return findCached(requestUrl.href)
        .then((cached) => {
          if (cached) {
            state.encodings[path] = "cache";
            return cached;
          }
          return fetchAsset(input, init, requestUrl.href, path, manifest);
        })
        .then((response) => trackDownload(path, response, entry.bytes));
    });
  };

  window.__suiCompressedAssets = {
    manifest: () => manifestPromise,
    snapshot: () => JSON.parse(JSON.stringify(state)),
    whenCached: () => Promise.all([...pendingWrites]),
    whenWorkerReady: () => workerReady,
    progress: progressSnapshot,
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
