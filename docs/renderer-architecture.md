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

One important caveat is that the current runtime feeds the renderer far more layers than intended: many ordinary widget paint operations become their own `SceneLayer` nodes. That makes retained compositor cost track widget-tree shape much more closely than the intended architecture.

## Current Architectural Drift

The current live path is not "one explicit surface per meaningful repaint boundary." Instead, `WidgetPod::paint` currently wraps each widget's paint output into a `SceneLayer` by default in `sui-runtime`. That means the retained compositor often sees:

- many tiny layers with only a few scene commands each
- high layer counts driven by wrappers such as `Padding`, `Stack`, `SizedBox`, and `Background`
- overlay and stack-surface layers mixed into an already fragmented layer tree

This matters because retained traversal, structural comparison, and packet upkeep are paid per layer or packet, not just per draw command.

The current architectural direction is to reduce default layerization and reserve `SceneLayer` for explicit repaint or composition boundaries.

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
4. Dirty or newly visible content is rebuilt.
5. Reusable retained fragments are composed into the final pass sequence.
6. The renderer submits GPU work and records frame statistics.

The steady-state goal is reuse. On a good frame, the renderer should mostly reposition or recompose retained content rather than regenerate everything.

Today, an important source of overhead is that the runtime often emits far more `SceneLayer` nodes than the renderer actually needs for correctness. That increases layer traversal, packet comparison, and structure-management cost before the GPU submission path even matters.

## Text And Image Flow

### Text

Text work is split across three crates.

- `sui-text` owns font registration, shaping, and text layout.
- `sui-scene` carries shaped text and text draw commands.
- `sui-render-wgpu` resolves fonts from the frame snapshot and rasterizes the result.

There are two important caches today:

- `TextSystem` caches shaped layouts by text and layout inputs.
- the renderer caches glyph-related data for repeated draws.

The renderer also owns a grayscale text coverage policy for glyph alpha generation. By default it resolves automatically from text luminance, so dark text and light text can coexist in the same window while using different coverage curves. The resolved policy is applied when rasterizing atlas glyphs and when emitting analytic text fallback coverage, which makes text-edge tuning a renderer concern rather than a widget or layout concern.

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
