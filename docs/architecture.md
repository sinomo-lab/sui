# SUI Core Architecture

This document turns the goals in [design.md](./design.md) into a concrete core architecture for the current workspace.

It sits between the high-level product design and the crate boundary plan in [crate-architecture.md](./crate-architecture.md). The focus here is the runtime model: what the core subsystems are, how data moves through them, which crate owns which responsibility, and how the current scaffold should grow into the intended toolkit.

## Scope

This document defines the architecture for the core SUI runtime only. It does not try to fully specify higher-level widget libraries, text editing features, language bindings, or every renderer optimization. Those remain separate concerns built on top of the same foundation.

The core architecture must preserve the design constraints from [design.md](./design.md):

- retained widget state and stable identity
- event-driven updates rather than diff-driven reconciliation
- explicit invalidation for layout, paint, semantics, hit testing, text, and resources
- scene-based rendering rather than direct GPU ownership in widgets
- platform normalization at the edge of the system
- accessibility semantics as a first-class output of the widget tree
- a bindings-aware execution model with explicit ownership and scheduling

## Architectural Summary

SUI should be built around a retained widget runtime that produces two outputs from the same tree:

1. a scene frame for rendering
2. a semantics tree for accessibility, automation, and testing

The runtime is driven by normalized platform events. Widgets mutate local state in response to those events and request explicit invalidation. The runtime then schedules only the necessary work across layout, hit testing, scene generation, semantics recomputation, text work, and renderer resource updates.

At the architectural level, the system is split into six layers:

1. public API and application bootstrap
2. platform normalization and host integration
3. retained runtime and scheduling
4. layout, scene, and semantics production
5. renderer backend and GPU resource management
6. tooling surfaces for testing, inspection, and future bindings

The current workspace already contains the phase-1 skeleton of this design:

- `sui-core` owns shared data types and contracts
- `sui-layout` owns constraints and sizing primitives
- `sui-runtime` owns the retained runtime and widget contract
- `sui-scene` owns render-neutral paint output
- `sui-text` owns shaping, font registration, and text layout
- `sui-render-wgpu` owns renderer execution
- `sui-platform` owns host integration
- `sui` acts as the public facade
- `sui-dev` acts as the development host and now launches a real desktop window by default

The remaining work is to deepen these crates rather than replace them.

## Core Runtime Model

### 1. Retained widget graph

The canonical UI representation is a retained widget graph with stable identity. Every node has a `WidgetId`, explicit state, geometry, and semantic contribution.

The current runtime now uses retained `WidgetPod` child ownership and window-level graph snapshots. That foundation should continue to grow into a richer runtime-owned graph with the following responsibilities:

- parent-child ownership and lifecycle
- stable identity for focus, testing, semantics, and future collaboration hooks
- cached layout results and paint bounds
- per-node dirty flags
- event routing metadata
- hit-test participation

Widgets remain user-defined stateful units. The runtime owns graph orchestration, while widgets own domain state and behavior.

### 2. Explicit phases

The runtime should execute work in explicit phases instead of hiding behavior behind implicit redraw or reconciliation loops.

The core phases are:

1. ingest platform or application events
2. route events through the widget graph
3. collect invalidation requests
4. resolve scheduled work in dependency order
5. produce scene and semantics outputs
6. hand immutable frame data to the renderer and host layer

This phase separation is essential for determinism, debugging, automation, and future bindings.

### 3. Dirty invalidation rather than diffing

Widgets do not re-declare the whole UI on every update. Instead, event handlers mutate state and request invalidation explicitly. The runtime merges those requests and schedules the smallest safe amount of work.

The invalidation kinds already present in `sui-core` are the correct foundation:

- `Layout`
- `Paint`
- `HitTest`
- `Text`
- `Semantics`
- `Resources`

The scheduler should treat them as separate queues with dependency ordering:

- layout invalidation may imply paint, hit-test, and semantics updates
- text invalidation may imply layout and paint updates
- resource invalidation may require scene regeneration and renderer cache updates
- semantics invalidation must not require a repaint unless visual output also changed

## Window and Frame Architecture

Each window should be managed as an independent runtime island with shared process-level services.

Per-window state should include:

- root widget graph
- focus state
- pointer capture state
- IME and text-input state
- dirty queues and frame scheduling state
- last computed layout tree
- last produced scene frame
- last produced semantics tree
- renderer-facing surface state handle

Shared process-level services should include:

- image and font registries
- timers and async wakeups
- platform clipboard and drag-and-drop adapters
- renderer device and cache pools where sharing is valid
- debug and inspection channels

This keeps windows isolated enough for correctness while still enabling shared caches and tooling.

## Event Architecture

### Event sources

All runtime work starts from normalized events. Event sources include:

