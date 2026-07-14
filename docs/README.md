# SUI Documentation

This directory contains both documentation for the current workspace and clearly separated design/roadmap material. Documents under **Current implementation** describe code that exists today. Documents under **Design and roadmap** are proposals, plans, or directional constraints and should not be read as shipped API guarantees.

## Current Implementation

Suggested reading order:

1. [user-api.md](./user-api.md) — supported Rust, Python, and JavaScript entry points.
2. [architecture.md](./architecture.md) — runtime and frame pipeline.
3. [crate-architecture.md](./crate-architecture.md) — crate ownership and dependency boundaries.
4. [layout-and-overflow.md](./layout-and-overflow.md) — sizing and overflow behavior.
5. [renderer-architecture.md](./renderer-architecture.md) — scene, compositor, and renderer details.
6. [testing.md](./testing.md) — headless tests, desktop harness tests, and visual artifacts.
7. [text-rendering-benchmarks.md](./text-rendering-benchmarks.md) — current text benchmark procedure and results.

The binding-specific READMEs contain build instructions and examples:

- [Python binding README](../crates/sui-python/README.md)
- [JavaScript binding README](../crates/sui-js/README.md)

## Design And Roadmap

- [design.md](./design.md) — long-term product and API direction.
- [stack-hosts.md](./stack-hosts.md) — planned stacking, popup-host, and multi-bounds model.
- [text-system.md](./text-system.md) — planned text-engine refactor and long-term text goals.
- [hdr-native-interface-manifesto.md](./hdr-native-interface-manifesto.md) — intended HDR-native visual language.
- [hdr-theme-token-schema-proposal.md](./hdr-theme-token-schema-proposal.md) — proposed HDR-aware token model.
- [plans/](./plans/) — implementation plans. The cross-language binding plan is historical context for the now-implemented alpha bindings; remaining items are not promises of shipped behavior.

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
- `sui-tui` owns accessibility-tree generated terminal UI support.
- `sui-bindings-core` owns the shared binding model and host-driven runtime adapters.
- `sui-python` and `sui-js` own the native Python and Node/Electron surfaces.
- `sui-demo` owns the main development host, widget book, story content, and screenshot-oriented tests.

One important implementation detail to keep in mind: the runtime is still in the layer-boundary transition. Explicit paint boundaries are the intended retained-compositor and retained-animation boundary, but some diagnostics still report emitted `SceneLayer` counts while that decoupling work finishes.

## Common Commands

Common commands:

```bash
cargo run -p sui-demo
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p sui-testing
cargo test -p sui-demo -- --nocapture
```

`cargo run -p sui-demo` launches the desktop development host.

The `sui-demo` host includes the widget book, focused benchmark views, and renderer settings in one floating workspace.

`cargo test -p sui-demo -- --nocapture` writes visual artifacts under `target/ui-artifacts/sui-demo/widget-book`.

## Mental Model

Most work in the repo follows the same path:

1. `sui-platform` turns host events into `sui_core::Event` values.
2. `sui-runtime` routes the event through the retained widget tree.
3. Widgets request explicit invalidation, timers, or animation frames.
4. The runtime runs measure, arrange, paint, semantics, and resource work as needed.
5. The runtime produces a `SceneFrame` plus diagnostics.
6. `sui-render-wgpu` updates retained compositor state and presents the result.

The key rule is that widgets do not talk directly to `wgpu` or host window APIs. They operate through runtime contexts and emit scene content. For efficient retained animation, explicit paint-boundary widgets may also expose presentation-only layer properties such as opacity and translation; the runtime and renderer handle the rest.
