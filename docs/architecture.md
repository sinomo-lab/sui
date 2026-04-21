# SUI Architecture Overview

## What SUI Is Right Now

SUI is a retained widget toolkit with an explicit runtime.

The current implementation centers on these ideas:

- widgets own state and child composition through `Widget`, `WidgetPod`, `SingleChild`, and `WidgetChildren`
- the runtime owns event routing, focus, pointer capture, invalidation, layout, scene generation, and semantics generation
- widgets paint into a renderer-neutral `Scene`
- the renderer consumes immutable `SceneFrame` snapshots and maintains retained compositor state
- semantics are generated alongside the scene and are used by accessibility, testing, and diagnostics

The public Rust entry point is `sui::Application`. With the default features enabled, `Application::run()` builds a `sui_runtime::Runtime` and hands execution to `sui_platform::DesktopPlatform`.

## The Main Execution Path

The normal desktop flow is:

1. User code creates an `Application` and one or more `WindowBuilder` values.
2. `sui::Application::run()` builds a `Runtime` with shared font and image registries.
3. `DesktopPlatform` creates native windows, wires them to a shared `WgpuRenderer`, and enters the `winit` event loop.
4. Native events are normalized into `sui_core::Event` values.
5. `Runtime::handle_event()` routes the event through the retained widget tree.
6. Widgets request explicit invalidation through runtime contexts.
7. When a window needs work, the runtime runs measure, arrange, paint, semantics, and resource updates as needed.
8. The runtime returns a `RenderOutput` containing a `SceneFrame` and diagnostics.
9. `sui-render-wgpu` updates retained compositor state, prepares GPU work, and presents the frame.

The same runtime contract is used by the headless platform for tests. The platform changes, but the runtime boundary does not.

## Runtime Model

### Retained widget graph

Each widget lives inside a `WidgetPod` with a stable `WidgetId`. The runtime keeps window-scoped graph state for:

- parent and child relationships
- measured size and arranged bounds
- focus state
- pointer capture
- last semantics snapshot
- per-window frame scheduling and diagnostics

Widgets are not responsible for global orchestration. They implement behavior through the `Widget` trait and use runtime contexts to request work.

### Explicit invalidation

SUI does not use a diff-based UI model. Widgets mutate state in response to events and request invalidation explicitly.

The intended model is:
- widgets own their local dirty state and know why they need work
- the runtime orchestrates measure, arrange, paint, semantics, and resource phases from the root of the affected window
- scene layers are optional paint/composition boundaries, not the default semantic unit of every widget

The current implementation has drifted from that model by treating many widget paint outputs as their own `SceneLayer` nodes. That over-layerization is now treated as an implementation detail under transition rather than a contract to preserve.

The current invalidation kinds are:

- `Measure`
- `Arrange`
- `Transform`
- `Clip`
- `Effect`
- `Visibility`
- `Paint`
- `HitTest`
- `Text`
- `Semantics`
- `Resources`

That split matters because the runtime and renderer treat geometry, paint, semantics, and resource work differently.

### Phase-driven frame work

The runtime operates as a sequence of explicit phases rather than a hidden redraw loop. The current order is:

1. event handling and routing
2. invalidation collection
3. measure and arrange work
4. scene generation
5. semantics generation
6. renderer submission
7. diagnostics publication

This is the structure that tests, debugging tools, and the widget book all rely on.

## Event Routing

`sui-platform` is responsible for translating host events into `sui_core::Event` values. The runtime then routes those events using explicit phases:

- capture
- target
- bubble

Routing decisions depend on the event type:

- pointer events use hit testing and pointer capture
- keyboard and IME events use focus state
- timers and async wakeups are scheduled by the runtime and re-enter as `WakeEvent`
- redraw and resize events enter through `WindowEvent`

Widgets can mark events handled, request focus, schedule timers, request async wakeups, and request invalidation. They do not reach into platform objects or renderer state directly.

## Layout, Paint, and Semantics

### Layout

The default widget-tree layout contract is already split into measure and arrange.

- `measure(&mut MeasureCtx, Constraints) -> Size`
- `arrange(&mut ArrangeCtx, Rect)`

This split means geometry-only changes do not need to look like full paint rebuilds.

Built-in widgets use this measure/arrange pipeline heavily, but it is meant to stay a reusable widget-side utility rather than a renderer concept or a mandatory framework-wide contract. Custom widgets should be able to drive layout work themselves, mix SUI layout helpers with arbitrary spatial systems, or bypass the default pipeline when building interfaces attached to non-standard environments such as 3D objects.

