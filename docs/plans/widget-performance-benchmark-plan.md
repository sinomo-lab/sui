# Widget initialization and layout benchmark suite

**Status: initial runnable suite implemented in `crates/sui-bench`.** Construction,
CPU updates, diagnostic work counters, offscreen WGPU rendering, fresh-process
startup, artifact collection, and matching-run comparison exist. Desktop mode
currently covers startup and requires a real display for execution. See the
[runner guide](../../crates/sui-bench/README.md) for supported commands and precise
fixture coverage; the broader matrix below also includes follow-up work.
Implementation baseline: `56d8969`, including prepared text reuse and the merged
classic-scrollbar layout changes. This plan covers measurement infrastructure;
optimizations follow measured bottlenecks.

The first optimization pass adds bounded natural/intrinsic query caches in
`WidgetPod`, uses natural probes in Grid and Flex, and preserves layout completed
by initial window events through the first render. Final measurement still
commits widget state before arrangement; cached scalar probes do not restore
layout handles or child geometry. Probe dependencies remain observable until
measurement invalidation discards the affected caches. Dirty/forced subtrees
continue to execute ordinary measurement callbacks. Diagnostic counters separate
probe hits from final measurement hits. Renderer packet granularity and native
first-present validation remain follow-up work.

## Questions the suite must answer

1. What does constructing and attaching a widget tree cost before rendering?
2. How long until the first content frame, and how much time is spent in layout,
   font/resource initialization, painting, renderer setup, and presentation?
3. How much work does a local change cause in an otherwise stable tree?
4. How do resize, structural churn, streaming content, and scrolling scale with
   total model size, mounted widgets, and the number of changed items?
5. Does an optimization reduce latency while preserving geometry, semantics,
   retained identity, and bounded memory?

Timing results, traces, screenshots, and environment snapshots belong under
`target/widget-bench/` or CI artifacts. Documentation contains the methodology
and reproduction commands; benchmark source belongs under `crates/`.

## Existing facilities and measurement gaps

| Existing facility | Reuse | Gap to address |
| --- | --- | --- |
| `Application::build` and `Runtime::render` | Public CPU construction/frame boundaries | Separate fixture construction from runtime attachment; retain the actual first frame. |
| `FramePhase` / `RenderDiagnostics` | Layout, paint, semantics, graph, invalidation, and rebuild data | `MeasureArrange` currently includes graph refresh; add non-overlapping internal phase boundaries. |
| `WidgetTimingSample` | Calls and inclusive time by widget/phase | Intrinsic probes are grouped with measure; early cache returns are absent. Add explicit probe/cache counters. |
| `WindowPerformanceSnapshot` | Host-frame, renderer, cache, and presentation diagnostics | Add startup milestones before launch/initialization, and associate updates with their completed content frame. |
| `TextSystem` preparation and layout snapshots | Text cache hits/misses, bytes, evictions, and size-only timing | Expose diagnostic deltas through the runtime without exposing mutable text-system ownership. |
| `TestApp` / `HeadlessPlatform` | Behavior verification, deterministic time, normalized input | Avoid implicit backend selection and eager initial rendering in cold measurements. |
| Desktop widget-book benchmarks | Real window/input/render path and existing fixtures | Add a fresh-process first-frame path; current steady-state tests begin after launch and an initial snapshot. |

Per-widget timings include descendant calls. Report them as inclusive hotspots;
do not sum them into a frame total. Report renderer preparation/submit times as
CPU measurements; GPU execution needs its own supported timestamp facility.

## Measurement boundaries

### Initialization and first content frame

Record milestones on one monotonic clock inside the child process:

| Metric | Start / end |
| --- | --- |
| `fixture_input_us` | Generate the deterministic model and embedded resource descriptors. |
| `widget_construct_us` | Build the real widget tree, properties, subscriptions, and closures from that model. |
| `runtime_attach_us` | Attach the built tree/resources through `Application::build`. |
| `first_runtime_frame_us` | First `Runtime::render` call, including lazy work triggered there. |
| `first_content_present_us` | Child entry until successful present-return for the frame containing the fixture's initial content revision. |
| `first_settled_content_us` | Child entry until that revision's required follow-up layout/resource work has completed and its final frame has been presented. |
| `teardown_us` | Drop/close the tree, runtime, and host after sampling. |

The cold total includes input generation, construction, attachment, lazy font
work, device/surface initialization, and rendering. The separate construction
measurement excludes input generation to make widget implementation comparisons
useful. Never initialize themes, fonts, images, the renderer, or the fixture
before the cold total starts. Embedded bytes may be compiled into the executable;
their decoding/registration still belongs inside the measured startup.

