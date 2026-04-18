# HDR And Wide-Gamut Display Support Proposal

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Add a platform-aware color pipeline to SUI that can render correctly on SDR, wide-gamut SDR, and HDR displays without regressing the current SDR path.

**Architecture:** Introduce explicit output color-space and dynamic-range concepts above the `wgpu` surface layer, keep the renderer internally linear, and split implementation into: capability detection, swapchain/output configuration, color conversion/tone mapping, and validation tooling. Prefer a conservative staged rollout: first make wide-gamut SDR and HDR-capable intermediate rendering correct, then enable native HDR presentation per platform where the stack actually supports it.

**Tech Stack:** `wgpu 29.0.1`, `winit 0.30.13`, current `sui-render-wgpu` surface configuration path, `sui-core::Color` / `ColorSpace`, platform-specific presentation via DXGI/Windows Advanced Color, Metal/CAMetalLayer EDR, and WebGPU canvas HDR + Display-P3 support.

---

## Why this work matters

SUI already has the beginnings of a color-management story, but not an end-to-end output pipeline.

Current state in the repo:

- `sui-core::ColorSpace` already includes:
  - `Srgb`
  - `LinearSrgb`
  - `DisplayP3`
- `shader_color()` in `crates/sui-render-wgpu/src/scene.rs` currently treats `DisplayP3` as if it used the sRGB transfer function directly and does **not** convert primaries.
- `configure_surface()` in `crates/sui-render-wgpu/src/scene.rs` currently picks the first sRGB-compatible surface format and does not expose any HDR or wide-gamut policy.
- `create_surface_state()` in `crates/sui-render-wgpu/src/lib.rs` creates a standard `wgpu` surface with no display capability negotiation beyond format/present mode.
- `sui-platform` creates normal `winit` windows and does not currently detect monitor HDR, wide gamut, EDR headroom, or desktop color space.

So the repo is currently in an in-between state:

- SUI can represent some color intent in CPU-side APIs.
- But the renderer and swapchain path still assume an SDR/sRGB-style output model.

This proposal aims to fix that in a way that preserves correctness on SDR displays while making HDR and wide-gamut output implementable and testable.

---

## Literal research summary

### 1. Current `wgpu` surface model is not enough by itself for native HDR correctness

`wgpu::SurfaceConfiguration` exposes:

- `usage`
- `format`
- `width`
- `height`
- `present_mode`
- `desired_maximum_frame_latency`
- `alpha_mode`
- `view_formats`

but not a portable native concept of:

- output color primaries
- HDR transfer function
- OS HDR metadata
- monitor luminance/headroom

That means SUI should **not** expect a single cross-platform `wgpu` switch to solve HDR presentation everywhere.

Implication for SUI:

- build a renderer architecture that can output correct HDR/wide-gamut content internally
- then layer platform-specific native presentation support where possible
- keep an SDR tone-mapped fallback everywhere

### 2. Windows Advanced Color prefers FP16/scRGB as the canonical composition path

Microsoft’s Advanced Color guidance says Windows composes HDR desktops in a canonical composition color space (CCCS):

- scRGB primaries (BT.709 / sRGB primaries)
- linear gamma
- FP16 precision

Important consequences:

- FP16 linear scRGB is the most natural internal representation for desktop HDR composition on Windows.
- Colors outside `[0, 1]` are meaningful and expected.
- The OS may clip or downconvert when the display cannot represent the full signal.
- Microsoft explicitly recommends reacting to monitor capabilities instead of relying on static HDR metadata delivery.

Implication for SUI:

- an internal linear FP16 scene/output path is the right foundational choice.
- native Windows HDR support should likely target a linear-FP16/scRGB-style swapchain path first, not PQ/HLG-first presentation.

### 3. Apple HDR presentation uses EDR with explicit colorspace and HDR opt-in

Apple’s Metal/CAMetalLayer HDR guidance says native HDR/EDR presentation needs:

- `wantsExtendedDynamicRangeContent = true`
- an extended-range color space on the layer
- typically an FP16 render target / pixel format such as `RGBA16Float`

Apple also distinguishes between:

- simply displaying content with a known transfer function and color space
- using system tone mapping
- doing your own tone mapping into available EDR headroom

Implication for SUI:

- on macOS, proper HDR output is not just “render brighter values”; the layer must opt into EDR and carry the right colorspace.
- if `wgpu` does not expose enough of that through `winit`/surface configuration, native HDR on macOS may require a targeted escape hatch or platform integration layer.

### 4. WebGPU now supports HDR canvas output and Display-P3 color spaces on the web

WebGPU / browser-side research shows:

