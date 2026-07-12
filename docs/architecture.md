# SUI Architecture Overview

## What SUI Is Right Now

SUI core is organized around window/viewport targets, normalized events, retained
widget cooperation, and immutable scene frames.

The hard contracts are:

- `WindowId` identifies the host render/input target. That target may be a native
  platform window or an embedded viewport/region.
- `WindowEvent` and `Event` carry normalized input, wakeups, and lifecycle
  changes into the target identified by the delivery path.
- `Widget`, `WidgetPod`, and the runtime contexts define the standard retained
  widget cooperation protocol.
- `SceneFrame` is the immutable renderer-facing frame for a `WindowId`.
- The renderer consumes scene frames and never walks widget internals.

The retained `Widget`/`WidgetPod` runtime is the default core model. It is not a
claim that SUI owns every widget tree, every child identity, every dispatch rule,
or every thread a widget may use internally. Widgets may use local retained
children, generated children, virtualized children, worker-thread state, remote
systems, or custom rendering internally, then rejoin SUI through widget contexts
and scene output.

The public Rust entry point is `sui::Application`. With default features enabled,
`Application::run()` builds a `sui_runtime::Runtime` and hands execution to
`sui_platform::DesktopPlatform`.

## Main Execution Path

The normal desktop flow is:

1. User code creates an `Application` and one or more `WindowBuilder` values.
2. `sui::Application::run()` builds a `Runtime` with shared font and image registries.
3. `DesktopPlatform` creates host windows or targets, wires them to a shared `WgpuRenderer`, and enters the `winit` event loop.
4. Host events are normalized into `sui_core::Event` values and delivered for a `WindowId`.
5. `Runtime::handle_event(window_id, event)` routes the event through the retained widget protocol.
6. Widgets request explicit invalidation, timers, animation frames, async wakeups, focus, and pointer capture through contexts.
7. When a target needs work, the runtime runs measure, arrange, paint, semantics, and resource updates as needed.
8. `Runtime::render(window_id)` returns `RenderOutput` containing a `SceneFrame`, semantics, IME state, title, and diagnostics.
9. `sui-platform` submits the `SceneFrame` to `sui-render-wgpu` for the same `WindowId`.

Headless tests use the same runtime-facing path through `HeadlessPlatform`.

## Window And Event Contract

`WindowId` is the v1 target identifier. The name remains for compatibility, but
the meaning is broader than "native OS window": it can identify a platform
window, an offscreen target, or an embedded viewport/region in a host page or
application.

`WindowEvent` is the lifecycle/input-target event family. The target identity is
not embedded in `WindowEvent`; callers deliver it through APIs such as
`Runtime::handle_event(window_id, event)` and `SceneFrame.window_id`.

SUI core does not prescribe how a widget internally dispatches events to
subsystems. The retained runtime provides capture, target, and bubble routing as
the standard local widget implementation.

## Widget Protocol

The retained widget protocol is a core SUI concept.

Each `Widget` can implement:

- `event(&mut EventCtx, &Event)`
- `measure(&mut MeasureCtx, Constraints) -> Size`
- `arrange(&mut ArrangeCtx, Rect)`
- `paint(&PaintCtx)`
- `semantics(&mut SemanticsCtx)`
- `visit_children` and `visit_children_mut`

`WidgetPod` is the standard local adapter that gives a widget stable identity,
layout state, routing participation, and graph visibility. It is not the only
possible internal model a widget may use.

Child enumeration is logical enumeration. A widget may expose retained children,
generated children, remote children represented by local pods, or a virtualized
subset. The runtime uses the exposed logical children for cooperation tasks such
as graph snapshots, event paths, focus, hit testing, invalidation propagation,
and diagnostics. SUI does not require the exposed children to be a complete dump
of a widget's private implementation.

No `Widget: Send + Sync` bound is required. Thread-friendly widgets should keep
their own synchronization model, move immutable or synchronized snapshots across
threads, and request UI work through existing wake and invalidation contexts.

## Runtime Model