- pointer input
- keyboard input
- text and IME input
- focus transitions
- drag-and-drop
- window lifecycle changes
- timers and async commands
- custom application events

Platform-specific details must be translated in `sui-platform` before they reach widgets.

Platform adapters should normalize host coordinates into logical units before dispatch. When widgets need device-aware behavior, runtime contexts such as layout and paint should expose DPI metadata including scale factor, effective DPI, optional raw DPI, and the logical versus physical surface sizes. That keeps input and layout deterministic while still letting widgets opt into pixel-sensitive rendering decisions such as hairline borders or caret widths.

### Routing model

SUI should use a small, explicit routing model inspired by capture and bubble semantics without inheriting browser complexity.

The target delivery order is:

1. global runtime preprocessors
2. capture path from root to target
3. target widget
4. bubble path from target back to root
5. fallback runtime handlers such as focus traversal or default shortcuts

Routing decisions depend on:

- hit testing for pointer events
- focus ownership for keyboard and text events
- capture ownership for drag or gesture flows
- explicit runtime targets for synthetic or custom events

The event context should stay narrow. Widgets may mark events handled, request invalidation, enqueue commands, and query relevant runtime metadata. They should not directly mutate unrelated runtime internals.

## Layout Architecture

The default layout contract remains the one-pass model already implied by `sui-layout`:

1. parent sends constraints
2. child returns a size
3. parent positions children

This should remain the common path because it is predictable and cheap. The architecture must also support opt-in advanced behavior for:

- intrinsic measurement
- deferred text measurement
- virtualized collections
- free-positioned canvas containers
- multi-pass containers where child measurement influences sibling constraints

The runtime should separate three concepts clearly:

- layout algorithm definitions in `sui-layout`
- per-widget layout participation in `sui-runtime`
- cached geometry results in the runtime-owned graph

The layout phase should produce a geometry tree that is reused by paint, hit testing, semantics, and debug overlays.

## Scene and Paint Architecture

### Scene as the stable visual boundary

Widgets do not render by issuing raw GPU commands. They emit render-neutral scene commands into `sui-scene` through `PaintCtx`.

That scene is the stable boundary between application logic and renderer implementation. It enables:

- batching and draw ordering decisions outside widget code
- cached layers and partial redraw policies
- debug overlays and repaint visualization
- test harnesses that can inspect visual intent without requiring GPU internals
- renderer backend replacement without rewriting widgets

### Scene structure

The current `Scene` and `SceneFrame` types are a minimal version of the right idea. The target scene model should grow to represent:

- draw primitives such as fills, strokes, images, and text runs
- transforms and clip stacks
- opacity and compositing groups
- retained cached layer descriptors
- paint bounds for invalidation and culling
- optional debug and diagnostic annotations

The workspace now has the first concrete step in that direction: `sui-scene` supports explicit text runs, image draws, stroke rectangles, and clip/transform stack commands in addition to fills and clears, while `sui-text` owns font registration, shaping, and text layout. `sui-render-wgpu` consumes those layouts to rasterize glyph outlines and images without pushing text-system details back into widgets.

`SceneFrame` should remain immutable once produced so render backends can consume it safely and so tooling can inspect a stable snapshot.

## Semantics Architecture

Semantics are produced from the same widget graph as the scene. They are not a bolt-on adapter.

Each widget may contribute one or more semantic nodes describing:

- role
- bounds
- name and description
- value and state
- supported actions
- parent-child relationships

The runtime should assemble these contributions into a semantic tree that is synchronized with layout and focus state. The tree should be usable by:

- platform accessibility adapters
- automated test harnesses
- debug tools
- future AI or automation integrations

Semantics invalidation must remain independent from paint invalidation so non-visual state updates remain cheap.

## Platform Boundary

`sui-platform` is the only layer that should know about host windowing systems, DOM hosts, IME services, clipboard APIs, drag-and-drop backends, or accessibility bridges.

Its job is to:

- create and own host windows or embedded surfaces
- normalize native input into `sui-core` events
- hand those events to `sui-runtime`
- present renderer output to the host surface
- bridge semantics and focus state to platform accessibility APIs

The platform layer must not become a second runtime. It is an adapter around the retained runtime, not a peer architecture.

## Renderer Boundary

`sui-render-wgpu` owns scene compilation and GPU execution. It must consume immutable frame data and renderer resources only.

It is responsible for:

- translating scene commands into render passes
- maintaining texture, glyph, tile, and offscreen caches
- applying color-management transforms
- exposing explicit interop hooks for advanced custom GPU passes

It is not responsible for:

- widget behavior
- layout decisions
- focus or semantics logic
- platform event interpretation

This separation is critical if SUI is going to support debug tooling, test harnesses, and multiple host environments without coupling everything to `wgpu`.