- web canvases can now be configured for HDR using float formats like `rgba16float`
- HDR headroom is enabled with `toneMapping: { mode: "extended" }`
- `PredefinedColorSpace` includes:
  - `Srgb`
  - `DisplayP3`

Implication for SUI:

- wasm/web may become the easiest place to ship a first-class HDR/wide-gamut prototype.
- the web path should not be forced to wait on native desktop support if SUI’s abstractions are designed correctly.

### 5. Learn-wgpu guidance still recommends HDR intermediate rendering plus tone mapping

The widely used Learn WGPU HDR tutorial still assumes:

- render into an HDR intermediate texture such as `Rgba16Float`
- tone map into a presentable surface format

Implication for SUI:

- even if native HDR presentation is delayed or partial, SUI should still add an HDR-capable internal render graph now.
- that internal path is also necessary for bloom, exposure, physically-based lighting, and reliable future HDR support.

---

## Design principles for SUI

1. **Keep the renderer internally linear.**
   - The renderer should do blending, compositing, gradients, text coverage, and image composition in a linear working space.
   - Avoid mixing output-transfer assumptions into draw command generation.

2. **Separate scene color encoding from display output encoding.**
   - Scene colors and textures should not be tightly coupled to the monitor format.
   - Output conversion should happen late, ideally in a final color-management pass.

3. **Treat HDR and wide gamut as related but separate features.**
   - Wide gamut without HDR is valid.
   - HDR without full BT.2020 gamut is valid.
   - The abstraction should model both dimensions independently.

4. **Prefer platform-native output where supported; fall back to SDR tone mapping everywhere else.**
   - SUI should always be able to render correctly on a plain SDR sRGB display.
   - Native HDR should be additive, not a prerequisite.

5. **Be conservative about API promises.**
   - `wgpu` + `winit` portability is not enough to promise fully identical HDR output across Windows, macOS, Linux, and web.
   - Make capability discovery explicit and expose “supported / emulated / unavailable” states.

---

## Proposed output model

Add an explicit output description that separates working space, display intent, and presentation path.

### New concepts

#### `WorkingColorSpace`
The renderer’s internal compositing space.

Initial choice:
- `LinearSrgb` for SDR-compatible rendering
- future option: `LinearDisplayP3`

Recommended first implementation:
- keep scene math in linear-sRGB-like working space
- use FP16 intermediates when HDR is enabled

#### `DisplayColorPrimaries`
The target display gamut intent.

Proposed enum:
- `Srgb`
- `DisplayP3`
- `Rec2020`
- `Unknown`

#### `DynamicRangeMode`
The target presentation range.

Proposed enum:
- `Sdr`
- `Extended` (values may exceed SDR white but are still display-referred / headroom-based)
- `Hdr10Pq` (platform-specific, explicit PQ path)

#### `OutputTransferFunction`
How the final output buffer should be interpreted.

Proposed enum:
- `Srgb`
- `Linear`
- `Pq` (ST.2084)
- `Hlg`

#### `DisplayCapabilities`
Per-window / per-monitor detected capability snapshot.

Proposed fields:
- `supports_wide_gamut: bool`
- `supports_hdr: bool`
- `preferred_primaries: DisplayColorPrimaries`
- `preferred_dynamic_range: DynamicRangeMode`
- `max_luminance_nits: Option<f32>`
- `sdr_white_nits: Option<f32>`
- `max_content_headroom: Option<f32>`
- `native_hdr_presentation_supported: bool`
- `notes: &'static str` or owned string for diagnostics

---

## Proposed API changes

### Task 1: Extend `sui-core::ColorSpace`

**Objective:** Make CPU-side color types expressive enough for wide gamut and HDR implementation.

**Files:**
- Modify: `crates/sui-core/src/color.rs`
- Test: `crates/sui-core/src/color.rs`

Recommended changes:

- Keep existing:
  - `Srgb`
  - `LinearSrgb`
  - `DisplayP3`
- Add:
  - `LinearDisplayP3`
  - optionally `Rec2020`
  - optionally `LinearRec2020`
- Stop assuming `DisplayP3` can be converted by just applying the sRGB transfer curve.

Important note:
- Display-P3 and sRGB share a D65 white point and use the sRGB-style transfer curve in common UI practice, but they **do not share primaries**.
- SUI currently handles transfer but not gamut conversion.

### Task 2: Add output policy types to runtime / diagnostics

**Objective:** Make output policy visible and configurable per window.

**Files:**
- Modify: `crates/sui-runtime/src/diagnostics.rs`
- Modify: `crates/sui/src/lib.rs`
- Modify: `crates/sui-platform/src/lib.rs`