Native window creation, adapter/device creation, surface configuration, first
resource uploads, surface acquisition, and present-return get their own spans.
These may overlap other startup work: retain timestamps and compute the total
from endpoints rather than adding overlapping durations.

The parent additionally records spawn-to-completion-notification wall time,
including process startup and IPC. Keep it separate from child-entry timings;
do not subtract unrelated parent/child clocks. Present-return is a host API
boundary, not physical scanout. A blank bootstrap frame does not satisfy the
content marker. Missing milestones, timeouts, and initialization errors are
explicit failed samples.

Default cold fixtures are finite and have animations/background work disabled.
"Settled" means the fixture's expected revision is complete with no remaining
required work, subject to an iteration/time limit. Do not wait indefinitely for
global idleness in an animation or streaming workload.

### Changing scenes

Measure three boundaries for each revision:

- mutation/event-dispatch time;
- mutation through the completed CPU scene revision;
- on desktop, mutation through the present-return for that revision.

Retain every frame required by an operation. An update that needs three passes
must not be represented by only its fastest or final pass. Use operation/revision
IDs to distinguish new work from a previously published snapshot.

Run two update policies separately:

- **Acknowledged updates:** apply the next mutation after the previous revision
  completes. This isolates service cost and supplies repeatable per-operation
  samples.
- **Scheduled updates:** replay a timestamped producer trace at a configured
  rate. Record offered updates, consumed updates, coalesced revisions, queue age,
  and displayed revision age. Completion rate alone can conceal a backlog or
  skipped intermediate states.

Manual time drives deterministic CPU animations. Wall-clock time measures their
cost; simulated elapsed time must never be used as a performance result.

## Execution modes

| Mode | Boundary and purpose |
| --- | --- |
| `construct` | Build/drop widget trees and attach runtimes; isolate initialization and allocation work. |
| `runtime` | Direct real-runtime frames without WGPU; isolate initialization, invalidation, layout, graph, paint-command generation, and semantics. |
| `offscreen` | Feed the same frames to the real WGPU renderer; add resource and renderer work without window-system pacing. |
| `desktop` | Visible native window and normal event loop; measure startup, input-to-content latency, surface wait, and presentation. |

Mode selection is explicit. A requested desktop/GPU run that cannot initialize
must fail or report `unsupported`; it must never silently become a CPU or
software-renderer result. Record the actual adapter/backend, including software
adapters when explicitly selected. Headless/runtime success does not satisfy
desktop first-frame acceptance.

Use the existing automated testing and platform machinery. `TestApp::new`
currently selects a backend based on the environment, and normal headless harness
construction calls `run_until_idle`. Start timers/observers before those actions,
or use a runner with explicit control of the initial pump. CPU timing should call
the runtime directly. Desktop startup hooks must be installed before launching
the harness/event loop. Do not put locator auto-waits, screenshots, synchronous
readback, or console output in timed runtime sections.

## Workload matrix

Use real built-in widgets for scored workloads. Small counting leaves are useful
only to validate instrumentation and isolate container algorithms. Give every
fixture a stable ID, version, seed, size description, and expected observations.

| Fixture | Construction/layout cases | Update cases | Correctness/work checks |
| --- | --- | --- | --- |
| `controls-grid` | Labels, buttons, inputs, icons, and form rows in a grid; shared versus per-control style configuration | One label, a clustered/distributed subset, and all labels; color-only versus geometry-changing values | Expected control count, text revision, finite bounds, and correct invalidation class. |
| `nested-flex` | Wide and deep row/column trees, fixed/flex children, wrapping labels, intrinsic sizing | Local leaf update; same-size versus size-changing content; splitter movement | Track intrinsic probes, repeated constraints, ancestor propagation, and unchanged sibling reuse. |
| `scrollbar-thresholds` | ScrollView, VirtualScrollView, VirtualList, Table/VirtualTable, and floating panes just below/above overflow | Cross each axis threshold; make one gutter trigger the opposite axis; alternate viewport widths | Correct gutters, content viewport, wrap height, scroll range, and bounded settling passes. Include hidden-scrollbar control. |
| `keyed-collection` | Stable-key rows at increasing collection sizes | Update items, append/remove a batch, reorder, replace a subtree; separate deliberate full-rebuild control | Preserve surviving IDs; count constructed/dropped/reused rows and subscription growth. |
| `virtual-collection` | Large logical models with a fixed visible viewport/overscan | Scroll, insert before viewport, update visible versus offscreen rows | Record logical versus mounted counts; keep mount/layout work tied to the visible window where supported, and preserve scroll anchors. |
| `streaming-document` | RichDocumentView with completed blocks and a streaming tail; plain/wrapped text surfaces | Append text to the tail, finish a block, add blocks, resize | Completed block identity, correct tail content, selection/caret geometry, and text-cache behavior. |
| `overlays-and-dialogs` | Auto-sized dialogs, menus, tabs, and split/floating panes | First open versus reopen; close/open cycles; switch content | Correct placement/focus; distinguish construction from reuse; detect retained-state growth after closing. |
| `scene-properties` | Retained boundaries with representative text/images | Paint-only color changes, transform/opacity changes, and layout changes as separate scenarios | Composition-only cases should avoid layout; confirm an observable scene/property change and appropriate packet work. |
| `application-shell` | Representative workspace plus an overlay-free widget-book gallery | Panel switches, resize, local state update, and content replacement | End-to-end first content and update latency with expected content present. Keep live performance overlays off. |

