# Tile-Based Retained Compositor Proposal

## Goal

Define a higher-ceiling rendering architecture for SUI that can plausibly sustain smooth `120 fps` presentation for highly dynamic widget trees, including cases where some widgets request repaint every frame and where layout or viewport changes are frequent.

This document describes the target design only. It does not define a migration plan, implementation ordering, or short-term compatibility steps.

The intended outcome is a full replacement of the current rendering path, not a compatibility layer that preserves the current renderer's structure, invalidation model, or failure modes.

## Why Replace the Current Renderer Architecture

The current architecture already has useful foundations:

- widgets emit render-neutral scene commands rather than raw GPU commands
- the runtime tracks explicit invalidation kinds
- the runtime can emit incremental layer updates rather than only full-frame paint results
- the renderer has a retained compositor scaffold with per-window state, property trees, and direct packets

That is not enough for a high-refresh, highly dynamic UI.

The current model still leaves too much work on the hot frame path:

- runtime-side paint still produces fresh scene content for dirty layers
- renderer-side direct packet rebuild still regenerates draw operations for content changes because tile reuse does not exist yet
- final batch preparation and vertex upload are still fundamentally frame-scoped
- scrolling, panning, transforms, and similar moves are not yet treated as first-class composition-only updates
- layout graph and hit-test graph maintenance are still more rebuild-oriented than incrementally retained

At `120 fps`, the total frame budget is about `8.33 ms`. Any architecture that regularly walks large parts of the widget scene, re-tessellates vector content, re-batches geometry, and re-uploads substantial vertex data every frame will run out of headroom quickly.

## Design Summary

SUI should keep the existing scene boundary, but replace the renderer-side execution model with a tile-based retained compositor.

The core shift is:

1. widgets still emit scene content
2. the runtime still owns widget state, layout, and invalidation
3. a retained compositor owns persistent visual composition state
4. the compositor breaks eligible layers into tiles, caches tile outputs, and handles property-only updates without repainting content
5. the wgpu backend becomes a compositor executor rather than the place where most frame work is rebuilt from scratch

The target frame pipeline is no longer "scene snapshot in, full draw preparation out". It becomes "scene delta in, retained composition state updated, minimal tile and pass work out".

## Replacement Stance

This proposal assumes a clean architectural break from the current rendering system.

That means:

- the current renderer should be treated as disposable implementation history, not as a compatibility target
- the new compositor should not inherit old abstractions just because they already exist
- runtime, scene, and renderer contracts may change aggressively where needed to fit the retained compositor model
- any temporary bridge between old and new code should be short-lived and intentionally deleted once the replacement path is viable
- the end state must be a single compositor-driven rendering architecture rather than a permanent mixture of old and new execution models

The purpose of staging is to manage engineering risk, not to preserve legacy behavior.

## Non-Goals

This proposal does not aim to:

- turn SUI into a browser-style CSS or DOM compositor
- require every widget subtree to render into an offscreen texture unconditionally
- eliminate direct drawing paths for tiny or extremely volatile content
- define the migration sequence from the current renderer
- standardize every future optimization up front
- preserve compatibility with the current renderer's internal architecture
- keep a long-lived dual path where both the old renderer and the new compositor remain first-class
- retain legacy invalidation, scene, or frame-building behavior if it blocks the retained compositor design

The design should leave room for multiple rendering strategies under one retained compositor instead of forcing all content through a single expensive path.

## Core Architectural Model

### 1. Scene remains the widget-facing paint boundary

Widgets continue to emit render-neutral scene commands through `PaintCtx` into `sui-scene`.

That remains the correct abstraction boundary because it preserves:

- widget and renderer decoupling
- testability and debug inspection
- renderer backend replacement
- explicit visual intent separate from GPU resource lifetime

The compositor is introduced after scene production, not in place of it.

### 2. Introduce a retained compositor state

The renderer should stop treating every frame as a mostly fresh compilation problem. Instead, each window should own a retained compositor state containing:

- a retained tree of compositor layers
- property trees for transform, clip, and visual effects
- per-layer content versions
- tile metadata and tile cache entries
- GPU-side resources such as tile textures, atlases, and offscreen targets
- cached render packets or draw bundles for volatile direct-draw content

