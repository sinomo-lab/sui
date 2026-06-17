# SUI Rendering Architecture

## Current Renderer Contract

The renderer boundary is:

- widgets paint into `sui-scene`
- the runtime packages the result as a `SceneFrame`
- `sui-render-wgpu` consumes the immutable frame snapshot
- `sui-platform` presents the renderer output on desktop or captures it offscreen in headless mode

The renderer does not walk widget internals, compute layout, or own application state.

## Scene Inputs

The current scene model provides more than a flat draw list.

`SceneFrame` includes:

- the root `Scene`
- layer descriptors and nested `SceneLayer` nodes
- typed `SceneLayerUpdate` records
- dirty regions
- font registry snapshots
- image registry snapshots

Layer descriptors currently carry:

- a stable `SceneLayerId`
- owner widget identity
- layer bounds
- content bounds
- paint bounds
- presentation-only `LayerProperties` such as opacity and translation
- composition mode hints
- stack-surface ordering and transient ownership metadata where relevant

That means the renderer does not have to infer basic layer ownership or bounds from raw draw commands. It also means the runtime's choice of where to emit `SceneLayer` nodes has a direct effect on retained-compositor cost.

## Retained Compositor Model

`crates/sui-render-wgpu` uses a retained compositor per window.

The important implementation consequences are:

- layer state survives across frames
- retained packets for direct draws are reused when possible
- property-like changes are handled separately from content rebuilds
- the old frame-global scene-to-draw-op compiler is no longer the live path

In practice, each window keeps retained state for:

- layer structure
- transform, clip, and effect nodes
- retained packet data for direct draws
- per-window GPU resources and submission stats

One important caveat is that retained-compositor cost still depends directly on where the runtime chooses to emit `SceneLayer` boundaries. That makes the runtime's boundary policy a first-order renderer concern even though the renderer itself does not walk widget internals.

The animation fast path builds on that same contract. `sui-render-wgpu` can cheaply update retained presentation state only when the runtime has emitted an explicit paint boundary and the scene update is limited to presentation-only properties such as opacity or translation. If content, ordering, or resources change, the renderer still has to rebuild retained packets.

## Current Architectural State

The current implementation is closer to the intended model, but the layer-boundary migration is not fully complete. Ordinary widgets are still supposed to treat scene layers as an opt-in repaint or composition boundary, while overlays, stack surfaces, and other explicit paint-boundary widgets are the main consumers of retained composition features.

That means the retained compositor now usually sees:

- explicit scroll, overlay, stack-surface, and other opt-in composition boundaries
- retained presentation-only updates on those explicit layers when opacity or translation changes without content rebuilds
- a transitional mix of explicit boundaries and older per-widget layer accounting in some diagnostics paths

This still matters because retained traversal, structural comparison, and packet upkeep are paid per layer or packet, not just per draw command. The architectural direction is to keep default paint as flat as practical, keep `SceneLayer` explicit, and optimize the smaller set of meaningful retained surfaces instead of reintroducing broad wrapper-driven layerization.

## Render Modes

The current practical retained path is:

- retained layer structure
- retained direct packets
- overlay, scroll, and stack-surface composition handling

Older retained-cache terminology still appears in parts of the repo and older docs, but broad retained tiling is no longer the preferred architectural direction for the main renderer path. Current work should assume that lowering structural layer cost is more important than reviving generic cache-policy knobs.

## Frame Pipeline

For a single window, the renderer-facing path is:

1. `Runtime::render()` produces a `SceneFrame`.
2. The renderer compares the new frame against retained compositor state.
3. Typed `SceneLayerUpdate` values invalidate retained content conservatively.
4. Property-only updates can stay on the retained fast path when they touch presentation data such as opacity or translation on an explicit paint boundary.
5. Dirty, structurally changed, or newly visible content is rebuilt.
6. Reusable retained fragments are composed into the final pass sequence.
7. The renderer submits GPU work and records frame statistics.

The steady-state goal is reuse. On a good frame, the renderer should mostly reposition or recompose retained content rather than regenerate everything.

Today, the main remaining source of overhead is no longer default wrapper-driven layerization. Instead, cost is concentrated in the smaller set of real retained boundaries plus the direct packets and rebuild work they drive. That makes layer counts, packet rebuild reasons, and direct-packet churn the most useful indicators when evaluating current renderer changes.

## Text And Image Flow

### Text

Text work is split across three crates.

- `sui-text` owns font registration, shaping, and text layout.
- `sui-scene` carries shaped text and text draw commands.
- `sui-render-wgpu` resolves fonts from the frame snapshot and rasterizes the result.

There are two important caches today:

- `TextSystem` caches shaped layouts by text and layout inputs.
- the renderer caches glyph-related data for repeated draws.