Include an idle control: with no event, mutation, or animation due, the host
should not schedule another content frame. Separately measure cached
`Runtime::render` retrieval if useful; do not count it as a newly rendered frame.
A same-value assignment is a different scenario and follows the documented
reactive API behavior rather than an assumed no-op guarantee.

### Size and input axes

- Nonvirtual trees: 100, 1,000, and 10,000 requested controls; report actual
  constructed/mounted widget counts because composites contain multiple pods.
- Nested layout: depth 4, 16, and 64 at matched approximate node counts; vary
  breadth separately so depth and total size are not confounded.
- Virtual models: 1,000, 10,000, and 100,000 items with fixed viewport/overscan.
- Mutations: one leaf, 1%, 10%, and all items; clustered and distributed dirty
  sets are separate cases. Record actual changed and measured counts.
- Text: repeated versus unique labels, short/long wrapping text, and a bounded
  multilingual variant. Use the bundled primary font and record fallback fonts.
- Constraints: fixed viewport, continuously new widths, and alternating widths.
  Repeated-width cache behavior must remain distinguishable from new-width reflow.
- Device scale: 1.0 by default; 1.5 and 2.0 in the extended renderer/desktop set.

Use named presets rather than the full Cartesian product. The default runtime
set uses medium-size fixtures; stress and platform matrices are explicit opt-ins.

## Metrics and instrumentation

### Headline metrics

- Cold startup: construction, attachment, first runtime frame, first content
  present, first settled content, and teardown distributions.
- Updates: operation-to-scene and operation-to-present latency, CPU frame work,
  event/queue delay, number of frames per operation, and update revision age.
- Scaling: cost versus total/mounted nodes, depth, and changed-item count.
- Memory: Rust allocation count/bytes when available, retained bytes after
  settling/teardown, process peak RSS, and cache footprints. Label Rust heap,
  process memory, and GPU/resource memory separately.

### Diagnostic metrics

Extend the existing optional diagnostics instead of adding a parallel profiler:

| Metric | Instrumentation point |
| --- | --- |
| Widget construction/drop/reuse | Fixture factories and `WidgetPod` lifecycle; separate actual constructions from existing rebuild records. |
| Measure requests, executions, cache hits, and forced calls | `WidgetPod::measure`, including its early-return path. |
| Intrinsic horizontal/vertical requests and time | `WidgetPod::intrinsic_size`, distinct from ordinary measure. |
| Arrange requests, executions, cache hits, and translations | `WidgetPod::arrange_with_transform` and descendant-translation path. |
| Distinct constraints and repeated probes per widget/operation | Measure/intrinsic entry points in diagnostic mode; bound trace storage. |
| Dirty roots, affected ancestors, and invalidation kinds | `build_measure_scope`, `build_arrange_scope`, and reactive invalidation drain. |
| Measure, arrange, and graph-refresh wall time | Disjoint spans within `run_measure_arrange_pass`; retain the existing aggregate for compatibility. |
| Layout settling passes and gutter-driven probes | Operation/frame collector and scrollbar resolution; count actual iterations, not an inferred constant. |
| Text preparation/layout work | Existing size-only timing and preparation/full-layout cache snapshot deltas. |
| Scene/renderer work | Widget/layer/command counts, dirty coverage, retained packet rebuild reasons, text atlas misses, uploaded bytes, surface wait and present spans. |

Normalize counters by actual operations and widgets, and keep their raw values.
Report requested measurement versus executed widget methods separately. A parent
may correctly remeasure an unchanged sibling when its allocated constraints
change; regression assertions must respect this dependency.

Use three explicit observation levels:

1. **Timing:** production settings, lightweight outer timers, no detailed widget
   traces, live overlay, allocation instrumentation, or screenshot capture.
