# SUI Crate Architecture and API Boundary Plan

This document turns the design in [design.md](./design.md) into a concrete workspace plan.

The intent is to keep the architecture implementable without over-fragmenting the codebase too early. The plan therefore defines:

- a small set of phase-1 crates that cover the core runtime
- strict boundaries around renderer, platform, and bindings code
- one stable Rust-facing facade crate
- a separate FFI-facing surface for Python and JavaScript

## Architecture Goals

The crate layout should preserve the core design constraints:

- event-driven state changes and dirty invalidation
- retained widget tree with stable identity
- scene-based rendering rather than direct GPU access from widgets
- platform normalization instead of platform logic leaking into widgets
- bindings-aware APIs that do not depend on Rust-only ownership patterns

## Recommended Workspace Layout

The current `crates/sui` binary should become the public Rust facade library. A demo or playground binary should move to a separate crate so the `sui` name is reserved for the user-facing library.

Recommended workspace target:

```text
crates/
  sui/                  # public Rust facade crate
  sui-core/             # shared core types and contracts
  sui-layout/           # constraints, sizing, placement, container algorithms
  sui-scene/            # paint graph, draw primitives, composition data
  sui-text/             # fonts, shaping, text layout, editing primitives
  sui-runtime/          # widget tree, event routing, focus, invalidation, scheduling
  sui-render-wgpu/      # scene-to-wgpu renderer, caches, color pipeline, surfaces
  sui-platform/         # native/web shell adapters, event normalization, IME, DnD
  sui-widgets/          # standard controls built on the runtime contracts
  sui-testing/          # deterministic UI harness, event injection, semantics queries
  sui-bindings-core/    # FFI-safe command/event/data boundary
  sui-python/           # Python binding crate
  sui-js/               # JavaScript/WASM binding crate
  sui-dev/              # demo app, playground, smoke tests
```

## Crate Responsibilities

### `sui`

Role: stable Rust application-facing facade.

Exports:

- application and window builders
- the public widget API and prelude
- common geometry, color, input, layout, and semantics types re-exported from lower crates
- feature flags that opt into platform and renderer backends

Rules:

- should contain very little logic
- may re-export types from other crates, but should avoid exposing internal scheduling or cache types
- is the crate applications depend on by default

### `sui-core`

Role: foundational contracts shared across the workspace.

Owns:

- IDs and handles such as `WidgetId`, `WindowId`, `SurfaceId`, `ImageHandle`, `FontHandle`
- geometry and math primitives used across layout, scene, and input
- color and color-management primitives that are renderer-agnostic
- normalized input events and event metadata
- accessibility and semantic model types
- error types and common result aliases
- invalidation kinds and dirty region data types

Rules:

- no `wgpu`, `winit`, `web-sys`, `wasm-bindgen`, or `pyo3`
- no widget tree implementation details
- should be safe to expose through Rust and FFI layers

### `sui-layout`

Role: reusable layout engine and container contracts.

Owns:

- constraints, measured sizes, axis helpers, alignment, padding, spacing
- default one-pass layout contract
- extension points for custom or multi-pass layout
- virtualization-oriented helpers for large collections

Rules:

- depends on `sui-core`
- no renderer or platform code
- no widget tree ownership logic

### `sui-scene`

Role: rendering-neutral scene and paint representation.

Owns:

- paint commands and draw primitives
- transforms, clips, opacity groups, cached layer descriptors
- image, gradient, vector path, and text draw descriptors
- hit-test and debug-overlay data that are paint-adjacent

Rules:

- depends on `sui-core` and `sui-text` only for text draw data, not widget logic
- widgets emit scene content through this crate
- renderers consume scene frames from this crate
- must not depend on `wgpu` or platform adapters

### `sui-text`

Role: first-class text subsystem.

Owns:

- font registration and font database APIs
- shaping and line-breaking
- text layout objects and editing primitives
- outline extraction for glyph-to-vector conversion

Rules:

- depends on `sui-core`
- returns text layout results that can be used by both `sui-layout` and `sui-scene`
- should not know about widget tree internals, windows, or GPU objects

### `sui-runtime`

