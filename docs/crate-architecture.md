# SUI Crate Guide

## Workspace Shape

The workspace currently contains these crates:

```text
crates/
  sui/
  sui-core/
  sui-debug/
  sui-dev/
  sui-layout/
  sui-platform/
  sui-render-wgpu/
  sui-runtime/
  sui-scene/
  sui-testing/
  sui-text/
  sui-widget-book/
  sui-widgets/
```

There are no bindings crates in the workspace today. Design discussions may mention future Python or JavaScript surfaces, but the implemented workspace is the crate set above.

## Dependency Layers

The codebase is easiest to understand as three stacked layers plus tooling.

### Public facade

- `sui`

### Engine

- `sui-core`
- `sui-layout`
- `sui-text`
- `sui-scene`
- `sui-runtime`
- `sui-render-wgpu`
- `sui-platform`

### Product-facing widget libraries

- `sui-widgets`
- `sui-debug`

### Development and test tooling

- `sui-testing`
- `sui-widget-book`
- `sui-dev`

## Crate Responsibilities

### `sui`

The public Rust facade.

It re-exports the user-facing API from the lower crates, provides the top-level `Application` type, and keeps feature-gated renderer and platform dependencies out of most application code.

This crate currently contains little policy logic. Most runtime policy lives in lower crates.

### `sui-core`

Shared value types and contracts used across the workspace.

This crate owns:

- geometry and math primitives
- colors and color spaces
- normalized input and window events
- semantics roles, values, and actions
- invalidation kinds and dirty regions
- IDs and handles such as `WidgetId`, `WindowId`, `FontHandle`, and `ImageHandle`
- the shared error and result types

This crate must stay free of platform, runtime, and renderer implementation details.

### `sui-layout`

Layout primitives and reusable utilities.

This crate owns constraints, padding, alignment, and size helpers. It does not own the widget graph or the frame scheduler.

If a type can be shared by containers and widgets without needing runtime state, it usually belongs here.

### `sui-text`

The text subsystem.

This crate owns:

- font registration data structures
- font fallback resolution
- shaping and text measurement
- text layout objects passed into the scene and renderer

It should not know about windows, widget identities, or `wgpu` surfaces.

### `sui-scene`

The renderer-neutral paint representation.

This crate owns:

- draw commands
- brushes and stroke styles
- scene layers and layer descriptors
- layer update records
- image registry data used by scene frames

Widgets paint into this crate through `PaintCtx`. Renderers consume it. No other paint representation should sit between widgets and the renderer.

### `sui-runtime`

The retained runtime.

This crate owns:

- `Application`, `Runtime`, and `WindowBuilder`
- the `Widget` trait and widget contexts
- `WidgetPod`, `SingleChild`, and `WidgetChildren`
- event routing
- focus and pointer capture
- timers and async wakeups
- invalidation scheduling
- measure and arrange execution
- scene generation
- semantics generation
- runtime-side diagnostics and snapshots

This crate is the main integration point for most framework changes.

### `sui-render-wgpu`

The `wgpu` backend and retained compositor.

This crate owns:

- device and queue setup
- surface registration per window
- offscreen rendering for headless runs
- retained compositor state per window
- tile and packet reuse logic
- text, image, and analytic path caches
- frame capture and renderer statistics

This is the only crate that should own `wgpu` details.

### `sui-platform`

Host integration.

This crate owns:

- desktop window lifecycle with `winit`
- native event normalization into `sui_core::Event`
- redraw scheduling
- accessibility snapshot bridging
- deterministic headless execution

It is the layer between the host environment and the runtime. It should not become a second runtime or a second renderer.

### `sui-widgets`

The built-in widget library.

This crate owns common controls and containers, plus the theme types that style them. It is the reference implementation for how widgets are expected to use runtime contexts, semantics, and scene painting.

### `sui-debug`

Reusable debug UI.

This crate owns development-facing widgets and inspectors. It should render runtime and renderer state that already exists; it should not be the only place where diagnostics are computed.

### `sui-testing`

The deterministic UI automation layer.

This crate owns:

- `TestApp`, `TestWindow`, `Locator`, and `Expectation`
- snapshot and artifact helpers
- semantics-first selectors
- high-level actions such as click, fill, and key press

It relies on the real runtime and platform contracts instead of a fake widget model.

### `sui-widget-book`

The gallery and screenshot-oriented validation crate.

This crate owns:

- the built-in widget gallery
- benchmark and stress surfaces used during renderer work
- performance overlay composition
- visual artifact generation and desktop-oriented tests

It is the main place where the widget set, diagnostics, and visual regressions appear together.

### `sui-dev`

The main development host.

This crate launches a desktop app with tabs for:

- the widget book gallery
- the 64-button benchmark surface
- the retained text benchmark surface
- renderer settings

It is the main desktop host for manual runtime and renderer validation.

## Directional Rules

The most important dependency rules are:

1. `sui-core` stays platform-neutral and renderer-neutral.
2. `sui-runtime` may depend on `sui-core`, `sui-layout`, `sui-scene`, and `sui-text`, but not on `wgpu` or `winit`.
3. `sui-scene` is the only paint model widgets emit.
4. `sui-render-wgpu` consumes scene output and resource snapshots, not widget internals.
5. `sui-platform` talks to the runtime through public runtime APIs, not private runtime state.
6. built-in widgets stay outside `sui-runtime`.
7. development tools should reuse real runtime outputs where possible.

## Practical Ownership Guide

When you need to change behavior, use this map.

- Add or change an event type: `sui-core`, then `sui-platform`, then `sui-runtime`.
- Change layout primitives: `sui-layout`.
- Change how widgets participate in layout or paint: `sui-runtime` and `sui-widgets`.
- Add a new draw command or layer behavior: `sui-scene`, then `sui-render-wgpu`.
- Change text shaping or measurement: `sui-text`, then validate `sui-runtime` and `sui-render-wgpu` callers.
- Change platform event handling or IME behavior: `sui-platform`.
- Add locator behavior or new test actions: `sui-testing`.
- Add gallery stories, screenshots, or performance panels: `sui-widget-book`.

## Common Mistakes To Avoid

- Do not add widget-specific policy to `sui-runtime` when it belongs in `sui-widgets`.
- Do not make widgets depend on `wgpu` types when the scene system already provides the boundary.
- Do not make tests depend on widget internals when semantics already exposes the intended surface.
- Do not put shared core data in `sui-dev` or `sui-widget-book` just because it is convenient for one experiment.
- Do not use `sui-platform` to patch around runtime bugs that should be fixed in the runtime itself.