2. **Counters/diagnostics:** replay the identical fixture/seed/trace with detailed
   phase, widget, invalidation, cache, and renderer collection enabled.
3. **Allocation/profile:** a separate run with allocator/profiler instrumentation.

Record the observation level in every result and compare like with like. Measure
the overhead of diagnostic mode on representative fixtures. Existing widget
timing samples are inclusive; any new exclusive metric needs explicit nesting
accounting rather than subtraction of unrelated aggregates.

## Reproducible runner and result format

Proposed code layout:

| Location | Responsibility |
| --- | --- |
| `crates/sui-bench/` (`publish = false`) | Runner, shared real-widget fixtures, deterministic update traces, aggregation, and comparison. Default mode uses CPU runtime dependencies. |
| `crates/sui-bench/src/fixtures/` | Construction, mutation, and expected-observation definitions shared across backends. |
| `crates/sui-bench/src/runners/` | Explicit construction/runtime/offscreen/desktop execution; GPU and desktop dependencies feature-gated. |
| `crates/sui-bench/tests/` | Counter/collector correctness, fixture smoke tests, result schema, and comparison tests. |
| `crates/sui-runtime/` | Optional layout/probe/lifecycle counters and disjoint phase spans. |
| `crates/sui-platform/` and `crates/sui-testing/` | Startup milestones and explicit backend/initial-pump control where needed. |

Reuse the widget-book application's existing public builders where appropriate;
keep its adapter optional so basic CPU benchmarks do not require demo startup or
unrelated demo features. Do not change widget behavior to accommodate the runner.

Build once in the workspace's release profile, then invoke the resulting binary
directly. Compilation is never part of a startup timing. Proposed CLI examples:

```sh
cargo build -p sinomo-ui-bench --release --locked
target/release/sui-bench run --preset smoke --mode runtime --output target/widget-bench/smoke
target/release/sui-bench run --preset startup --mode runtime --cold-processes 30 --output target/widget-bench/startup
target/release/sui-bench run --preset updates --mode runtime --trials 5 --steps 200 --output target/widget-bench/updates
cargo build -p sinomo-ui-bench --release --locked --features desktop
target/release/sui-bench run --preset startup --mode desktop --vsync on --cold-processes 30 --output target/widget-bench/desktop
target/release/sui-bench compare target/widget-bench/before target/widget-bench/after
```

Each run writes:

- `manifest.json`: schema/fixture versions, commit and dirty-tree status, binary
  and lockfile fingerprints, rustc/profile/features, OS/CPU, adapter/backend,
  display/refresh rate, viewport/DPR, vsync, output policy, fonts/resources,
  seed, trace version, observation level, and cache/startup regime;
- `samples.jsonl`: raw operation/frame/startup samples, revision IDs, milestones,
  counters, and explicit success/timeout/unsupported/error status;
- `summary.json` and a concise terminal table: distributions, throughput and
  scaling summaries, sample counts, and confidence information;
- optional diagnostics/traces and correctness screenshots from separate runs.

Unsupported metrics are null with a reason, never fabricated zeros. Preserve
failed samples and their errors; exclude them from successful timing summaries
while reporting their frequency. Comparison refuses mismatched fixture/input,
backend, profile, observation level, or font/display configuration by default.

## Sampling and comparison policy

- Cold mode launches one fresh child per sample and performs no warmup. This is
  process-cold, not a guarantee of cold OS page caches or cold GPU driver caches.
  Do not flush host caches; record the regime and alternate revision order.
- Default startup sampling uses 30 independent processes: report median, p90,
  maximum, and raw samples. Collect at least 100 before reporting startup p95;
  avoid unsupported tail precision from a handful of launches.
- Warm update mode starts from a fresh fixture per trial, uses a fixed documented
  warmup trace (initially 32 operations), then records five trials of 200 updates.
  Stress presets can change these counts explicitly; never silently discard slow
  early samples beyond the declared warmup.
- Report median and p95 per trial and across the run. Report p99 only with a
  larger explicit sample set. Estimate uncertainty across independent
  process/trial summaries, not by treating correlated adjacent frames as
  independent experiments.
- Run cases serially. Compare revisions on the same machine using interleaved
  A/B ordering and identical fixture data. Record power/governor conditions and
  competing load; retain outliers and investigate bimodal distributions.
- Desktop user-latency runs keep vsync on and record refresh rate. Throughput
  runs may use vsync off as a separate configuration. Runtime/offscreen duration
  and present-return cadence are separate outputs.
- Record teardown and repeated open/close memory behavior separately from the
  startup headline. Bound runner-owned samples/traces and serialize outside
  timed sections so the collector does not create the leak or latency measured.

