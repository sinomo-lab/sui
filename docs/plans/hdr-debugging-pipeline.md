# HDR Debugging Pipeline and Windows HDR Bring-Up Implementation Plan

**Goal:** Build a standard HDR debugging pipeline for SUI, use it to diagnose and fix Windows HDR rendering, then clean up SUI Dev HDR-mode visual/performance issues.

**Architecture:** Extend the existing renderer/debug infrastructure with stage-aware capture APIs that can inspect both scene-linear HDR intermediates and final composed SDR/HDR outputs. Ship EXR-first HDR export plus SDR-derived debugging images, surface the interface through standard renderer/platform/testing APIs, then use those artifacts to debug the Windows HDR path on real hardware.

**Tech Stack:** Rust 2024, wgpu 29.0.1, Winit 0.30.13, existing `sui-render-wgpu` output-transform path, `sui-platform` diagnostics, `sui-debug` UI panels, EXR export via a Rust crate, PNG for SDR debug images.

---

## Scope guardrails
- Keep the first HDR export format to **EXR**.
- Keep first SDR debug exports to **PNG**.
- Support at least these capture stages:
  1. renderer HDR intermediate / scene-linear output
  2. final output-transform result used for composed output / screenshot-style comparison
- Design the public interface so future formats (JXR / HEIC / AVIF) can plug in later without redesigning the capture model.
- Preserve the current safe crate boundaries (`sui-platform` remains safe).
- Prefer focused commits per milestone.

---

### Task 1: Capture current renderer/debugging architecture in a durable plan note
**Objective:** Record the current constraints before code changes so the implementation stays aligned with the repo.

**Files:**
- Modify: `docs/plans/2026-04-19-hdr-debugging-pipeline.md`

**Step 1:** Document current facts:
- `capture_rgba()` is SDR-only readback from `offscreen_targets`
- `render_offscreen()` already renders through `Rgba16Float` intermediate
- `submit_output_transform_pass()` is the stage boundary for SDR/HDR debug inspection
- `window_output_diagnostics()` already publishes output-strategy info

**Step 2:** Verify with `git diff -- docs/plans/2026-04-19-hdr-debugging-pipeline.md`

---

### Task 2: Add renderer-facing HDR debug capture model (test-first)
**Objective:** Define a standard, reusable debugging interface for capture stage + export intent.

**Files:**
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Test: `crates/sui-render-wgpu/src/lib.rs`

**Step 1:** Write failing tests for pure API behavior:
- default/stable `DebugCaptureStage` values
- helper deciding whether a stage needs HDR intermediate readback vs final composed readback
- helper describing whether a stage is HDR-capable

**Suggested API surface:**
- `pub enum DebugCaptureStage { HdrIntermediate, FinalComposed }`
- `pub enum DebugSdrVisualization { ToneMappedColor, LuminanceHeatmap, HeadroomHeatmap, ClipMask }`
- `pub enum DebugCaptureEncoding { Exr, Png }`
- `pub struct DebugCaptureRequest { ... }`

**Step 2:** Run
`cargo test -p sui-render-wgpu --lib tests::debug_capture_stage_helpers_classify_hdr_and_final_outputs -- --exact`
Expected: FAIL

**Step 3:** Implement minimal API.

**Step 4:** Re-run the exact test and make it pass.

**Step 5:** Commit with message like:
`feat: add hdr debug capture API model`

---

### Task 3: Add raw renderer readback for HDR intermediate buffers (test-first)
**Objective:** Let the renderer read back HDR intermediate pixels instead of only SDR RGBA screenshots.

**Files:**
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Test: `crates/sui-render-wgpu/src/lib.rs`

**Step 1:** Write failing tests around readback helpers for:
- padded row stripping for float readback
- converting float-channel buffers into tightly packed linear RGBA rows
- if feasible, a targeted render test that confirms `HdrIntermediate` capture returns float content

**Step 2:** Run targeted tests and verify failure.

**Step 3:** Implement minimal readback path:
- add `COPY_SRC` usage to the HDR intermediate target
- add a generic readback helper for RGBA8 and HDR float targets
- add a renderer method like `capture_debug_frame(&mut self, request: &DebugCaptureRequest) -> Result<DebugCaptureArtifact>`

**Step 4:** Re-run targeted tests and `cargo check -p sui-render-wgpu`

**Step 5:** Commit with message like:
`feat: add hdr intermediate debug readback`

---

