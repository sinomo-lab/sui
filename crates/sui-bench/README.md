# Widget performance benchmarks

`sui-bench` measures widget construction, runtime attachment, initial content,
and updates through SUI's real retained runtime. Results go to an explicit
artifact directory, never into documentation. The runner refuses to overwrite
a nonempty directory.

## Run

Build once, then invoke the executable directly so compilation is excluded:

```sh
cargo build -p sinomo-ui-bench --release --locked
target/release/sui-bench list
target/release/sui-bench run --preset smoke --output target/widget-bench/smoke
target/release/sui-bench run --preset startup --fixture controls-grid --size 1000 --cold-processes 30 --output target/widget-bench/startup
target/release/sui-bench run --preset updates --size 128 --trials 5 --steps 200 --output target/widget-bench/updates
```

Every cold sample and every independent warm trial runs in its own child
process. Startup has no warmup. Update trials construct and settle the initial
tree, execute the declared warmup trace, then retain that tree for measured
updates. `smoke` checks all fixtures with small inputs; use larger sizes and
more samples for conclusions about performance.

| Mode | Measurement |
| --- | --- |
| `construct` | Input preparation, real widget construction, attachment, and teardown; no frame rendering. |
| `runtime` | CPU scene production, layout, graph, paint commands, and semantics. No GPU dependency in the default build. |
| `offscreen` | Runtime plus actual WGPU submission. A final pixel readback validates nonempty/nonuniform output outside timing. |
| `desktop` | Visible native startup through first content present-return and first settled content. Currently accepts the `startup` preset only. |

GPU and native runners are opt-in:

```sh
cargo build -p sinomo-ui-bench --release --locked --features gpu
target/release/sui-bench run --mode offscreen --fixture controls-grid --output target/widget-bench/offscreen
cargo build -p sinomo-ui-bench --release --locked --features desktop
target/release/sui-bench run --preset startup --mode desktop --fixture application-shell --cold-processes 30 --vsync on --output target/widget-bench/desktop
```

Missing display/GPU support produces an explicit unsuccessful `unsupported`
trial and a nonzero exit. Software adapters require `--allow-software` and are
identified in the artifacts. Desktop DPR must match the requested `--dpr`;
actual native DPR is recorded. Present-return is a host API timestamp, not
physical scanout. Offscreen timings describe CPU work/submission, not GPU
execution. GPU timestamp metrics are unavailable rather than reported as zero.

## Public startup and redraw controls

`--builder runtime` uses the lower-level runtime builder. `--builder public`
uses `sui::App`, including its normal resource and renderer configuration setup;
build with `--features public-api,gpu,diagnostics` to measure the GPU-enabled
public facade even in `construct` mode. Both builders use the same widget tree.

`--redraw natural` is the default: diagnostics start collectors directly without
injecting an event. `--redraw requested` dispatches `RedrawRequested` before each
render, independently of diagnostics. Compare these lanes separately. Native
mode always follows its platform's event flow.

Fixture version 2 places the content revision marker in its own semantic leaf,
so verification does not invalidate the entire semantic tree on each mutation.
The modal fixture keeps its marker on the root ancestor because unrelated
semantic leaves are filtered while a modal is active. Benchmark adapters opt
into `Widget::supports_output_reuse`; widget content and mutation traces match
between revisions.

## Fixtures and mutations

| Fixture | Implemented workload |
| --- | --- |
| `controls-grid` | Grid of dynamic labels and buttons; local, distributed, or whole-set text changes. |
| `nested-flex` | Real nested Flex/Padding containers with a dynamic label population; configurable depth. |
| `scrollbar-thresholds` | ScrollView with content near the horizontal gutter threshold and vertically overflowing content; alternating widths exercise coupled gutters. |
| `keyed-collection` | VirtualList backed by a keyed model; move, prepend, and remove cycles. |
| `virtual-collection` | Large logical VirtualList model with a bounded viewport/overscan; alternating scroll direction. |
| `streaming-document` | RichDocumentView with retained completed paragraphs and tail appends/new blocks. |
| `overlays-and-dialogs` | A retained SwitchView opens/closes a Dialog, separating initial construction from reuse. |
| `scene-properties` | Paint-only changes around a retained controls tree; diagnostic tests require zero layout execution. |
| `application-shell` | Split workspace with search/actions and a scrollable controls grid. |