`sui-layout` provides the shared constraint and geometry primitives, while `sui-runtime` owns the standard widget-tree layout contexts, scheduling, and caching. That runtime pipeline should stay separable enough that advanced widgets can initiate measurement or arrangement work without depending on a standard paint, window, or renderer context.

### Paint

Widgets render through `PaintCtx` into `sui-scene`. They do not emit raw GPU commands.

The current scene layer includes:

- fills and strokes
- paths
- text draws and shaped text draws
- image draws
- transforms, clips, and nested layers
- layer descriptors with composition, ordering, bounds, and ownership metadata

`SceneFrame` is the renderer-facing snapshot. It also carries font and image registry snapshots so the renderer can resolve resources without pulling mutable runtime state.

A critical current-state detail: the runtime still wraps each widget paint result in a `SceneLayer` by default, even for simple wrapper widgets. That behavior made the retained compositor the de facto repaint granularity, but it is not the intended long-term architecture. The active transition direction is to flatten ordinary widget paint into parent scenes and reserve `SceneLayer` for explicit repaint or composition boundaries such as scroll surfaces, overlays, and stack surfaces.

### Semantics

Widgets contribute `SemanticsNode` values through `SemanticsCtx`. The runtime assembles those nodes into the window's semantics snapshot.

That snapshot is used by:

- accessibility bridges in `sui-platform`
- locators and expectations in `sui-testing`
- debug UIs in `sui-debug`
- widget-book tests and screenshot helpers

User-observable and automation-observable changes usually affect the semantics path as well as paint.

## Platform Boundary

`sui-platform` is the host integration layer.

Today it has two important entry points:

- `DesktopPlatform` for the real `winit` event loop and `wgpu` surfaces
- `HeadlessPlatform` for deterministic tests, manual time control, and offscreen rendering

The platform layer owns:

- host window creation and teardown
- native event normalization
- redraw scheduling
- accessibility snapshots and bridge updates
- renderer registration for each window

It does not own widget logic, layout rules, or scene production.

## Renderer Boundary

`sui-render-wgpu` is the only crate that owns `wgpu` concepts.

Today it provides:

- a shared device and queue setup
- per-window surface or offscreen target registration
- retained compositor state per window
- text, image, and analytic path caches
- capture helpers for headless screenshots and desktop harnesses
- per-frame renderer statistics consumed by diagnostics tooling

The renderer consumes `SceneFrame` snapshots. It does not walk widget internals or compute layout.

See [renderer-architecture.md](./renderer-architecture.md) for the current renderer model and its constraints.

## Diagnostics And Tooling

Diagnostics are part of the main runtime path, not a separate simulation.

The current stack includes:

- `WindowPerformanceSnapshot` and related phase timing types in `sui-runtime`
- per-frame renderer submission stats published by `sui-platform`
- the widget book performance overlay in `sui-widget-book`
- reusable inspector widgets in `sui-debug`
- semantics-first locators and artifact capture in `sui-testing`

The current tooling uses real snapshots and real runtime outputs. The existing runtime surfaces already provide the main testing and diagnostics data.

## What Is In Scope Today

These parts are implemented and are active in day-to-day development:

- retained widget runtime with stable widget identity
- measure and arrange layout phases
- scene-based paint output
- semantics snapshots
- desktop host integration
- deterministic headless testing
- retained `wgpu` compositor path
- widget book gallery and visual artifact generation

These parts are intentionally not yet a first-class workspace surface:

- Python bindings
- JavaScript or WASM bindings
- a separate bindings-core crate
- mobile-specific platform integration beyond placeholders

The design doc still discusses some future directions, but the current Rust desktop and testing stack is the implemented source of truth.

## Where To Start When Making Changes

Choose the entry point that matches the change.

- Widget behavior: start in `sui-runtime::widget` and `sui-widgets`.
- Event delivery or window lifecycle: start in `sui-platform` and `sui-runtime`.
- Layout or invalidation behavior: start in `sui-runtime` and `sui-layout`.
- Paint or scene changes: start in `sui-scene`, then follow through `sui-render-wgpu`.
- Accessibility or locator behavior: start in semantics generation, then check `sui-testing` and `sui-platform::accessibility`.
- Renderer performance: start in `sui-render-wgpu`, then validate with widget-book diagnostics and targeted tests.
