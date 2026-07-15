# HDR and Wide-Gamut Display Roadmap

This is the active roadmap for SUI's display-output pipeline. The original
proposal has been replaced with the current implementation contract and the
remaining work. The filename is retained so existing links remain valid.

For theme authoring, see [HDR theme tokens](../hdr-theme-token-schema-proposal.md).
For the visual design direction, see the
[HDR-native interface manifesto](../hdr-native-interface-manifesto.md).

## Status at a glance

| Area | Status | Current contract |
| --- | --- | --- |
| Color values | Shipped | `ColorSpace` distinguishes encoded and linear sRGB and Display-P3 values. |
| Primary conversion | Shipped | `Color::to_linear_srgb()` decodes transfer functions and converts Display-P3 primaries. |
| Output intent | Shipped | `WindowRenderOptions` expresses automatic, SDR, wide-gamut, and HDR preferences. |
| Capability reporting | Shipped, platform-dependent | Each window publishes detected capabilities and the active renderer strategy. |
| Renderer output selection | Shipped | The renderer chooses SDR, wide-gamut, or native-HDR presentation conservatively. |
| Windows native HDR | Shipped with runtime gates | DXGI Advanced Color detection and an FP16/scRGB presentation path are available when the monitor, OS, and surface all qualify. |
| macOS | Partial | Display-P3 SDR is assumed conservatively; EDR headroom detection and EDR layer configuration are not implemented. |
| Web | Partial | Gamut/HDR media signals and explicit launch hints are detected; final canvas color management remains browser-controlled and is not treated as native HDR. |
| Linux and other desktop targets | SDR fallback | No native wide-gamut or HDR capability probe is wired yet. |
| HDR capture and inspection | Shipped | Tests and artifact tooling can capture linear HDR data or SDR diagnostic visualizations. |

"Shipped" here describes the code path, not a promise that every display stack
will expose the same result. `wgpu`, the window system, the OS compositor, the
monitor mode, and the physical display must all support a native output path.

## Current architecture

The pipeline deliberately separates six concerns:

1. **Authoring.** `sui_core::Color` records both channel values and their
   `ColorSpace` (`Srgb`, `LinearSrgb`, `DisplayP3`, or `LinearDisplayP3`).
2. **Theme policy.** `HdrThemeTokens` selects SDR, wide-gamut, constrained-HDR,
   or full-HDR widget styling. Theme policy does not claim display support.
3. **Window intent.** `WindowRenderOptions` records the application's preferred
   primaries, dynamic range, tone mapping, and SDR reference-white behavior.
4. **Platform detection.** `sinomo-ui-platform` builds a `DisplayCapabilities`
   snapshot for the window's current monitor.
5. **Renderer resolution.** `sinomo-ui-render-wgpu` combines intent, capabilities,
   and available surface formats into an `OutputStrategy`.
6. **Diagnostics.** `WindowOutputDiagnostics` reports both the requested policy
   and the strategy that was actually selected.

This separation is important: an HDR-themed accent can still render safely on
an SDR surface, and requesting HDR never makes an unsupported platform pretend
that native HDR presentation succeeded.

## Authoring colors

Use a constructor that matches the encoding of the values you have:

```rust
use sui::Color;

let sdr = Color::srgba(0.10, 0.72, 0.86, 1.0);
let p3 = Color::display_p3(0.05, 0.78, 0.92, 1.0);
let hdr = Color::linear_display_p3(0.08, 0.92, 1.12, 1.0);
```

`display_p3` stores encoded Display-P3 values. `linear_display_p3` stores
linear values and may carry channels above `1.0` for headroom. Do not label
sRGB values as Display-P3 merely to obtain more saturation; the color-space tag
is a conversion contract.

SUI currently uses linear sRGB as its common renderer working space. Display-P3
colors are converted into that space, preserving out-of-sRGB values where the
floating-point pipeline allows them.

## Requesting an output policy

The safe default is `Automatic`. Use an explicit preference when an application
has a reason to select a display mode or when exercising the pipeline in tests.

