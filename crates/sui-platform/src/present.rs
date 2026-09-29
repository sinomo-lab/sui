//! Presenting a window's frame. Every host (the desktop and headless
//! platforms and the live test harness) presents through
//! [`present_window_frame`], so windows render, report their output, and
//! style HDR content the same way everywhere.

use sui_core::{Result, WindowId};
use sui_render_wgpu::{
    DisplayCapabilities, DisplayColorPrimaries, FeatheringOptions, OutputStrategy, WgpuRenderer,
};
use sui_runtime::{
    OutputColorRange, RenderOutput, Runtime, WindowRenderOptions, set_window_output_color_range,
    window_render_options, window_scene_statistics_detail_mode,
};
use web_time::Instant;

use crate::{
    WindowOutputDiagnostics, map_window_color_management, map_window_stem_darkening,
    map_window_text_coverage_policy, map_window_text_hinting, map_window_text_subpixel_order,
    publish_window_output_diagnostics, resolve_sdr_content_brightness_nits,
};

/// What presenting a frame produced, for the host's own bookkeeping.
pub struct PresentedFrame {
    pub output: RenderOutput,
    pub runtime_time_ms: f64,
    pub renderer_time_ms: f64,
    /// When the renderer returned from presenting, before diagnostics were
    /// published.
    pub presented_at: Instant,
}

/// Render `window_id`'s next frame and present it with `renderer`.
///
/// This applies the window's render options to the renderer, records what
/// its output can show so widgets style HDR content to fit
/// ([`OutputColorRange`]), renders and presents the runtime's frame, and
/// publishes the window's output diagnostics. Hosts set the window's display
/// capabilities on the renderer beforehand, and afterwards service debug
/// captures and do their own bookkeeping with the result.
pub fn present_window_frame(
    runtime: &mut Runtime,
    renderer: &mut WgpuRenderer,
    window_id: WindowId,
) -> Result<PresentedFrame> {
    let setup_started = Instant::now();
    let options = apply_render_options(renderer, window_id)?;
    // Before the runtime paints, so widgets style for this frame's output.
    if let Some(strategy) = renderer.window_output_strategy(window_id) {
        set_window_output_color_range(window_id, output_color_range(strategy));
    }
    let setup_time = setup_started.elapsed();

    let runtime_started = Instant::now();
    let output = runtime.render(window_id)?;
    let runtime_time_ms = runtime_started.elapsed().as_secs_f64() * 1000.0;

    let renderer_started = Instant::now();
    renderer.render(&output.frame)?;
    let presented_at = Instant::now();
    publish_output_diagnostics(renderer, window_id, &options);
    let renderer_time_ms = (setup_time + renderer_started.elapsed()).as_secs_f64() * 1000.0;

    Ok(PresentedFrame {
        output,
        runtime_time_ms,
        renderer_time_ms,
        presented_at,
    })
}

/// The window's options as the renderer applied them.
struct AppliedOptions {
    options: WindowRenderOptions,
    sdr_content_brightness_nits: f32,
}

fn apply_render_options(
    renderer: &mut WgpuRenderer,
    window_id: WindowId,
) -> Result<AppliedOptions> {
    renderer.set_runtime_diagnostics_enabled(
        window_scene_statistics_detail_mode(window_id).is_detailed(),
    );
    let render_options = window_render_options(window_id);
    renderer.set_runtime_feathering_override(
        render_options.map(|options| {
            FeatheringOptions::new(options.feathering_enabled, options.feather_width)
        }),
    );
    renderer.set_runtime_text_hinting_override(
        render_options.map(|options| map_window_text_hinting(options.text_hinting)),
    );
    renderer.set_runtime_stem_darkening_override(
        render_options.map(|options| map_window_stem_darkening(options.stem_darkening)),
    );
    renderer.set_runtime_text_coverage_policy_override(
        render_options.map(|options| map_window_text_coverage_policy(options.text_coverage_policy)),
    );
    renderer.set_runtime_text_subpixel_order_override(
        render_options.map(|options| map_window_text_subpixel_order(options.text_subpixel_order)),
    );

    // Without options, color management uses the defaults.
    let options = render_options.unwrap_or_else(|| WindowRenderOptions::new(false, 0.0));
    let sdr_content_brightness_nits = resolve_sdr_content_brightness_nits(
        options.sdr_content_brightness_nits,
        options.use_system_sdr_content_brightness,
        &renderer
            .window_display_capabilities(window_id)
            .unwrap_or_default(),
    );
    renderer.set_window_color_management(
        window_id,
        map_window_color_management(
            options.color_management_mode,
            options.output_color_primaries,
            options.dynamic_range_mode,
            options.tone_mapping_mode,
            sdr_content_brightness_nits,
        ),
    )?;
    Ok(AppliedOptions {
        options,
        sdr_content_brightness_nits,
    })
}

/// What an output presenting with `strategy` can show.
fn output_color_range(strategy: OutputStrategy) -> OutputColorRange {
    match strategy {
        OutputStrategy::HdrNativeSurface { .. } => OutputColorRange::HighDynamicRange,
        OutputStrategy::WideGamutSurface {
            primaries: DisplayColorPrimaries::DisplayP3,
            ..
        }
        | OutputStrategy::HdrIntermediateThenToneMap {
            primaries: DisplayColorPrimaries::DisplayP3,
            ..
        } => OutputColorRange::WideGamut,
        _ => OutputColorRange::Standard,
    }
}

fn publish_output_diagnostics(
    renderer: &WgpuRenderer,
    window_id: WindowId,
    applied: &AppliedOptions,
) {
    let (Some(mut display_capabilities), Some(active_output_strategy)) = (
        renderer.window_display_capabilities(window_id),
        renderer.window_output_strategy(window_id),
    ) else {
        return;
    };
    if let Some(formats) = renderer.window_surface_formats(window_id) {
        append_note(
            &mut display_capabilities,
            &format!("Surface formats: {formats:?}."),
        );
    }
    let options = &applied.options;
    publish_window_output_diagnostics(
        window_id,
        WindowOutputDiagnostics {
            system_sdr_content_brightness_nits: display_capabilities.sdr_white_nits,
            display_capabilities,
            requested_color_management_mode: options.color_management_mode,
            requested_output_primaries: options.output_color_primaries,
            requested_dynamic_range_mode: options.dynamic_range_mode,
            requested_tone_mapping_mode: options.tone_mapping_mode,
            requested_sdr_content_brightness_nits: applied.sdr_content_brightness_nits,
            configured_sdr_content_brightness_nits: options.sdr_content_brightness_nits,
            use_system_sdr_content_brightness: options.use_system_sdr_content_brightness,
            active_output_strategy,
        },
    );
}

fn append_note(capabilities: &mut DisplayCapabilities, note: &str) {
    if !capabilities.notes.is_empty() {
        capabilities.notes.push(' ');
    }
    capabilities.notes.push_str(note);
}