### Task 4: Add EXR export and SDR-derived debug images (test-first)
**Objective:** Export real HDR captures and practical SDR analysis images.

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/sui-render-wgpu/Cargo.toml`
- Modify: `crates/sui-testing/src/screenshot.rs`
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Test: `crates/sui-render-wgpu/src/lib.rs`
- Test: `crates/sui-testing/src/screenshot.rs`

**Step 1:** Write failing tests for:
- luminance-map derivation from linear HDR pixels
- headroom-map derivation relative to SDR white
- clip-mask derivation
- export filename/format selection for EXR vs PNG

**Step 2:** Run the exact tests and verify failure.

**Step 3:** Implement:
- EXR dependency
- exporter helpers that can write linear RGBA float data to EXR
- PNG export for SDR debug buffers
- keep AVIF export optional in the workflow; current high-quality rav1e still-image encoding is much slower than EXR/PNG output and should not be treated as the fast iteration path

**Step 4:** Extend screenshot/testing artifacts with filenames like:
- `frame-hdr-intermediate.exr`
- `frame-final-composed.png`
- `frame-headroom-map.png`
- `frame-clip-mask.png`

**Step 5:** Run:
- targeted tests
- `cargo check -p sui-render-wgpu`
- `cargo check -p sui-testing`

**Step 6:** Commit with message like:
`feat: add exr hdr export and sdr debug images`

---

### Task 5: Surface the standard debugging interface through platform/testing APIs (test-first)
**Objective:** Make the new debug pipeline usable by both SUI development and library users.

**Files:**
- Modify: `crates/sui-platform/src/headless.rs`
- Modify: `crates/sui-platform/src/lib.rs`
- Modify: `crates/sui-testing/src/harness.rs`
- Modify: `crates/sui-dev/src/app.rs`
- Possibly modify: `crates/sui-debug/src/lib.rs`

**Step 1:** Write failing tests proving:
- platform/testing can request a debug capture by stage
- SUI Dev exposes enough debug controls/actions to trigger captures
- diagnostics can describe selected stage/format/visualization

**Step 2:** Implement minimal wiring:
- renderer-level capture request
- platform/headless forwarding method
- testing harness helper for debug captures
- SUI Dev debug panel/button/hotkey/action for capture

**Step 3:** Verify with:
- targeted tests
- `cargo check -p sui-platform`
- `cargo check -p sui-testing`
- `cargo check -p sui-dev`

**Step 4:** Commit with message like:
`feat: expose standard hdr debugging interface`

---

### Task 6: Use the new debugging pipeline to diagnose Windows HDR failure in the validation widget
**Objective:** Produce artifacts that show why HDR content is not visibly behaving correctly even though the app enters HDR mode.

**Files:**
- Modify: `crates/sui-dev/src/app.rs` only if extra debug UI is needed
- Create: debug output artifacts under an appropriate artifacts directory if useful

**Step 1:** Run SUI Dev on the Windows PC with HDR enabled and capture at least:
- HDR intermediate EXR
- final composed PNG/debug result
- luminance/headroom/clip maps

**Step 2:** Compare whether the validation swatches are:
- bright/distinct in the intermediate but lost in final output transform
- clipped/tone-mapped too early
- never exceeding SDR in the intermediate at all
- losing color due to wrong color conversion or output-strategy assumptions

**Step 3:** Record findings grounded in artifacts.

---

### Task 7: Fix Windows HDR rendering based on captured evidence (test-first where possible)
**Objective:** Make HDR content actually survive to the Windows HDR path.

**Files:**
- Likely modify: `crates/sui-render-wgpu/src/lib.rs`
- Likely modify: `crates/sui-render-wgpu/src/scene.rs`
- Possibly modify: `crates/sui-platform/src/display_capabilities.rs`

**Step 1:** Add a failing regression test for the identified issue.

**Step 2:** Run the test and verify failure.

**Step 3:** Implement the minimal fix.

**Step 4:** Verify with:
- the new regression test
- existing HDR strategy tests
- `cargo check -p sui-render-wgpu`
- `cargo check -p sui-dev`

**Step 5:** Re-capture artifacts on Windows and confirm the fix.

**Step 6:** Commit with message like:
`fix: preserve hdr content through windows output path`

---

### Task 8: Clean up SUI Dev HDR-mode visuals and performance issues
**Objective:** Use the same debug tooling to fix user-visible HDR-mode issues beyond the core bug.

**Files:**
- Modify: `crates/sui-dev/src/app.rs`
- Modify: `crates/sui-widget-book/src/lib.rs`
- Possibly modify: renderer/platform files if instrumentation reveals extra issues

**Step 1:** Check visual correctness:
- unreadable labels
- poor contrast
- clipped debug panel output
- validation view layout issues

**Step 2:** Check performance:
- use existing perf diagnostics plus debug capture frequency constraints
- avoid idle redraw churn / capture-induced stalls

**Step 3:** Add focused tests where practical.

**Step 4:** Verify targeted checks for `sui-dev` / `sui-widget-book`.

**Step 5:** Commit with message like:
`fix: polish sui-dev hdr mode diagnostics and performance`

---

## Suggested first validation commands
```powershell
cargo test -p sui-render-wgpu --lib tests::debug_capture_stage_helpers_classify_hdr_and_final_outputs -- --exact
cargo check -p sui-render-wgpu
cargo check -p sui-testing
cargo check -p sui-platform
cargo check -p sui-dev
```

## Definition of done
- Standard debug capture API exists and is reusable by library users.
- HDR intermediate capture works and exports EXR.
- Final composed capture works and exports SDR debug imagery.
- SUI Dev can trigger/debug captures through standard infrastructure.
- The new pipeline is used to diagnose the Windows HDR issue.
- Windows HDR content in the validation view is visibly and artifact-wise correct.
- SUI Dev HDR-mode visual/performance issues are addressed.
- Progress is reported hourly in this thread.
