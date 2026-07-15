# Debugging HDR and wide-gamut output

SUI can capture the linear HDR intermediate, the final composed output, SDR
previews, luminance/headroom maps, clip masks, and output diagnostics without
requiring the review machine to have an HDR display. This guide documents the
current pipeline; the remaining native-platform work is tracked separately in
the [HDR output roadmap](./plans/hdr-wide-gamut-display-proposal.md).

## Generate the standard bundle

From the workspace root:

```bash
cargo run -p sui-demo --bin sui-demo-artifacts
```

The command writes under `target/ui-artifacts/sui-demo/widget-book`. Its HDR
validation directory includes, when supported by the active configuration:

- ordinary screenshot, semantics, and widget overlays;
- linear floating-point `hdr-intermediate.exr` and `final-composed.exr` files;
- HDR AVIF versions of those captures;
- luminance, headroom, and clipping PNG visualizations;
- a text snapshot of the requested policy, detected display capabilities, and
  active renderer output strategy;
- numeric maximum-channel and luminance measurements.

AVIF encoding is intentionally high quality and is usually the slowest part of
the command. Use EXR and PNG while iterating if you write a focused capture
test.

## Choose a capture

`DebugCaptureRequest` has three independent choices:

| Field | Values | Use |
| --- | --- | --- |
| `stage` | `HdrIntermediate`, `FinalComposed` | Inspect scene-linear content before output conversion, or the renderer's final composed target |
| `encoding` | `Exr`, `Png` | Preserve linear floating-point data, or request an SDR-viewable image |
| `sdr_visualization` | `ToneMappedColor`, `LuminanceHeatmap`, `HeadroomHeatmap`, `ClipMask` | Select how HDR pixels are mapped when the requested encoding is PNG |

The default is final-composed, PNG, tone-mapped color. An EXR request returns
`DebugCaptureArtifact::HdrLinearRgbaF32`; a PNG request returns
`DebugCaptureArtifact::SdrRgba8`.

`HdrIntermediate` answers whether scene content contains the expected
extended-range signal. `FinalComposed` answers what remains after the selected
output transform. Comparing both isolates errors in content, composition, tone
mapping, gamut conversion, or presentation policy.

## Capture from a test

Add `sui-testing` and `sui-render-wgpu` as development dependencies, build the
application through `TestApp`, and capture only after the runtime reaches idle:

```rust,no_run
use sui::prelude::*;
use sui::Error;
use sui_render_wgpu::{
    DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest,
    DebugCaptureStage, DebugSdrVisualization,
};
use sui_testing::prelude::*;

fn capture_hdr() -> Result<()> {
    let app = TestApp::new_no_vsync(|| {
        Application::new().window(
            WindowBuilder::new()
                .title("HDR capture")
                .root(Label::new("Validation surface")),
        )
    })?;
    let window = app.main_window()?;

    let artifact = window.capture_debug_frame(DebugCaptureRequest {
        stage: DebugCaptureStage::HdrIntermediate,
        encoding: DebugCaptureEncoding::Exr,
        sdr_visualization: DebugSdrVisualization::ToneMappedColor,
    })?;

    let DebugCaptureArtifact::HdrLinearRgbaF32(image) = artifact else {
        return Err(Error::new("expected a linear HDR capture"));
    };

    write_hdr_exr(&image, "target/hdr-debug/intermediate.exr")?;
    hdr_luminance_heatmap(&image)?
        .write_png("target/hdr-debug/luminance.png")?;
    hdr_headroom_heatmap(&image, 1.0)?
        .write_png("target/hdr-debug/headroom.png")?;
    hdr_clip_mask(&image, 1.0)?
        .write_png("target/hdr-debug/clip-mask.png")?;
    Ok(())
}
```

The `sui-testing` helpers create parent directories automatically. The `1.0`
reference in this example means scene-linear SDR white; use the same reference
white convention as the render options under test.

## Read the visualizations

- **Tone-mapped color** is the easiest preview for reviewers on SDR monitors.
  It is useful for composition and gross color errors, but it cannot prove
  that extended values survived.
- **Luminance heatmap** makes relative light output visible and helps find a
  bright effect that accidentally dominates the frame.
- **Headroom heatmap** emphasizes values relative to reference white. It is the
  fastest way to confirm that intended accents use headroom while structural
  UI stays near SDR white.
- **Clip mask** marks channels above the selected threshold. It is diagnostic,
  not automatically a failure: extended values above `1.0` are expected in an
  HDR intermediate.
- **EXR** is the source-of-truth inspection artifact because it retains linear
  floating-point channels. Use it for numeric comparisons and downstream HDR
  analysis.

## Inspect output policy

A visually plausible image does not prove that the requested presentation path
was selected. Record `WindowOutputDiagnostics` with every platform-specific
bug report. The useful fields include:

- requested color management, primaries, dynamic range, tone mapping, and SDR
  content brightness;
- whether the detected display reports wide gamut or HDR;
- whether SUI can use native HDR presentation on that platform;
- the preferred dynamic range and capability notes;
- the renderer's active output strategy.

Treat capability detection and renderer selection as separate questions. A
display may report HDR while the active platform integration still uses a
tone-mapped SDR fallback.

## Validation workflow

For an output change:

1. Run focused color/math and renderer tests.
2. Generate the standard artifact bundle.
3. Compare intermediate and final EXR measurements.
4. Review SDR previews, headroom, and clipping maps.
5. Check output diagnostics on every platform the change claims to support.
6. Exercise a real HDR display for native-presentation claims; capture files
   alone cannot validate the OS compositor, display mode, or panel response.
7. Re-run the ordinary SDR widget book and screenshots to catch regressions.

Useful commands:

```bash
cargo test -p sui-core color
cargo test -p sui-render-wgpu
cargo test -p sui-testing
cargo run -p sui-demo --bin sui-demo-artifacts
```

For architectural context, see [Rendering architecture](./renderer-architecture.md).
For token-level authoring, see [HDR theme tokens](./hdr-theme-token-schema-proposal.md).
