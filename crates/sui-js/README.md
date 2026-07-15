# SUI JavaScript bindings

`@sui/ui` is the native SUI addon for Node.js and Electron. It uses napi-rs and
ships CommonJS entry points plus TypeScript declarations in `index.d.ts`.

The binding supports retained widget trees, desktop event-loop execution,
host-driven rendering, thread-safe state updates, JavaScript-owned custom
widgets, accessibility semantics, renderer-neutral paint commands, image and
font resources, and external-surface descriptors. It is an alpha,
source-built package; the npm package and prebuilt native binaries are not
published yet.

## Prerequisites

- Rust 1.90 or newer and Cargo;
- Node.js with npm;
- napi-rs CLI 3.x;
- for `App.run()`, a desktop supported by SUI's `winit` and `wgpu` backends.

Install the compatible napi-rs CLI once:

```bash
npm install --global @napi-rs/cli@3
```

## Build the native addon

From `crates/sui-js`:

```bash
npm run build
node -e 'console.log(require(".").App)'
```

From the workspace root:

```bash
npm --prefix crates/sui-js run build
node -e 'console.log(require("./crates/sui-js").App)'
```

The build runs `napi build --platform --release`. The generated `.node` file
must remain next to `index.js`, where the package loader searches for it. It is
a local build artifact and should not be committed.

## Run the examples

After building, run these commands from `crates/sui-js`:

```bash
node examples/counter.js
node examples/custom-widget.js
node examples/external-surface.js
```

Or run them from the workspace root:

```bash
node crates/sui-js/examples/counter.js
node crates/sui-js/examples/custom-widget.js
node crates/sui-js/examples/external-surface.js
```

The examples deliberately use `App.start()`. They render in process and print
snapshot or event information; they do not open desktop windows.

- [`counter.js`](examples/counter.js) covers `State`, built-in controls, a
  posted UI task, and rerendering.
- [`custom-widget.js`](examples/custom-widget.js) supplies JavaScript
  measurement, event, semantics, and paint callbacks.
- [`external-surface.js`](examples/external-surface.js) renders a CPU-RGBA
  texture through `ExternalSurface`.

## Open a desktop window

Use `App.run()` when JavaScript owns the normal desktop event loop:

```javascript
"use strict";

const sui = require("@sui/ui");

const root = sui.Column(
  [
    sui.Label("Ready"),
    sui.Button("No-op", () => {}),
  ],
  8
);

const window = new sui.Window("Hello from JavaScript");
window.root(root);

const app = new sui.App();
app.window(window);
app.run();
```

When running directly from this checkout, replace `require("@sui/ui")` with
`require(".")` from `crates/sui-js`, or
`require("./crates/sui-js")` from the workspace root.

`App.run()` blocks until the desktop application exits. Use
`App.runWithHandle(callback)` when startup code needs the thread-safe
`UiHandle` after the event loop is ready.

Use `App.start()` for embedding, deterministic tests, or host-driven rendering:

```javascript
const running = app.start();
const snapshot = running.render();
console.log(snapshot.commandCount, snapshot.semanticsCount);
```

The returned `RunningApp` can render windows, dispatch binding event
descriptors, drain posted work, request redraws, and expose window handles. It
does not create or present a native desktop surface by itself.

## State and threading

`State` values used by an application are attached to its UI task queue when
the app starts or runs. Updates from outside the UI drain path are queued and
mark the affected windows for redraw. `UiHandle.post(callback)` is the normal
way to schedule arbitrary work back onto the UI thread.

Widget and custom-paint callbacks run synchronously on the UI thread. Keep them
short; perform blocking I/O or long computation elsewhere, then publish the
result through `State` or `UiHandle`.

## Custom widgets and resources

An object wrapped with `new sui.Widget(object)` may implement:

- `measure(constraints)` to return a `Size`;
- `event(event)` to process binding-safe event descriptors;
- `semantics(semantics)` to expose roles, names, values, ranges, and actions;
- `paint(paint)` to emit validated scene commands.

The paint surface supports styled text, paths and path clips, rounded
rectangles, shadows, transforms, image quads, and validated built-in shaders.
Applications can register fonts and RGBA, PNG, or SVG images from buffers or
files. See [`examples/custom-widget.js`](examples/custom-widget.js) for a
complete custom control.

`ExternalSurface` accepts CPU-upload, shared-texture, and shared-render-target
descriptors. The CPU-RGBA path renders today. Shared descriptors are validated
and retained for host integration, but zero-copy renderer composition is not
implemented yet.

## TypeScript

`index.d.ts` declares the checked-in binding surface. A local CommonJS project
can point at the package directory while developing:

```json
{
  "dependencies": {
    "@sui/ui": "file:../path/to/sui/crates/sui-js"
  }
}
```

Build the native addon before importing that dependency at runtime.

## Current limitations

- The npm package, native binaries, and release automation are not published;
  users build from source.
- This package targets Node.js and Electron. Browser/WASM bindings are not
  implemented.
- The JavaScript surface is intentionally smaller than the Rust facade. Some
  newer specialized widgets, including `PasswordInput` and `DateTimeInput`,
  remain Rust-only.
- Desktop `run` entry points exist, but the repository does not yet have broad
  real-window smoke coverage for every supported platform.
- Custom WGSL, arbitrary shader resources, and uniforms are not exposed;
  custom paint can use only validated built-in shaders.
- Shared textures and shared render targets are descriptor-level APIs today;
  only the portable CPU-upload external surface is rendered end to end.
- The API is pre-release and may change before the first stable release.

## Validate binding changes

Check the CommonJS loader and Rust-side binding tests:

```bash
npm run check:loader
cargo test -p sui-js
```

From the workspace root, use:

```bash
npm --prefix crates/sui-js run check:loader
cargo test -p sui-js
```

After a native build, run all three JavaScript examples as the package-level
smoke test.

## More documentation

- [Examples catalog](../../docs/examples.md)
- [Rust API guide](../../docs/api/README.md)
- [Testing guide](../../docs/testing.md)
- [Cross-language binding roadmap](../../docs/plans/cross-language-bindings-plan.md)
- [Documentation index](../../docs/README.md)
