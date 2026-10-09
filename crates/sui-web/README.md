# sinomo-ui-web

The WebAssembly build behind the browser entry point of the `@sui/ui`
JavaScript package (`crates/sui-js/web.js`). It exposes the same native
surface as the Node addon in `crates/sui-js`, so the shared JavaScript layer
(`crates/sui-js/runtime.js`) runs on both.

Build it into `crates/sui-js/web/` with:

```sh
cargo xtask web build
```

The crate only compiles to code on `wasm32-unknown-unknown`; on other
targets it is empty.