Add window-level configuration types such as:

- `WindowOutputColorPrimaries`
- `WindowDynamicRangeMode`
- `WindowToneMappingMode`
- `WindowColorManagementMode`

Suggested initial modes:
- `Automatic`
- `ForceSdr`
- `PreferWideGamut`
- `PreferHdr`

### Task 3: Add display capability detection layer

**Objective:** Separate platform capability discovery from renderer logic.

**Files:**
- Create: `crates/sui-platform/src/display_capabilities.rs`
- Modify: `crates/sui-platform/src/desktop.rs`
- Modify: `crates/sui-platform/src/lib.rs`

Responsibilities:
- detect the monitor associated with each window
- refresh capabilities when a window moves between monitors
- cache per-monitor capability snapshots
- expose the result to runtime diagnostics and renderer configuration

Platform notes:

- **Windows:** query Advanced Color / monitor luminance / color-space capability via DXGI-side interop if possible
- **macOS:** detect EDR-capable displays and current EDR headroom
- **web:** detect support for float canvas formats, Display-P3, and extended tone mapping
- **Linux:** likely start with “unknown / SDR-only” unless a backend path is clearly available

### Task 4: Refactor surface configuration into a richer output path

**Objective:** Move surface choice from “pick first sRGB format” to “pick the best presentation path for this window.”

**Files:**
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Modify: `crates/sui-render-wgpu/src/gpu.rs`

Current code:
- `preferred_surface_format()` prefers `TextureFormat::is_srgb()`
- `configure_surface()` only picks format + present mode

Proposed change:

Split into:

- `select_output_strategy(capabilities, requested_policy) -> OutputStrategy`
- `configure_surface_for_strategy(...)`

Proposed `OutputStrategy` variants:
- `SdrSurface { format }`
- `WideGamutSurface { format, primaries }`
- `HdrNativeSurface { format, primaries, transfer }`
- `HdrIntermediateThenToneMap { intermediate_format, surface_format }`

This keeps native and fallback paths explicit.

---

## Renderer changes

### Stage A: Make color conversion actually correct

**Why first:** The repo already has a `DisplayP3` tag, but current rendering is not gamut-correct.

**Files:**
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Modify: `crates/sui-core/src/color.rs`
- Test: `crates/sui-render-wgpu/src/lib.rs`

Required work:
- implement proper transfer decoding per encoded color space
- implement 3x3 matrix conversion between:
  - sRGB / linear-sRGB
  - Display-P3 / linear-Display-P3
  - future Rec.2020 if added
- make `shader_color()` output a known working linear space, not merely “linearized channels” with unchanged primaries

Without this step, wide-gamut support will be cosmetically incorrect.

### Stage B: Add an HDR-capable intermediate render target

**Objective:** Allow the scene to carry values above SDR white even before native HDR presentation exists.

**Files:**
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Modify: `crates/sui-render-wgpu/src/gpu.rs`
- Create: `crates/sui-render-wgpu/src/output.rs`
- Create: shader(s) under `crates/sui-render-wgpu/src/shaders/`

Recommended first format:
- `Rgba16Float`

Pipeline shape:
1. render scene into FP16 intermediate target
2. final output pass does:
   - gamut conversion
   - tone mapping if needed
   - output transfer encoding if needed
3. write into swapchain texture

Benefits:
- gives SUI a real HDR scene path immediately
- enables future bloom/exposure/filmic operators
- works even when final presentation remains SDR

### Stage C: Add output transform / tone-mapping pass

**Objective:** Make the final presentation path programmable and explicit.

Output pass inputs:
- working-space FP16 scene texture
- output strategy / display capabilities uniform
- optional UI reference-white parameters

Output pass responsibilities:
- clamp or preserve extended values depending on strategy
- convert working primaries to output primaries
- tone map HDR -> SDR for fallback surfaces
- optionally scale SDR white relative to HDR headroom

Need at least these tone-mapping modes:
- `Identity` (for native extended / scRGB-like presentation)
- `ClampToSdr`
- `FilmicSdr` (for fallback)
- future: `ReferencePqEncode`

### Stage D: Native HDR presentation per platform

#### Windows proposal

Preferred first native path:
- FP16 / scRGB-like output if `wgpu` backend and platform permit it
- use monitor capability data to decide whether native HDR presentation is worthwhile
- otherwise keep HDR intermediate + SDR tone mapping

Key implementation note:
- SUI should avoid relying on static HDR metadata as the primary correctness mechanism.
- Tone map into reported display capabilities rather than expecting metadata to be honored perfectly.

#### macOS proposal

