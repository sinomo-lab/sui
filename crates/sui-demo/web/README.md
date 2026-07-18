SUI Demo web build

Trunk is integrated for the browser build.

Development server

  cd crates/sui-demo/web
  trunk serve

Or from the workspace root:

  trunk serve --config crates/sui-demo/web/Trunk.toml

Production build

  trunk build --config crates/sui-demo/web/Trunk.toml --release
  node crates/sui-demo/web/prepare-dist.mjs crates/sui-demo/web/dist

Output goes to:

  crates/sui-demo/web/dist

The preparation step writes Brotli and gzip sidecars for compressible assets,
`compression-manifest.json`, and an `_headers` file for static hosts that support
the Cloudflare Pages/Netlify header format. Fingerprinted application assets and
versioned font payloads receive a one-year immutable cache lifetime on those
hosts, while HTML and the manifest remain revalidated.

GitHub Pages always controls its own response headers. It currently serves gzip
but does not negotiate the generated Brotli sidecars or allow a custom cache
lifetime. The browser loader therefore retains the server-decoded Wasm and font
responses in a revisioned Cache Storage entry. Pages supplies gzip on the first
load, while later loads avoid its ten-minute cache limit. Static hosts that
support `_headers` and Brotli negotiation use the immutable response rules and
the generated `.br` sidecars directly. The Pages workflow runs the preparation
step automatically before uploading the artifact.

After startup, `asset-cache-worker.js` adds the fingerprinted JavaScript module
to that same revisioned cache and serves it on later visits. HTML, the
compression manifest, and the worker itself remain revalidated so deployments
can advance without pinning the entry point.

Trunk's Wasm preload remains active for parallel cold-start fetching. On later
visits the service worker satisfies that preload from the same revisioned cache,
so it does not bypass the long-lived cache or start a network transfer.

Benchmark mode

The web build can launch focused benchmark surfaces by query string:

  http://127.0.0.1:8080/?benchmark=retained-text
  http://127.0.0.1:8080/?benchmark=text-editing
  http://127.0.0.1:8080/?benchmark=text-comparison
  http://127.0.0.1:8080/?benchmark=widget-book
  http://127.0.0.1:8080/?benchmark=dev

The development workspace can open a current feature surface directly:

  http://127.0.0.1:8080/?benchmark=dev&demo=rich-documents
  http://127.0.0.1:8080/?benchmark=dev&demo=layout
  http://127.0.0.1:8080/?benchmark=dev&demo=commands

Optional tuning parameters:

  ?benchmark=retained-text&warmup=60&frames=180

Behavior:
- the Rust app selects a focused benchmark surface from the query string
- `benchmark=dev&demo=...` selects a development card without navigating the launcher
- `text-comparison` opens the side-by-side text rendering checklist added for grayscale, hinted, darkened, and LCD validation
- the page runs a requestAnimationFrame benchmark after startup
- results are written into the page overlay and also logged to the browser console as:

  SUI_BENCHMARK_RESULT { ...json... }

Notes:
- Trunk builds ../Cargo.toml as the Rust/WASM asset.
- The config enables --no-default-features plus the web feature.
- `compression-loader.js` is inactive when no compression manifest is present,
  so normal `trunk serve` development keeps using the original assets.
- The watch config includes the `sinomo-ui-demo` package and the workspace root so edits in the Rust sources trigger rebuilds.