Initial comparison produces reports, not universal millisecond pass/fail gates.
On a pinned runner, establish repeatability before enabling a regression gate;
require both a material relative change and an absolute change above its noise
floor, supported by repeated-run uncertainty. Keep structural correctness gates
deterministic and active in CI.

## Correctness gates

- Every scored operation reaches its expected content revision; sampled frame
  indices advance when a new frame is expected. Coalescing is recorded explicitly.
- Bounds, overflow, scrollbars, visible content, focus, and semantics agree with
  the fixture's expected behavior. Validate representative screenshots outside
  timing for renderer/desktop cases.
- Stable keys preserve surviving widget identities; shrinking/closing a fixture
  releases obsolete subscriptions, registry handles, and retained state.
- Local changes leave independent siblings reusable when constraints stay the
  same. Layout changes that affect siblings remain correctly propagated.
- Composition-only scenarios avoid measure/arrange and preserve content packets
  where the selected effect permits it. Paint-only cases still produce the
  intended observable result.
- Virtualized fixtures expose both logical and mounted counts and maintain the
  declared viewport/overscan behavior as logical size grows.
- Scrollbar threshold cases settle without oscillation or unbounded probing.
- A fixture cannot pass by painting an empty scene, skipping the requested
  mutation, silently changing backend, or omitting required semantics work.

Use the existing automated semantics and geometry facilities for these gates.
Correctness assertions and captures happen outside the primary timed region or
in a matched diagnostic replay; retain inexpensive revision tokens in timed runs.

## Implementation sequence and acceptance

Current delivery implements the core of steps 1–3 and the CPU scheduled-update,
lifetime-counter, comparison, and smoke-test portions of steps 4–5. Remaining
extensions are native update traces, allocation instrumentation, full widget-book
startup, composition-only transform workloads, the complete scrollbar-container
matrix, per-widget repeated-constraint traces, and pinned-hardware regression
gates. The guide separates these extensions from the available runner modes.

1. **Runner and timing boundaries:** versioned result schema; subprocess cold
   runner; explicit runtime mode; construction/attachment/first-frame timers;
   idle and counting-leaf self-checks. Prove no eager initial render is hidden.
2. **CPU workload suite:** controls/grid, nested flex, scrollbar thresholds,
   keyed/virtual collections, and streaming text. Add layout/probe counters and
   deterministic correctness gates. Record a baseline before optimization.
3. **End-to-end startup:** offscreen and visible desktop modes, startup observer,
   first-content/settled markers, renderer attribution, and application-shell
   cases. Verify CPU results against the real presentation path.
4. **Frequent updates and lifetime:** timestamped producers, coalescing/queue-age
   metrics, scene-property and overlay cycles, allocation/profile mode, and
   retained-memory checks.
5. **Regression workflow:** matching-run comparison, uncertainty/noise reporting,
   a small correctness/counter CI preset, and opt-in pinned-machine performance
   runs. Keep raw results in artifacts, separate from documentation.

The suite is ready to guide optimization when all headline boundaries have a
working runner, every fixture has a correctness gate, cold runs are genuinely
fresh processes, diagnostic overhead is quantified, and matched runs can be
compared without mixing hardware/cache/backend regimes. CPU-only delivery is a
useful intermediate milestone; it does not close first-presented-frame coverage.

## Current source references

- Construction and attachment: `crates/sui-runtime/src/app.rs`.
- Frame pipeline and invalidation scopes: `crates/sui-runtime/src/lib.rs`.
- Pod measurement, intrinsic sizing, and arrangement: `crates/sui-runtime/src/widget.rs`.
- Existing counters and timings: `crates/sui-runtime/src/diagnostics.rs`.
- Text preparation diagnostics: `crates/sui-text/src/system.rs` and
  `crates/sui-text/src/prepared.rs`.
- Harness backend selection and initial pumping: `crates/sui-testing/src/app.rs`
  and `crates/sui-testing/src/harness.rs`.
- Native presentation: `crates/sui-platform/src/desktop.rs`.
- Existing desktop workloads: `crates/sui-demo/tests/desktop_e2e.rs`.
- Existing headless widget-book workloads: `crates/sui-demo/src/widget_book/tests.rs`.
- Scrollbar layout: `crates/sui-widgets/src/containers.rs`,
  `crates/sui-widgets/src/collection.rs`, `crates/sui-widgets/src/data.rs`, and
  `crates/sui-widgets/src/panes.rs`.
- Related methodology: [Text layout performance](../text-layout-performance.md),
  [Text rendering benchmarks](../text-rendering-benchmarks.md), and
  [Testing guide](../testing.md).
