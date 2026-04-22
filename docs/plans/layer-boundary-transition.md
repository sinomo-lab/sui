# Layer Boundary Transition Plan

> **For Hermes:** Use `subagent-driven-development` and `test-driven-development` to execute this plan task-by-task. Prefer small commits and benchmark after each phase.

**Goal:** Restore SUI's intended runtime architecture so widget-owned invalidation and root-driven repaint remain the primary model, while `SceneLayer` becomes an explicit repaint/composition boundary rather than the default output of every widget paint, and the measure/arrange/composition pipeline stays a reusable widget-side utility rather than a mandatory framework/renderer concept.

**Architecture:** Keep the retained renderer, but stop coupling widget identity to scene-layer identity by default. Ordinary widgets should flatten their paint output into the parent scene; only explicit boundaries such as scroll surfaces, overlays, stack surfaces, and future opt-in repaint surfaces should emit `SceneLayer` nodes. The default widget-tree measure/arrange pipeline should remain only one caller of shared layout utilities, so advanced widgets can drive measurement/composition work independently or mix it with arbitrary systems.

**Tech Stack:** Rust workspace, `sui-layout`, `sui-runtime`, `sui-scene`, `sui-render-wgpu`, `sui-widgets`, `sui-widget-book`, `sui-dev`.

---

## Recommendation

Do **not** spend more time micro-optimizing the current per-widget layer model first.

The current architecture has drifted such that:
- `WidgetPod::paint()` unconditionally wraps each widget paint result in a `SceneLayer`
- simple wrappers like `Padding`, `Stack`, `SizedBox`, and `Background` become compositor layers
- retained compositor cost scales with widget-tree structure rather than only real composition boundaries

The right direction is:
1. preserve retained rendering where it is genuinely useful
2. re-establish root-driven repaint and widget-owned invalidation as the default runtime contract
3. reserve `SceneLayer` for explicit repaint/composition boundaries
4. only then optimize the smaller remaining layer set

---

## Current Context / Ground Truth

### Code paths to keep in mind

- `crates/sui-runtime/src/widget.rs`
  - `WidgetPod::paint()` currently paints into a child `PaintCtx`, then always calls `parent_ctx.push_layer(...)`
- `crates/sui-runtime/src/lib.rs`
  - render scheduling, graph updates, invalidation collection, and runtime render orchestration
- `crates/sui-layout/src/lib.rs`
  - shared constraints, geometry, and layout utilities that should stay usable without a standard window/render path
- `crates/sui-scene/src/lib.rs`
  - `Scene`, `SceneLayer`, `SceneLayerDescriptor`, `SceneLayerUpdate`
- `crates/sui-render-wgpu/src/retained.rs`
  - retained compositor snapshot traversal, per-layer structure comparison, packet rebuild logic
- `crates/sui-widgets/src/containers.rs`
  - many wrappers paint by simply delegating to their children
- `crates/sui-widgets/src/composites.rs`
  - overlays / popovers / stack-surface behavior that should remain explicit boundaries

### Current docs already updated to reflect drift

- `docs/README.md`
- `docs/architecture.md`
- `docs/renderer-architecture.md`
- `docs/crate-architecture.md`

Use those updated docs as the narrative baseline while implementing this transition.

---

## Design Targets

### Target 1: Default paint is flat

For ordinary widgets:
- widget paint appends scene commands into the parent scene
- widget identity remains in the runtime graph and semantics tree
- widget invalidation remains explicit and widget-driven
- repaint dispatch still starts from the root/runtime orchestration path

### Target 2: Layers are explicit boundaries

A `SceneLayer` should exist only for widgets that truly need a boundary, including:
- scroll surfaces
- overlay composition
- stack surfaces / floating surfaces
- host-local ordering surfaces
- future opt-in repaint/caching surfaces if they prove useful again

### Target 3: Dirty propagation is runtime-managed but widget-defined

Widgets should continue to decide **why** they are dirty.
The runtime should decide **how far** repaint/layout/semantics work must propagate.
That propagation must no longer assume `widget == layer`.

### Target 4: Layout utilities stay optional and reusable

The built-in widget set should keep using measure/arrange/composition helpers heavily, but SUI should not treat that pipeline as the only valid layout model.

Required properties:
- shared layout utilities remain renderer-neutral
- the standard retained widget-tree pipeline is only one caller of those utilities
- custom widgets can initiate measurement/composition work without a standard paint or window context
- arbitrary spatial systems can coexist with, wrap, or bypass the default layout helpers

### Target 5: Renderer sees fewer, more meaningful layers

