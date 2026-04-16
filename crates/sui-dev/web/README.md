SUI Dev web build

Trunk is now integrated for the browser build.

From this directory:

  cd crates/sui-dev/web
  trunk serve

Or from the workspace root:

  trunk serve --config crates/sui-dev/web/Trunk.toml

Production build:

  trunk build --config crates/sui-dev/web/Trunk.toml --release

Output goes to:

  crates/sui-dev/web/dist

Notes:
- Trunk builds ../Cargo.toml as the Rust/WASM asset.
- The config enables --no-default-features plus the web feature.
- The watch config includes the sui-dev crate and the workspace root so edits in the Rust sources trigger rebuilds.