```rust,no_run
use sui::prelude::*;
use sui::{
    WindowColorManagementMode, WindowDynamicRangeMode,
    WindowOutputColorPrimaries,
};

fn main() -> Result<()> {
    let options = WindowRenderOptions::new(true, 1.0)
        .with_color_management_mode(WindowColorManagementMode::PreferHdr)
        .with_output_color_primaries(WindowOutputColorPrimaries::Automatic)
        .with_dynamic_range_mode(WindowDynamicRangeMode::Automatic);

    App::new()
        .render_options(options)
        .window(Window::new("Display-aware SUI").root(Label::new("Ready")))
        .run()
}
```

`PreferHdr` is a preference, not a force switch. If native HDR cannot be
presented end to end, normal presentation stays SDR. Use
`PreferWideGamut` for wide-gamut SDR without requesting HDR, and `ForceSdr` for
deterministic SDR output such as baseline screenshots.

The lower-level controls are:

| Type | Variants | Purpose |
| --- | --- | --- |
| `WindowColorManagementMode` | `Automatic`, `ForceSdr`, `PreferWideGamut`, `PreferHdr` | High-level application policy. |
| `WindowOutputColorPrimaries` | `Automatic`, `Srgb`, `DisplayP3` | Requested output gamut. |
| `WindowDynamicRangeMode` | `Automatic`, `StandardDynamicRange`, `HighDynamicRange` | Requested output range. |
| `WindowToneMappingMode` | `Automatic`, `Clamp`, `Reinhard` | SDR conversion policy where a conversion path uses tone mapping. |

`sdr_content_brightness_nits` defaults to 203 nits. When
`use_system_sdr_content_brightness` is enabled, a valid platform-reported value
takes precedence. This setting anchors SDR content in an HDR composition; it is
not a monitor-brightness control.

## How output selection behaves

The renderer resolves the active strategy conservatively:

- `ForceSdr` always selects an SDR surface.
- An HDR request selects `HdrNativeSurface` only when platform detection marks
  native presentation as supported **and** a suitable float surface format is
  available.
- An HDR request without that end-to-end support falls back to `SdrSurface`.
  Normal presentation does not silently use a diagnostic tone-mapped HDR path.
- A wide-gamut request selects `WideGamutSurface` only when the detected display
  supports it; otherwise it falls back to SDR.
- `HdrIntermediateThenToneMap` is reserved for offscreen diagnostics and debug
  captures.

Automatic mode re-evaluates this choice when the platform refreshes the
window's capabilities. The desktop path currently refreshes at startup and on
scale-factor changes; complete same-scale monitor migration is part of the
remaining platform work below.

## Inspecting the active output

`sui::window_output_diagnostics(window_id)` returns the latest
`WindowOutputDiagnostics` for a live window. It includes:

- the detected `DisplayCapabilities`
- requested color-management, primaries, dynamic range, and tone-mapping modes
- configured and system-derived SDR content brightness
- the active `OutputStrategy`
- platform notes suitable for a diagnostics panel or bug report

Always report the active strategy alongside the request. A request of
`PreferHdr` with an active `SdrSurface` is an intentional fallback, not evidence
that HDR is active.

## Platform behavior

### Windows

The Windows path probes the current monitor through DXGI Advanced Color. It
records color space, bit depth, chromaticity data, luminance data, and the
system SDR-white level when available. Native HDR is gated on:

- an active scRGB or HDR10/BT.2020 Advanced Color monitor mode
- a valid SDR-white value from the system
- a float16 surface format exposed by the adapter/surface pair

When all gates pass, SUI presents through a linear scRGB path and configures the
native DXGI color space. scRGB uses sRGB/BT.709 primaries; values above reference
white carry HDR headroom. If any gate fails, SUI remains on the SDR path.

### macOS

The current probe assumes Display-P3 SDR conservatively. It does not query EDR
headroom and does not opt the native layer into extended dynamic range. Native
HDR/EDR presentation is therefore not supported yet, even on an HDR-capable
Mac display.

### Web

The web path observes `(color-gamut: p3)`, `(color-gamut: rec2020)`, and
`(dynamic-range: high)` media queries. The demo can also supply explicit query
hints for canvas format, color space, tone mapping, policy, and SDR-white nits.
These signals are diagnostic input; browser color management still owns the
canvas-to-display path, and SUI currently reports
`native_hdr_presentation_supported = false` on the web.