Native HDR support likely requires:
- EDR opt-in on the underlying layer
- explicit extended-range colorspace
- FP16 presentation path

Big risk:
- `wgpu` / `winit` may not expose enough `CAMetalLayer` control directly.

So the proposal should assume a two-step plan:
1. build the renderer/output abstractions now
2. add macOS-native EDR layer integration only after confirming what current `wgpu`/`winit` exposes

#### Web proposal

WebGPU should support an earlier delivery milestone:
- use float16 canvas config where available
- set `toneMapping: { mode: "extended" }`
- set canvas color space to `display-p3` where supported

This is likely the first place where SUI can offer user-visible HDR/wide-gamut output with fewer native-platform escape hatches.

#### Linux proposal

Treat Linux HDR as experimental / unavailable initially.

Rationale:
- HDR support varies by compositor, GPU stack, protocol, and driver maturity.
- SUI should avoid promising portability here until the platform story is concrete.

Recommended initial behavior:
- wide-gamut internal color correctness still works
- HDR intermediate rendering still works
- final presentation falls back to SDR tone mapping unless a backend-specific path is validated

---

## UI and diagnostics proposal

Add a new “Display output” diagnostics/control surface in `sui-dev`.

**Files:**
- Modify: `crates/sui-dev/src/app.rs`
- Modify: `crates/sui-widget-book/src/lib.rs`

Show:
- current monitor identifier
- reported display capabilities
- current output strategy
- surface format
- whether native HDR presentation is active or falling back
- current reference white / headroom if known
- toggles for:
  - automatic vs forced SDR
  - prefer wide gamut
  - prefer HDR
  - tone-mapping mode

Also add comparison surfaces for:
- sRGB-only color ramps
- out-of-sRGB but in-P3 swatches
- highlight rolloff / SDR white scaling
- HDR clipping tests

---

## Exact repo gaps this proposal addresses

1. `DisplayP3` currently lacks primary conversion.
2. Surface configuration assumes “prefer sRGB” instead of “select output strategy.”
3. No per-window output policy exists.
4. No display capability detection exists.
5. No HDR intermediate render target exists.
6. No final tone-mapping / gamut-mapping output pass exists.
7. No diagnostics exist for monitor color capability or active output path.

---

## Recommended implementation order

### Phase 1 — Correct color math before HDR

**Goal:** make wide-gamut color correct in the existing SDR pipeline.

Tasks:
1. extend color-space enums
2. implement proper linearization + gamut transforms
3. add tests proving P3 colors are not treated as sRGB primaries
4. add widget-book color validation surface

### Phase 2 — Introduce output policy and capability model

**Goal:** prepare runtime and platform layers for multiple display classes.

Tasks:
1. add output policy enums to runtime diagnostics
2. add platform capability detection abstraction
3. thread capability snapshots into renderer surface creation
4. surface diagnostics in `sui-dev`

### Phase 3 — Add HDR intermediate renderer path

**Goal:** enable physically meaningful bright values independent of final swapchain capability.

Tasks:
1. introduce `Rgba16Float` intermediate output
2. add final output transform pass
3. support SDR tone-mapped presentation everywhere
4. verify no regressions on current SDR path

### Phase 4 — Web HDR / wide-gamut path

**Goal:** ship the first real HDR-capable output target where APIs are relatively explicit.

Tasks:
1. extend wasm/web configuration for float16 canvas output
2. add Display-P3 and extended tone-mapping options
3. create browser validation page and screenshots

### Phase 5 — Windows native HDR output

**Goal:** use the platform’s Advanced Color model where support is reliable.

Tasks:
1. implement monitor capability detection
2. select native HDR/scRGB output strategy when available
3. fall back cleanly to SDR tone mapping
4. validate on SDR and HDR monitors

### Phase 6 — macOS EDR output

**Goal:** add native HDR output through EDR once platform integration details are confirmed.

Tasks:
1. determine whether `wgpu`/`winit` expose needed `CAMetalLayer` configuration
2. add EDR opt-in and colorspace wiring if feasible
3. otherwise document the missing backend hook and keep SDR/wide-gamut fallback path

---

## Testing strategy

### Unit tests

**Files:**
- `crates/sui-core/src/color.rs`
- `crates/sui-render-wgpu/src/lib.rs`

Add tests for:
- sRGB transfer decode
- Display-P3 transfer decode
- sRGB <-> linear-sRGB round-trip
- Display-P3 -> linear working space conversion
- gamut mapping matrices
- tone-mapping curves
- output strategy selection

### Renderer tests

Add tests to prove:
- FP16 intermediate path preserves values > 1.0
- SDR fallback pass clamps / tone maps predictably
- out-of-gamut P3 colors are transformed rather than silently reinterpreted