Conceptually:

```rust
struct CompositorState {
    root_layer: LayerId,
    layers: SlotMap<LayerId, RetainedLayer>,
    transforms: SlotMap<TransformNodeId, TransformNode>,
    clips: SlotMap<ClipNodeId, ClipNode>,
    effects: SlotMap<EffectNodeId, EffectNode>,
    tiles: HashMap<TileKey, TileEntry>,
    surfaces: HashMap<SurfaceId, SurfaceEntry>,
}

struct RetainedLayer {
    widget_id: WidgetId,
    bounds: Rect,
    content_bounds: Rect,
    transform: TransformNodeId,
    clip: ClipNodeId,
    effect: EffectNodeId,
    content: LayerContentHandle,
    content_version: u64,
    structure_version: u64,
    render_mode: LayerRenderMode,
    tile_grid: TileGrid,
}
```

The important point is persistence. Layers and their visual properties survive across frames and change incrementally.

### 3. Separate content from composition

The compositor must distinguish two very different classes of updates:

- content updates: pixels would change if the layer were re-rendered
- composition updates: the already-produced content is moved, clipped, faded, reordered, or otherwise recomposited without changing its internal pixels

Examples:

- changing scroll offset inside a scrolled viewport is usually a composition update for the scrolled content container plus a visibility update for edge tiles
- moving a floating panel is a transform update
- animating opacity is an effect update
- resizing a viewport may be arrange work plus tile visibility churn, not a full repaint of every descendant
- changing button fill color is a content update for that button layer

This separation is the central reason to build a compositor at all.

### 4. Use property trees rather than flattening everything into layer-local state

Transform, clip, and effect inheritance should be represented explicitly rather than baked repeatedly into each cached fragment.

The compositor should maintain three retained property trees:

- transform tree
- clip tree
- effect tree

This gives SUI a predictable way to handle:

- nested scrolling and panning
- transforms on large subtrees
- clip stack reuse
- opacity and blend groups
- partial subtree invalidation when only one property branch changes

This is a better fit for a high-refresh compositor than repeatedly re-hashing flattened state into fragment cache keys.

## Tile Model

### Tile granularity

Each cacheable compositor layer should be divided into logical-space tiles.

Tile size should be fixed in device pixels per scale bucket, for example:

- `256 x 256` as the default
- `128 x 128` for text-heavy or clip-heavy content where smaller damage is common
- `512 x 512` only for unusually simple broad surfaces

The exact size is a tuning choice, but the architecture should assume a fixed-size grid with optional border padding for anti-aliasing and filter safety.

### Tile cache entries

Each tile entry should represent a reusable piece of already-produced content. The output may be:

- a texture tile for cached content
- a retained GPU draw packet for content that is cheaper to redraw than to raster-cache

The compositor should support both, but texture tiles are the primary mechanism for expensive or spatially large retained content.

Conceptually:

```rust
struct TileKey {
    layer: LayerId,
    tile_x: i32,
    tile_y: i32,
    scale_bucket: u32,
    content_version: u64,
    resource_epoch: u64,
}

struct TileEntry {
    key: TileKey,
    dirty: bool,
    visible: bool,
    last_used_frame: u64,
    memory_cost: usize,
    payload: TilePayload,
}

enum TilePayload {
    Texture(TextureTileHandle),
    DirectPacket(RenderPacketHandle),
}
```

The key must reflect content identity, not transient compositor position. A translated layer should not invalidate all its tiles just because the viewport moved.

### Layer render modes

Not every layer should use the same caching strategy. The compositor should choose between three primary modes:

```rust
enum LayerRenderMode {
    Direct,
    CachedTiles,
    OffscreenSurface,
}
```

`Direct`

- for tiny, simple, or extremely volatile content
- content is compiled into a retained draw packet and submitted directly
- avoids tile overhead when caching would churn every frame anyway

`CachedTiles`

- default for large retained content, scrolling content, canvas-like views, and complex vector scenes
- content is cached per tile and recomposited when only transforms or clips change

`OffscreenSurface`

- for effect groups that require intermediate compositing, such as backdrop-like effects, complex blend groups, or shader-based subtree treatment
- should be explicit and relatively rare because it can be expensive