Role: retained UI tree and execution model.

Owns:

- widget storage and identity
- event routing, including capture/target/bubble phases where enabled
- focus management and keyboard navigation plumbing
- invalidation scheduling for layout, paint, hit-testing, text, and resources
- lifecycle, update scheduling, timers, async wakeups, and command dispatch
- semantic tree assembly from widget-provided semantic data

Rules:

- depends on `sui-core`, `sui-layout`, `sui-scene`, and `sui-text`
- may expose extension traits for widgets and custom containers
- must not depend directly on platform-windowing or `wgpu`
- must not expose its internal node arena or scheduling queues as stable public API

### `sui-render-wgpu`

Role: concrete renderer backend.

Owns:

- scene compilation into GPU work
- batch generation and sort keys
- texture, atlas, glyph, tile, and offscreen-surface caches
- color-management execution path and presentation transforms
- render-to-texture and custom pass interop points

Rules:

- depends on `sui-core`, `sui-scene`, and `sui-text`
- is the only core crate that directly owns `wgpu` concepts
- consumes immutable scene frames and renderer resources, not widget state
- raw `wgpu` access for Rust applications should be mediated through explicit interop APIs rather than leaked everywhere

### `sui-platform`

Role: shell and host integration.

Owns:

- window and surface lifecycle integration
- native event capture and normalization into `sui-core` event types
- IME, clipboard, drag-and-drop, cursor, and accessibility host bridges
- desktop, mobile, and web entry points behind features or submodules

Rules:

- depends on `sui-core`, `sui-runtime`, and `sui-render-wgpu`
- should be the only core crate that knows about `winit`, `raw-window-handle`, browser DOM hosts, or mobile shell adapters
- platform specifics stop here; widgets should only see normalized runtime contracts

### `sui-widgets`

Role: standard widget library.

Owns:

- buttons, toggles, fields, sliders, tables, breadcrumbs, and containers
- shared styling hooks for built-in controls
- default semantics and focus behavior for standard widgets

Rules:

- depends on `sui-core`, `sui-layout`, `sui-scene`, `sui-text`, and `sui-runtime`
- does not know about `wgpu` or windowing internals
- should serve as the reference implementation for custom-widget APIs

### `sui-testing`

Role: deterministic UI testing surface.

Owns:

- headless or harness-driven runtime bootstrapping
- deterministic event injection
- semantic tree inspection and widget lookup helpers
- screenshot or surface-based regression helpers where supported

Rules:

- depends on `sui-core`, `sui-runtime`, `sui-scene`, and selected platform shims
- tests should target semantics and observable state before internal widget details

### `sui-bindings-core`

Role: stable cross-language boundary.

Owns:

- opaque handles for windows, widgets, images, fonts, timers, and subscriptions
- FFI-safe enums and structs for events, commands, results, and snapshots
- command queue API used by language bindings to mutate the runtime safely
- callback registration and dispatch contracts that keep Rust in control of lifetime and threading

Rules:

- no direct exposure of `&T`, `&mut T`, generic-heavy traits, or Rust-owned iterator protocols
- no direct `wgpu` surface or device exposure
- designed around copyable payloads, opaque handles, and explicit ownership transfer

### `sui-python` and `sui-js`

Role: language-idiomatic wrappers over `sui-bindings-core`.

Owns:

- Python object model and callback adapters
- JavaScript async and WASM host integration
- packaging, build, and runtime glue specific to those ecosystems

Rules:

- should not bypass `sui-bindings-core`
- may feel idiomatic in each language, but should preserve the same high-level capability set

### `sui-dev`

Role: development host and smoke-test app.

Owns:

- examples and manual testing surfaces
- performance experiments and renderer diagnostics
- not part of the stable API surface

Current workspace status:

- `cargo run -p sui-dev` launches the real desktop playground window

## Dependency Rules

The architecture should enforce these directional rules:

```text
sui
  -> sui-widgets
  -> sui-platform
  -> sui-runtime
  -> sui-core

sui-widgets
  -> sui-runtime
  -> sui-layout
  -> sui-scene
  -> sui-text
  -> sui-core

sui-platform
  -> sui-render-wgpu
  -> sui-runtime
  -> sui-core

sui-runtime
  -> sui-layout
  -> sui-scene
  -> sui-text
  -> sui-core

sui-render-wgpu
  -> sui-scene
  -> sui-text
  -> sui-core

sui-scene
  -> sui-text
  -> sui-core

sui-layout
  -> sui-core

sui-text
  -> sui-core

sui-testing
  -> sui-runtime
  -> sui-scene
  -> sui-core

sui-bindings-core
  -> sui-runtime
  -> sui-core

sui-python
  -> sui-bindings-core

sui-js
  -> sui-bindings-core
```

Hard rules:

1. `sui-core` must remain platform-neutral and renderer-neutral.
2. `sui-runtime` may not depend on `wgpu`, `winit`, or browser host APIs.
3. `sui-scene` is the only paint representation widgets may emit.
4. `sui-render-wgpu` may not inspect widget internals or layout internals beyond scene output.
5. `sui-platform` may not mutate runtime internals except through explicit runtime APIs.
6. `sui-bindings-core` is the only supported path for Python and JavaScript integration.
7. Built-in widgets belong outside the runtime so the runtime stays policy-light.

## API Boundary Plan

### 1. Rust application boundary

The Rust-facing API should be defined by `sui` and built around a small number of concepts:

- `Application`
- `WindowBuilder`
- `Widget`
- `EventCtx`, `LayoutCtx`, `PaintCtx`, `SemanticsCtx`
- `Theme`, `Style`, `Brush`, `Color`, `ImageHandle`, `FontHandle`
- normalized event types such as `PointerEvent`, `KeyboardEvent`, `ImeEvent`, and `WindowEvent`

Target shape:

```rust
use sui::prelude::*;

fn main() -> sui::Result<()> {
    Application::new()
        .window(
            WindowBuilder::new()
                .title("SUI")
                .root(AppRoot::new()),
        )
        .run()
}

struct AppRoot;

impl Widget for AppRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        // mutate widget state and request invalidation explicitly
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        Size::new(constraints.max.width, constraints.max.height)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        // emit scene commands, never raw wgpu calls
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        // expose role, name, state, actions
    }
}
```

This surface is the primary stable Rust API. Applications should not need to know about scene compilation, window handles, or invalidation scheduler internals.

### 2. Widget-to-runtime boundary

Widgets are allowed to:

- receive normalized events
- mutate their own state
- request layout, paint, hit-test, text, or semantic invalidation
- emit scene commands through `PaintCtx`
- expose semantics through `SemanticsCtx`

Widgets are not allowed to:

- own or submit raw GPU command buffers directly in the normal path
- access platform-windowing internals directly
- bypass the runtime scheduler to force immediate redraw
- depend on native shell event types

This keeps widgets portable across desktop, mobile, and web.

### 3. Runtime-to-renderer boundary

The runtime should produce a scene frame and renderer resource requests. The renderer should consume that output without knowing about widget classes, focus graphs, or layout details.

Crossing this boundary is allowed to include:

- scene graph or display list frames
- cache keys and resource handles
- invalidated regions
- text layout results already resolved into drawable content

Crossing this boundary must not include:

- widget references
- runtime node arena indexes as public contracts
- callbacks into widget code during rendering

### 4. Platform-to-runtime boundary

All host-specific input and lifecycle data should be normalized before it reaches widgets.

Examples:

- `winit` mouse and touch events become `PointerEvent`
- browser IME and composition events become `ImeEvent`
- platform drag-and-drop data becomes a runtime `DragEvent` model

The runtime should expose APIs like:

- `Runtime::handle_event(window_id, event)`
- `Runtime::tick(frame_time)`
- `Runtime::render(window_id)`

The platform layer should not reach into widget internals or focus internals directly.

### 5. Semantics and testing boundary

Semantics are shared infrastructure for accessibility, testing, and automation. The semantic tree should therefore be a stable boundary, not a private widget-library convenience.

Publicly queryable data should include:

- semantic role
- accessible name and description
- state such as checked, selected, disabled, expanded, focused, busy
- available actions
- bounds in window or surface space

