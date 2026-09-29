# Debugging HDR and wide-gamut output

SUI can capture the linear HDR intermediate, the final composed output, SDR
previews, luminance/headroom maps, clip masks, and output diagnostics without
requiring the review machine to have an HDR display. This guide documents the
current pipeline; the remaining native-platform work is tracked separately in
the [HDR output roadmap](./plans/hdr-wide-gamut-display-proposal.md).

## Check the output in the demo

Open **HDR validation** from the demo picker (`cargo run -p sinomo-ui-demo`;
in the web build, the `demo=hdr-validation` query parameter opens it). The page
starts with a verdict naming the window's
output (SDR, wide-gamut SDR, or native HDR with its headroom above SDR white)
and the output controls, which edit the same options as Settings. Each probe
below says what it should look like on the current output:

- **Brightness headroom**: a ramp and a white ladder up to 16× SDR white, with
  the display's reported peak marked. HDR output keeps brightening up to the
  peak; SDR output shows everything from 1× up as the same white.
- **Highlight fitting**: each output channel of an orange as it brightens, and
  a grid of saturated colors whose cells are split into a half the GPU fits and
  a half fitted on the CPU with `fit_to_sdr`. The halves should match on every
  output; a seam means the GPU fits differently.
- **Wide gamut**: Display P3 tiles whose left half is clipped to sRGB. The
  halves differ only where the output reaches past sRGB.
- **Gradients and banding**, and **HDR in UI**, which shows the same controls
  under each HDR theme mode.
- **Capture and report** captures both stages of the next frame, writes the
  bundle described below under `target/ui-artifacts/sui-demo/hdr-validation`,
  and copies a report for bug reports.

## How SDR outputs fit highlights

SDR outputs cannot show light above SDR white, so the renderer fits it. Colors
within SDR range always pass through unchanged.

| Tone mapping | Above SDR white |
| --- | --- |
| `Clamp` (and `Automatic` on SDR outputs) | Scaled until the brightest channel reaches SDR white, keeping the hue |
| `Reinhard` | Scaled the same way, then turned toward white by `1 - 1/peak`, so brighter highlights read brighter |

Native HDR output sends extended range as is. Fitting happens after converting
to the output's primaries, so Display P3 colors on a P3 output are fitted as P3
colors. Wide-gamut colors arrive with negative channels in the working space
(extended linear sRGB); scRGB outputs keep them, and other outputs clip them
after the conversion.

## HDR theme modes follow the output

A theme's HDR mode says how far widgets may go; the window's output decides
how far they can. Widgets style HDR content for the lesser of the two, which
the platform records before each frame as the window's `OutputColorRange`:

| Output | Constrained and Full HDR | Wide-gamut only |
| --- | --- | --- |
| Native HDR | As the theme says | As the theme says |
| Wide-gamut SDR | Fall back to wide-gamut only | As the theme says |
| sRGB SDR | Fall back to the SDR baseline | Fall back to the SDR baseline |

So an HDR accent never reaches an SDR display as a clipped near-white fill.
Custom widgets do the same with `ctx.output_color_range()` while painting and
`HdrThemeTokens::limited_to`; reading the range observes it, so the widget is
painted again when the output changes.

## Test on any display

Tests can render as if on a given display, whatever the machine has; see
[Simulate a display](./testing.md#simulate-a-display). A simulated HDR display
presents scRGB, reports its headroom in the diagnostics, and captures the
final output as linear floating point, so HDR paths are testable anywhere.

## Generate the standard bundle

From the workspace root:

```bash
cargo run -p sinomo-ui-demo --bin sui-demo-artifacts
```

The command writes under `target/ui-artifacts/sui-demo/widget-book`. Its
`hdr-validation` directory includes, when supported by the active
configuration:

- ordinary screenshot, semantics, and widget overlays;
- linear floating-point `hdr-intermediate.exr` and `final-composed.exr` files;
- HDR AVIF versions of those captures;
- luminance, headroom, and clipping PNG visualizations;
- a text snapshot of the requested policy, detected display capabilities, and
  active renderer output strategy;
- maximum-channel and luminance measurements and the share of pixels above SDR
  white. Final-stage values are relative to SDR white in that image
  (`final_sdr_white`): native HDR finals are scRGB, where 1.0 is 80 nits.

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
`DebugCaptureArtifact::SdrRgba8`. Tone-mapped PNGs, like screenshots, fit
highlights with the window's tone mapping.

`HdrIntermediate` answers whether scene content contains the expected
extended-range signal. `FinalComposed` answers what remains after the selected
output transform. Comparing both isolates errors in content, composition, tone
mapping, gamut conversion, or presentation policy.

## Capture from a test

Add `sinomo-ui-testing` and `sinomo-ui-render-wgpu` as development dependencies, build the
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

The `sinomo-ui-testing` helpers create parent directories automatically. The `1.0`
reference in this example means scene-linear SDR white; use the same reference
white convention as the render options under test.

## Capture from the running app

An app can capture its own window. Ask for a capture with a wake token from
the widget's event context; after the next redraw the platform makes it and
wakes the widget, which collects it:

```rust,no_run
use sui::prelude::*;
use sui::{
    DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest, DebugCaptureStage,
    DebugCaptureTicket, PointerEventKind, WakeEvent, request_window_debug_capture,
    take_window_debug_capture,
};

struct CaptureOnClick {
    ticket: Option<DebugCaptureTicket>,
}

impl Widget for CaptureOnClick {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Up => {
                let wake = ctx.register_async_wakeup();
                let request = DebugCaptureRequest {
                    stage: DebugCaptureStage::HdrIntermediate,
                    encoding: DebugCaptureEncoding::Exr,
                    ..Default::default()
                };
                self.ticket =
                    Some(request_window_debug_capture(ctx.window_id(), request, Some(wake)));
                ctx.request_paint();
            }
            Event::Wake(WakeEvent::Async { .. }) => {
                let result = self.ticket.take().and_then(take_window_debug_capture);
                if let Some(Ok(DebugCaptureArtifact::HdrLinearRgbaF32(image))) = result {
                    println!("captured {} x {}", image.width(), image.height());
                }
            }
            _ => {}
        }
    }
}
```

Wake events go only to the widget that registered the token. Captures are
native-only; in a browser the result is an error. A host that runs its own
event loop keeps redrawing while `has_pending_window_debug_captures` and calls
`service_window_debug_captures` after each redraw, delivering the returned
tokens with `Runtime::wake_async`.

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

Diagnostics describe the last presented frame, so they trail a change of
options by a frame. A widget that shows them should read them with
`ctx.observe(&window_output_diagnostics_signal(ctx.window_id()))` in `measure`
(or `paint`), which invalidates it whenever a presented frame's diagnostics
change.

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
cargo test -p sinomo-ui-core color
cargo test -p sinomo-ui-render-wgpu
cargo test -p sinomo-ui-testing
cargo run -p sinomo-ui-demo --bin sui-demo-artifacts
```

For architectural context, see [Rendering architecture](./renderer-architecture.md).
For token-level authoring, see [HDR theme tokens](./hdr-theme-token-schema-proposal.md).
