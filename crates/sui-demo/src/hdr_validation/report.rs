//! Reports and capture bundles for HDR bugs: the window's output
//! diagnostics as text, light measurements of a capture, and (natively) the
//! files a capture is written as.

use std::fmt::Write as _;

use sui::diagnostics::{HdrRgbaImage, OutputStrategy, WindowOutputDiagnostics};

/// The value SDR white has in a final-output capture. Native HDR output is
/// scRGB, where 1.0 is 80 nits, so SDR white sits at its brightness over 80.
pub(crate) fn final_output_sdr_white(diagnostics: Option<&WindowOutputDiagnostics>) -> f32 {
    match diagnostics {
        Some(diagnostics)
            if matches!(
                diagnostics.active_output_strategy,
                OutputStrategy::HdrNativeSurface { .. }
            ) =>
        {
            (diagnostics.requested_sdr_content_brightness_nits / 80.0).max(f32::MIN_POSITIVE)
        }
        _ => 1.0,
    }
}

/// `diagnostics` as `key=value` lines, for bug reports and capture bundles.
pub(crate) fn output_diagnostics_report(
    view: &str,
    diagnostics: Option<&WindowOutputDiagnostics>,
) -> String {
    let mut report = format!("view={view}\n");
    let Some(diagnostics) = diagnostics else {
        report.push_str("output_diagnostics=unavailable\n");
        return report;
    };
    let capabilities = &diagnostics.display_capabilities;
    let optional = |value: Option<f32>| {
        value.map_or_else(|| "unreported".to_string(), |value| format!("{value:.1}"))
    };
    let _ = write!(
        report,
        "requested_color_management_mode={:?}\n\
         requested_output_primaries={:?}\n\
         requested_dynamic_range_mode={:?}\n\
         requested_tone_mapping_mode={:?}\n\
         requested_sdr_content_brightness_nits={:.0}\n\
         configured_sdr_content_brightness_nits={:.0}\n\
         system_sdr_content_brightness_nits={}\n\
         use_system_sdr_content_brightness={}\n\
         supports_wide_gamut={}\n\
         supports_hdr={}\n\
         native_hdr_presentation_supported={}\n\
         preferred_primaries={:?}\n\
         preferred_dynamic_range={:?}\n\
         max_luminance_nits={}\n\
         sdr_white_nits={}\n\
         max_content_headroom={}\n\
         active_output_strategy={:?}\n\
         notes={}\n",
        diagnostics.requested_color_management_mode,
        diagnostics.requested_output_primaries,
        diagnostics.requested_dynamic_range_mode,
        diagnostics.requested_tone_mapping_mode,
        diagnostics.requested_sdr_content_brightness_nits,
        diagnostics.configured_sdr_content_brightness_nits,
        optional(diagnostics.system_sdr_content_brightness_nits),
        diagnostics.use_system_sdr_content_brightness,
        capabilities.supports_wide_gamut,
        capabilities.supports_hdr,
        capabilities.native_hdr_presentation_supported,
        capabilities.preferred_primaries,
        capabilities.preferred_dynamic_range,
        optional(capabilities.max_luminance_nits),
        optional(capabilities.sdr_white_nits),
        optional(capabilities.max_content_headroom),
        diagnostics.active_output_strategy,
        capabilities.notes,
    );
    report
}

/// How much light a linear capture holds, relative to SDR white at 1.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LightMetrics {
    pub(crate) max_channel: f32,
    pub(crate) max_luminance: f32,
    /// Share of pixels with a channel above SDR white, from 0 to 1.
    pub(crate) above_sdr_white: f32,
}

impl LightMetrics {
    pub(crate) fn of(image: &HdrRgbaImage) -> Self {
        Self::relative_to(image, 1.0)
    }

    /// Measured in multiples of `sdr_white`, the value SDR white has in
    /// `image`.
    pub(crate) fn relative_to(image: &HdrRgbaImage, sdr_white: f32) -> Self {
        let mut max_channel = 0.0_f32;
        let mut max_luminance = 0.0_f32;
        let mut above = 0_usize;
        let mut count = 0_usize;
        for rgba in image.pixels().chunks_exact(4) {
            let [red, green, blue] = [rgba[0], rgba[1], rgba[2]].map(|channel| channel / sdr_white);
            let peak = red.max(green).max(blue);
            max_channel = max_channel.max(peak);
            max_luminance = max_luminance.max(red * 0.2126 + green * 0.7152 + blue * 0.0722);
            // Rounding can put SDR white a hair above 1.
            above += usize::from(peak > 1.001);
            count += 1;
        }
        Self {
            max_channel,
            max_luminance,
            above_sdr_white: if count == 0 {
                0.0
            } else {
                above as f32 / count as f32
            },
        }
    }

    /// An 8-bit SDR capture, which cannot exceed SDR white.
    pub(crate) const SDR: Self = Self {
        max_channel: 1.0,
        max_luminance: 1.0,
        above_sdr_white: 0.0,
    };
}

/// Measurements of a capture of both stages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CaptureMetrics {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) intermediate: LightMetrics,
    /// Whether the final stage came back as linear floating point.
    pub(crate) final_is_hdr: bool,
    /// The value SDR white has in the final stage; its measurements are
    /// relative to it.
    pub(crate) final_sdr_white: f32,
    pub(crate) final_output: LightMetrics,
}

