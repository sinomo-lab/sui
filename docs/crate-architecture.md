# SUI Crate Guide

## Workspace Shape

The workspace currently contains these crates:

```text
crates/
  sui/
  sui-animation/
  sui-core/
  sui-debug/
  sui-demo/
  sui-layout/
  sui-platform/
  sui-render-wgpu/
  sui-runtime/
  sui-scene/
  sui-testing/
  sui-text/
  sui-widgets/
```

There is no separate surface protocol crate. Core window/viewport, event,
widget, context, and frame contracts live in the existing core/runtime/scene
layers.

## Dependency Layers

The codebase is easiest to understand as three stacked layers plus tooling.

### Public facade

- `sinomo-ui`

### Engine

- `sinomo-ui-core`
- `sinomo-ui-animation`
- `sinomo-ui-layout`
- `sinomo-ui-text`
- `sinomo-ui-scene`
- `sinomo-ui-runtime`
- `sinomo-ui-render-wgpu`
- `sinomo-ui-platform`

### Product-facing widget libraries

- `sinomo-ui-widgets`
- `sinomo-ui-debug`

### Development and test tooling

- `sinomo-ui-testing`
- `sinomo-ui-demo`

## Crate Responsibilities

### `sinomo-ui`

The public Rust facade.

It re-exports the user-facing API from lower crates and provides the top-level
`App`, `Window`, `ResourceRegistry`, and `UiHandle` types. Normal application
code should register resources, add windows, then either build a `Runtime` for
embedding/tests or run the default desktop/web event loop.

### `sinomo-ui-core`

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

### `sinomo-ui-animation`

Pure animation timelines, documents, and evaluators.

It depends only on `sinomo-ui-core`. It must not know about widget identity, runtime
scheduling, platform events, renderer details, or application persistence
backends. Runtime-facing playback bridges belong in `sinomo-ui-widgets` or
application code.

### `sinomo-ui-layout`

Layout primitives and reusable utilities.

This crate owns constraints, padding, alignment, size helpers, and shared
measure/arrange utilities. It does not own the widget graph, the frame
scheduler, or renderer-specific context.

### `sinomo-ui-text`

The text subsystem.

This crate owns font registration data structures, fallback resolution, shaping,
measurement, and text layout objects passed into scenes and renderers. It should
not know about windows, widget identities, or `wgpu` surfaces.

### `sinomo-ui-scene`

The renderer-neutral paint representation.

This crate owns draw commands, brushes, stroke styles, scene layers, layer
descriptors, presentation-only layer properties, layer updates, and `SceneFrame`.

`SceneFrame.window_id` is the renderer submission target. The target may be a
native platform window or an embedded viewport/region represented by `WindowId`.
Renderers consume this crate; widgets and runtime contexts produce it.

### `sinomo-ui-runtime`

The standard retained widget runtime and widget cooperation protocol.

This crate owns:

- `Application`, `Runtime`, and `WindowBuilder`
- the `Widget` trait and widget contexts
- `WidgetPod`, `SingleChild`, and `WidgetChildren`
- event routing
- the typed command queue, application/window controllers, and multicast
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

### `sinomo-ui-render-wgpu`

The `wgpu` backend and retained compositor.

This crate owns device and queue setup, per-target surface/offscreen
registration, retained compositor state per `WindowId`, packet reuse, text and
image caches, frame capture, and renderer statistics.

This is the only crate that should own `wgpu` details.

### `sinomo-ui-platform`

Host integration.

This crate owns desktop lifecycle with `winit`, host event normalization into
`sui_core::Event`, redraw scheduling for `WindowId` targets, accessibility
bridging, separate reactive/command scheduler wakes, deterministic headless
execution, and renderer submission. Its
Windows accessibility bridge publishes runtime semantic snapshots through
AccessKit's native UI Automation adapter and routes requested actions back into
the runtime event loop.

It should not become a second runtime or renderer. In this pass, desktop and
headless paths drive `sui_runtime::Runtime` directly.

### `sinomo-ui-widgets`

The built-in widget library.

This crate owns common controls and containers plus theme types. It is the
reference implementation for how widgets use runtime contexts, semantics, and
scene painting, but custom widgets may use different internal models.

### `sinomo-ui-debug`

