# Renderer pipeline performance review

Measured on 2026-09-22 against `aadf658`, with diagnostic-test additions. This
review identifies optimization candidates. The measurements in sections 1–3
describe that baseline; the implementation update below records subsequent work.

## Implementation update: composed and prepared fragment reuse

The first implementation caches composed geometry and prepared GPU passes by
immutable packet identity plus presentation and resource-slot dependencies.
Exact content/state equality also bypasses repeated packet hashing. Snapshot
traversal and shared-arena batching remain future work.

Paired before/after/after/before runs of the same small-demo workload, with
`WGPU_DEBUG=0` and `WGPU_VALIDATION=0` for both binaries, measured:

| Cost | Before | After |
|---|---:|---:|
| Full-repaint frame median | 2.78–2.93 ms | 2.32–2.39 ms |
| Renderer average | 1.92–2.11 ms | 1.54–1.62 ms |
| Composition average | 0.23–0.25 ms | 0.033–0.035 ms |
| Retained-state update average | 0.40–0.45 ms | 0.21–0.22 ms |

A separate matched pair with the normal development backend flags also improved:
full-repaint median fell from 3.83 ms to 3.42 ms, and renderer average from
3.08 ms to 2.65 ms. No backend or validation default was changed by the optimization.

Animated frames prepare one fragment and reuse 92. Unchanged frames reuse all
93 submitted fragments; 109 logical packets include culled/empty work. The
16-command invalidation policy and 160 draws are preserved. Zoom measurements
overlap between revisions, so no zoom speedup is established.

Validation includes the 230 previously active renderer tests and two new
pixel-comparison tests covering cache hits, localized edits, translation,
opacity, clipping, DPI changes, removal, and analytic-path arena remapping.

## Measurement setup

- Windows, NVIDIA RTX 3090, DirectX 12, driver `32.0.16.1656`.
- Optimized development test profile, 1440 x 900 actual node-demo window,
  including the five-node graph, custom widgets, sidebar, and application shell.
- 20 warmup frames and 90 measured frames per scenario; 17 ms pacing delay
  outside the timed work. Animation events use the runtime scheduler.
- Wall-clock CPU timings include offscreen rendering and queue submission.
  They exclude the pacing delay, native presentation, and GPU execution timing.
- Backend flag comparisons use the same binary. Disabling native debug and
  validation flags is not equivalent to testing a complete release build.
- Stage timings are averages; frame percentiles are computed separately.
  `command_finish_us` is part of `submit_us`, and packet build time is part of
  retained-state update time. Nested widget timings must not be summed.

## 1. Cache hits still perform substantial whole-scene CPU work

The actual demo's unchanged-frame probe produced zero packet rebuilds and
zero vertex-upload bytes, yet submitted 109 packets and 160 draws each frame.
With native debug/validation disabled, its renderer alone averaged 1.91–1.96 ms.
Default debug/validation increased that to 3.01–3.09 ms.

Even with backend debug/validation disabled, an unchanged frame spent about
0.58 ms traversing the scene, 0.40 ms updating retained state, and 0.22 ms
composing fragments. These are separate stages. This probe deliberately calls
the renderer: it does not imply the platform continuously renders an idle window.
The same repeated work matters when one small animated element needs a frame.

Code evidence:

- `RetainedCompositorState::refresh_frame_state` in
  [retained.rs](../crates/sui-render-wgpu/src/retained.rs) always builds a new
  snapshot. `build_snapshot` resets property trees and walks the scene.
- `upsert_packet` normalizes packet state, hashes content, and compares scene
  and raster state before deciding that the cached packet can be reused.
- `append_items_to_submission_for_phase` creates fresh `DrawOpArena` values.
  [draw.rs](../crates/sui-render-wgpu/src/draw.rs) copies cached geometry into
  them; transformed fragments are cloned, transformed, then appended.
- [submission.rs](../crates/sui-render-wgpu/src/submission.rs) prepares vertex
  batches again for every fragment. [uploads.rs](../crates/sui-render-wgpu/src/uploads.rs)
  compares the resulting bytes against CPU shadows to avoid GPU uploads.