The renderer also owns a grayscale text coverage policy for glyph alpha generation. It defaults to linear coverage for all text colors, and `WindowRenderOptions` can override it for an active window. The resolved policy is applied when rasterizing atlas glyphs and when emitting analytic text fallback coverage, which makes text-edge tuning a renderer/runtime concern rather than a widget or layout concern.

This split keeps text measurement and shaping out of renderer internals while still letting widgets and the runtime share those utilities and letting the renderer optimize repeated output.

### Images

Image handles are registered through the runtime and snapshot into the frame. The renderer resolves them from the immutable image registry snapshot, uploads textures as needed, and caches GPU-side representations.

## Diagnostics

Renderer diagnostics are already part of the normal frame path.

The renderer publishes metrics including:

- render pass count
- draw count
- uploaded vertex bytes
- visible layer count
- direct packet count
- retained packet rebuild totals and reasons
- retained packet build and composition cost
- text-atlas and analytic-path timings
- GPU upload, encode, submit, and present timings

Those metrics are surfaced by `sui-platform`, shown by the widget-book overlay, and available to tests and debugging tools.

The current live diagnostics also pair renderer metrics with runtime animation counters, so the widget book can distinguish animation frames that repainted content from frames that only updated retained transform or opacity state.

## Current Benchmark Snapshot

In this environment, the live desktop benchmark tests need a real display server, so the current-status snapshot below was captured with the headless widget-book diagnostic benchmarks in `crates/sui-widget-book/src/lib.rs`.

Current headless scroll snapshot:

- full widget-book scroll surface
  - avg frame time: `3.036 ms` (`329.3 fps`)
  - p95 frame time: `4.186 ms`
  - avg visible layers: `20.62`
  - avg direct packets: `11.83`
  - avg packet rebuilds: `9.00`
  - avg repaint boundaries / scene layers: `6.88` / `6.88`
- overlay-free gallery-only scroll surface
  - avg frame time: `1.871 ms` (`534.4 fps`)
  - p95 frame time: `2.944 ms`
  - avg visible layers: `14.62`
  - avg direct packets: `9.83`
  - avg packet rebuilds: `8.00`
  - avg repaint boundaries / scene layers: `4.88` / `4.88`

These numbers are not a replacement for real desktop benchmarking on a machine with an active display server, but they provide a stable current-status snapshot of retained traversal, rebuild, and layer-cardinality cost after the explicit-boundary transition work.

## Current Constraints

These rules are the ones most likely to matter when changing renderer code.

### 1. Treat `SceneFrame` as immutable input

Renderer code should derive GPU work from the frame snapshot and retained renderer state. Do not pull mutable runtime state into the renderer as a shortcut.

### 2. Invalidate by renderer needs, not only by minimal repaint area

The runtime may paint only a narrow region, but the renderer still has to invalidate any retained fragments that are no longer valid. Cache invalidation coverage must follow renderer correctness, not only the minimal runtime repaint set.

### 3. Prune caches when scene layers disappear

Widget ids are monotonic and dynamic subtrees churn. Retained renderer caches must discard entries for layers that are no longer present in the current scene or memory will grow without bound.

### 4. Keep scrolling and transform work on the retained fast path

Many regressions show up first in scroll-heavy views. If a change forces scroll-like updates back into broad content rebuilds, it will usually show up quickly in widget-book performance runs.

### 5. Keep diagnostics cheap enough for live use

The repo already uses live diagnostics in the widget book. Detailed metrics are valuable, but they still have to coexist with normal frame rendering.

## Known Performance Shape

The current renderer is already on the retained compositor path, but there are still active performance constraints worth knowing before tuning code.

- text-heavy frames can still dominate vertex upload volume
- layer-heavy frames can bottleneck on retained traversal, packet upkeep, and composition structure management even when raw scene-command counts look modest
- surface acquisition can be a visible tail-latency component on desktop
- bind-group and upload behavior around analytic paths and text atlas updates still matters on miss-heavy frames

The widget book benchmark surfaces and overlay are the preferred way to validate renderer changes against those costs. For architecture work, also track widget count, scene-layer count, stack-surface count, and packet rebuild reasons, because those often explain performance better than raw draw counts.

## Where To Work On Common Problems

- wrong layer invalidation or stale visuals: `sui-runtime` plus `sui-render-wgpu`
- new scene command support: `sui-scene` plus `sui-render-wgpu`
- cache behavior or memory growth: `sui-render-wgpu`
- text rendering regressions: `sui-text`, `sui-scene`, and `sui-render-wgpu`
- window presentation or surface issues: `sui-platform` plus `sui-render-wgpu`

## Validation Workflow

For renderer work, the usual validation loop is:

1. run `cargo run -p sui-dev`
2. exercise the widget book and benchmark tabs
3. check the live performance overlay
4. run targeted tests, especially widget-book desktop tests when the change affects rendering or scrolling

If a renderer change can only be argued from code inspection and not from the widget book, benchmark surfaces, or tests, the validation is probably too weak.