`--mutation` overrides the fixture default with `local`, `distributed`, `all`,
`resize`, `paint`, `reorder`, `scroll`, `rebuild`, or `idle`. The controls-grid
`rebuild` control recreates its entire subtree for each local text change, making
construction churn observable alongside the retained `local` case. Unsupported fixture/mutation
combinations are rejected. `--change-fraction` controls distributed text changes.
`--seed` controls deterministic labels and mutation positions. `--size` is the
logical item/control count, not the actual pod count; samples record mounted
widgets separately. Composite widgets add pods, while virtual lists mount only
their viewport window.

Further controls are `--depth`, `--width`, `--height`, `--dpr`, `--warmup`,
`--steps`, `--trials`, `--cold-processes`, and `--timeout-secs`. Run serially on a
quiet machine. Built-in controls retain their normal theme fonts; fixture labels
use the bundled Noto Sans. The manifest records the bundled font and installed
fallback-font inventory fingerprint.

Acknowledged updates are the default. `--rate-hz` replays scheduled updates:
when service falls behind, all due mutations are applied before the next frame.
Samples retain offered/coalesced update counts, service time, and oldest-update
queue age. The fixed simulation clock drives runtime timers; wall-clock time
measures cost. There is no wall-clock sleep in acknowledged CPU updates.

Every fixture exposes a revision token through semantics and verifies the latest
revision, changed labels where applicable, finite geometry, and nonempty content.
Virtual-list runs additionally reject unbounded mounting at large model sizes.
An idle control checks scheduling and counts cached scene retrieval as zero new
frames. Settling is bounded to 16 frames, and subprocesses have a timeout.

## Diagnose work

Use a separate build/run for detailed observations:

```sh
cargo build -p sinomo-ui-bench --release --locked --features diagnostics
target/release/sui-bench run --preset updates --fixture controls-grid --size 128 --diagnostics --output target/widget-bench/diagnostics
```

The runtime `layout-diagnostics` feature supplies thread-local construction/drop,
measure/cache/constraint, size-only hook executions, local/window query cache hits,
paint/semantics reuse hits, and intrinsic cache hits,
arrange/cache/translation, paint, semantics,
and gutter-iteration counters. Disjoint root measure, arrange, and graph spans
complement the existing aggregate frame phase. These call sites compile away in
normal builds without the feature; the collector is inactive until requested.

Diagnostic samples also retain bounded inclusive widget hotspots, invalidation
details, rebuild counts, and text-cache snapshots. GPU diagnostic runs include
renderer draw/upload/atlas/retained-packet statistics and device, target,
text-engine, pipeline creation, composition, batching, upload, encode, and queue
submission timings and submission counts. Atlas setup separates allocation,
explicit clearing, growth copying and bind-group creation; automatic WGPU
initialization during partial uploads is included in upload/resource timing.
Pipeline creation is included in pass encoding; do not add
these overlapping spans. Target preparation currently describes the offscreen
path. The fixture's transparent child adapter forwards both size and axis queries,
as custom wrappers can do through `SingleChild`. Vertex upload bytes count
actual buffer writes, including partial updates; atlas upload bytes exclude GPU
page clears. Warmup and teardown work
are identified separately. Text cache values are cumulative snapshots; use
adjacent snapshots for deltas. Do not sum inclusive widget timings into totals.

Primary runs disable the profiling environment variables in each child. Detailed
observation adds overhead, so compare only matching observation levels and build
features. Allocation instrumentation is not included; use an external profiler
in a separate run. Linux process RSS is reported after teardown and includes
the runner and allocator's retained memory, not just live widgets or GPU memory.

## Artifacts and comparison

- `manifest.json`: fixture/schema versions, build commit/source fingerprint,
  dirty build state, binary/lockfile fingerprints, compiler/profile/features,
  CPU/OS/font information, configuration, actual backends, and completion status.
- `samples.jsonl`: one record per process/trial with raw startup/update samples,
  explicit errors, parent spawn-to-exit time, diagnostics, and teardown data.
- `<fixture>-<trial>.json` and `.log`: child result and captured output, including
  evidence for failed or timed-out processes.
- `summary.json`: grouped medians/tails, independent trial summaries, bootstrap
  intervals of trial medians when enough trials exist, and failure counts.

Serialization and terminal output occur outside timed operations. Per-operation
correctness/diagnostic collection still contributes runner overhead between
scheduled arrivals; inspect service time separately from queue age. A scheduled
diagnostic replay should not be used as an uninstrumented throughput claim.