## Crate Ownership Model

The current crate split is already close to the desired architecture. The ownership model should be:

### `sui-core`

Shared contracts and value types:

- IDs, geometry, color, events, invalidation, semantics, errors

This crate must remain free of renderer, runtime-graph, and platform dependencies.

### `sui-layout`

Reusable layout primitives and algorithms:

- constraints, alignment, padding, sizing helpers, container algorithms

It should remain runtime-neutral and renderer-neutral.

### `sui-scene`

The render-neutral visual representation:

- scene commands, brushes, frame snapshots, future layer and clip structures

This crate is the output target for widget painting and the input source for renderers.

### `sui-runtime`

The retained UI engine:

- widget graph
- event routing
- focus and capture
- invalidation scheduler
- layout orchestration
- scene production
- semantics assembly

This crate is the heart of the framework.

### `sui-render-wgpu`

The concrete GPU backend:

- scene compilation
- GPU resources
- caches
- presentation

### `sui-platform`

Host integration:

- windows and surfaces
- event normalization
- IME, clipboard, drag-and-drop, accessibility bridges

### `sui`

The public facade:

- stable application-facing API
- re-exports of core types
- feature-gated platform and renderer entry points

### `sui-dev`

The manual testbed and development app.

It should remain outside the stable API surface and evolve freely as the architecture grows.

Current workspace status:

- `cargo run -p sui-dev` launches the real desktop host through `sui::Application::run()`

## End-to-End Data Flow

The normal frame loop should be:

1. host receives native input or lifecycle changes
2. `sui-platform` translates them into normalized `sui-core::Event` values
3. `sui-runtime` routes the event through the retained widget graph
4. widgets mutate local state and emit invalidation requests
5. the runtime scheduler resolves the required layout, text, semantics, hit-test, paint, and resource work
6. widgets emit scene commands into a new `SceneFrame`
7. widgets emit semantic nodes into a new semantics snapshot
8. `sui-render-wgpu` consumes the scene frame and updates GPU resources as needed
9. `sui-platform` presents the result and updates platform accessibility state from the semantics snapshot

This gives SUI a clear unidirectional execution path without forcing a virtual-DOM-style diffing model.

## Scheduling and Threading

The scheduler should be single-owner even when work is parallelized.

That means:

- the runtime owns authoritative widget state on one thread or executor context
- scene preparation, text shaping, tile generation, and renderer compilation may be parallelized when safe
- platform and renderer callbacks must re-enter runtime state through explicit queues or commands

This model keeps state transitions deterministic and makes language bindings feasible. Python and JavaScript integrations can interact through explicit commands and snapshots rather than shared mutable Rust internals.

## Testing and Diagnostics Hooks

The architecture should treat inspection as a built-in capability, not a side effect.

The runtime should expose stable hooks for:

- event tracing
- invalidation tracing
- layout tree inspection
- scene snapshot inspection
- semantics snapshot inspection
- deterministic event injection

These hooks should operate on the same core artifacts used by the real runtime so tests and diagnostics observe real behavior instead of a separate simulation path.

## Phase-1 Target Versus Future Expansion

For the current workspace, the recommended phase-1 target is:

- keep `sui-core`, `sui-layout`, `sui-scene`, `sui-runtime`, `sui-render-wgpu`, `sui-platform`, `sui`, and `sui-dev`
- keep deepening `sui-runtime` beyond the current retained graph, scheduler, and focus foundations
- deepen `sui-scene` into a richer paint graph
- keep `sui-render-wgpu` as the only GPU-owning crate
- keep `sui-platform` as the only host-owning crate

After that foundation is stable, the next architecture-safe additions are:

- `sui-text` for first-class shaping and text layout
- `sui-widgets` for standard controls outside the runtime core
- `sui-testing` for deterministic automation
- `sui-bindings-core`, `sui-python`, and `sui-js` for cross-language adoption

These should be added by extending the existing boundaries, not by moving responsibilities out of the core crates retroactively.

## Concrete Next Implementation Steps

The architecture above implies the following implementation order:

1. deepen the retained `WidgetPod` model into richer container, hit-test, and lifecycle primitives without regressing stable identity
2. expand the frame scheduler with timers, async wakeups, and renderer-resource work queues
3. add pointer capture, richer event targeting, and default focus traversal to `sui-runtime`
4. evolve `sui-scene` from a flat command list into a richer scene frame with clips, transforms, text, and cached-layer descriptors
5. move platform execution in `sui-platform` to a real event pump that drives `sui-runtime` rather than calling a one-shot render path
6. expand semantics generation into a tree synchronized with layout, focus, and future accessibility actions

If these steps are followed, SUI will preserve the intent of the design document while staying compatible with the existing workspace structure.