The retained compositor should operate on a much smaller set of real boundaries so that:
- structural traversal cost falls
- packet rebuild bookkeeping falls
- command counts matter more than wrapper count
- scroll-heavy demos do not explode in layer count

---

## Phase 0: Instrumentation and Baseline

### Task 0.1: Add explicit diagnostics vocabulary

**Objective:** Separate widget count, repaint-boundary count, and scene-layer count in diagnostics.

**Files:**
- Modify: `crates/sui-runtime/src/diagnostics.rs`
- Modify: `crates/sui-runtime/src/lib.rs`
- Modify: `crates/sui-widget-book/src/lib.rs`
- Modify: `crates/sui-dev/src/app.rs`

**Steps:**
1. Add diagnostics fields for:
   - total widget count
   - scene-layer count
   - stack-surface count
   - overlay-layer count
   - packet rebuild reason totals if available
2. Surface those numbers in widget-book / dev diagnostics.
3. Verify the numbers appear without changing runtime behavior.

**Validation:**
- `cargo check -p sui-runtime`
- `cargo check -p sui-widget-book`
- `cargo check -p sui-dev`

### Task 0.2: Capture baseline performance numbers before architectural changes

**Objective:** Preserve a before/after benchmark story.

**Files:**
- Modify: `crates/sui-widget-book/tests/desktop_e2e.rs`
- Optional docs note: `docs/renderer-architecture.md`

**Steps:**
1. Add or update benchmark/report output so it prints layer counts and packet rebuild totals.
2. Capture baseline for:
   - full widget-book scroll benchmark
   - overlay-free widget-book gallery benchmark
   - isolated HDR lab if practical
3. Save representative numbers in the plan or commit message notes.

**Validation:**
- `cargo test -p sui-widget-book widget_book_scroll_fps_benchmark -- --nocapture`
- `cargo test -p sui-widget-book widget_book_scroll_fps_benchmark_without_live_overlay -- --ignored --nocapture`

---

## Phase 1: Clarify Reusable Layout Utilities and Explicit Paint-Boundary Semantics

### Task 1.1: Clarify the shared layout-utility boundary

**Objective:** Keep measure/arrange/composition helpers reusable outside the standard retained widget-tree path.

**Files:**
- Modify: `crates/sui-layout/src/lib.rs`
- Modify: `crates/sui-runtime/src/widget.rs`
- Modify: `crates/sui-runtime/src/lib.rs`
- Modify: `crates/sui/src/lib.rs` if re-exports are needed

**Proposed direction:**
- keep shared constraints, sizing helpers, and layout utilities renderer-neutral
- make runtime layout contexts clearly the default retained-widget caller, not the only caller
- avoid APIs that require a window, `PaintCtx`, or renderer state just to perform measurement/composition work
- allow advanced widgets to initiate layout passes manually or mix them with arbitrary spatial systems

**Validation:**
- `cargo check -p sui-layout`
- `cargo check -p sui-runtime`
- targeted unit tests for manual or decoupled measure/arrange invocation if practical

### Task 1.2: Define a runtime concept for paint boundaries

**Objective:** Stop using implicit per-widget layering as the only boundary mechanism.

**Files:**
- Modify: `crates/sui-runtime/src/widget.rs`
- Modify: `crates/sui-runtime/src/lib.rs`
- Modify: `crates/sui-scene/src/lib.rs`
- Modify: `crates/sui/src/lib.rs` if re-exports are needed

**Proposed direction:**
Add a small, explicit runtime-facing concept such as one of:
- `PaintBoundaryMode`
- `LayerBoundaryPolicy`
- extend `LayerOptions` with a `boundary` / `emit_layer` / `flatten_default` flag

The critical requirement is:
- default ordinary widgets **do not** emit a `SceneLayer`
- widgets with explicit boundary semantics **do** emit a `SceneLayer`

**Validation:**
- `cargo check -p sui-runtime`
- targeted runtime unit tests around scene generation

### Task 1.3: Keep existing special surfaces explicit

**Objective:** Preserve behavior for widgets that truly need layers.

**Files:**
- Modify: `crates/sui-widgets/src/containers.rs`
- Modify: `crates/sui-widgets/src/composites.rs`
- Modify: `crates/sui-widgets/src/controls.rs`
- Possibly modify: `crates/sui-widget-book/src/lib.rs`

**Surfaces to preserve as explicit boundaries:**
- scroll views / virtual scroll views
- popovers / overlays
- stack-host surfaces
- selects / menus / other floating UI that depend on overlay ordering

**Validation:**
- existing widget-book overlay/scroll tests
- popover and stack-host tests

---

## Phase 2: Flatten Ordinary Widget Paint