impl CaptureMetrics {
    pub(crate) fn report(&self) -> String {
        format!(
            "capture_size={}x{}\n\
             intermediate_max_channel={}\n\
             intermediate_max_luminance={}\n\
             intermediate_above_sdr_white_percent={:.2}\n\
             final_artifact_kind={}\n\
             final_sdr_white={}\n\
             final_max_channel={}\n\
             final_max_luminance={}\n\
             final_above_sdr_white_percent={:.2}\n",
            self.width,
            self.height,
            self.intermediate.max_channel,
            self.intermediate.max_luminance,
            self.intermediate.above_sdr_white * 100.0,
            if self.final_is_hdr { "hdr" } else { "sdr" },
            self.final_sdr_white,
            self.final_output.max_channel,
            self.final_output.max_luminance,
            self.final_output.above_sdr_white * 100.0,
        )
    }

    /// The measurements as sentences, for the page.
    pub(crate) fn summary_lines(&self) -> Vec<String> {
        let stage = |name: &str, metrics: LightMetrics| {
            format!(
                "{name}: brightest channel {:.2}× SDR white, brightest luminance {:.2}×, {:.1}% of pixels above SDR white.",
                metrics.max_channel,
                metrics.max_luminance,
                metrics.above_sdr_white * 100.0
            )
        };
        vec![
            format!("Captured {} × {} pixels.", self.width, self.height),
            stage("Scene (HDR intermediate)", self.intermediate),
            if self.final_is_hdr {
                stage("Final output", self.final_output)
            } else {
                "Final output: 8-bit SDR, so nothing is above SDR white.".to_string()
            },
        ]
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{fs, path::Path};

    use sui::diagnostics::{DebugCaptureArtifact, HdrRgbaImage};
    use sui::{Error, Result};
    use sui_testing::{
        Screenshot, hdr_clip_mask, hdr_headroom_heatmap, hdr_luminance_heatmap, write_hdr_avif,
        write_hdr_exr,
    };

    use super::{CaptureMetrics, LightMetrics};

    /// Visualizations made while writing a bundle, for showing in the app.
    pub(crate) struct CaptureMaps {
        pub(crate) headroom: Screenshot,
        pub(crate) clip_mask: Screenshot,
    }

    /// Write a capture of both stages to `dir`: EXR (and optionally AVIF)
    /// images, luminance, headroom, and clip maps, the diagnostics report,
    /// and the measurements.
    pub(crate) fn write_capture_bundle(
        dir: &Path,
        intermediate: &HdrRgbaImage,
        final_output: &DebugCaptureArtifact,
        final_sdr_white: f32,
        diagnostics_report: &str,
        with_avif: bool,
    ) -> Result<(CaptureMetrics, CaptureMaps)> {
        fs::create_dir_all(dir).map_err(|error| io_error(dir, error))?;
        write_hdr_exr(intermediate, dir.join("hdr-intermediate.exr"))?;
        if with_avif {
            write_hdr_avif(intermediate, dir.join("hdr-intermediate.avif"), 1.0)?;
        }
        hdr_luminance_heatmap(intermediate)?.write_png(dir.join("luminance-map.png"))?;
        let headroom = hdr_headroom_heatmap(intermediate, 1.0)?;
        headroom.write_png(dir.join("headroom-map.png"))?;
        let clip_mask = hdr_clip_mask(intermediate, 1.0)?;
        clip_mask.write_png(dir.join("clip-mask.png"))?;

        let (final_is_hdr, final_metrics) = match final_output {
            DebugCaptureArtifact::HdrLinearRgbaF32(image) => {
                write_hdr_exr(image, dir.join("final-composed.exr"))?;
                if with_avif {
                    write_hdr_avif(image, dir.join("final-composed.avif"), 1.0)?;
                }
                hdr_luminance_heatmap(image)?.write_png(dir.join("final-luminance-map.png"))?;
                (true, LightMetrics::relative_to(image, final_sdr_white))
            }
            DebugCaptureArtifact::SdrRgba8(image) => {
                Screenshot::new(image.width(), image.height(), image.pixels().to_vec())?
                    .write_png(dir.join("final-composed.png"))?;
                (false, LightMetrics::SDR)
            }
        };
        let metrics = CaptureMetrics {
            width: intermediate.width(),
            height: intermediate.height(),
            intermediate: LightMetrics::of(intermediate),
            final_is_hdr,
            final_sdr_white,
            final_output: final_metrics,
        };
        write_text(&dir.join("output-diagnostics.txt"), diagnostics_report)?;
        write_text(&dir.join("capture-metrics.txt"), &metrics.report())?;
        Ok((
            metrics,
            CaptureMaps {
                headroom,
                clip_mask,
            },
        ))
    }

    fn write_text(path: &Path, contents: &str) -> Result<()> {
        fs::write(path, contents).map_err(|error| io_error(path, error))
    }

    fn io_error(path: &Path, error: std::io::Error) -> Error {
        Error::new(format!("failed to write {}: {error}", path.display()))
    }
}