`sui-runtime` is the default retained-widget coordinator.

It owns:

- `Application`, `Runtime`, and `WindowBuilder`
- the `Widget` trait and widget contexts
- `WidgetPod`, `SingleChild`, and `WidgetChildren`
- event routing, focus, pointer capture, timers, animation-frame wakeups, and async wakeups
- invalidation scheduling
- measure and arrange execution
- scene and semantics generation
- runtime-side diagnostics and snapshots

The runtime does not own widget-local state policy. Widgets decide how to store
state, whether child systems live locally or remotely, and how much of their
internal model they expose through logical child enumeration.

## Layout, Paint, And Semantics

Layout is split into measure and arrange:

- `measure(&mut MeasureCtx, Constraints) -> Size`
- `arrange(&mut ArrangeCtx, Rect)`

Built-in widgets use this pipeline heavily, but layout helpers remain reusable
utilities. Custom widgets can combine SUI layout primitives with custom spatial
systems as long as they rejoin the retained runtime at the synchronization
points they choose to expose.

Widgets paint through `PaintCtx` into `sui-scene`. The renderer-facing payload is
`SceneFrame`, which carries `window_id`, viewport size, dirty regions, layer
updates, scene commands, and resource snapshots.

Semantics are produced through `SemanticsCtx` and included in `RenderOutput`.
They are consumed by accessibility bridges, testing locators, debug tooling, and
widget-book validation. On Windows desktop, `sui-platform` translates the same
immutable semantic snapshot into an AccessKit tree and publishes it through the
native UI Automation provider. UI Automation actions return through the host
event loop as typed semantic actions, so widget-owned state is still mutated by
the runtime's normal UI-thread event dispatch.

## Platform Boundary

`sui-platform` is the host integration layer.

It owns:

- desktop window lifecycle with `winit`
- host event normalization into `sui_core::Event`
- redraw scheduling for `WindowId` targets
- accessibility snapshot bridging, including native Windows UI Automation
- deterministic headless execution
- renderer registration and submission for each target

It does not own widget state, widget-local layout policy, child identity, or
scene production policy. In this implementation pass, desktop and headless
platforms continue to drive `sui_runtime::Runtime` directly.

## Renderer Boundary

`sui-render-wgpu` is the only crate that owns `wgpu` concepts.

It provides:

- shared device and queue setup
- per-target surface or offscreen registration
- retained compositor state per `WindowId`
- text, image, and analytic path caches
- capture helpers for tests and desktop harnesses
- renderer statistics consumed by diagnostics tooling

The renderer consumes `SceneFrame` snapshots. It does not know whether the scene
came from a conventional retained widget tree, a virtualized widget, a widget
using worker-thread state, or a remote subsystem represented through local
commands.

See [renderer-architecture.md](./renderer-architecture.md) for the renderer
model and constraints.

## Diagnostics And Tooling

Diagnostics are part of the real runtime path.

The current stack includes:

- `WindowPerformanceSnapshot` and related phase timings in `sui-runtime`
- renderer submission stats published by `sui-platform`
- animation counters for active widgets and frame wakeups
- the widget book performance overlay in `sui-demo`
- inspector widgets in `sui-debug`
- semantics-first locators and artifact capture in `sui-testing`

Tests and tools should reuse real runtime outputs instead of inventing a second
widget or rendering model.

## Where To Start When Making Changes

- Window/event target semantics: start in `sui-core`, then adapt `sui-runtime`
  and `sui-platform`.
- Widget protocol, routing, invalidation, or graph behavior: start in
  `sui-runtime::widget` and `sui-runtime`.
- Layout primitives: start in `sui-layout`.
- Paint payloads or scene commands: start in `sui-scene`, then follow through
  `sui-render-wgpu`.
- Accessibility or locator behavior: start in semantics generation, then check
  `sui-testing` and `sui-platform::accessibility`.
- Renderer behavior or performance: start in `sui-render-wgpu`, then validate
  with widget-book diagnostics and targeted tests.