### Task 2.1: Change `WidgetPod::paint()` default behavior

**Objective:** Make normal widgets paint directly into the parent scene.

**Files:**
- Modify: `crates/sui-runtime/src/widget.rs`

**Current behavior to replace:**
- child paint always records into child `PaintCtx`
- result is always wrapped in `push_layer(...)`

**Target behavior:**
- if widget is an explicit paint/composition boundary, keep current layer path
- otherwise append child scene commands directly into parent scene

**Important:**
- preserve invalidation forwarding
- preserve IME composition rect forwarding
- preserve semantics behavior
- do not regress transforms/clips already expressed in scene commands

**Validation:**
- runtime render tests
- semantics snapshot tests
- targeted widget-book gallery tests

### Task 2.2: Verify simple wrappers no longer create layers

**Objective:** Collapse obvious wrapper-induced layer explosions.

**Files:**
- Modify: `crates/sui-widgets/src/containers.rs`
- Add/update tests in `crates/sui-runtime/src/lib.rs` or `crates/sui-widget-book/src/lib.rs`

**Targets:**
- `Padding`
- `SizedBox`
- `Stack`
- `Background`
- other pure layout/decoration wrappers that do not need independent composition

**Validation:**
Add a regression test that renders a known subtree and asserts scene-layer count falls significantly while visuals remain identical.

---

## Phase 3: Retarget Invalidation Propagation

### Task 3.1: Decouple dirty widget identity from layer identity

**Objective:** Ensure repaint invalidation still works once most widgets stop emitting layers.

**Files:**
- Modify: `crates/sui-runtime/src/lib.rs`
- Modify: `crates/sui-runtime/src/widget.rs`
- Inspect: invalidation collection / graph change logic / paint bounds logic

**Required behavior:**
- dirty widgets still request `Paint`, `Measure`, `Arrange`, `Semantics`, etc.
- runtime resolves repaint work to the nearest repaint boundary ancestor
- geometry/transform/order invalidations still behave correctly

**Validation:**
- focused invalidation tests
- scrolling tests
- popup/overlay tests
- theme preview / widget-book redraw tests

### Task 3.2: Preserve special fast paths

**Objective:** Keep explicit retained fast paths for real surfaces.

**Files:**
- Modify: `crates/sui-runtime/src/lib.rs`
- Modify: `crates/sui-render-wgpu/src/retained.rs`

**Must preserve:**
- scroll transform fast path
- ordering-only updates for stack surfaces
- overlay composition ordering
- visibility/effect/clip invalidation where layers remain meaningful

**Validation:**
- widget-book scroll tests
- stack-host ordering tests
- popover open/close tests

---

## Phase 4: Simplify Renderer Assumptions

### Task 4.1: Audit retained compositor for assumptions that every widget is a layer

**Objective:** Remove renderer assumptions tied to broad layer cardinality.

**Files:**
- Modify: `crates/sui-render-wgpu/src/retained.rs`
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Modify: `crates/sui-debug/src/lib.rs` if diagnostics formatting needs updates

**Audit focus:**
- snapshot building
- packet rebuild reason tracking
- layer update handling
- pruning logic
- any path where wrapper-layer churn is assumed or normalized

**Validation:**
- `cargo check -p sui-render-wgpu`
- targeted renderer tests

### Task 4.2: Re-benchmark and compare against baseline

**Objective:** Prove the architectural change fixed the real cost center.

**Files:**
- Modify benchmark/report tests if needed: `crates/sui-widget-book/tests/desktop_e2e.rs`
- Optional docs update: `docs/renderer-architecture.md`

**Success criteria:**
- scene-layer count in widget-book / HDR lab drops dramatically
- retained traversal / packet upkeep metrics improve
- widget-book HDR demos scroll more smoothly
- no regressions in overlay correctness or ordering

**Current status note:**
- the live desktop benchmark tests in `crates/sui-widget-book/tests/desktop_e2e.rs` still require a real display server, so current numbers in this environment were captured with headless diagnostic benchmarks in `crates/sui-widget-book/src/lib.rs`
- current headless widget-book scroll snapshot:
  - full widget-book scroll surface: avg `3.036 ms`, p95 `4.186 ms`, avg visible layers `20.62`, avg direct packets `11.83`, avg packet rebuilds `9.00`, avg repaint boundaries / scene layers `6.88` / `6.88`
  - overlay-free gallery-only surface: avg `1.871 ms`, p95 `2.944 ms`, avg visible layers `14.62`, avg direct packets `9.83`, avg packet rebuilds `8.00`, avg repaint boundaries / scene layers `4.88` / `4.88`