### Linux and other desktop platforms

Targets without a native probe use an SDR/sRGB capability profile. This avoids
advertising HDR merely because the GPU supports float textures; compositor and
monitor presentation support must also be known.

## Debug captures

The debug pipeline can inspect either `HdrIntermediate` or `FinalComposed`:

- `DebugCaptureEncoding::Exr` preserves linear floating-point RGBA data.
- `DebugCaptureEncoding::Png` produces an SDR image using tone-mapped color,
  luminance heatmap, headroom heatmap, or clip-mask visualization.
- `sui_testing::TestWindow::capture_debug_frame` exposes captures to tests.

Generate the full widget-book artifact set with:

```bash
cargo run -p sinomo-ui-demo --bin sui-demo-artifacts
```

Artifacts are written below `target/ui-artifacts/sui-demo/widget-book/` and
include HDR data, SDR previews, heatmaps, clip masks, diagnostics, and metrics.
See the [HDR debugging guide](../hdr-debugging.md) for capture recipes and the
[testing guide](../testing.md) for the general artifact workflow.

## Remaining roadmap

### P0: complete native platform presentation

- **macOS:** detect current/potential EDR headroom, select a float presentation
  format, configure the native layer for extended-range content, attach the
  correct color space, and refresh the policy as a window moves between screens.
- **Linux:** discover compositor/output color capabilities and only enable a
  wide-gamut or HDR surface when the window-system protocol can communicate the
  required color space and transfer behavior.
- **Web:** configure supported WebGPU canvases with explicit color-space and
  extended tone-mapping options, then verify the effective configuration rather
  than inferring presentation from media queries alone.

### P0: hardware validation and failure reporting

- Maintain a hardware/OS/browser matrix with SDR, wide-gamut SDR, and HDR
  displays.
- Validate window migration between unlike monitors and OS HDR toggles.
- Verify reference-white handling against system controls on Windows.
- Make failed native color-space configuration visible in diagnostics without
  leaving the window in an ambiguous state.

### P1: broaden color-management inputs

- Add explicit Rec.2020 authoring and output primaries only after conversion and
  validation coverage exists.
- Define image and external-texture color metadata instead of assuming every
  sampled resource is sRGB.
- Decide whether PQ and HLG are public presentation contracts or remain
  platform/media interop details.
- Add calibrated comparison fixtures for gamut conversion, tone mapping,
  clipping, and alpha compositing.

### P1: integrate policy with themes

`HdrThemeMode` and the renderer output policy are intentionally separate today.
A future integration may derive a safe theme mode from output diagnostics, but
it must preserve application overrides and never enable luminous styling from
GPU capability alone. Track the authoring side in
[HDR theme tokens](../hdr-theme-token-schema-proposal.md).

## Completion criteria

This roadmap is complete when:

- Windows, macOS, web, and the supported Linux display stack have an honest,
  tested capability-to-presentation path
- moving a window or changing display mode updates strategy without corrupting
  color or losing the surface
- native HDR is only reported when pixels are presented through an HDR-capable
  surface and compositor path
- SDR screenshots and ordinary SDR applications remain visually stable
- color, output-policy, and debug-capture APIs have rustdoc examples and
  cross-platform tests
- the hardware validation matrix is reproducible from documented commands and
  retained artifacts

## Source map

- Color representation and conversion:
  `crates/sui-core/src/color.rs`
- Public window policy: `crates/sui-runtime/src/diagnostics.rs`
- Platform detection and diagnostics:
  `crates/sui-platform/src/display_capabilities.rs` and
  `crates/sui-platform/src/display_capabilities/windows_display.rs`
- Windows Advanced Color integration:
  `crates/sui-platform/src/display_capabilities/windows_display.rs` and
  `crates/sui-render-wgpu/src/scene/windows_surface.rs`
- Output strategy and capture pipeline: `crates/sui-render-wgpu/src/lib.rs` and
  `crates/sui-render-wgpu/src/scene.rs`
- Theme policy: `crates/sui-widgets/src/hdr_theme.rs`
