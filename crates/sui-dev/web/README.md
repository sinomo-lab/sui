SUI Dev web build

Trunk is integrated for the browser build.

Development server

  cd crates/sui-dev/web
  trunk serve

Or from the workspace root:

  trunk serve --config crates/sui-dev/web/Trunk.toml

Production build

  trunk build --config crates/sui-dev/web/Trunk.toml --release

Output goes to:

  crates/sui-dev/web/dist

Benchmark mode

The web build can launch focused benchmark surfaces by query string:

  http://127.0.0.1:8080/?benchmark=button-grid
  http://127.0.0.1:8080/?benchmark=retained-text
  http://127.0.0.1:8080/?benchmark=text-editing
  http://127.0.0.1:8080/?benchmark=text-comparison
  http://127.0.0.1:8080/?benchmark=widget-book
  http://127.0.0.1:8080/?benchmark=dev

Optional tuning parameters:

  ?benchmark=button-grid&warmup=60&frames=180

Behavior:
- the Rust app selects a focused benchmark surface from the query string
- `text-comparison` opens the side-by-side text rendering checklist added for grayscale, hinted, darkened, and LCD validation
- the page runs a requestAnimationFrame benchmark after startup
- results are written into the page overlay and also logged to the browser console as:

  SUI_BENCHMARK_RESULT { ...json... }

Notes:
- Trunk builds ../Cargo.toml as the Rust/WASM asset.
- The config enables --no-default-features plus the web feature.
- The watch config includes the sui-dev crate and the workspace root so edits in the Rust sources trigger rebuilds.