```sh
target/release/sui-bench compare target/widget-bench/before target/widget-bench/after
```

Comparison rejects incomplete runs or differences in inputs, fixture version,
configuration, compiler/features, fonts, or hardware/backend metadata. Build
commit and source fingerprints may differ: those identify the compared revisions.
Compile both revisions with the same profile/toolchain and interleave their run
order. Results include all successful samples; failures are retained separately.
Do not infer performance from a smoke run or from one outlier.

Startup is process-cold with uncontrolled OS/driver caches. The first 30 samples
support a median and p90; p95 requires at least 100 samples. p99 requires a larger
update run. The comparison reports descriptive changes without an automatic
machine-dependent regression gate. Establish repeatability on a pinned host
before selecting thresholds.

## Scope and follow-ups

The suite covers construction, CPU updates, offscreen rendering, and native
startup. Native scheduled-update automation, allocation profiling, full widget-book
startup, composition-only transforms, and the full Table/VirtualTable/floating-pane
scrollbar matrix remain extensions. The native startup callback is compiled and
tested through interface checks on headless hosts; measuring it requires a real
display server.

See the [suite design](../../docs/plans/widget-performance-benchmark-plan.md) for
the complete measurement model and extension criteria. Benchmark source stays in
this crate; measurements stay under `target/widget-bench/` or CI artifacts.

## Renderer and nested-layout comparisons

Use the same fixture inputs, feature set, CPU affinity, and observation level on
both revisions. Keep a copy of each executable before editing/building the next
revision. Run CPU timing separately from detailed GPU diagnostics:

```sh
target/release/sui-bench run --preset updates --size 128 --trials 5 --steps 200 --output target/widget-bench/cpu-updates
for depth in 2 4 8; do
  target/release/sui-bench run --preset updates --fixture nested-flex --size 128 --depth "$depth" --trials 3 --steps 100 --output "target/widget-bench/depth-$depth"
done
# Build with --all-features for these instrumented GPU runs.
target/release/sui-bench run --preset startup --mode offscreen --fixture controls-grid --size 128 --cold-processes 30 --diagnostics --output target/widget-bench/gpu-startup
target/release/sui-bench run --preset updates --mode offscreen --size 128 --trials 3 --steps 100 --diagnostics --output target/widget-bench/gpu-updates
```

Check local changes against whole-set text changes, resize, paint-only changes,
and scrolling. For retained rendering, inspect packet build work and actual
vertex/atlas uploads together with latency. The renderer tests compare chunked
packets with an unsplit reference, including clips, transforms, text policy,
DPI changes, atlas recycling, buffer growth, and window/resource lifetimes.
The transparent `Child` adapter explicitly forwards `measure_size`, as built-in
wrappers do; fixture content, topology, constraints, and mutations are unchanged.
Custom containers retain the full-measure fallback until they opt into this hook.
Repeat comparisons in reverse run order on shared hosts. Native presentation
still needs a display-equipped runner.

## Text, output reuse, and startup preparation

Paragraph preparation shares glyph shaping across alignment and wrapping changes;
the retained line-layout key still includes width, alignment, and wrap policy.
Button measurement and painting share one persistent natural layout. Check both
preparation misses and materialized-layout misses when diagnosing cold text or
whole-set updates, and preserve caret, selection, bidi, and optical geometry.

Tracked sources read only in paint or semantics invalidate that widget's output
in that phase. Explicit subtree/window requests and cross-phase or event observers
retain their conservative behavior. Output-cache hits borrow shared fragments and
append into the destination directly. The byte budgets, context generations,
opaque-widget fallback, and text-handle checks still apply.

Offscreen startup creates the backend and calls `WgpuRenderer::prepare_device`
before the initial size/DPI event. Desktop startup uses `prepare_window` with the
actual surface, runs CPU layout, then completes `register_window`. All of this
work remains inside startup timing. Adapter/device work may overlap CPU layout:
`device_prepare_us` reports that work and `device_wait_us` reports the time first
use blocks waiting for it. Do not add the overlapping work to frame duration.
`command_finish_us` is nested inside `queue_submit_us`; it separates command-buffer
finishing from the queue call. Other targets retain their existing initialization
path. Compare startup using matching harness ordering, and retain failed/cancelled
initialization evidence alongside successful trials.
Native external-texture clients keep synchronous registration so their GPU context
remains available during initial layout.