This hybrid model matters. A pure tile cache is too rigid, and a pure direct path leaves performance on the table.

### Tile lifecycle

Each frame, a tile can be:

- reused unchanged
- reprojected or repositioned without rerendering
- marked dirty by content invalidation
- newly exposed because viewport or clip changed
- evicted because of memory pressure or scale change

The expected steady-state behavior for smooth scrolling is heavy tile reuse, not heavy tile regeneration.

## Frame Pipeline

The target pipeline per window is:

1. runtime ingests events and updates widget state
2. runtime resolves layout, arrange, text, resource, and semantics work
3. widgets emit scene updates for layers whose content changed
4. compositor updates retained layer state and property trees
5. compositor computes visible tiles and damage for the current viewport
6. compositor schedules raster or packet rebuild work only for dirty or newly exposed tiles
7. tile generation or packet compilation runs in parallel where possible
8. compositor builds a small set of composition passes from retained tiles, direct packets, and effect surfaces
9. wgpu backend executes those passes and presents the final image

The critical difference from the current path is that steps `5` through `9` are incremental and retained rather than frame-global.

## Invalidation Model

The invalidation system needs to grow from render-adjacent categories into compositor-aware categories.

The important distinction is not only what changed, but where the change should stop.

### Required invalidation classes

- `Measure`: desired size or intrinsic metrics changed
- `Arrange`: final placement changed but measured size may still be reusable
- `Content`: layer pixels changed
- `Transform`: layer or subtree moved in compositor space
- `Clip`: clip stack or clip geometry changed
- `Effect`: opacity, blend, or surface-group behavior changed
- `Visibility`: viewport exposure changed without content mutation
- `Resources`: images, fonts, GPU handles, or shader resources changed
- `Semantics`: accessibility or test-facing state changed without visual impact

### Propagation rules

These classes should not all propagate equally.

- `Content` invalidation marks only the relevant layer tiles dirty
- `Transform` invalidation should usually preserve tile payloads and only update composition data
- `Clip` invalidation should preserve interior tile payloads when the clip shrinks or moves, but may expose new edge tiles
- `Arrange` invalidation may produce `Transform`, `Clip`, or `Content` invalidation depending on what actually changed
- `Resources` invalidation must bump epochs for dependent layers or tiles without forcing unrelated content rebuild

The architecture should be explicit about this. Treating every visual change as generic paint invalidation is what prevents a retained compositor from paying off.

## Runtime and Scene Requirements

The compositor architecture places new requirements on runtime and scene output.

### 1. Stable layer identity

Every compositor-participating layer needs stable identity across frames.

The existing `WidgetId` foundation is a good start, but the scene model should be able to distinguish:

- widget identity
- compositor layer identity
- effect or surface-group identity when a widget contributes multiple retained surfaces

A single widget may eventually own multiple compositor layers.

### 2. Explicit layer descriptors in the scene

The scene should evolve from a flat command stream with nested `Layer` nodes into a richer layer description that includes:

- bounds
- content bounds
- clip behavior
- transform anchor or property references
- opacity and compositing-group hints
- cache preference hints
- paint bounds
- z-order or stacking constraints

Widgets still should not choose GPU implementation details, but they should be able to express composition-relevant intent.

### 3. Damage bounds must be first-class

For content invalidation to map to tile invalidation efficiently, scene production should emit conservative paint bounds and, when practical, smaller damage rects inside the layer.

That allows the compositor to dirty only intersecting tiles instead of invalidating the entire layer every time.

## Layout and Viewport Interaction

This compositor design assumes SUI adopts a stronger distinction between measurement and arrangement.

The two-phase layout direction described in [layout-proposal.md](./layout-proposal.md) is not optional if SUI wants a high-refresh compositor to pay off.

The reason is simple:

- scrolling, panning, viewport shifts, and many panel moves should often be arrange-only changes
- arrange-only changes should frequently map to compositor `Transform` or `Clip` invalidation rather than `Content` invalidation
- if those updates still trigger broad remeasurement and repaint, tile retention will help much less than it should

The compositor therefore depends on layout becoming more incremental and more explicit about geometry-only changes.

## Text and Image Strategy

### Text

Text should use two retained layers of caching:

- shaping and layout cache at the text system boundary
- glyph atlas or glyph mask cache at the renderer/compositor boundary

For normal UI text, the preferred steady-state path is not repeated outline tessellation every frame. It is retained glyph atlas content plus cheap quad or instance emission inside tiles or direct packets.

Vector-outline fallback can still exist for zoom-heavy creative surfaces or special effects, but it should not be the default path for inspector text, labels, buttons, and menus.

### Images

Images should behave as retained compositor resources with explicit versioning.

Image updates should invalidate only dependent tiles or direct packets. They should not force unrelated scene recompilation.

## GPU Execution Model

The wgpu backend should shift from frame-centric scene compilation to retained pass execution.

It should own:

- tile raster targets and tile texture atlases
- GPU resources for direct packets
- composition pipelines for textured tile quads and direct vector or image packets
- effect-surface pipelines for explicit offscreen layers
- memory budgeting and eviction policy

It should not remain responsible for repeated whole-frame draw-op reconstruction when nothing structurally meaningful changed.

### Expected pass types

- tile generation passes for dirty tiles only
- optional direct-packet generation for volatile layers
- offscreen effect-surface passes for explicit effect groups
- final composition pass over visible tiles, packets, and surfaces

The final composition pass should be cheap relative to content generation. That is the point of retaining tiles.

## Parallelism Model

The retained compositor is a natural place to introduce controlled parallel preparation.

Parallel work should include:

- tile raster scheduling
- text shaping where runtime rules allow it
- dirty tile generation
- image decode or upload staging
- direct-packet compilation for independent layers

The runtime should still remain the single authoritative owner of widget state. Parallelism happens after scene updates have been captured into immutable work descriptions.

That keeps event handling deterministic and compatible with future bindings.

## Memory and Budgeting

Tile-based systems fail if memory is treated as an afterthought.

The compositor should budget separately for:

- tile textures
- glyph atlases
- direct-packet GPU buffers
- offscreen surfaces
- CPU-side retained scene or packet metadata

Eviction policy should prefer removing:

- invisible least-recently-used tiles
- tiles from layers marked highly volatile
- tiles at non-current scale buckets

The compositor should avoid cache thrash by allowing layers to opt out of tile caching when volatility is too high.

## Diagnostics Requirements

The retained compositor should expose diagnostics as first-class runtime outputs.

Per-frame diagnostics should include at least:

- visible layer count
- visible tile count
- reused tile count
- regenerated tile count
- direct-packet count
- offscreen surface count
- tile memory bytes
- atlas memory bytes
- tile generation time
- final composition time
- damage coverage by area

These metrics should be available to `sui-debug`, `sui-testing`, and the widget-book so the architecture can be profiled in real workloads instead of only in renderer microbenchmarks.

## Crate Boundary Implications

This document does not lock in a crate split, but the target ownership model is clear.

`sui-runtime`

- owns widget state, layout, invalidation, and scene production

`sui-scene`

- owns render-neutral scene and layer descriptors expressive enough for compositor consumption

`sui-render-wgpu`

- owns retained compositor state, tile caches, atlases, offscreen surfaces, and GPU execution

A separate compositor crate may eventually make sense, but this document does not require that decision yet.

## Phased Implementation Plan

The implementation should be staged, but not compatibility-driven. The goal is to reach a finished replacement architecture, not to keep the old renderer limping along while the new one grows beside it.

Each phase is an architecture milestone, not a release gate. It is acceptable for intermediate states to be disruptive or incomplete as long as they move the codebase decisively toward the final retained-compositor architecture.

The plan should follow three hard rules:

- do not preserve old contracts just to reduce short-term churn
- do not keep a long-lived legacy fallback path once a replacement subsystem exists
- do not stop at a mixed architecture that still depends on old frame-global rendering behavior

### Phase 0: Baseline, kill boundaries, and replacement rules

Before changing the architecture, establish a baseline for representative workloads and make the current bottlenecks easy to observe.