**Candidate:** retain immutable composition snapshots and prepared GPU-ready
packet data, with explicit content, property, and resource generations. Cache
hits should avoid reconstructing and comparing the same intermediate data.
Correctness must still account for atlas recycling, inherited text backdrops,
clip/transform/opacity changes, viewport/DPI changes, and external resources.
The measurements establish the cost; the saving from this redesign is unmeasured.

## 2. Fine cache granularity is coupled to GPU submission granularity

The default `packet_draw_limit` is 16. Each packet becomes a submission fragment
and owns separate retained vertex buffers. Matching draws cannot combine across
those buffer boundaries, even when adjacent geometry uses the same pipeline.

A controlled 1,024-dot renderer-only test varied the private packet limit,
in forward and reverse order, while keeping the scene identical:

| Commands per packet | Draws | Retained vertex buffers | Unchanged median renderer time | Commands rebuilt by one-dot edit |
|---|---:|---:|---:|---:|
| 16 | 66 | 66 | 1.57–1.64 ms | 16 |
| 64 | 18 | 18 | 1.07–1.19 ms | 64 |
| 256 | 6 | 6 | 1.02–1.09 ms | 256 |

Counts include scene and output draws; buffer counts cover retained vertex
buffers, not all renderer resources. Unchanged frames rebuilt nothing and
uploaded zero bytes at every limit. A one-dot edit uploaded only 16 bytes at
every limit, but CPU rebuilding expanded with packet size. Pixel comparisons
check both unchanged and edited output against the default partitioning.

**Candidate:** separate invalidation blocks from GPU allocation and batching.
Keep localized content invalidation while allocating compatible geometry in
shared arenas and combining adjacent compatible draws. Preserve ordering,
clipping, material, and layer-composition semantics. Merely raising the global
limit trades away local-edit efficiency; the synthetic result is not a measured
speedup for the complete demo or for a shared-arena implementation.

## 3. Much of the earlier command-finalization cost was development overhead

Two runs in each configuration of the actual demo gave:

| Backend flags | Full-repaint frame median | Command finalization average |
|---|---:|---:|
| Development defaults | 3.82–3.97 ms | 1.24–1.35 ms |
| `WGPU_DEBUG=0`, `WGPU_VALIDATION=0` | 2.71–2.77 ms | 0.32–0.34 ms |

[device.rs](../crates/sui-render-wgpu/src/device.rs) uses WGPU's environment-aware
instance defaults. In this build those defaults enable native debugging and
validation. The earlier approximately 1.3 ms command-finalization measurement
therefore should not be treated as an unavoidable driver or GPU cost.

Disabling only SUI renderer timing diagnostics made little difference in the
paired probe: 3.97 versus 3.99 ms frame median with backend validation enabled,
and 2.77 versus 2.76 ms with it disabled. This distinguishes timing-collection
overhead from backend validation overhead. The desktop host already enables
renderer diagnostics only when detailed scene statistics are requested.
Zero stage counters in the uninstrumented scenario mean counters were disabled,
not that rendering work disappeared.

**Candidate:** benchmark performance with explicitly recorded backend and
validation settings. Cached render bundles could reduce stable command replay,
but that implementation and its benefit have not been tested here.

## Priority and verification

First prototype generation-based reuse of composition and prepared geometry.
Then prototype shared GPU storage/batching independently of the 16-command
invalidation policy. Both target work that persisted with zero cache misses.
Measure steady animation, complete repaint, single-element edits, pan/zoom,
text/atlas changes, and multiple windows before selecting a policy.

No evidence from these probes points to repeated glyph rasterization, curve
uploads, or vertex-upload bandwidth as the steady-frame bottleneck. Zoom is a
different workload: the demo rebuilt 85 packets and uploaded roughly 104 KB per
frame, so steady-state improvements alone do not establish a zoom improvement.

Run the probes serially in PowerShell:

```powershell
$env:SUI_PROFILE_WIDGET_TIMINGS = '1'
cargo test -p sinomo-ui-demo --features sui-runtime/layout-diagnostics --lib small_node_demo_paint_profile -- --ignored --nocapture
cargo test -p sinomo-ui-render-wgpu --lib retained_pipeline_granularity_profile -- --ignored --nocapture --test-threads=1
```

For the separate backend-flag comparison, set `WGPU_DEBUG=0` and
`WGPU_VALIDATION=0` in a fresh process and run the same demo probe. Production
packet limits and backend defaults were not changed by this review.
