use sui_core::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatheringOptions {
    pub enabled: bool,
    pub width: f32,
}

impl Default for FeatheringOptions {
    fn default() -> Self {
        Self::new(false, crate::resources::DEFAULT_FEATHER_WIDTH)
    }
}

impl FeatheringOptions {
    pub const fn new(enabled: bool, width: f32) -> Self {
        Self { enabled, width }
    }

    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            width: self.width.max(0.0),
        }
    }

    pub fn effective_width(self) -> f32 {
        if self.enabled {
            self.width.max(0.0)
        } else {
            0.0
        }
    }
}

pub use sui_scene::TextRenderMode;

/// The renderer's name for [`sui_scene::TextRenderHinting`].
pub type TextHinting = sui_scene::TextRenderHinting;

/// The renderer's name for [`sui_scene::TextRenderStemDarkening`].
pub type StemDarkening = sui_scene::TextRenderStemDarkening;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextCoveragePolicy {
    #[default]
    Perceptual,
    /// Resolved luminances for the perceptual curve. Prefer `Perceptual` to
    /// let the renderer resolve these from the text and its solid backdrop.
    PerceptualLuminance {
        text: f32,
        background: f32,
    },
    Linear,
    Gamma(f32),
    CoverageBoost(f32),
    TwoCoverageMinusCoverageSq,
}

impl TextCoveragePolicy {
    pub fn normalized(self) -> Self {
        match self {
            Self::Perceptual => Self::Perceptual,
            Self::PerceptualLuminance { text, background }
                if text.is_finite() && background.is_finite() =>
            {
                Self::PerceptualLuminance {
                    text: text.clamp(0.0, 1.0),
                    background: background.clamp(0.0, 1.0),
                }
            }
            Self::PerceptualLuminance { .. } => Self::Linear,
            Self::Linear => Self::Linear,
            Self::Gamma(gamma) if gamma.is_finite() && gamma > 0.0 => Self::Gamma(gamma),
            Self::Gamma(_) => Self::Linear,
            Self::CoverageBoost(amount) if amount.is_finite() && amount > 0.0 => {
                Self::CoverageBoost(amount.clamp(0.0, 1.0))
            }
            Self::CoverageBoost(_) => Self::Linear,
            Self::TwoCoverageMinusCoverageSq => Self::TwoCoverageMinusCoverageSq,
        }
    }

    pub fn resolved_for_text_color(self, color: Color) -> Self {
        self.resolved_for_text_background(color, None)
    }

    pub(crate) fn resolved_for_text_background(
        self,
        color: Color,
        background: Option<Color>,
    ) -> Self {
        match self.normalized() {
            Self::Perceptual => {
                if !is_sdr_color(color) || background.is_some_and(|c| !is_sdr_color(c)) {
                    return Self::Linear;
                }
                let text = encoded_srgb_luminance(color);
                Self::PerceptualLuminance {
                    text,
                    background: background.map(encoded_srgb_luminance).unwrap_or(1.0 - text),
                }
            }
            policy => policy,
        }
    }

    pub fn apply(self, coverage: f32) -> f32 {
        let coverage = coverage.clamp(0.0, 1.0);
        match self.normalized() {
            Self::Perceptual => perceptual_text_coverage(coverage, 0.0, 1.0),
            Self::PerceptualLuminance { text, background } => {
                perceptual_text_coverage(coverage, text, background)
            }
            Self::Linear => coverage,
            Self::Gamma(gamma) => coverage.powf(gamma),
            Self::CoverageBoost(amount) => apply_coverage_boost(coverage, amount),
            Self::TwoCoverageMinusCoverageSq => (2.0 * coverage) - (coverage * coverage),
        }
    }
}

pub(crate) fn is_sdr_color(color: Color) -> bool {
    let c = color.to_linear_srgb();
    [c.red, c.green, c.blue]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
}