`sui-testing` should query this boundary instead of private widget structs whenever possible.

### 6. Rust-to-FFI boundary

Python and JavaScript bindings should cross into Rust through `sui-bindings-core`, using:

- opaque handles
- command submission APIs
- copied event payloads
- explicit subscription lifetimes
- serialized or POD-style semantic and scene snapshots

The FFI boundary should avoid:

- borrowed references into runtime-owned state
- trait-object callbacks crossing the ABI directly
- implicit thread hopping
- exposing raw `wgpu::Device`, `wgpu::Queue`, or surface internals

Good FFI shape:

```rust
pub struct WindowHandle(u64);
pub struct WidgetHandle(u64);

pub enum UiCommand {
    SetRoot { window: WindowHandle, widget: WidgetHandle },
    SetProperty { widget: WidgetHandle, key: PropertyKey, value: Value },
    DispatchEvent { target: WidgetHandle, event: EventPayload },
}

pub enum CallbackEvent {
    Action(ActionEvent),
    TextChanged(TextChangedEvent),
    WindowClosed(WindowHandle),
}
```

The runtime remains authoritative. Bindings submit commands and receive events; they do not directly own the UI engine.

## Backend and Feature Strategy

Recommended features on the `sui` facade crate:

- `desktop` for desktop shell integration
- `web` for browser and WASM support
- `mobile` for mobile shell integration
- `wgpu` for the default renderer backend
- `testing` for harness and test helpers
- `python-bindings` and `js-bindings` only where packaging needs them

The default build for early development can enable `desktop` and `wgpu`.

## What Must Stay Out of the Stable API

These implementation details should remain private or unstable until the runtime is mature:

- node arena layout and storage format
- dirty queue representation
- cache eviction heuristics
- atlas packing internals
- exact event routing pipeline internals beyond observable behavior
- render batch structure and shader-pipeline partitioning
- platform backend module structure

Stabilize behavior, not incidental data structures.

## Phase Plan

### Phase 0: Repository reshape

1. Convert `crates/sui` from a binary to the public library facade.
2. Move the current executable to `crates/sui-dev` or an `examples/` target.
3. Add the foundational crates with empty or minimal APIs and explicit dependency rules.

### Phase 1: Minimal vertical slice

Implement:

- `sui-core`
- `sui-layout`
- `sui-scene`
- `sui-runtime`
- `sui-render-wgpu`
- `sui-platform`
- `sui`

Exit criteria:

- one window
- retained widget tree
- pointer and keyboard input
- one-pass layout
- scene emission
- GPU presentation through `wgpu`
- explicit dirty invalidation

### Phase 2: Usable toolkit baseline

Implement:

- `sui-text`
- `sui-widgets`
- `sui-testing`
- semantic tree integration through the runtime

Exit criteria:

- basic text input and IME
- standard widgets
- semantic inspection
- deterministic automated UI tests

### Phase 3: Editor-oriented capabilities

Extend existing crates before adding many new ones. Only split new crates when the seam is already real in code.

Likely additions:

- `sui-canvas` for infinite-canvas and tiled-surface helpers
- `sui-media` for media widgets and timing adapters
- `sui-debug` for overlays, tracing, and diagnostics

### Phase 4: Bindings and advanced integration

Implement:

- `sui-bindings-core`
- `sui-python`
- `sui-js`

Exit criteria:

- supported Python and JavaScript object models
- command/event bridge with stable handles
- web embedding strategy aligned with the platform layer

## Immediate Repository Actions

The current repository is still at the starting point, so the next concrete implementation steps should be:

1. Reserve `sui` as the facade library crate.
2. Add `sui-core`, `sui-runtime`, `sui-layout`, `sui-scene`, `sui-render-wgpu`, and `sui-platform` as empty crates with documented dependencies.
3. Define the first public contracts in `sui-core`: IDs, geometry, color, events, semantics, and invalidation types.
4. Define the initial `Widget` contract in `sui-runtime` and re-export it from `sui`.
5. Keep all `wgpu` and host-windowing code out of the runtime from the start.

If these boundaries hold early, the later text, testing, and binding work stays additive instead of forcing a rewrite.