### Visual validation surfaces

Add widget-book/dev surfaces for:
- P3-only color swatches next to sRGB-clipped equivalents
- HDR highlight ladder (1.0, 2.0, 4.0, 8.0 values)
- SDR white scaling examples
- tone-mapping curve comparisons

### Platform validation

#### Windows
- SDR monitor
- HDR monitor with Advanced Color off
- HDR monitor with Advanced Color on
- multi-monitor move between SDR and HDR displays

#### macOS
- non-EDR display
- EDR-capable display
- monitor brightness changes affecting available headroom

#### Web
- browser with no HDR canvas support
- browser with Display-P3 only
- browser with `rgba16float` + extended tone mapping

---

## Risks and pitfalls

1. **`DisplayP3` correctness is currently incomplete.**
   - This is a correctness bug even before HDR support lands.

2. **Portable `wgpu` HDR presentation may remain limited.**
   - Build abstractions that survive platform-specific escape hatches.

3. **UI content and scene content may need different treatment.**
   - SDR UI over HDR content needs clear reference-white rules.

4. **Text rendering needs extra care.**
   - HDR/wide-gamut output should not accidentally break LCD/subpixel assumptions.
   - Text atlas sampling and final output conversion must preserve text sharpness.

5. **Authoring vs display-referred behavior differs by platform.**
   - Windows scRGB and Apple EDR are not the same conceptual model as PQ mastering.
   - SUI should model “extended output” separately from “HDR10 encoding.”

6. **Linux may lag.**
   - Do not block the renderer architecture on Linux-native HDR support.

---

## Concrete file-level implementation map

### Core color model
- Modify: `crates/sui-core/src/color.rs`

### Runtime output policy / diagnostics
- Modify: `crates/sui-runtime/src/diagnostics.rs`
- Modify: `crates/sui/src/lib.rs`
- Modify: `crates/sui-platform/src/lib.rs`

### Platform capability detection
- Create: `crates/sui-platform/src/display_capabilities.rs`
- Modify: `crates/sui-platform/src/desktop.rs`

### Renderer output strategy and surface config
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Modify: `crates/sui-render-wgpu/src/lib.rs`
- Modify: `crates/sui-render-wgpu/src/gpu.rs`
- Create: `crates/sui-render-wgpu/src/output.rs`
- Create: `crates/sui-render-wgpu/src/shaders/output_color.wgsl`

### Dev diagnostics / validation surfaces
- Modify: `crates/sui-dev/src/app.rs`
- Modify: `crates/sui-widget-book/src/lib.rs`

### Documentation
- Modify: `docs/text-system.md` only if text output interactions need HDR notes
- Create or modify: `docs/plans/...` follow-up plan for implementation after proposal approval
- Optional: `docs/color-management.md`

---

## Recommendation

Implement this as a **two-track effort**:

### Track 1 — foundation (do first)
- fix color correctness
- add output policy abstractions
- add FP16 intermediate output path
- add SDR tone-mapped fallback

### Track 2 — native output enablement (do per platform)
- web first
- Windows second
- macOS third
- Linux experimental last

This order minimizes risk because it makes SUI’s rendering architecture HDR-ready even before every native presentation backend is solved.

---

## References used for this proposal

Repo findings:
- `crates/sui-core/src/color.rs`
- `crates/sui-render-wgpu/src/scene.rs`
- `crates/sui-render-wgpu/src/lib.rs`
- `crates/sui-render-wgpu/src/gpu.rs`
- `crates/sui-platform/src/desktop.rs`
- `Cargo.toml` workspace versions (`wgpu 29.0.1`, `winit 0.30.13`)

External research:
- Learn WGPU HDR tutorial: offscreen `Rgba16Float` HDR + tone mapping
- `wgpu::SurfaceConfiguration` docs: portable surface configuration limits
- Chrome 129 WebGPU HDR canvas support: `rgba16float` + `toneMapping: { mode: "extended" }`
- WebGPU predefined color spaces: `Srgb`, `DisplayP3`
- Microsoft Advanced Color / HDR guidance: DWM scRGB FP16 composition and capability-driven adaptation
- Apple Metal/CAMetalLayer HDR/EDR docs: EDR opt-in, explicit colorspace, FP16 path

---

## Proposed next step

If this proposal is accepted, the next implementation plan should focus on **Phase 1 + Phase 2 only**:

1. color correctness for Display-P3 / wide gamut
2. output policy abstractions
3. display capability detection scaffolding

That keeps the first landing manageable and de-risks later HDR-native presentation work.