- add compositor-oriented diagnostics fields even before the compositor exists, including repaint coverage, dirty layer counts, layout counts, scene command counts, draw counts, uploaded bytes, and frame phase timings
- add stress scenarios to `sui-dev` and `sui-widget-book` for scrolling lists, moving panels, animated opacity, and many independently animating widgets
- define target workloads for `60 fps`, `120 fps`, and degraded fallback behavior under load
- add repeatable profiling procedures for desktop development builds and release builds
- identify which current runtime, scene, and renderer APIs are compatibility hazards and explicitly mark them for removal or redesign
- define which current renderer subsystems are temporary scaffolding only and must not survive the replacement

Exit criteria:

- the repo has stable workloads that expose current hot paths
- the diagnostics panel shows enough data to tell whether later phases actually reduce frame work
- the team has explicit agreement on which old contracts may be broken or deleted during the replacement

### Phase 1: Break runtime and layout contracts into compositor-friendly form

The compositor depends on cleaner invalidation and more incremental geometry updates than the current runtime contract provides.

- implement the measure and arrange split from [layout-proposal.md](./layout-proposal.md)
- distinguish measurement changes from arrange-only changes in runtime scheduling
- stop treating viewport shifts, scrolling, and child translation as generic layout or paint work when content did not change
- move widget graph maintenance toward persistent retained state rather than broad rebuilds where practical
- replace coarse invalidation plumbing with compositor-oriented categories instead of mapping everything back to legacy paint semantics
- remove or rewrite runtime contracts that assume scene production is equivalent to final rendering work

Exit criteria:

- scrolling and similar geometry-only updates can complete without broad remeasurement
- runtime scheduling can distinguish content changes from transform or clip-like changes
- the runtime no longer depends on old renderer behavior to express geometry-only updates

### Phase 2: Replace scene-layer contracts for compositor consumption

Before building the compositor, the scene model needs to describe composition-relevant intent more directly.

- extend `sui-scene` layer descriptions to carry stable layer identity, content bounds, paint bounds, cache hints, and composition hints
- separate scene content structure from transient frame metadata
- make damage bounds first-class on scene layers or layer updates
- ensure a widget can contribute more than one compositor-relevant layer when necessary
- keep scene output immutable once produced for a frame or update batch
- remove scene assumptions that exist only to support the current frame-global renderer path

Exit criteria:

- scene output provides enough information for a compositor to reason about content, bounds, and composition without re-deriving everything from raw draw commands
- new scene contracts are the authoritative path forward rather than an adapter around legacy frame types

### Phase 3: Install the retained compositor scaffold and retire the old frame compiler

Status: complete. Live rendering now goes through per-window retained compositor state in `sui-render-wgpu`, and the old frame-global scene-to-draw-op compiler has been deleted rather than kept as fallback scaffolding.

The first compositor milestone should not start with tiles. It should start by replacing frame-global renderer compilation with retained layer and property state while still rendering layers directly.

- add a per-window retained compositor state in `sui-render-wgpu`
- introduce retained layer records plus transform, clip, and effect property trees
- teach the renderer to update retained compositor state incrementally from scene updates
- implement `Direct` layer mode first, backed by retained render packets or equivalent persistent draw data
- keep the final composition path explicit even if it initially composes only direct layers
- delete the old frame-global scene-to-draw-op path instead of keeping it as compatibility scaffolding once the retained direct path can render core workloads

Exit criteria:

- the renderer no longer treats each frame as a fresh global scene compilation problem
- transform, clip, and effect changes can update retained compositor state without forcing full content rebuild of unaffected layers
- the old renderer path is deleted; there is no supported frame-global fallback architecture

### Phase 4: Tile cache infrastructure

Once retained compositor state exists, add tile storage and tile invalidation for cacheable layers.

- implement layer tile grids, tile keys, tile entries, and memory budgeting
- add dirty-tile computation from content damage bounds
- add visibility tracking for tiles against the current viewport and clip state
- implement GPU tile payload allocation and eviction policy
- keep tile generation limited to content changes and newly exposed tiles
- do not reintroduce legacy full-layer repaint paths as the normal fallback for cacheable content

Exit criteria:

- large retained layers can reuse previously generated tiles across frames
- viewport motion and scrolling mostly reuse tiles instead of regenerating them
- cacheable layers are actually driven by tile invalidation rather than falling back to old whole-layer rebuild habits

### Phase 5: Composition-only fast paths