Reusable debug UI.

This crate owns development-facing widgets and inspectors. It should render
the renderer-neutral `WindowInspectorSnapshot` produced by the runtime; it
should not be the only place where diagnostics are computed. Accessibility
validation remains shared platform infrastructure, and widget-specific
diagnostics are collected only when a snapshot is requested.

### `sinomo-ui-testing`

The deterministic UI automation layer.

This crate owns `TestApp`, `TestWindow`, locators, expectations, snapshots,
artifact helpers, and high-level actions. It relies on the real runtime and
platform contracts instead of a fake widget model.

### `sinomo-ui-demo`

The main development host, widget gallery, and visual validation package.

This crate launches the desktop app used for manual runtime, widget, and
renderer validation. Its `widget_book` module owns the built-in widget gallery,
benchmark and stress targets, animation demos, diagnostics surfaces,
performance overlays, and visual artifact generation.

## Directional Rules

1. `sinomo-ui-core` stays platform-neutral and renderer-neutral.
2. `WindowId` remains the common target identifier for events, runtime work, and scene frames.
3. `WindowEvent` remains the lifecycle/input-target event name.
4. `Widget`, `WidgetPod`, contexts, and runtime graph snapshots are core retained-widget protocol concepts.
5. Logical child enumeration does not imply SUI owns or can fully inspect widget internals.
6. `sinomo-ui-runtime` may depend on `sinomo-ui-core`, `sinomo-ui-layout`, `sinomo-ui-scene`, and `sinomo-ui-text`, but not on `wgpu` or `winit`.
7. `sinomo-ui-scene` is the renderer-facing paint model.
8. `sinomo-ui-render-wgpu` consumes scene output and resource snapshots, not widget internals.
9. `sinomo-ui-platform` normalizes host events and submits renderer frames; it does not own widget logic.
10. Built-in widgets stay outside `sinomo-ui-runtime`.
11. Scheduler wakes and typed command delivery remain distinct; neither is
    represented as an implicit root custom event.
12. Window overlay ordering, nesting, focus, dismissal, and diagnostics belong
    to `sinomo-ui-runtime`; concrete presentation widgets and placement policy
    belong to `sinomo-ui-widgets`.
13. Native file dialog handles and external file-drop normalization belong to
    `sinomo-ui-platform`; applications remain responsible for storage and
    import policy.

## Practical Ownership Guide

- Add or change an event type: `sinomo-ui-core`, then `sinomo-ui-platform`, then retained-runtime routing if needed.
- Add a typed application command: define its `CommandKey` near the owning
  feature, then register application/window subscribers through the facade;
  do not add a new string custom event.
- Change `WindowId` target semantics: update `sinomo-ui-core`, `sinomo-ui-runtime`, `sinomo-ui-platform`, and docs together.
- Change widget participation in layout, paint, semantics, routing, or child enumeration: `sinomo-ui-runtime` and `sinomo-ui-widgets`.
- Change layout primitives: `sinomo-ui-layout`.
- Add a new draw command or layer behavior: `sinomo-ui-scene`, then `sinomo-ui-render-wgpu`.
- Change text shaping or measurement: `sinomo-ui-text`, then validate runtime and renderer callers.
- Change platform event handling or IME behavior: `sinomo-ui-platform`, preserving `WindowId + Event` delivery.
- Add locator behavior or test actions: `sinomo-ui-testing`.
- Add gallery stories, screenshots, or performance panels: `sinomo-ui-demo`.

## Common Mistakes To Avoid

- Do not add widget-specific policy to `sinomo-ui-runtime` when it belongs in `sinomo-ui-widgets`.
- Do not require `Widget: Send + Sync` to make SUI thread-friendly.
- Do not wrap `Event` and `SceneFrame` in a second surface protocol layer.
- Do not put widget IDs, event contexts, or renderer concepts in `sinomo-ui-animation`.
- Do not turn `sinomo-ui-runtime` into a framework-owned animation engine just because it owns wake scheduling.
- Do not make widgets depend on `wgpu` types when the scene system already provides the boundary.
- Do not make tests depend on widget internals when semantics or graph snapshots expose the intended contract.
- Do not use `sinomo-ui-platform` to patch around runtime bugs that should be fixed in the runtime itself.
