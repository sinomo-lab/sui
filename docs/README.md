# SUI Documentation

This directory contains documentation for the current workspace.

The design goals still live in [design.md](./design.md). The other documents describe the code that exists today: how the workspace is organized, how frames move through the runtime, how the renderer works, and how tests are expected to be written.

## Read Order

Suggested reading order:

1. [architecture.md](./architecture.md) for the runtime and frame pipeline.
2. [stack-hosts.md](./stack-hosts.md) for the planned stacking, popup-host, and multi-bounds model.
3. [crate-architecture.md](./crate-architecture.md) for crate ownership and dependency boundaries.
4. [text-system.md](./text-system.md) for the planned text-engine refactor and long-term text goals.
5. [renderer-architecture.md](./renderer-architecture.md) for scene, compositor, and renderer details.
6. [testing.md](./testing.md) for headless tests, desktop harness tests, and visual artifacts.
7. [design.md](./design.md) for the long-term product and API direction.

## Workspace At A Glance

The current workspace is organized around a retained widget runtime.

- `sui` is the public Rust facade.
- `sui-core` owns shared types such as events, geometry, color, semantics, and invalidation kinds.
- `sui-layout` owns layout primitives and constraints.
- `sui-runtime` owns windows, the retained widget graph, event routing, invalidation scheduling, layout, scene generation, and semantics generation.
- `sui-scene` defines the renderer-neutral scene representation.
- `sui-text` owns font registration, shaping, measurement, and text layout.
- `sui-render-wgpu` owns the retained compositor and the `wgpu` backend.
- `sui-platform` owns desktop and headless platform integration.
- `sui-widgets` owns built-in widgets and theme types.
- `sui-testing` owns deterministic UI automation helpers.
- `sui-debug` owns reusable debug widgets and inspectors.
- `sui-widget-book` owns the gallery, story content, and screenshot-oriented tests.
- `sui-dev` is the main development host application.

## Common Commands

Common commands:

```bash
cargo run -p sui-dev
cargo test
cargo test -p sui-testing
cargo test -p sui-widget-book -- --nocapture
```

`cargo run -p sui-dev` launches the desktop development host.

`cargo test -p sui-widget-book -- --nocapture` writes visual artifacts under `target/ui-artifacts/sui-widget-book`.

## Mental Model

Most work in the repo follows the same path:

1. `sui-platform` turns host events into `sui_core::Event` values.
2. `sui-runtime` routes the event through the retained widget tree.
3. Widgets request explicit invalidation.
4. The runtime runs measure, arrange, paint, semantics, and resource work as needed.
5. The runtime produces a `SceneFrame` plus diagnostics.
6. `sui-render-wgpu` updates retained compositor state and presents the result.

The key rule is that widgets do not talk directly to `wgpu` or host window APIs. They operate through runtime contexts and emit scene content.