/// Contrast/gamma compensation modeled on Skia's mask-gamma construction,
/// converted back to coverage for our linear-light framebuffer. The 1.8
/// perceptual exponent is calibrated against Chrome's light/dark UI text;
/// it is not the framebuffer transfer function. Keep in sync with both text
/// atlas shaders. Endpoint preservation also keeps glyph padding transparent.
pub(crate) fn perceptual_text_coverage(coverage: f32, text: f32, background: f32) -> f32 {
    let c = coverage.clamp(0.0, 1.0);
    if c == 0.0 || c == 1.0 {
        return c;
    }
    let gamma = 1.8;
    let a = apply_coverage_boost(c, 0.5 * background.powf(gamma));
    let fg = encoded_srgb_to_linear_unit(text);
    let bg = encoded_srgb_to_linear_unit(background);
    if (fg - bg).abs() < 1e-4 {
        return a;
    }
    let perceptual = (text.powf(gamma) * a + background.powf(gamma) * (1.0 - a)).powf(1.0 / gamma);
    ((encoded_srgb_to_linear_unit(perceptual) - bg) / (fg - bg)).clamp(0.0, 1.0)
}

fn encoded_srgb_to_linear_unit(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Chrome/Skia's per-channel sRGB mask correction, followed by conversion of
/// the resulting encoded-space composite into coverage for our linear target.
/// Uses the Windows reference contrast of 1.0. This is LCD-only; the portable
/// grayscale policy keeps its separately calibrated luminance curve.
#[cfg(test)]
pub(crate) fn lcd_text_coverage(coverage: f32, text: f32, background: f32) -> f32 {
    let c = coverage.clamp(0.0, 1.0);
    if c == 0.0 || c == 1.0 {
        return c;
    }
    let guessed_bg = 1.0 - text;
    let guessed_linear = encoded_srgb_to_linear_unit(guessed_bg);
    let foreground_linear = encoded_srgb_to_linear_unit(text);
    let a = apply_coverage_boost(c, guessed_linear);
    let encoded_alpha = if (text - guessed_bg).abs() < 1.0 / 256.0 {
        a
    } else {
        (linear_srgb_to_encoded_unit(foreground_linear * a + guessed_linear * (1.0 - a))
            - guessed_bg)
            / (text - guessed_bg)
    }
    .clamp(0.0, 1.0);
    let bg = encoded_srgb_to_linear_unit(background);
    if (foreground_linear - bg).abs() < 1e-4 {
        return c;
    }
    let desired = encoded_srgb_to_linear_unit(background + (text - background) * encoded_alpha);
    ((desired - bg) / (foreground_linear - bg)).clamp(0.0, 1.0)
}

pub(crate) fn pack_lcd_background(color: Color) -> f32 {
    let c = color.to_linear_srgb();
    let [r, g, b] =
        [c.red, c.green, c.blue].map(|v| (linear_srgb_to_encoded_unit(v) * 255.0).round() as u32);
    // Exactly representable as an f32 integer and interpolated flat.
    ((r << 16) | (g << 8) | b) as f32
}

pub(crate) fn apply_coverage_boost(coverage: f32, amount: f32) -> f32 {
    coverage + (coverage * (1.0 - coverage) * amount.clamp(0.0, 1.0))
}

pub(crate) fn encoded_srgb_luminance(color: Color) -> f32 {
    let linear = color.to_linear_srgb();
    let red = linear_srgb_to_encoded_unit(linear.red);
    let green = linear_srgb_to_encoded_unit(linear.green);
    let blue = linear_srgb_to_encoded_unit(linear.blue);
    ((0.2126 * red) + (0.7152 * green) + (0.0722 * blue)).clamp(0.0, 1.0)
}

pub(crate) fn linear_srgb_to_encoded_unit(channel: f32) -> f32 {
    let value = channel.clamp(0.0, 1.0);
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        (1.055 * value.powf(1.0 / 2.4)) - 0.055
    }
}
