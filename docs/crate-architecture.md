# SUI Crate Guide

## Workspace Shape

The workspace currently contains these crates:

```text
crates/
  sui/
  sui-animation/
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

There is no separate surface protocol crate. Core window/viewport, event,
widget, context, and frame contracts live in the existing core/runtime/scene
layers.

## Dependency Layers

The codebase is easiest to understand as three stacked layers plus tooling.

### Public facade

- `sui`

### Engine

- `sui-core`
- `sui-animation`
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

It re-exports the user-facing API from lower crates and provides the top-level
`App`, `Window`, `ResourceRegistry`, and `UiHandle` types. Normal application
code should register resources, add windows, then either build a `Runtime` for
embedding/tests or run the default desktop/web event loop.

### `sui-core`

Shared value types and core contracts.

This crate owns:

- geometry and math primitives
- colors and color spaces
- `WindowId` as the host render/input target identifier
- normalized input and `WindowEvent` values
- semantics roles, values, and actions
- invalidation kinds and dirty regions
- IDs and handles such as `WidgetId`, `WindowId`, `FontHandle`, and `ImageHandle`
- the shared error and result types

This crate must stay free of platform, runtime, and renderer implementation
details.

### `sui-animation`

Pure animation timelines, documents, and evaluators.

It depends only on `sui-core`. It must not know about widget identity, runtime
scheduling, platform events, renderer details, or application persistence
backends. Runtime-facing playback bridges belong in `sui-widgets` or
application code.

### `sui-layout`

Layout primitives and reusable utilities.

This crate owns constraints, padding, alignment, size helpers, and shared
measure/arrange utilities. It does not own the widget graph, the frame
scheduler, or renderer-specific context.

### `sui-text`

The text subsystem.

This crate owns font registration data structures, fallback resolution, shaping,
measurement, and text layout objects passed into scenes and renderers. It should
not know about windows, widget identities, or `wgpu` surfaces.

### `sui-scene`

The renderer-neutral paint representation.

This crate owns draw commands, brushes, stroke styles, scene layers, layer
descriptors, presentation-only layer properties, layer updates, and `SceneFrame`.

`SceneFrame.window_id` is the renderer submission target. The target may be a
native platform window or an embedded viewport/region represented by `WindowId`.
Renderers consume this crate; widgets and runtime contexts produce it.

### `sui-runtime`

The standard retained widget runtime and widget cooperation protocol.

This crate owns:

- `Application`, `Runtime`, and `WindowBuilder`
- the `Widget` trait and widget contexts
- `WidgetPod`, `SingleChild`, and `WidgetChildren`
- event routing
- focus and pointer capture
- timers, animation-frame wakes, and async wakeups
- invalidation scheduling
- default widget-tree measure and arrange execution
- scene and semantics generation
- runtime-side diagnostics and snapshots

`Widget`, `WidgetPod`, `MeasureCtx`, `PaintCtx`, event contexts, and graph
snapshots are core protocol concepts. They let independently authored widgets
cooperate through logical child enumeration, invalidation, layout, paint,
semantics, and event routing.

The runtime does not require widgets to expose every internal child, store
children locally, dispatch events internally in a specific way, or keep all child
systems on the same thread/process/machine. A widget may expose logical children
that are retained, generated, virtualized, remote, or partial.

Do not add `Widget: Send + Sync`. Thread-friendly behavior should be expressed
through widget-owned synchronization, runtime wakeups, immutable snapshots, and
scene-frame submission to the renderer.

### `sui-render-wgpu`

The `wgpu` backend and retained compositor.

This crate owns device and queue setup, per-target surface/offscreen
registration, retained compositor state per `WindowId`, packet reuse, text and
image caches, frame capture, and renderer statistics.

This is the only crate that should own `wgpu` details.

### `sui-platform`

Host integration.

This crate owns desktop lifecycle with `winit`, host event normalization into
`sui_core::Event`, redraw scheduling for `WindowId` targets, accessibility
bridging, deterministic headless execution, and renderer submission.

It should not become a second runtime or renderer. In this pass, desktop and
headless paths drive `sui_runtime::Runtime` directly.

### `sui-widgets`

The built-in widget library.

This crate owns common controls and containers plus theme types. It is the
reference implementation for how widgets use runtime contexts, semantics, and
scene painting, but custom widgets may use different internal models.

### `sui-debug`

Reusable debug UI.

This crate owns development-facing widgets and inspectors. It should render
runtime and renderer state that already exists; it should not be the only place
where diagnostics are computed.

### `sui-testing`

The deterministic UI automation layer.

This crate owns `TestApp`, `TestWindow`, locators, expectations, snapshots,
artifact helpers, and high-level actions. It relies on the real runtime and
platform contracts instead of a fake widget model.

### `sui-widget-book`

The gallery and screenshot-oriented validation crate.

This crate owns the built-in widget gallery, benchmark and stress targets,
animation demos, diagnostics surfaces, performance overlays, and visual artifact
generation.

### `sui-dev`

The main development host.

This crate launches the desktop app used for manual runtime, widget, and
renderer validation.

## Directional Rules

1. `sui-core` stays platform-neutral and renderer-neutral.
2. `WindowId` remains the common target identifier for events, runtime work, and scene frames.
3. `WindowEvent` remains the lifecycle/input-target event name.
4. `Widget`, `WidgetPod`, contexts, and runtime graph snapshots are core retained-widget protocol concepts.
5. Logical child enumeration does not imply SUI owns or can fully inspect widget internals.
6. `sui-runtime` may depend on `sui-core`, `sui-layout`, `sui-scene`, and `sui-text`, but not on `wgpu` or `winit`.
7. `sui-scene` is the renderer-facing paint model.
8. `sui-render-wgpu` consumes scene output and resource snapshots, not widget internals.
9. `sui-platform` normalizes host events and submits renderer frames; it does not own widget logic.
10. Built-in widgets stay outside `sui-runtime`.

## Practical Ownership Guide

- Add or change an event type: `sui-core`, then `sui-platform`, then retained-runtime routing if needed.
- Change `WindowId` target semantics: update `sui-core`, `sui-runtime`, `sui-platform`, and docs together.
- Change widget participation in layout, paint, semantics, routing, or child enumeration: `sui-runtime` and `sui-widgets`.
- Change layout primitives: `sui-layout`.
- Add a new draw command or layer behavior: `sui-scene`, then `sui-render-wgpu`.
- Change text shaping or measurement: `sui-text`, then validate runtime and renderer callers.
- Change platform event handling or IME behavior: `sui-platform`, preserving `WindowId + Event` delivery.
- Add locator behavior or test actions: `sui-testing`.
- Add gallery stories, screenshots, or performance panels: `sui-widget-book`.

## Common Mistakes To Avoid

- Do not add widget-specific policy to `sui-runtime` when it belongs in `sui-widgets`.
- Do not require `Widget: Send + Sync` to make SUI thread-friendly.
- Do not wrap `Event` and `SceneFrame` in a second surface protocol layer.
- Do not put widget IDs, event contexts, or renderer concepts in `sui-animation`.
- Do not turn `sui-runtime` into a framework-owned animation engine just because it owns wake scheduling.
- Do not make widgets depend on `wgpu` types when the scene system already provides the boundary.
- Do not make tests depend on widget internals when semantics or graph snapshots expose the intended contract.
- Do not use `sui-platform` to patch around runtime bugs that should be fixed in the runtime itself.