This phase is where the retained compositor starts paying off for high-refresh UI motion.

- map arrange-only updates onto compositor transform and clip updates wherever possible
- ensure scroll offset, floating panel movement, and opacity animation can run without content invalidation for unchanged layers
- make final composition consume retained tiles and direct packets together
- add scale-bucket handling so translated layers keep tile identity while zoom or DPI changes can re-resolve cache entries safely
- aggressively remove any remaining code paths that convert transform-only or clip-only changes back into content rebuilds unless strictly required

Exit criteria:

- representative motion-heavy scenarios are dominated by composition cost rather than repaint cost
- frame diagnostics show high tile reuse and low regenerated-tile counts during steady-state scrolling or movement
- the system no longer depends on legacy repaint assumptions for core interaction patterns

### Phase 6: Offscreen surfaces, effect groups, and advanced content heuristics

Only after retained direct and tiled rendering are working should the system grow more expensive effect capabilities.

- add explicit offscreen-surface support for effect groups that require intermediate composition
- implement heuristics or hints for choosing `Direct`, `CachedTiles`, or `OffscreenSurface`
- add atlas-backed text rendering as the default path for normal UI text inside direct packets and tiles
- refine image, text, and vector resource versioning so `Resources` invalidation remains narrowly scoped
- replace remaining renderer subsystems that still assume frame-global geometry rebuild for standard UI text or image presentation

Exit criteria:

- the compositor can support common effect-group cases without regressing normal retained-content fast paths
- direct versus tiled mode selection is observable and debuggable
- ordinary UI text and images flow through retained compositor-friendly paths by default

### Phase 7: Parallel preparation and memory hardening

Parallelism and memory policy should be layered onto a working retained compositor, not used to compensate for missing retention.

- parallelize dirty tile generation and independent direct-packet compilation from immutable scene work descriptions
- add memory budgets for tiles, atlases, buffers, and offscreen surfaces with clear eviction metrics
- harden scale changes, resource reloads, subtree churn, and window resize behavior
- add stress tests for cache thrash, rapid scrolling, deep clip trees, and many simultaneously animating layers
- remove any remaining temporary bridge layers that were kept only to ease the cutover

Exit criteria:

- the compositor remains stable under long-running churn and memory pressure
- parallel preparation improves frame time in real workloads without changing runtime ownership rules
- the replacement architecture stands on its own without depending on old-system compatibility scaffolding

### Phase 8: Optimization, diagnostics, and defaults

After the architecture is functionally complete, spend time tuning defaults and making the system understandable.

- tune tile sizes, scale buckets, cache budgets, and render-mode heuristics using real workloads
- surface retained-compositor diagnostics in `sui-debug`, widget-book panels, and test snapshots
- add regression tests for tile reuse, transform-only updates, clip-only updates, and resource-version invalidation
- document expected fast paths and anti-patterns for widget and renderer authors
- delete obsolete documentation and diagnostics that describe the legacy renderer as a supported architecture

Exit criteria:

- the default compositor policy performs well on the target workloads without per-app hand tuning
- diagnostics make it obvious when a workload falls off the retained fast path
- there is one supported rendering architecture in the repo, and it is the retained compositor

## Open Design Questions

The architecture still leaves several design questions open:

1. How much composition-relevant intent should widgets express directly versus how much should the runtime infer from scene structure?
2. Should tile generation use a dedicated software raster path for some content, or should all tile generation remain GPU-driven?
3. Which layer heuristics decide between `Direct` and `CachedTiles`, and how much should that be developer-tunable?
4. How should zoom-heavy canvas content balance vector fidelity against tile reuse across scale buckets?

These are important, but they are tuning and refinement questions. They do not change the core decision that SUI should target a retained compositor with tile-based caching and composition-aware invalidation.

## Recommended Direction

The target rendering architecture for SUI should be:

- retained widget runtime
- explicit measure and arrange layout pipeline
- scene-based paint boundary
- tile-based retained compositor with property trees
- hybrid layer modes for direct, tiled, and offscreen rendering
- wgpu backend focused on incremental content generation and final composition

That combination gives SUI the best chance of reaching smooth high-refresh rendering for large, dynamic widget hierarchies without collapsing the architecture back into full-frame immediate rendering.