Recommended commands for reproducing the current snapshot here:
```bash
cargo test -p sui-widget-book --lib widget_book_headless_scroll_current_status_benchmark -- --ignored --nocapture
cargo test -p sui-widget-book --lib widget_book_headless_gallery_only_scroll_current_status_benchmark -- --ignored --nocapture
```

---

## Files Likely To Change

### Primary runtime / scene / renderer files
- `crates/sui-layout/src/lib.rs`
- `crates/sui-runtime/src/widget.rs`
- `crates/sui-runtime/src/lib.rs`
- `crates/sui-runtime/src/diagnostics.rs`
- `crates/sui-scene/src/lib.rs`
- `crates/sui-render-wgpu/src/retained.rs`
- `crates/sui-render-wgpu/src/lib.rs`

### Widget files likely to require explicit-boundary review
- `crates/sui-widgets/src/containers.rs`
- `crates/sui-widgets/src/composites.rs`
- `crates/sui-widgets/src/controls.rs`
- `crates/sui-widgets/src/text_surface.rs`

### Validation / diagnostics surfaces
- `crates/sui-widget-book/src/lib.rs`
- `crates/sui-widget-book/tests/desktop_e2e.rs`
- `crates/sui-dev/src/app.rs`
- `crates/sui-debug/src/lib.rs`

### Docs
- `docs/design.md`
- `docs/architecture.md`
- `docs/renderer-architecture.md`
- `docs/crate-architecture.md`
- `docs/README.md`
- `docs/stack-hosts.md` if stack-boundary semantics need clarification after implementation

---

## Test and Validation Strategy

### Runtime-focused
- add regression tests for flattened wrapper paint
- add tests for repaint-boundary ancestor resolution
- keep semantics and IME behavior stable

Suggested commands:
```bash
cargo test -p sui-runtime --lib
cargo check -p sui-runtime
```

### Renderer-focused
- verify retained compositor still handles explicit boundaries correctly
- compare layer counts, rebuild reasons, and traversal costs

Suggested commands:
```bash
cargo test -p sui-render-wgpu --lib
cargo check -p sui-render-wgpu
```

### Widget-book / end-to-end
- widget-book scroll benchmark
- overlay-free widget-book benchmark
- popover / tooltip / stack-host ordering tests
- HDR demo visibility + performance sanity checks

Suggested commands:
```bash
cargo test -p sui-widget-book --lib
cargo test -p sui-widget-book --test desktop_e2e -- --nocapture
cargo check -p sui-widget-book
cargo check -p sui-dev
```

---

## Risks and Tradeoffs

### Risk 1: invalidation becomes incorrect when layer identity disappears
Mitigation:
- add repaint-boundary ancestor tests early
- keep explicit boundary widgets unchanged first

### Risk 2: scroll/overlay fast paths regress
Mitigation:
- preserve scroll, overlay, and stack surfaces as explicit boundaries in Phase 1
- benchmark those paths after each phase

### Risk 3: some widgets implicitly depend on current per-widget layerization
Mitigation:
- start with obvious wrappers first
- add before/after layer-count regression tests
- keep a temporary opt-in escape hatch for widgets that still need layers

### Risk 4: docs and code drift again during migration
Mitigation:
- update docs per phase, not only at the end
- keep the plan file current if architecture decisions change

---

## Open Questions To Resolve During Implementation

1. What exact runtime API should express an explicit repaint/composition boundary?
2. Should scroll surfaces remain `SceneLayer` boundaries, or can some scroll behavior move to flat scene + transform bookkeeping?
3. Do we want a temporary compatibility mode so selected widgets can keep current behavior until audited?
4. Should packet rebuild statistics be exposed more directly in `sui-dev` during the migration?
5. Once default layerization is reduced, do any widget-scoped retained subdivision concepts still deserve to survive outside specialized widgets such as infinite canvas?

---

## Definition of Done

This transition is complete when:
- ordinary wrapper widgets no longer create `SceneLayer` nodes by default
- explicit invalidation still behaves correctly from root-driven runtime orchestration
- scroll / overlay / stack-surface semantics remain correct
- widget-book HDR lab layer count drops substantially
- performance improves on scroll-heavy demos
- docs describe the new architecture accurately

---

## Suggested Commit Boundaries

1. `docs: clarify layer drift and transition direction`
2. `feat: add explicit repaint boundary semantics`
3. `refactor: flatten default widget paint into parent scene`
4. `refactor: retarget invalidation to repaint boundaries`
5. `perf: reduce retained compositor overhead from wrapper layerization`
6. `docs: update renderer and runtime architecture after layer transition`
