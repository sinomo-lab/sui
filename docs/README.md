# SUI Documentation

This directory contains documentation for the current workspace.

The design goals still live in [design.md](./design.md). The other documents describe the code that exists today: how the workspace is organized, how frames move through the runtime, how the renderer works, and how tests are expected to be written.

## Read Order

Suggested reading order:

1. [architecture.md](./architecture.md) for the runtime and frame pipeline.
2. [stack-hosts.md](./stack-hosts.md) for the planned stacking, popup-host, and multi-bounds model.
3. [crate-architecture.md](./crate-architecture.md) for crate ownership and dependency boundaries.
4. [layout-and-overflow.md](./layout-and-overflow.md) for the simplified sizing and overflow model.
5. [text-system.md](./text-system.md) for the planned text-engine refactor and long-term text goals.
6. [renderer-architecture.md](./renderer-architecture.md) for scene, compositor, and renderer details.
7. [testing.md](./testing.md) for headless tests, desktop harness tests, and visual artifacts.
8. [design.md](./design.md) for the long-term product and API direction.
9. [hdr-native-interface-manifesto.md](./hdr-native-interface-manifesto.md) for the intended HDR-native visual language and design constraints.
10. [hdr-theme-token-schema-proposal.md](./hdr-theme-token-schema-proposal.md) for the proposed HDR-aware theme and token model.

## Workspace At A Glance

The current workspace is organized around a retained widget runtime.

- `sui` is the public Rust facade.
- `sui-core` owns shared types such as events, geometry, color, semantics, and invalidation kinds.
- `sui-animation` owns pure animation timelines, keyframes, playback state, editor state, and sampled values.
- `sui-layout` owns layout primitives, constraints, and reusable measure/arrange utilities.
- `sui-runtime` owns windows, the retained widget graph, event routing, invalidation scheduling, the default widget-tree layout pipeline, scene generation, and semantics generation.
- `sui-scene` defines the renderer-neutral scene representation.
- `sui-text` owns font registration, shaping, measurement, and text layout.
- `sui-render-wgpu` owns the retained compositor and the `wgpu` backend.
- `sui-platform` owns desktop and headless platform integration.
- `sui-widgets` owns built-in widgets and theme types.
- `sui-testing` owns deterministic UI automation helpers.
- `sui-debug` owns reusable debug widgets and inspectors.
- `sui-widget-book` owns the gallery, story content, and screenshot-oriented tests.
- `sui-dev` is the main development host application.

One important implementation detail to keep in mind: the runtime is still in the layer-boundary transition. Explicit paint boundaries are the intended retained-compositor and retained-animation boundary, but some diagnostics still report emitted `SceneLayer` counts while that decoupling work finishes.

## Common Commands

Common commands:

```bash
cargo run -p sui-dev
cargo test
cargo test -p sui-testing
cargo test -p sui-widget-book -- --nocapture
```

`cargo run -p sui-dev` launches the desktop development host.

The `sui-dev` host includes the widget book, focused benchmark views, and renderer settings in one floating workspace.

`cargo test -p sui-widget-book -- --nocapture` writes visual artifacts under `target/ui-artifacts/sui-widget-book`.

## Mental Model

Most work in the repo follows the same path:

1. `sui-platform` turns host events into `sui_core::Event` values.
2. `sui-runtime` routes the event through the retained widget tree.
3. Widgets request explicit invalidation, timers, or animation frames.
4. The runtime runs measure, arrange, paint, semantics, and resource work as needed.
5. The runtime produces a `SceneFrame` plus diagnostics.
6. `sui-render-wgpu` updates retained compositor state and presents the result.

The key rule is that widgets do not talk directly to `wgpu` or host window APIs. They operate through runtime contexts and emit scene content. For efficient retained animation, explicit paint-boundary widgets may also expose presentation-only layer properties such as opacity and translation; the runtime and renderer handle the rest.
