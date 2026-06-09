use sui_core::Color;
use sui_layout::Padding as Insets;
use sui_text::TextStyle;

use crate::animation::Easing;
use crate::hdr_theme::HdrThemeTokens;

/// Motion design tokens: a shared vocabulary of animation durations and easing
/// curves so widgets and applications animate consistently.
///
/// Durations are expressed in **seconds** (matching the `delta` supplied by
/// [`crate::animation::AnimatedValue::tick`] and the `time`/`delta` fields of
/// `WakeEvent::AnimationFrame`). Easing curves are built from the
/// [`Easing`] enum and are [`Copy`], keeping [`DefaultTheme`] `Copy`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeMotion {
    /// No animation: state changes apply immediately (0.0s).
    pub duration_instant: f32,
    /// Quick feedback such as hover/press state changes (~0.12s).
    pub duration_fast: f32,
    /// The default transition duration for most state changes (~0.2s).
    pub duration_normal: f32,
    /// Larger or more prominent transitions, e.g. expanding panels (~0.32s).
    pub duration_slow: f32,
    /// The standard easing curve: gentle acceleration, firm deceleration.
    /// Use for the majority of UI transitions.
    pub easing_standard: Easing,
    /// A more expressive curve for prominent, attention-drawing motion.
    pub easing_emphasized: Easing,
    /// Decelerate curve: enters quickly, settles softly. Good for elements
    /// entering the screen.
    pub easing_decelerate: Easing,
    /// Accelerate curve: starts softly, exits quickly. Good for elements
    /// leaving the screen.
    pub easing_accelerate: Easing,
}

impl ThemeMotion {
    /// The standard motion tokens shared by every built-in theme.
    pub const fn standard() -> Self {
        Self {
            duration_instant: 0.0,
            duration_fast: 0.12,
            duration_normal: 0.2,
            duration_slow: 0.32,
            easing_standard: Easing::CubicBezier {
                x1: 0.2,
                y1: 0.0,
                x2: 0.0,
                y2: 1.0,
            },
            // Material-style emphasized curve: a more expressive, slightly
            // overshooting-feeling ease distinct from `standard`.
            easing_emphasized: Easing::CubicBezier {
                x1: 0.05,
                y1: 0.7,
                x2: 0.1,
                y2: 1.0,
            },
            easing_decelerate: Easing::CubicBezier {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 1.0,
            },
            easing_accelerate: Easing::CubicBezier {
                x1: 0.3,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
        }
    }
}

impl Default for ThemeMotion {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeFontStack {
    pub primary: &'static str,
    pub fallbacks: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeFontFamilies {
    pub sans: ThemeFontStack,
    pub serif: ThemeFontStack,
    pub mono: ThemeFontStack,
}

impl Default for ThemeFontFamilies {
    fn default() -> Self {
        Self {
            sans: ThemeFontStack {
                primary: "ui-sans-serif",
                fallbacks: &[
                    "system-ui",
                    "sans-serif",
                    "Apple Color Emoji",
                    "Segoe UI Emoji",
                    "Segoe UI Symbol",
                    "Noto Color Emoji",
                ],
            },
            serif: ThemeFontStack {
                primary: "ui-serif",
                fallbacks: &["Georgia", "Cambria", "Times New Roman", "Times", "serif"],
            },
            mono: ThemeFontStack {
                primary: "ui-monospace",
                fallbacks: &[
                    "SFMono-Regular",
                    "Menlo",
                    "Monaco",
                    "Consolas",
                    "Liberation Mono",
                    "Courier New",
                    "monospace",
                ],
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeColorScheme {
    #[default]
    Light,
    Dark,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeDensity {
    Compact,
    #[default]
    Comfortable,
    Touch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SemanticTone {
    #[default]
    Neutral,
    Accent,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    pub name: &'static str,
    pub scheme: ThemeColorScheme,
    pub base_100: Color,
    pub base_200: Color,
    pub base_300: Color,
    pub base_content: Color,
    pub primary: Color,
    pub primary_content: Color,
    pub secondary: Color,
    pub secondary_content: Color,
    pub accent: Color,
    pub accent_content: Color,
    pub neutral: Color,
    pub neutral_content: Color,
    pub info: Color,
    pub info_content: Color,
    pub success: Color,
    pub success_content: Color,
    pub warning: Color,
    pub warning_content: Color,
    pub error: Color,
    pub error_content: Color,
}

impl ThemeColors {
    pub fn light() -> Self {
        Self {
            name: "light",
            scheme: ThemeColorScheme::Light,
            base_100: Color::rgba(0.965, 0.973, 0.984, 1.0),
            base_200: Color::rgba(1.0, 1.0, 1.0, 1.0),
            base_300: Color::rgba(0.815, 0.842, 0.878, 1.0),
            base_content: Color::rgba(0.105, 0.137, 0.184, 1.0),
            primary: Color::rgba(0.045, 0.384, 0.645, 1.0),
            primary_content: Color::rgba(0.985, 0.995, 1.0, 1.0),
            secondary: Color::rgba(0.125, 0.565, 0.498, 1.0),
            secondary_content: Color::rgba(0.975, 1.0, 0.995, 1.0),
            accent: Color::rgba(0.080, 0.520, 0.600, 1.0),
            accent_content: Color::rgba(0.960, 1.0, 1.0, 1.0),
            neutral: Color::rgba(0.180, 0.215, 0.270, 1.0),
            neutral_content: Color::rgba(0.950, 0.965, 0.985, 1.0),
            info: Color::rgba(0.075, 0.455, 0.780, 1.0),
            info_content: Color::rgba(0.960, 0.985, 1.0, 1.0),
            success: Color::rgba(0.075, 0.555, 0.345, 1.0),
            success_content: Color::rgba(0.960, 1.0, 0.980, 1.0),
            warning: Color::rgba(0.740, 0.470, 0.080, 1.0),
            warning_content: Color::rgba(1.0, 0.985, 0.930, 1.0),
            error: Color::rgba(0.760, 0.165, 0.200, 1.0),
            error_content: Color::rgba(1.0, 0.960, 0.965, 1.0),
        }
    }

    pub fn dark() -> Self {
        Self {
            name: "dark",
            scheme: ThemeColorScheme::Dark,
            base_100: Color::rgba(0.050, 0.055, 0.066, 1.0),
            base_200: Color::rgba(0.075, 0.084, 0.100, 1.0),
            base_300: Color::rgba(0.225, 0.245, 0.285, 1.0),
            base_content: Color::rgba(0.900, 0.925, 0.960, 1.0),
            primary: Color::rgba(0.250, 0.820, 1.0, 1.0),
            primary_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            secondary: Color::rgba(0.330, 0.980, 0.720, 1.0),
            secondary_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            accent: Color::rgba(1.0, 0.840, 0.120, 1.0),
            accent_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            neutral: Color::rgba(0.120, 0.130, 0.150, 1.0),
            neutral_content: Color::rgba(1.0, 1.0, 1.0, 1.0),
            info: Color::rgba(0.250, 0.820, 1.0, 1.0),
            info_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            success: Color::rgba(0.360, 1.0, 0.620, 1.0),
            success_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            warning: Color::rgba(1.0, 0.840, 0.120, 1.0),
            warning_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            error: Color::rgba(1.0, 0.360, 0.420, 1.0),
            error_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
        }
    }

    pub fn high_contrast() -> Self {
        Self {
            name: "high-contrast",
            scheme: ThemeColorScheme::HighContrast,
            base_100: Color::rgba(0.0, 0.0, 0.0, 1.0),
            base_200: Color::rgba(0.065, 0.070, 0.080, 1.0),
            base_300: Color::rgba(0.760, 0.800, 0.860, 1.0),
            base_content: Color::rgba(1.0, 1.0, 1.0, 1.0),
            primary: Color::rgba(0.250, 0.820, 1.0, 1.0),
            primary_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            secondary: Color::rgba(0.330, 0.980, 0.720, 1.0),
            secondary_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            accent: Color::rgba(1.0, 0.840, 0.120, 1.0),
            accent_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            neutral: Color::rgba(0.120, 0.130, 0.150, 1.0),
            neutral_content: Color::rgba(1.0, 1.0, 1.0, 1.0),
            info: Color::rgba(0.250, 0.820, 1.0, 1.0),
            info_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            success: Color::rgba(0.360, 1.0, 0.620, 1.0),
            success_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            warning: Color::rgba(1.0, 0.840, 0.120, 1.0),
            warning_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
            error: Color::rgba(1.0, 0.360, 0.420, 1.0),
            error_content: Color::rgba(0.0, 0.0, 0.0, 1.0),
        }
    }

    pub fn with_scheme(scheme: ThemeColorScheme) -> Self {
        match scheme {
            ThemeColorScheme::Light => Self::light(),
            ThemeColorScheme::Dark => Self::dark(),
            ThemeColorScheme::HighContrast => Self::high_contrast(),
        }
    }
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::light()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBreakpoints {
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
}

impl Default for ThemeBreakpoints {
    fn default() -> Self {
        Self {
            sm: 640.0,
            md: 768.0,
            lg: 1024.0,
            xl: 1280.0,
            _2xl: 1536.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeContainers {
    pub _3xs: f32,
    pub _2xs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
    pub _3xl: f32,
    pub _4xl: f32,
    pub _5xl: f32,
    pub _6xl: f32,
    pub _7xl: f32,
}

impl Default for ThemeContainers {
    fn default() -> Self {
        Self {
            _3xs: 256.0,
            _2xs: 288.0,
            xs: 320.0,
            sm: 384.0,
            md: 448.0,
            lg: 512.0,
            xl: 576.0,
            _2xl: 672.0,
            _3xl: 768.0,
            _4xl: 896.0,
            _5xl: 1024.0,
            _6xl: 1152.0,
            _7xl: 1280.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTextToken {
    pub size: f32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTextScale {
    pub xs: ThemeTextToken,
    pub sm: ThemeTextToken,
    pub base: ThemeTextToken,
    pub lg: ThemeTextToken,
    pub xl: ThemeTextToken,
    pub _2xl: ThemeTextToken,
    pub _3xl: ThemeTextToken,
    pub _4xl: ThemeTextToken,
    pub _5xl: ThemeTextToken,
    pub _6xl: ThemeTextToken,
    pub _7xl: ThemeTextToken,
    pub _8xl: ThemeTextToken,
    pub _9xl: ThemeTextToken,
}

impl Default for ThemeTextScale {
    fn default() -> Self {
        Self {
            xs: ThemeTextToken {
                size: 12.0,
                line_height: 16.0,
            },
            sm: ThemeTextToken {
                size: 14.0,
                line_height: 20.0,
            },
            base: ThemeTextToken {
                size: 16.0,
                line_height: 24.0,
            },
            lg: ThemeTextToken {
                size: 18.0,
                line_height: 28.0,
            },
            xl: ThemeTextToken {
                size: 20.0,
                line_height: 28.0,
            },
            _2xl: ThemeTextToken {
                size: 24.0,
                line_height: 32.0,
            },
            _3xl: ThemeTextToken {
                size: 30.0,
                line_height: 36.0,
            },
            _4xl: ThemeTextToken {
                size: 36.0,
                line_height: 40.0,
            },
            _5xl: ThemeTextToken {
                size: 48.0,
                line_height: 48.0,
            },
            _6xl: ThemeTextToken {
                size: 60.0,
                line_height: 60.0,
            },
            _7xl: ThemeTextToken {
                size: 72.0,
                line_height: 72.0,
            },
            _8xl: ThemeTextToken {
                size: 96.0,
                line_height: 96.0,
            },
            _9xl: ThemeTextToken {
                size: 128.0,
                line_height: 128.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeFontWeights {
    pub thin: u16,
    pub extralight: u16,
    pub light: u16,
    pub normal: u16,
    pub medium: u16,
    pub semibold: u16,
    pub bold: u16,
    pub extrabold: u16,
    pub black: u16,
}

impl Default for ThemeFontWeights {
    fn default() -> Self {
        Self {
            thin: 100,
            extralight: 200,
            light: 300,
            normal: 400,
            medium: 500,
            semibold: 600,
            bold: 700,
            extrabold: 800,
            black: 900,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTracking {
    pub tighter: f32,
    pub tight: f32,
    pub normal: f32,
    pub wide: f32,
    pub wider: f32,
    pub widest: f32,
}

impl Default for ThemeTracking {
    fn default() -> Self {
        Self {
            tighter: -0.05,
            tight: -0.025,
            normal: 0.0,
            wide: 0.025,
            wider: 0.05,
            widest: 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeLeading {
    pub tight: f32,
    pub snug: f32,
    pub normal: f32,
    pub relaxed: f32,
    pub loose: f32,
}

impl Default for ThemeLeading {
    fn default() -> Self {
        Self {
            tight: 1.25,
            snug: 1.375,
            normal: 1.5,
            relaxed: 1.625,
            loose: 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeRadii {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
    pub _3xl: f32,
    pub _4xl: f32,
}

impl Default for ThemeRadii {
    fn default() -> Self {
        Self {
            xs: 1.0,
            sm: 2.0,
            md: 4.0,
            lg: 6.0,
            xl: 8.0,
            _2xl: 12.0,
            _3xl: 16.0,
            _4xl: 24.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeShadowLayer {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeShadow {
    pub first: Option<ThemeShadowLayer>,
    pub second: Option<ThemeShadowLayer>,
}

impl ThemeShadow {
    pub const fn empty() -> Self {
        Self {
            first: None,
            second: None,
        }
    }

    pub const fn single(layer: ThemeShadowLayer) -> Self {
        Self {
            first: Some(layer),
            second: None,
        }
    }

    pub const fn double(first: ThemeShadowLayer, second: ThemeShadowLayer) -> Self {
        Self {
            first: Some(first),
            second: Some(second),
        }
    }
}

impl ThemeShadowLayer {
    /// Convert this theme shadow layer into the renderer primitive
    /// [`sui_scene::ShadowParams`] consumed by `PaintCtx::draw_shadow`.
    pub fn to_shadow_params(&self) -> sui_scene::ShadowParams {
        sui_scene::ShadowParams {
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            blur: self.blur,
            spread: self.spread,
            color: self.color,
        }
    }

    /// An outer (drop) shadow casts beyond the surface edge; an inset layer
    /// renders an inner shadow. Only outer layers are paintable today.
    pub const fn is_outer(&self) -> bool {
        !self.inset
    }
}

/// Paint the outer (drop) layers of a [`ThemeShadow`] behind a rounded-rect
/// surface. The tighter `second` layer is drawn first and the wider/more-diffuse
/// `first` layer on top — matching CSS `box-shadow`, where the first-listed
/// shadow is topmost.
///
/// Inset layers are skipped: inner shadows are future work.
///
/// The caller MUST invoke this BEFORE filling the surface background and BEFORE
/// pushing any clip tight to the widget, so the soft shadow renders behind the
/// fill and is not clipped away.
pub fn paint_theme_shadow(
    paint: &mut sui_runtime::PaintCtx,
    rect: sui_core::Rect,
    radii: [f32; 4],
    shadow: &ThemeShadow,
) {
    // Draw the tighter `second` layer first, then the wider/more-diffuse `first`
    // layer on top (CSS box-shadow order: the first-listed shadow is topmost).
    if let Some(layer) = shadow.second {
        if layer.is_outer() {
            paint.draw_shadow(rect, radii, layer.to_shadow_params());
        }
        // inset layers are inner shadows -> future work
    }
    if let Some(layer) = shadow.first {
        if layer.is_outer() {
            paint.draw_shadow(rect, radii, layer.to_shadow_params());
        }
        // inset layers are inner shadows -> future work
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBoxShadowScale {
    pub _2xs: ThemeShadow,
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
    pub md: ThemeShadow,
    pub lg: ThemeShadow,
    pub xl: ThemeShadow,
    pub _2xl: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeInsetShadowScale {
    pub _2xs: ThemeShadow,
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeDropShadowScale {
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
    pub md: ThemeShadow,
    pub lg: ThemeShadow,
    pub xl: ThemeShadow,
    pub _2xl: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTextShadowScale {
    pub _2xs: ThemeShadow,
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
    pub md: ThemeShadow,
    pub lg: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeShadows {
    pub box_shadow: ThemeBoxShadowScale,
    pub inset: ThemeInsetShadowScale,
    pub drop: ThemeDropShadowScale,
    pub text: ThemeTextShadowScale,
}

impl Default for ThemeShadows {
    fn default() -> Self {
        let black_005 = Color::BLACK.with_alpha(0.05);
        let black_01 = Color::BLACK.with_alpha(0.1);
        let black_012 = Color::BLACK.with_alpha(0.12);
        let black_015 = Color::BLACK.with_alpha(0.15);
        let black_02 = Color::BLACK.with_alpha(0.2);
        let black_025 = Color::BLACK.with_alpha(0.25);
        let black_075 = Color::BLACK.with_alpha(0.075);

        Self {
            box_shadow: ThemeBoxShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black_005, false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, black_005, false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 3.0, 0.0, black_01, false),
                    shadow_layer(0.0, 1.0, 2.0, -1.0, black_01, false),
                ),
                md: ThemeShadow::double(
                    shadow_layer(0.0, 4.0, 6.0, -1.0, black_01, false),
                    shadow_layer(0.0, 2.0, 4.0, -2.0, black_01, false),
                ),
                lg: ThemeShadow::double(
                    shadow_layer(0.0, 10.0, 15.0, -3.0, black_01, false),
                    shadow_layer(0.0, 4.0, 6.0, -4.0, black_01, false),
                ),
                xl: ThemeShadow::double(
                    shadow_layer(0.0, 20.0, 25.0, -5.0, black_01, false),
                    shadow_layer(0.0, 8.0, 10.0, -6.0, black_01, false),
                ),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 25.0, 50.0, -12.0, black_025, false)),
            },
            inset: ThemeInsetShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black_005, true)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black_005, true)),
                sm: ThemeShadow::single(shadow_layer(0.0, 2.0, 4.0, 0.0, black_005, true)),
            },
            drop: ThemeDropShadowScale {
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black_005, false)),
                sm: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, black_015, false)),
                md: ThemeShadow::single(shadow_layer(0.0, 3.0, 3.0, 0.0, black_012, false)),
                lg: ThemeShadow::single(shadow_layer(0.0, 4.0, 4.0, 0.0, black_015, false)),
                xl: ThemeShadow::single(shadow_layer(0.0, 9.0, 7.0, 0.0, black_01, false)),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 25.0, 25.0, 0.0, black_015, false)),
            },
            text: ThemeTextShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black_015, false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black_02, false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 0.0, 0.0, black_075, false),
                    shadow_layer(0.0, 1.0, 1.0, 0.0, black_075, false),
                ),
                md: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 1.0, 0.0, black_01, false),
                    shadow_layer(0.0, 2.0, 4.0, 0.0, black_01, false),
                ),
                lg: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 2.0, 0.0, black_01, false),
                    shadow_layer(0.0, 4.0, 8.0, 0.0, black_01, false),
                ),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBlurScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
    pub _3xl: f32,
}

impl Default for ThemeBlurScale {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
            _2xl: 40.0,
            _3xl: 64.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePerspective {
    pub dramatic: f32,
    pub near: f32,
    pub normal: f32,
    pub midrange: f32,
    pub distant: f32,
}

impl Default for ThemePerspective {
    fn default() -> Self {
        Self {
            dramatic: 100.0,
            near: 300.0,
            normal: 500.0,
            midrange: 800.0,
            distant: 1200.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeAspectRatios {
    pub video: f32,
}

impl Default for ThemeAspectRatios {
    fn default() -> Self {
        Self { video: 16.0 / 9.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPalette {
    pub text: Color,
    pub text_muted: Color,
    pub placeholder: Color,
    pub surface: Color,
    pub surface_raised: Color,
    pub control: Color,
    pub control_hover: Color,
    pub control_active: Color,
    pub surface_hover: Color,
    pub surface_pressed: Color,
    pub surface_focus: Color,
    pub border: Color,
    pub border_strong: Color,
    pub border_hover: Color,
    pub border_focus: Color,
    pub focus_ring: Color,
    pub caret: Color,
    pub selection: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub accent_border: Color,
    pub accent_border_hover: Color,
    pub accent_border_focus: Color,
    pub accent_text: Color,
    pub info: Color,
    pub info_text: Color,
    pub success: Color,
    pub success_text: Color,
    pub warning: Color,
    pub warning_text: Color,
    pub danger: Color,
    pub danger_text: Color,
}

impl ControlPalette {
    pub fn from_colors(colors: &ThemeColors) -> Self {
        let is_dark = matches!(
            colors.scheme,
            ThemeColorScheme::Dark | ThemeColorScheme::HighContrast
        );
        let surface = colors.base_100;
        let surface_raised = colors.base_200;
        let control = colors.base_200;
        let control_hover = interactive_surface(control, colors.scheme, 0.035);
        let control_active = interactive_surface(control, colors.scheme, 0.075);
        let text_muted = mix(
            colors.base_content,
            surface,
            if is_dark { 0.34 } else { 0.16 },
        );
        let placeholder = mix(
            colors.base_content,
            surface,
            if is_dark { 0.50 } else { 0.22 },
        );
        let border = if is_dark {
            mix(colors.base_300, surface, 0.22)
        } else {
            colors.base_300
        };
        let border_strong = mix(
            colors.base_300,
            colors.base_content,
            if is_dark { 0.18 } else { 0.10 },
        );
        let border_hover = mix(border, colors.primary, if is_dark { 0.28 } else { 0.18 });
        let border_focus = colors.primary;
        let focus_alpha = if colors.scheme == ThemeColorScheme::HighContrast {
            0.72
        } else {
            0.32
        };
        let selection = mix(surface, colors.primary, if is_dark { 0.30 } else { 0.14 });

        Self {
            text: colors.base_content,
            text_muted,
            placeholder,
            surface,
            surface_raised,
            control,
            control_hover,
            control_active,
            surface_hover: control_hover,
            surface_pressed: control_active,
            surface_focus: mix(control, colors.primary, if is_dark { 0.14 } else { 0.08 }),
            border,
            border_strong,
            border_hover,
            border_focus,
            focus_ring: colors.primary.with_alpha(focus_alpha),
            caret: colors.primary,
            selection,
            accent: colors.primary,
            accent_hover: interactive_variant(colors.primary, colors.scheme, 0.08),
            accent_pressed: interactive_variant(colors.primary, colors.scheme, 0.16),
            accent_border: interactive_variant(colors.primary, colors.scheme, 0.12),
            accent_border_hover: interactive_variant(colors.primary, colors.scheme, 0.2),
            accent_border_focus: colors.primary,
            accent_text: colors.primary_content,
            info: colors.info,
            info_text: colors.info_content,
            success: colors.success,
            success_text: colors.success_content,
            warning: colors.warning,
            warning_text: colors.warning_content,
            danger: colors.error,
            danger_text: colors.error_content,
        }
    }
}

impl Default for ControlPalette {
    fn default() -> Self {
        Self::from_colors(&ThemeColors::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePalette {
    pub dark: bool,
    pub window: Color,
    pub sidebar: Color,
    pub panel: Color,
    pub titlebar: Color,
    pub field: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_faint: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub on_accent: Color,
    pub hover: Color,
    pub selected: Color,
    pub overlay_scrim: Color,
    pub tooltip: Color,
    pub tooltip_border: Color,
    pub tooltip_text: Color,
    pub canvas: Color,
    pub canvas_grid: Color,
    pub canvas_axis_x: Color,
    pub canvas_axis_y: Color,
    pub pixel_canvas_paper: Color,
    pub pixel_canvas_document_edge: Color,
    pub pixel_canvas_shadow_near: Color,
    pub pixel_canvas_shadow_far: Color,
    pub pixel_canvas_grid: Color,
    pub canvas_ruler: Color,
    pub canvas_ruler_border: Color,
    pub canvas_ruler_tick: Color,
    pub canvas_ruler_text: Color,
    pub checkerboard_light: Color,
    pub checkerboard_dark: Color,
    pub color_picker_chrome_border: Color,
    pub color_picker_plane_border: Color,
    pub color_picker_bar_border: Color,
    pub color_picker_marker_outer: Color,
    pub color_picker_marker_dark: Color,
    pub color_picker_marker_light: Color,
    pub color_picker_sdr_marker: Color,
    pub color_picker_hdr_divider: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,
}

impl SurfacePalette {
    pub fn from_theme_parts(colors: &ThemeColors, controls: &ControlPalette) -> Self {
        let dark = matches!(
            colors.scheme,
            ThemeColorScheme::Dark | ThemeColorScheme::HighContrast
        );
        let text_muted = mix(
            controls.text,
            controls.surface,
            if dark { 0.34 } else { 0.18 },
        );
        let text_faint = mix(
            controls.text,
            controls.surface,
            if dark { 0.50 } else { 0.28 },
        );

        Self {
            dark,
            window: controls.surface,
            sidebar: if dark {
                mix(controls.surface, controls.surface_raised, 0.22)
            } else {
                controls.surface
            },
            panel: controls.surface_raised,
            titlebar: if dark {
                mix(controls.surface_raised, controls.control, 0.35)
            } else {
                controls.surface_raised
            },
            field: controls.control,
            border: controls.border,
            border_strong: controls.border_strong,
            text: controls.text,
            text_muted,
            text_faint,
            accent: controls.accent,
            accent_hover: controls.accent_hover,
            on_accent: controls.accent_text,
            hover: controls.text.with_alpha(if dark { 0.06 } else { 0.045 }),
            selected: controls.selection,
            overlay_scrim: Color::rgba(0.06, 0.08, 0.12, if dark { 0.38 } else { 0.24 }),
            tooltip: if dark {
                controls.surface_raised
            } else {
                controls.text
            },
            tooltip_border: if dark {
                controls.border_strong
            } else {
                mix(controls.text, controls.surface, 0.20)
            },
            tooltip_text: if dark {
                controls.text
            } else {
                controls.surface
            },
            canvas: controls.surface,
            canvas_grid: controls.border.with_alpha(if dark { 0.30 } else { 0.18 }),
            canvas_axis_x: colors.error.with_alpha(if dark { 0.72 } else { 0.55 }),
            canvas_axis_y: colors.success.with_alpha(if dark { 0.72 } else { 0.55 }),
            pixel_canvas_paper: if dark {
                mix(controls.surface_raised, controls.text, 0.10)
            } else {
                Color::rgba(0.975, 0.980, 0.988, 1.0)
            },
            pixel_canvas_document_edge: controls.text.with_alpha(if dark { 0.82 } else { 0.72 }),
            pixel_canvas_shadow_near: Color::rgba(0.05, 0.07, 0.10, if dark { 0.30 } else { 0.16 }),
            pixel_canvas_shadow_far: Color::rgba(0.05, 0.07, 0.10, if dark { 0.18 } else { 0.08 }),
            pixel_canvas_grid: controls.text.with_alpha(if dark { 0.32 } else { 0.28 }),
            canvas_ruler: controls.surface_raised,
            canvas_ruler_border: controls.border.with_alpha(0.78),
            canvas_ruler_tick: controls.text_muted.with_alpha(0.72),
            canvas_ruler_text: controls.text.with_alpha(0.76),
            checkerboard_light: if dark {
                mix(controls.surface_raised, controls.text, 0.18)
            } else {
                Color::rgba(0.980, 0.980, 0.990, 1.0)
            },
            checkerboard_dark: if dark {
                mix(controls.surface_raised, controls.text, 0.10)
            } else {
                Color::rgba(0.900, 0.920, 0.950, 1.0)
            },
            color_picker_chrome_border: controls.text.with_alpha(if dark { 0.24 } else { 0.18 }),
            color_picker_plane_border: controls.text.with_alpha(if dark { 0.22 } else { 0.16 }),
            color_picker_bar_border: controls.text.with_alpha(if dark { 0.20 } else { 0.14 }),
            color_picker_marker_outer: controls.surface_raised.with_alpha(0.92),
            color_picker_marker_dark: Color::BLACK.with_alpha(0.84),
            color_picker_marker_light: Color::WHITE.with_alpha(0.95),
            color_picker_sdr_marker: controls.surface_raised.with_alpha(0.30),
            color_picker_hdr_divider: controls.surface_raised.with_alpha(0.28),
            good: colors.success,
            warn: colors.warning,
            bad: colors.error,
        }
    }
}

impl Default for SurfacePalette {
    fn default() -> Self {
        let colors = ThemeColors::default();
        let controls = ControlPalette::from_colors(&colors);
        Self::from_theme_parts(&colors, &controls)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlTypography {
    pub body_font_size: f32,
    pub body_line_height: f32,
}

impl ControlTypography {
    pub fn from_text_scale(text: &ThemeTextScale) -> Self {
        Self {
            body_font_size: text.sm.size,
            body_line_height: text.sm.line_height,
        }
    }
}

impl Default for ControlTypography {
    fn default() -> Self {
        Self::from_text_scale(&ThemeTextScale::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlStateMetrics {
    pub hover_blend: f32,
    pub pressed_blend: f32,
    pub selected_blend: f32,
    pub tab_selected_blend: f32,
    pub disabled_opacity: f32,
    pub disabled_content_opacity: f32,
    pub pressed_offset: f32,
    pub active_indicator_thickness: f32,
}

impl ControlStateMetrics {
    pub fn for_density(density: ThemeDensity) -> Self {
        match density {
            ThemeDensity::Compact => Self {
                hover_blend: 0.78,
                pressed_blend: 0.88,
                selected_blend: 0.20,
                tab_selected_blend: 0.07,
                disabled_opacity: 0.70,
                disabled_content_opacity: 0.46,
                pressed_offset: 0.5,
                active_indicator_thickness: 2.0,
            },
            ThemeDensity::Comfortable => Self {
                hover_blend: 0.86,
                pressed_blend: 1.0,
                selected_blend: 0.22,
                tab_selected_blend: 0.08,
                disabled_opacity: 0.74,
                disabled_content_opacity: 0.50,
                pressed_offset: 1.0,
                active_indicator_thickness: 3.0,
            },
            ThemeDensity::Touch => Self {
                hover_blend: 0.94,
                pressed_blend: 1.0,
                selected_blend: 0.24,
                tab_selected_blend: 0.09,
                disabled_opacity: 0.78,
                disabled_content_opacity: 0.54,
                pressed_offset: 1.5,
                active_indicator_thickness: 4.0,
            },
        }
    }
}

impl Default for ControlStateMetrics {
    fn default() -> Self {
        Self::for_density(ThemeDensity::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMetrics {
    pub min_height: f32,
    pub touch_target_size: f32,
    pub button_min_width: f32,
    pub button_padding: Insets,
    pub checkbox_padding: Insets,
    pub checkbox_indicator_size: f32,
    pub checkbox_gap: f32,
    pub icon_label_gap: f32,
    pub separator_thickness: f32,
    pub icon_size: f32,
    pub icon_button_size: f32,
    pub switch_track_width: f32,
    pub switch_track_height: f32,
    pub switch_thumb_inset: f32,
    pub slider_min_width: f32,
    pub slider_padding: Insets,
    pub slider_track_height: f32,
    pub slider_thumb_size: f32,
    pub number_input_stepper_width: f32,
    pub text_input_min_width: f32,
    pub text_input_padding: Insets,
    pub text_area_min_height: f32,
    pub select_menu_max_height: f32,
    pub select_menu_gap: f32,
    pub select_menu_edge_padding: f32,
    pub tab_height: f32,
    pub tab_min_width: f32,
    pub tab_gap: f32,
    pub tab_padding: Insets,
    pub tab_panel_padding: Insets,
    pub tab_panel_gap: f32,
    pub menu_row_height: f32,
    pub menu_padding: Insets,
    pub menu_item_padding: Insets,
    pub menu_shortcut_width: f32,
    pub popover_padding: Insets,
    pub popover_gap: f32,
    pub popover_reveal_offset: f32,
    pub tooltip_padding: Insets,
    pub tooltip_min_width: f32,
    pub tooltip_gap: f32,
    pub tooltip_reveal_offset: f32,
    pub dialog_min_width: f32,
    pub dialog_max_width: f32,
    pub dialog_outer_margin: f32,
    pub dialog_padding: Insets,
    pub dialog_title_font_size: f32,
    pub dialog_title_line_height: f32,
    pub dialog_description_gap: f32,
    pub dialog_body_gap: f32,
    pub dialog_footer_gap: f32,
    pub dialog_action_gap: f32,
    pub dialog_action_min_width: f32,
    pub toolbar_extent: f32,
    pub toolbar_padding: Insets,
    pub toolbar_spacing: f32,
    pub command_group_padding: Insets,
    pub command_group_spacing: f32,
    pub command_group_radius: f32,
    pub tool_palette_item_size: f32,
    pub tool_palette_icon_size: f32,
    pub preset_strip_item_height: f32,
    pub preset_strip_item_min_width: f32,
    pub preset_strip_item_padding: Insets,
    pub preset_strip_gap: f32,
    pub preset_strip_label_padding: Insets,
    pub action_card_min_width: f32,
    pub action_card_min_height: f32,
    pub action_card_padding: Insets,
    pub action_card_icon_box_size: f32,
    pub action_card_icon_size: f32,
    pub action_card_icon_gap: f32,
    pub action_card_text_gap: f32,
    pub action_card_trailing_gap: f32,
    pub action_card_chevron_size: f32,
    pub action_card_accent_width: f32,
    pub action_card_accent_inset: f32,
    pub status_bar_height: f32,
    pub status_bar_segment_padding: f32,
    pub status_bar_segment_min_width: f32,
    pub status_bar_separator_inset: f32,
    pub progress_bar_min_width: f32,
    pub progress_bar_height: f32,
    pub progress_bar_value_height: f32,
    pub progress_bar_label_padding: Insets,
    pub property_row_label_width: f32,
    pub property_row_inline_gap: f32,
    pub property_row_stacked_gap: f32,
    pub form_row_label_width: f32,
    pub form_row_control_width: f32,
    pub form_row_gap: f32,
    pub field_group_spacing: f32,
    pub form_section_padding: Insets,
    pub form_section_body_gap: f32,
    pub form_section_header_gap: f32,
    pub form_section_description_gap: f32,
    pub form_section_max_width: f32,
    pub form_section_radius: f32,
    pub panel_section_gap: f32,
    pub panel_section_action_gap: f32,
    pub panel_section_disclosure_size: f32,
    pub dock_panel_header_height: f32,
    pub dock_panel_padding: Insets,
    pub data_viewport_padding: Insets,
    pub data_row_padding: Insets,
    pub data_row_icon_size: f32,
    pub data_row_icon_gap: f32,
    pub data_row_trailing_gap: f32,
    pub data_scroll_thumb_width: f32,
    pub data_scroll_thumb_inset: f32,
    pub data_scroll_thumb_radius: f32,
    pub data_scroll_thumb_min_length: f32,
    pub data_scroll_thumb_opacity: f32,
    pub list_row_height: f32,
    pub layer_row_height: f32,
    pub layer_action_size: f32,
    pub layer_action_icon_inset: f32,
    pub layer_lock_icon_inset: f32,
    pub layer_visibility_stroke_width: f32,
    pub layer_visibility_slash_stroke_width: f32,
    pub layer_thumbnail_size: f32,
    pub layer_thumbnail_inset: f32,
    pub layer_thumbnail_radius: f32,
    pub layer_thumbnail_disabled_opacity: f32,
    pub layer_thumbnail_disabled_border_opacity: f32,
    pub tree_row_height: f32,
    pub tree_indent: f32,
    pub tree_disclosure_size: f32,
    pub tree_disclosure_gap: f32,
    pub table_row_height: f32,
    pub table_header_height: f32,
    pub table_cell_padding: f32,
    pub table_header_separator_inset: f32,
    pub table_separator_width: f32,
    pub table_row_border_opacity: f32,
    pub breadcrumb_height: f32,
    pub breadcrumb_item_padding: Insets,
    pub breadcrumb_gap: f32,
    pub breadcrumb_separator_size: f32,
    pub image_corner_radius: f32,
    pub color_swatch_width: f32,
    pub color_swatch_height: f32,
    pub color_swatch_inner_inset: f32,
    pub color_swatch_checker_size: f32,
    pub color_palette_swatch_size: f32,
    pub color_palette_gap: f32,
    pub color_palette_swatch_inset: f32,
    pub color_palette_selected_swatch_inset: f32,
    pub color_palette_checker_size: f32,
    pub brush_preview_min_width: f32,
    pub brush_preview_min_height: f32,
    pub brush_preview_padding: Insets,
    pub brush_preview_swatch_width: f32,
    pub brush_preview_swatch_gap: f32,
    pub brush_preview_checker_size: f32,
    pub brush_preview_text_height: f32,
    pub brush_preview_text_font_size: f32,
    pub brush_preview_text_line_height: f32,
    pub color_picker_content_inset: f32,
    pub color_picker_panel_gap: f32,
    pub color_picker_top_bar_height: f32,
    pub color_picker_swatch_width: f32,
    pub color_picker_swatch_gap: f32,
    pub color_picker_section_gap: f32,
    pub color_picker_wheel_size: f32,
    pub color_picker_map_size: f32,
    pub color_picker_row_height: f32,
    pub color_picker_row_gap: f32,
    pub color_picker_right_panel_width: f32,
    pub color_picker_field_height: f32,
    pub color_picker_field_gap: f32,
    pub color_picker_dropdown_gap: f32,
    pub color_picker_encoding_menu_row_height: f32,
    pub scroll_bar_thickness: f32,
    pub scroll_bar_min_thumb_length: f32,
    pub split_view_divider_thickness: f32,
    pub split_view_drag_target_thickness: f32,
    pub floating_workspace_margin: f32,
    pub floating_view_title_bar_height: f32,
    pub floating_view_title_padding: Insets,
    pub floating_view_resize_handle_size: f32,
    pub canvas_ruler_extent: f32,
    pub canvas_ruler_major_tick: f32,
    pub canvas_ruler_minor_tick: f32,
    pub canvas_ruler_target_major_spacing: f32,
    pub canvas_ruler_label_padding: Insets,
    pub canvas_ruler_label_max_width: f32,
    pub canvas_grid_step: f32,
    pub canvas_axis_overscan: f32,
    pub pixel_canvas_fit_padding: f32,
    pub pixel_canvas_grid_zoom: f32,
    pub pixel_canvas_nearest_sampling_zoom: f32,
    pub pixel_canvas_zoom_step: f32,
    pub corner_radius: f32,
    pub indicator_corner_radius: f32,
    pub border_width: f32,
    pub focus_ring_width: f32,
    pub focus_ring_outset: f32,
    pub caret_width: f32,
}

impl ControlMetrics {
    pub fn from_tokens(spacing: f32, radius: ThemeRadii, density: ThemeDensity) -> Self {
        let unit = spacing.max(1.0);
        let (
            min_height,
            touch_target_size,
            button_padding,
            checkbox_padding,
            checkbox_indicator_size,
            icon_size,
            icon_button_size,
            switch_track_width,
            switch_track_height,
            switch_thumb_inset,
            slider_min_width,
            slider_padding,
            slider_track_height,
            slider_thumb_size,
            number_input_stepper_width,
            text_input_min_width,
            text_input_padding,
            text_area_min_height,
            select_menu_max_height,
            tab_height,
            tab_min_width,
            tab_padding,
            tab_panel_padding,
            tab_panel_gap,
            menu_row_height,
            menu_padding,
            menu_item_padding,
            popover_padding,
            action_card_min_width,
            action_card_min_height,
            action_card_padding,
            action_card_icon_box_size,
            action_card_icon_size,
            action_card_icon_gap,
            action_card_text_gap,
            action_card_trailing_gap,
            action_card_chevron_size,
            action_card_accent_width,
            action_card_accent_inset,
            status_bar_height,
            status_bar_segment_padding,
            status_bar_segment_min_width,
            status_bar_separator_inset,
            progress_bar_min_width,
            progress_bar_height,
            progress_bar_value_height,
            progress_bar_label_padding,
            data_viewport_padding,
            data_row_padding,
            data_row_icon_size,
            data_row_icon_gap,
            data_row_trailing_gap,
            list_row_height,
            layer_row_height,
            layer_action_size,
            layer_thumbnail_size,
            tree_row_height,
            tree_indent,
            tree_disclosure_size,
            tree_disclosure_gap,
            table_row_height,
            table_header_height,
            table_cell_padding,
            breadcrumb_height,
            breadcrumb_item_padding,
            breadcrumb_gap,
            breadcrumb_separator_size,
        ) = match density {
            ThemeDensity::Compact => (
                22.0,
                28.0,
                Insets {
                    left: unit * 1.5,
                    top: unit * 0.75,
                    right: unit * 1.5,
                    bottom: unit * 0.75,
                },
                Insets {
                    left: unit,
                    top: unit * 0.5,
                    right: unit,
                    bottom: unit * 0.5,
                },
                12.0,
                12.0,
                22.0,
                26.0,
                14.0,
                2.0,
                120.0,
                Insets {
                    left: unit * 1.5,
                    top: unit * 0.5,
                    right: unit * 1.5,
                    bottom: unit * 0.5,
                },
                3.0,
                12.0,
                22.0,
                150.0,
                Insets {
                    left: unit * 1.5,
                    top: unit * 0.75,
                    right: unit * 1.5,
                    bottom: unit * 0.75,
                },
                64.0,
                176.0,
                28.0,
                84.0,
                Insets {
                    left: unit * 2.0,
                    top: unit * 0.75,
                    right: unit * 2.0,
                    bottom: unit * 0.75,
                },
                Insets::all(unit * 3.0),
                unit * 2.0,
                24.0,
                Insets::all(unit),
                Insets {
                    left: unit * 2.0,
                    top: unit * 0.5,
                    right: unit * 2.0,
                    bottom: unit * 0.5,
                },
                Insets::all(unit * 2.5),
                252.0,
                84.0,
                Insets {
                    left: unit * 3.0,
                    top: unit * 2.5,
                    right: unit * 2.5,
                    bottom: unit * 2.5,
                },
                32.0,
                16.0,
                unit * 2.5,
                unit,
                18.0,
                14.0,
                2.0,
                unit * 2.0,
                24.0,
                unit * 2.0,
                72.0,
                unit * 1.25,
                180.0,
                14.0,
                22.0,
                Insets::all(unit * 0.5),
                Insets::all(unit * 1.5),
                Insets {
                    left: unit * 3.0,
                    top: unit * 0.5,
                    right: unit * 2.0,
                    bottom: unit * 0.5,
                },
                12.0,
                unit * 1.5,
                unit * 2.0,
                24.0,
                38.0,
                22.0,
                26.0,
                24.0,
                unit * 4.0,
                10.0,
                unit,
                24.0,
                26.0,
                unit * 1.5,
                24.0,
                Insets {
                    left: unit * 2.0,
                    top: unit * 0.75,
                    right: unit * 2.0,
                    bottom: unit * 0.75,
                },
                unit * 4.0,
                9.0,
            ),
            ThemeDensity::Comfortable => (
                24.0,
                32.0,
                Insets {
                    left: unit * 2.0,
                    top: unit * 1.25,
                    right: unit * 2.0,
                    bottom: unit * 1.25,
                },
                Insets {
                    left: unit * 1.5,
                    top: unit,
                    right: unit * 1.5,
                    bottom: unit,
                },
                14.0,
                14.0,
                26.0,
                28.0,
                16.0,
                2.0,
                140.0,
                Insets {
                    left: unit * 2.0,
                    top: unit,
                    right: unit * 2.0,
                    bottom: unit,
                },
                3.0,
                14.0,
                24.0,
                180.0,
                Insets {
                    left: unit * 2.0,
                    top: unit * 1.25,
                    right: unit * 2.0,
                    bottom: unit * 1.25,
                },
                80.0,
                200.0,
                32.0,
                96.0,
                Insets {
                    left: unit * 2.5,
                    top: unit,
                    right: unit * 2.5,
                    bottom: unit,
                },
                Insets::all(unit * 4.0),
                unit * 3.0,
                28.0,
                Insets::all(unit * 1.5),
                Insets {
                    left: unit * 3.0,
                    top: unit,
                    right: unit * 3.0,
                    bottom: unit,
                },
                Insets::all(unit * 3.5),
                280.0,
                104.0,
                Insets {
                    left: unit * 4.0,
                    top: unit * 3.5,
                    right: unit * 3.5,
                    bottom: unit * 3.5,
                },
                38.0,
                20.0,
                unit * 3.0,
                unit * 1.25,
                22.0,
                16.0,
                3.0,
                unit * 2.5,
                28.0,
                unit * 2.5,
                86.0,
                unit * 1.5,
                240.0,
                18.0,
                18.0,
                Insets::all(unit * 0.5),
                Insets::all(unit * 2.0),
                Insets {
                    left: 14.0,
                    top: unit,
                    right: 10.0,
                    bottom: unit,
                },
                14.0,
                unit * 2.0,
                unit * 3.0,
                28.0,
                46.0,
                26.0,
                34.0,
                30.0,
                18.0,
                12.0,
                6.0,
                28.0,
                30.0,
                unit * 2.0,
                28.0,
                Insets {
                    left: unit * 2.0,
                    top: unit,
                    right: unit * 2.0,
                    bottom: unit,
                },
                unit * 5.0,
                10.0,
            ),
            ThemeDensity::Touch => (
                44.0,
                44.0,
                Insets {
                    left: unit * 3.0,
                    top: unit * 2.0,
                    right: unit * 3.0,
                    bottom: unit * 2.0,
                },
                Insets {
                    left: unit * 2.5,
                    top: unit * 2.0,
                    right: unit * 2.5,
                    bottom: unit * 2.0,
                },
                18.0,
                18.0,
                44.0,
                42.0,
                24.0,
                3.0,
                180.0,
                Insets {
                    left: unit * 3.0,
                    top: unit * 2.0,
                    right: unit * 3.0,
                    bottom: unit * 2.0,
                },
                4.0,
                22.0,
                36.0,
                220.0,
                Insets {
                    left: unit * 3.0,
                    top: unit * 2.0,
                    right: unit * 3.0,
                    bottom: unit * 2.0,
                },
                112.0,
                260.0,
                44.0,
                112.0,
                Insets {
                    left: unit * 3.5,
                    top: unit * 2.0,
                    right: unit * 3.5,
                    bottom: unit * 2.0,
                },
                Insets::all(unit * 5.0),
                unit * 4.0,
                44.0,
                Insets::all(unit * 2.0),
                Insets {
                    left: unit * 3.5,
                    top: unit * 2.0,
                    right: unit * 3.5,
                    bottom: unit * 2.0,
                },
                Insets::all(unit * 4.5),
                320.0,
                128.0,
                Insets {
                    left: unit * 5.0,
                    top: unit * 4.5,
                    right: unit * 4.5,
                    bottom: unit * 4.5,
                },
                48.0,
                24.0,
                unit * 3.5,
                unit * 1.5,
                28.0,
                20.0,
                4.0,
                unit * 3.0,
                44.0,
                unit * 3.0,
                104.0,
                unit * 2.0,
                280.0,
                24.0,
                44.0,
                Insets::all(unit),
                Insets::all(unit * 2.5),
                Insets {
                    left: unit * 4.0,
                    top: unit * 2.0,
                    right: unit * 3.0,
                    bottom: unit * 2.0,
                },
                18.0,
                unit * 2.5,
                unit * 4.0,
                44.0,
                56.0,
                40.0,
                42.0,
                44.0,
                24.0,
                16.0,
                unit * 1.5,
                44.0,
                46.0,
                unit * 3.0,
                44.0,
                Insets {
                    left: unit * 3.0,
                    top: unit * 2.0,
                    right: unit * 3.0,
                    bottom: unit * 2.0,
                },
                unit * 6.0,
                12.0,
            ),
        };

        let (
            popover_reveal_offset,
            tooltip_padding,
            tooltip_min_width,
            tooltip_gap,
            tooltip_reveal_offset,
            dialog_min_width,
            dialog_max_width,
            dialog_outer_margin,
            dialog_padding,
            dialog_title_font_size,
            dialog_title_line_height,
            dialog_description_gap,
            dialog_body_gap,
            dialog_footer_gap,
            dialog_action_gap,
            dialog_action_min_width,
        ) = match density {
            ThemeDensity::Compact => (
                8.0,
                Insets {
                    left: unit * 2.0,
                    top: unit * 1.5,
                    right: unit * 2.0,
                    bottom: unit * 1.5,
                },
                80.0,
                unit * 2.0,
                6.0,
                240.0,
                460.0,
                unit * 4.0,
                Insets::all(unit * 3.5),
                18.0,
                22.0,
                unit * 1.5,
                unit * 3.0,
                unit * 3.5,
                unit * 2.0,
                92.0,
            ),
            ThemeDensity::Comfortable => (
                10.0,
                Insets {
                    left: unit * 2.25,
                    top: unit * 2.25,
                    right: unit * 2.25,
                    bottom: unit * 2.25,
                },
                96.0,
                unit * 2.5,
                8.0,
                280.0,
                520.0,
                unit * 6.0,
                Insets::all(18.0),
                20.0,
                24.0,
                unit * 2.0,
                14.0,
                18.0,
                unit * 2.5,
                110.0,
            ),
            ThemeDensity::Touch => (
                12.0,
                Insets {
                    left: unit * 3.0,
                    top: unit * 2.5,
                    right: unit * 3.0,
                    bottom: unit * 2.5,
                },
                112.0,
                unit * 3.0,
                10.0,
                320.0,
                600.0,
                unit * 8.0,
                Insets::all(unit * 6.0),
                22.0,
                28.0,
                unit * 2.5,
                unit * 4.5,
                unit * 5.0,
                unit * 3.0,
                128.0,
            ),
        };

        let (
            layer_action_icon_inset,
            layer_lock_icon_inset,
            layer_visibility_stroke_width,
            layer_visibility_slash_stroke_width,
            layer_thumbnail_inset,
            layer_thumbnail_radius,
            layer_thumbnail_disabled_opacity,
            layer_thumbnail_disabled_border_opacity,
        ) = match density {
            ThemeDensity::Compact => (4.5, 3.5, 1.25, 1.45, 1.5, radius.md, 0.34, 0.52),
            ThemeDensity::Comfortable => (5.0, 4.0, 1.4, 1.6, 2.0, radius.md, 0.36, 0.55),
            ThemeDensity::Touch => (7.0, 6.0, 1.8, 2.0, 3.0, radius.lg, 0.40, 0.60),
        };

        let (
            data_scroll_thumb_width,
            data_scroll_thumb_inset,
            data_scroll_thumb_radius,
            data_scroll_thumb_min_length,
            data_scroll_thumb_opacity,
            table_header_separator_inset,
            table_separator_width,
            table_row_border_opacity,
        ) = match density {
            ThemeDensity::Compact => (3.0, 5.0, radius.sm, 24.0, 0.68, 3.0, 1.0, 0.50),
            ThemeDensity::Comfortable => (4.0, 6.0, radius.sm, 28.0, 0.75, 4.0, 1.0, 0.55),
            ThemeDensity::Touch => (6.0, 8.0, radius.md, 44.0, 0.78, 8.0, 1.5, 0.60),
        };

        let (
            toolbar_extent,
            toolbar_padding,
            toolbar_spacing,
            command_group_padding,
            command_group_spacing,
            command_group_radius,
            tool_palette_item_size,
            tool_palette_icon_size,
            preset_strip_item_height,
            preset_strip_item_min_width,
            preset_strip_item_padding,
            preset_strip_gap,
            preset_strip_label_padding,
        ) = match density {
            ThemeDensity::Compact => (
                40.0,
                Insets::all(unit * 1.5),
                unit * 1.5,
                Insets::all(unit * 0.25),
                unit * 0.5,
                radius.md,
                30.0,
                16.0,
                24.0,
                36.0,
                Insets {
                    left: unit * 2.0,
                    top: unit,
                    right: unit * 2.0,
                    bottom: unit,
                },
                unit,
                Insets::all(unit * 0.75),
            ),
            ThemeDensity::Comfortable => (
                52.0,
                Insets::all(unit * 2.0),
                unit * 2.0,
                Insets::all(unit * 0.5),
                unit * 0.75,
                radius.lg,
                40.0,
                20.0,
                28.0,
                44.0,
                Insets {
                    left: unit * 3.0,
                    top: unit,
                    right: unit * 3.0,
                    bottom: unit,
                },
                unit * 1.5,
                Insets::all(unit),
            ),
            ThemeDensity::Touch => (
                64.0,
                Insets::all(unit * 2.5),
                unit * 2.5,
                Insets::all(unit),
                unit,
                radius.xl,
                48.0,
                24.0,
                44.0,
                56.0,
                Insets {
                    left: unit * 4.0,
                    top: unit * 2.0,
                    right: unit * 4.0,
                    bottom: unit * 2.0,
                },
                unit * 2.0,
                Insets::all(unit * 2.0),
            ),
        };

        let (
            property_row_label_width,
            property_row_inline_gap,
            property_row_stacked_gap,
            form_row_label_width,
            form_row_control_width,
            form_row_gap,
            field_group_spacing,
            form_section_padding,
            form_section_body_gap,
            form_section_header_gap,
            form_section_description_gap,
            form_section_max_width,
            form_section_radius,
            panel_section_gap,
            panel_section_action_gap,
            panel_section_disclosure_size,
            dock_panel_header_height,
            dock_panel_padding,
        ) = match density {
            ThemeDensity::Compact => (
                96.0,
                unit * 1.5,
                unit,
                112.0,
                300.0,
                unit * 2.0,
                unit * 1.5,
                Insets {
                    left: unit * 2.5,
                    top: unit * 2.0,
                    right: unit * 2.5,
                    bottom: unit * 2.5,
                },
                unit * 2.0,
                unit * 2.0,
                unit * 0.5,
                600.0,
                radius.md,
                unit * 1.5,
                unit,
                14.0,
                28.0,
                Insets {
                    left: unit * 2.0,
                    top: unit * 1.5,
                    right: unit * 2.0,
                    bottom: unit * 1.5,
                },
            ),
            ThemeDensity::Comfortable => (
                112.0,
                unit * 2.0,
                unit * 1.5,
                128.0,
                340.0,
                unit * 3.0,
                unit * 2.0,
                Insets {
                    left: 14.0,
                    top: unit * 3.0,
                    right: 14.0,
                    bottom: 14.0,
                },
                unit * 3.0,
                unit * 2.5,
                unit * 0.75,
                640.0,
                radius.lg,
                unit * 2.0,
                unit * 1.5,
                16.0,
                34.0,
                Insets {
                    left: unit * 2.5,
                    top: unit * 2.0,
                    right: unit * 2.5,
                    bottom: unit * 2.0,
                },
            ),
            ThemeDensity::Touch => (
                136.0,
                unit * 3.0,
                unit * 2.0,
                144.0,
                380.0,
                unit * 4.0,
                unit * 3.0,
                Insets {
                    left: unit * 4.5,
                    top: unit * 4.0,
                    right: unit * 4.5,
                    bottom: unit * 4.5,
                },
                unit * 4.0,
                unit * 3.0,
                unit,
                720.0,
                radius.xl,
                unit * 3.0,
                unit * 2.0,
                20.0,
                44.0,
                Insets {
                    left: unit * 3.5,
                    top: unit * 3.0,
                    right: unit * 3.5,
                    bottom: unit * 3.0,
                },
            ),
        };

        let (
            image_corner_radius,
            color_swatch_width,
            color_swatch_height,
            color_swatch_inner_inset,
            color_swatch_checker_size,
            color_palette_swatch_size,
            color_palette_gap,
            color_palette_swatch_inset,
            color_palette_selected_swatch_inset,
            color_palette_checker_size,
            brush_preview_min_width,
            brush_preview_min_height,
            brush_preview_padding,
            brush_preview_swatch_width,
            brush_preview_swatch_gap,
            brush_preview_checker_size,
            brush_preview_text_height,
            brush_preview_text_font_size,
            brush_preview_text_line_height,
            color_picker_content_inset,
            color_picker_panel_gap,
            color_picker_top_bar_height,
            color_picker_swatch_width,
            color_picker_swatch_gap,
            color_picker_section_gap,
            color_picker_wheel_size,
            color_picker_map_size,
            color_picker_row_height,
            color_picker_row_gap,
            color_picker_right_panel_width,
            color_picker_field_height,
            color_picker_field_gap,
            color_picker_dropdown_gap,
            color_picker_encoding_menu_row_height,
            scroll_bar_thickness,
            scroll_bar_min_thumb_length,
            split_view_divider_thickness,
            split_view_drag_target_thickness,
            floating_workspace_margin,
            floating_view_title_bar_height,
            floating_view_title_padding,
            floating_view_resize_handle_size,
            canvas_ruler_extent,
            canvas_ruler_major_tick,
            canvas_ruler_minor_tick,
            canvas_ruler_target_major_spacing,
            canvas_ruler_label_padding,
            canvas_ruler_label_max_width,
            canvas_grid_step,
            canvas_axis_overscan,
            pixel_canvas_fit_padding,
            pixel_canvas_grid_zoom,
            pixel_canvas_nearest_sampling_zoom,
            pixel_canvas_zoom_step,
        ) = match density {
            ThemeDensity::Compact => (
                radius.md,
                48.0,
                28.0,
                1.0,
                5.0,
                24.0,
                unit * 1.25,
                2.0,
                3.0,
                5.0,
                220.0,
                58.0,
                Insets::all(unit * 1.5),
                46.0,
                unit * 2.0,
                5.0,
                15.0,
                10.0,
                13.0,
                unit * 3.0,
                unit * 2.5,
                40.0,
                64.0,
                unit * 2.0,
                14.0,
                128.0,
                132.0,
                24.0,
                unit * 2.0,
                150.0,
                28.0,
                12.0,
                unit,
                28.0,
                10.0,
                24.0,
                1.0,
                10.0,
                unit * 2.0,
                30.0,
                Insets {
                    left: unit * 2.5,
                    top: unit * 1.5,
                    right: unit * 2.5,
                    bottom: unit * 1.5,
                },
                16.0,
                20.0,
                9.0,
                4.0,
                84.0,
                Insets {
                    left: unit * 0.5,
                    top: unit * 0.5,
                    right: unit * 0.5,
                    bottom: unit * 0.5,
                },
                48.0,
                32.0,
                72.0,
                20.0,
                6.0,
                1.0,
                1.1,
            ),
            ThemeDensity::Comfortable => (
                radius.lg,
                56.0,
                32.0,
                1.0,
                6.0,
                28.0,
                unit * 1.5,
                2.0,
                3.0,
                5.0,
                260.0,
                70.0,
                Insets::all(unit * 2.0),
                54.0,
                unit * 2.5,
                6.0,
                16.0,
                11.0,
                14.0,
                14.0,
                14.0,
                52.0,
                96.0,
                unit * 2.5,
                14.0,
                166.0,
                210.0,
                24.0,
                unit * 2.0,
                226.0,
                30.0,
                12.0,
                unit,
                28.0,
                12.0,
                28.0,
                1.0,
                12.0,
                unit * 3.0,
                32.0,
                Insets {
                    left: 14.0,
                    top: unit * 2.0,
                    right: 14.0,
                    bottom: unit * 2.0,
                },
                18.0,
                22.0,
                10.0,
                5.0,
                96.0,
                Insets {
                    left: unit * 0.75,
                    top: unit * 0.5,
                    right: unit * 0.75,
                    bottom: unit * 0.5,
                },
                54.0,
                40.0,
                80.0,
                24.0,
                6.0,
                1.0,
                1.1,
            ),
            ThemeDensity::Touch => (
                radius.xl,
                72.0,
                44.0,
                1.5,
                8.0,
                40.0,
                unit * 2.0,
                3.0,
                4.0,
                7.0,
                320.0,
                88.0,
                Insets::all(unit * 3.0),
                72.0,
                unit * 3.5,
                8.0,
                18.0,
                12.0,
                16.0,
                unit * 4.5,
                unit * 4.5,
                64.0,
                112.0,
                unit * 3.0,
                18.0,
                210.0,
                240.0,
                44.0,
                unit * 3.0,
                280.0,
                44.0,
                16.0,
                unit * 2.0,
                44.0,
                18.0,
                44.0,
                2.0,
                44.0,
                unit * 4.5,
                52.0,
                Insets {
                    left: unit * 4.5,
                    top: unit * 3.5,
                    right: unit * 4.5,
                    bottom: unit * 3.5,
                },
                28.0,
                32.0,
                16.0,
                8.0,
                120.0,
                Insets::all(unit),
                72.0,
                48.0,
                96.0,
                32.0,
                6.0,
                1.0,
                1.1,
            ),
        };

        Self {
            min_height,
            touch_target_size,
            button_min_width: 64.0,
            button_padding,
            checkbox_padding,
            checkbox_indicator_size,
            checkbox_gap: 6.0,
            icon_label_gap: 6.0,
            separator_thickness: 1.0,
            icon_size,
            icon_button_size,
            switch_track_width,
            switch_track_height,
            switch_thumb_inset,
            slider_min_width,
            slider_padding,
            slider_track_height,
            slider_thumb_size,
            number_input_stepper_width,
            text_input_min_width,
            text_input_padding,
            text_area_min_height,
            select_menu_max_height,
            select_menu_gap: 6.0,
            select_menu_edge_padding: 8.0,
            tab_height,
            tab_min_width,
            tab_gap: 6.0,
            tab_padding,
            tab_panel_padding,
            tab_panel_gap,
            menu_row_height,
            menu_padding,
            menu_item_padding,
            menu_shortcut_width: 108.0,
            popover_padding,
            popover_gap: unit * 2.0,
            popover_reveal_offset,
            tooltip_padding,
            tooltip_min_width,
            tooltip_gap,
            tooltip_reveal_offset,
            dialog_min_width,
            dialog_max_width,
            dialog_outer_margin,
            dialog_padding,
            dialog_title_font_size,
            dialog_title_line_height,
            dialog_description_gap,
            dialog_body_gap,
            dialog_footer_gap,
            dialog_action_gap,
            dialog_action_min_width,
            toolbar_extent,
            toolbar_padding,
            toolbar_spacing,
            command_group_padding,
            command_group_spacing,
            command_group_radius,
            tool_palette_item_size,
            tool_palette_icon_size,
            preset_strip_item_height,
            preset_strip_item_min_width,
            preset_strip_item_padding,
            preset_strip_gap,
            preset_strip_label_padding,
            action_card_min_width,
            action_card_min_height,
            action_card_padding,
            action_card_icon_box_size,
            action_card_icon_size,
            action_card_icon_gap,
            action_card_text_gap,
            action_card_trailing_gap,
            action_card_chevron_size,
            action_card_accent_width,
            action_card_accent_inset,
            status_bar_height,
            status_bar_segment_padding,
            status_bar_segment_min_width,
            status_bar_separator_inset,
            progress_bar_min_width,
            progress_bar_height,
            progress_bar_value_height,
            progress_bar_label_padding,
            property_row_label_width,
            property_row_inline_gap,
            property_row_stacked_gap,
            form_row_label_width,
            form_row_control_width,
            form_row_gap,
            field_group_spacing,
            form_section_padding,
            form_section_body_gap,
            form_section_header_gap,
            form_section_description_gap,
            form_section_max_width,
            form_section_radius,
            panel_section_gap,
            panel_section_action_gap,
            panel_section_disclosure_size,
            dock_panel_header_height,
            dock_panel_padding,
            data_viewport_padding,
            data_row_padding,
            data_row_icon_size,
            data_row_icon_gap,
            data_row_trailing_gap,
            data_scroll_thumb_width,
            data_scroll_thumb_inset,
            data_scroll_thumb_radius,
            data_scroll_thumb_min_length,
            data_scroll_thumb_opacity,
            list_row_height,
            layer_row_height,
            layer_action_size,
            layer_action_icon_inset,
            layer_lock_icon_inset,
            layer_visibility_stroke_width,
            layer_visibility_slash_stroke_width,
            layer_thumbnail_size,
            layer_thumbnail_inset,
            layer_thumbnail_radius,
            layer_thumbnail_disabled_opacity,
            layer_thumbnail_disabled_border_opacity,
            tree_row_height,
            tree_indent,
            tree_disclosure_size,
            tree_disclosure_gap,
            table_row_height,
            table_header_height,
            table_cell_padding,
            table_header_separator_inset,
            table_separator_width,
            table_row_border_opacity,
            breadcrumb_height,
            breadcrumb_item_padding,
            breadcrumb_gap,
            breadcrumb_separator_size,
            image_corner_radius,
            color_swatch_width,
            color_swatch_height,
            color_swatch_inner_inset,
            color_swatch_checker_size,
            color_palette_swatch_size,
            color_palette_gap,
            color_palette_swatch_inset,
            color_palette_selected_swatch_inset,
            color_palette_checker_size,
            brush_preview_min_width,
            brush_preview_min_height,
            brush_preview_padding,
            brush_preview_swatch_width,
            brush_preview_swatch_gap,
            brush_preview_checker_size,
            brush_preview_text_height,
            brush_preview_text_font_size,
            brush_preview_text_line_height,
            color_picker_content_inset,
            color_picker_panel_gap,
            color_picker_top_bar_height,
            color_picker_swatch_width,
            color_picker_swatch_gap,
            color_picker_section_gap,
            color_picker_wheel_size,
            color_picker_map_size,
            color_picker_row_height,
            color_picker_row_gap,
            color_picker_right_panel_width,
            color_picker_field_height,
            color_picker_field_gap,
            color_picker_dropdown_gap,
            color_picker_encoding_menu_row_height,
            scroll_bar_thickness,
            scroll_bar_min_thumb_length,
            split_view_divider_thickness,
            split_view_drag_target_thickness,
            floating_workspace_margin,
            floating_view_title_bar_height,
            floating_view_title_padding,
            floating_view_resize_handle_size,
            canvas_ruler_extent,
            canvas_ruler_major_tick,
            canvas_ruler_minor_tick,
            canvas_ruler_target_major_spacing,
            canvas_ruler_label_padding,
            canvas_ruler_label_max_width,
            canvas_grid_step,
            canvas_axis_overscan,
            pixel_canvas_fit_padding,
            pixel_canvas_grid_zoom,
            pixel_canvas_nearest_sampling_zoom,
            pixel_canvas_zoom_step,
            corner_radius: radius.md,
            indicator_corner_radius: radius.sm + 1.0,
            border_width: 1.0,
            focus_ring_width: 2.0,
            focus_ring_outset: 2.0,
            caret_width: 2.0,
        }
    }
}

impl Default for ControlMetrics {
    fn default() -> Self {
        Self::from_tokens(4.0, ThemeRadii::default(), ThemeDensity::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultTheme {
    pub fonts: ThemeFontFamilies,
    pub colors: ThemeColors,
    pub density: ThemeDensity,
    pub spacing: f32,
    pub breakpoints: ThemeBreakpoints,
    pub containers: ThemeContainers,
    pub text: ThemeTextScale,
    pub font_weights: ThemeFontWeights,
    pub tracking: ThemeTracking,
    pub leading: ThemeLeading,
    pub radius: ThemeRadii,
    pub shadows: ThemeShadows,
    pub blur: ThemeBlurScale,
    pub perspective: ThemePerspective,
    pub aspect: ThemeAspectRatios,
    pub motion: ThemeMotion,
    pub hdr: HdrThemeTokens,
    pub palette: ControlPalette,
    pub surfaces: SurfacePalette,
    pub typography: ControlTypography,
    pub interaction: ControlStateMetrics,
    pub metrics: ControlMetrics,
}

impl DefaultTheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn light() -> Self {
        Self::from_colors(ThemeColors::light())
    }

    pub fn dark() -> Self {
        Self::from_colors(ThemeColors::dark())
    }

    pub fn high_contrast() -> Self {
        Self::from_colors(ThemeColors::high_contrast())
    }

    pub fn compact() -> Self {
        Self::default().with_density(ThemeDensity::Compact)
    }

    pub fn comfortable() -> Self {
        Self::default().with_density(ThemeDensity::Comfortable)
    }

    pub fn touch() -> Self {
        Self::default().with_density(ThemeDensity::Touch)
    }

    pub fn from_colors(colors: ThemeColors) -> Self {
        let text = ThemeTextScale::default();
        let radius = ThemeRadii::default();
        let spacing = 4.0;
        let density = ThemeDensity::Comfortable;
        let hdr = HdrThemeTokens::from_colors(colors);
        let palette = ControlPalette::from_colors(&colors);
        let surfaces = SurfacePalette::from_theme_parts(&colors, &palette);

        let mut theme = Self {
            fonts: ThemeFontFamilies::default(),
            colors,
            density,
            spacing,
            breakpoints: ThemeBreakpoints::default(),
            containers: ThemeContainers::default(),
            text,
            font_weights: ThemeFontWeights::default(),
            tracking: ThemeTracking::default(),
            leading: ThemeLeading::default(),
            radius,
            shadows: ThemeShadows::default(),
            blur: ThemeBlurScale::default(),
            perspective: ThemePerspective::default(),
            aspect: ThemeAspectRatios::default(),
            motion: ThemeMotion::default(),
            hdr,
            palette,
            surfaces,
            typography: ControlTypography::from_text_scale(&text),
            interaction: ControlStateMetrics::for_density(density),
            metrics: ControlMetrics::from_tokens(spacing, radius, density),
        };
        theme.apply_scheme_overrides();
        theme
    }

    pub fn with_density(mut self, density: ThemeDensity) -> Self {
        self.density = density;
        self.sync_derived_fields();
        self
    }

    pub fn sync_derived_fields(&mut self) {
        self.hdr.sync_semantic_defaults(self.colors);
        self.palette = ControlPalette::from_colors(&self.colors);
        self.surfaces = SurfacePalette::from_theme_parts(&self.colors, &self.palette);
        self.typography = ControlTypography::from_text_scale(&self.text);
        self.interaction = ControlStateMetrics::for_density(self.density);
        self.metrics = ControlMetrics::from_tokens(self.spacing, self.radius, self.density);
        self.apply_scheme_overrides();
    }

    fn apply_scheme_overrides(&mut self) {
        if self.colors.scheme == ThemeColorScheme::HighContrast {
            self.metrics.border_width = 1.5;
            self.metrics.focus_ring_width = 2.5;
            self.metrics.focus_ring_outset = 2.0;
        }
    }

    pub fn text_style(&self, color: Color) -> TextStyle {
        TextStyle {
            font_size: self.typography.body_font_size.max(1.0),
            line_height: self.typography.body_line_height.max(1.0),
            color,
            ..TextStyle::default()
        }
    }

    pub fn semantic_tone_colors(&self, tone: SemanticTone) -> (Color, Color) {
        match tone {
            SemanticTone::Neutral => (self.palette.control, self.palette.text),
            SemanticTone::Accent => (self.palette.accent, self.palette.accent_text),
            SemanticTone::Info => (self.palette.info, self.palette.info_text),
            SemanticTone::Success => (self.palette.success, self.palette.success_text),
            SemanticTone::Warning => (self.palette.warning, self.palette.warning_text),
            SemanticTone::Danger => (self.palette.danger, self.palette.danger_text),
        }
    }

    pub fn semantic_tone_color(&self, tone: SemanticTone) -> Color {
        self.semantic_tone_colors(tone).0
    }

    pub fn semantic_tone_text_color(&self, tone: SemanticTone) -> Color {
        self.semantic_tone_colors(tone).1
    }

    pub fn body_text_style(&self) -> TextStyle {
        self.text_style(self.palette.text)
    }

    pub fn placeholder_text_style(&self) -> TextStyle {
        self.text_style(self.palette.placeholder)
    }

    pub fn button_text_style(&self) -> TextStyle {
        self.text_style(self.palette.accent_text)
    }
}

impl Default for DefaultTheme {
    fn default() -> Self {
        Self::light()
    }
}

fn mix(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);

    Color::new(
        from.space,
        from.red + (to.red - from.red) * amount,
        from.green + (to.green - from.green) * amount,
        from.blue + (to.blue - from.blue) * amount,
        from.alpha + (to.alpha - from.alpha) * amount,
    )
    .clamped()
}

fn interactive_variant(color: Color, scheme: ThemeColorScheme, amount: f32) -> Color {
    match scheme {
        ThemeColorScheme::Light => mix(color, Color::BLACK, amount),
        ThemeColorScheme::Dark | ThemeColorScheme::HighContrast => mix(color, Color::WHITE, amount),
    }
}

fn interactive_surface(color: Color, scheme: ThemeColorScheme, amount: f32) -> Color {
    match scheme {
        ThemeColorScheme::Light => mix(color, Color::BLACK, amount),
        ThemeColorScheme::Dark | ThemeColorScheme::HighContrast => mix(color, Color::WHITE, amount),
    }
}

fn shadow_layer(
    offset_x: f32,
    offset_y: f32,
    blur: f32,
    spread: f32,
    color: Color,
    inset: bool,
) -> ThemeShadowLayer {
    ThemeShadowLayer {
        offset_x,
        offset_y,
        blur,
        spread,
        color,
        inset,
    }
}

#[cfg(test)]
mod tests {
    use super::{Color, DefaultTheme, SemanticTone, ThemeColorScheme, ThemeDensity};
    use crate::hdr_theme::HdrThemeMode;

    #[test]
    fn default_theme_uses_body_text_scale_for_typography() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.typography.body_font_size, theme.text.sm.size);
        assert_eq!(theme.typography.body_line_height, theme.text.sm.line_height);
        assert_eq!(theme.typography.body_font_size, 14.0);
        assert_eq!(theme.typography.body_line_height, 20.0);
        assert_eq!(theme.density, ThemeDensity::Comfortable);
        assert_eq!(theme.metrics.min_height, 24.0);
    }

    #[test]
    fn density_presets_update_control_metrics_and_interactions() {
        let compact = DefaultTheme::compact();
        let comfortable = DefaultTheme::comfortable();
        let touch = DefaultTheme::touch();

        assert_eq!(compact.density, ThemeDensity::Compact);
        assert_eq!(comfortable.density, ThemeDensity::Comfortable);
        assert_eq!(touch.density, ThemeDensity::Touch);
        assert!(compact.metrics.min_height < comfortable.metrics.min_height);
        assert!(comfortable.metrics.min_height < touch.metrics.min_height);
        assert!(compact.metrics.menu_row_height < comfortable.metrics.menu_row_height);
        assert!(comfortable.metrics.menu_row_height < touch.metrics.menu_row_height);
        assert!(compact.metrics.list_row_height < comfortable.metrics.list_row_height);
        assert!(comfortable.metrics.list_row_height < touch.metrics.list_row_height);
        assert!(compact.metrics.layer_row_height < comfortable.metrics.layer_row_height);
        assert!(comfortable.metrics.layer_row_height < touch.metrics.layer_row_height);
        assert!(
            compact.metrics.layer_action_icon_inset < comfortable.metrics.layer_action_icon_inset
        );
        assert!(
            comfortable.metrics.layer_action_icon_inset < touch.metrics.layer_action_icon_inset
        );
        assert!(
            compact.metrics.layer_visibility_stroke_width
                < comfortable.metrics.layer_visibility_stroke_width
        );
        assert!(
            comfortable.metrics.layer_visibility_stroke_width
                < touch.metrics.layer_visibility_stroke_width
        );
        assert!(compact.metrics.layer_thumbnail_inset < comfortable.metrics.layer_thumbnail_inset);
        assert!(comfortable.metrics.layer_thumbnail_inset < touch.metrics.layer_thumbnail_inset);
        assert!(
            compact.metrics.layer_thumbnail_radius <= comfortable.metrics.layer_thumbnail_radius
        );
        assert!(comfortable.metrics.layer_thumbnail_radius < touch.metrics.layer_thumbnail_radius);
        assert!(compact.metrics.table_row_height < comfortable.metrics.table_row_height);
        assert!(comfortable.metrics.table_row_height < touch.metrics.table_row_height);
        assert!(
            compact.metrics.data_scroll_thumb_width < comfortable.metrics.data_scroll_thumb_width
        );
        assert!(
            comfortable.metrics.data_scroll_thumb_width < touch.metrics.data_scroll_thumb_width
        );
        assert!(
            compact.metrics.data_scroll_thumb_min_length
                < comfortable.metrics.data_scroll_thumb_min_length
        );
        assert!(
            comfortable.metrics.data_scroll_thumb_min_length
                < touch.metrics.data_scroll_thumb_min_length
        );
        assert!(
            compact.metrics.table_header_separator_inset
                < comfortable.metrics.table_header_separator_inset
        );
        assert!(
            comfortable.metrics.table_header_separator_inset
                < touch.metrics.table_header_separator_inset
        );
        assert!(compact.metrics.breadcrumb_height < comfortable.metrics.breadcrumb_height);
        assert!(comfortable.metrics.breadcrumb_height < touch.metrics.breadcrumb_height);
        assert!(
            compact.metrics.action_card_min_height < comfortable.metrics.action_card_min_height
        );
        assert!(comfortable.metrics.action_card_min_height < touch.metrics.action_card_min_height);
        assert!(compact.metrics.status_bar_height < comfortable.metrics.status_bar_height);
        assert!(comfortable.metrics.status_bar_height < touch.metrics.status_bar_height);
        assert!(compact.metrics.progress_bar_height < comfortable.metrics.progress_bar_height);
        assert!(comfortable.metrics.progress_bar_height < touch.metrics.progress_bar_height);
        assert!(compact.metrics.tooltip_gap < comfortable.metrics.tooltip_gap);
        assert!(comfortable.metrics.tooltip_gap < touch.metrics.tooltip_gap);
        assert!(compact.metrics.tooltip_min_width < comfortable.metrics.tooltip_min_width);
        assert!(comfortable.metrics.tooltip_min_width < touch.metrics.tooltip_min_width);
        assert!(compact.metrics.popover_reveal_offset < comfortable.metrics.popover_reveal_offset);
        assert!(comfortable.metrics.popover_reveal_offset < touch.metrics.popover_reveal_offset);
        assert!(compact.metrics.dialog_max_width < comfortable.metrics.dialog_max_width);
        assert!(comfortable.metrics.dialog_max_width < touch.metrics.dialog_max_width);
        assert!(
            compact.metrics.dialog_action_min_width < comfortable.metrics.dialog_action_min_width
        );
        assert!(
            comfortable.metrics.dialog_action_min_width < touch.metrics.dialog_action_min_width
        );
        assert!(compact.metrics.toolbar_extent < comfortable.metrics.toolbar_extent);
        assert!(comfortable.metrics.toolbar_extent < touch.metrics.toolbar_extent);
        assert!(
            compact.metrics.tool_palette_item_size < comfortable.metrics.tool_palette_item_size
        );
        assert!(comfortable.metrics.tool_palette_item_size < touch.metrics.tool_palette_item_size);
        assert!(
            compact.metrics.preset_strip_item_height < comfortable.metrics.preset_strip_item_height
        );
        assert!(
            comfortable.metrics.preset_strip_item_height < touch.metrics.preset_strip_item_height
        );
        assert!(
            compact.metrics.property_row_label_width < comfortable.metrics.property_row_label_width
        );
        assert!(
            comfortable.metrics.property_row_label_width < touch.metrics.property_row_label_width
        );
        assert!(compact.metrics.form_row_gap < comfortable.metrics.form_row_gap);
        assert!(comfortable.metrics.form_row_gap < touch.metrics.form_row_gap);
        assert!(compact.metrics.field_group_spacing < comfortable.metrics.field_group_spacing);
        assert!(comfortable.metrics.field_group_spacing < touch.metrics.field_group_spacing);
        assert!(
            compact.metrics.form_section_max_width < comfortable.metrics.form_section_max_width
        );
        assert!(comfortable.metrics.form_section_max_width < touch.metrics.form_section_max_width);
        assert!(compact.metrics.panel_section_gap < comfortable.metrics.panel_section_gap);
        assert!(comfortable.metrics.panel_section_gap < touch.metrics.panel_section_gap);
        assert!(
            compact.metrics.dock_panel_header_height < comfortable.metrics.dock_panel_header_height
        );
        assert!(
            comfortable.metrics.dock_panel_header_height < touch.metrics.dock_panel_header_height
        );
        assert!(compact.metrics.tab_height < comfortable.metrics.tab_height);
        assert!(comfortable.metrics.tab_height < touch.metrics.tab_height);
        assert!(compact.metrics.scroll_bar_thickness < comfortable.metrics.scroll_bar_thickness);
        assert!(comfortable.metrics.scroll_bar_thickness < touch.metrics.scroll_bar_thickness);
        assert!(
            compact.metrics.scroll_bar_min_thumb_length
                < comfortable.metrics.scroll_bar_min_thumb_length
        );
        assert!(
            comfortable.metrics.scroll_bar_min_thumb_length
                < touch.metrics.scroll_bar_min_thumb_length
        );
        assert!(
            compact.metrics.split_view_drag_target_thickness
                < comfortable.metrics.split_view_drag_target_thickness
        );
        assert!(
            comfortable.metrics.split_view_drag_target_thickness
                < touch.metrics.split_view_drag_target_thickness
        );
        assert!(
            compact.metrics.floating_workspace_margin
                < comfortable.metrics.floating_workspace_margin
        );
        assert!(
            comfortable.metrics.floating_workspace_margin < touch.metrics.floating_workspace_margin
        );
        assert!(
            compact.metrics.floating_view_title_bar_height
                < comfortable.metrics.floating_view_title_bar_height
        );
        assert!(
            comfortable.metrics.floating_view_title_bar_height
                < touch.metrics.floating_view_title_bar_height
        );
        assert!(
            compact.metrics.floating_view_resize_handle_size
                < comfortable.metrics.floating_view_resize_handle_size
        );
        assert!(
            comfortable.metrics.floating_view_resize_handle_size
                < touch.metrics.floating_view_resize_handle_size
        );
        assert!(compact.metrics.canvas_ruler_extent < comfortable.metrics.canvas_ruler_extent);
        assert!(comfortable.metrics.canvas_ruler_extent < touch.metrics.canvas_ruler_extent);
        assert!(
            compact.metrics.canvas_ruler_label_max_width
                < comfortable.metrics.canvas_ruler_label_max_width
        );
        assert!(
            comfortable.metrics.canvas_ruler_label_max_width
                < touch.metrics.canvas_ruler_label_max_width
        );
        assert!(
            compact.metrics.canvas_ruler_target_major_spacing
                < comfortable.metrics.canvas_ruler_target_major_spacing
        );
        assert!(
            comfortable.metrics.canvas_ruler_target_major_spacing
                < touch.metrics.canvas_ruler_target_major_spacing
        );
        assert!(compact.metrics.canvas_grid_step < comfortable.metrics.canvas_grid_step);
        assert!(comfortable.metrics.canvas_grid_step < touch.metrics.canvas_grid_step);
        assert!(
            compact.metrics.pixel_canvas_fit_padding < comfortable.metrics.pixel_canvas_fit_padding
        );
        assert!(
            comfortable.metrics.pixel_canvas_fit_padding < touch.metrics.pixel_canvas_fit_padding
        );
        assert!(compact.metrics.icon_size < comfortable.metrics.icon_size);
        assert!(comfortable.metrics.icon_size < touch.metrics.icon_size);
        assert!(
            compact.interaction.tab_selected_blend < comfortable.interaction.tab_selected_blend
        );
        assert!(comfortable.interaction.tab_selected_blend < touch.interaction.tab_selected_blend);
        assert!(compact.interaction.pressed_offset < comfortable.interaction.pressed_offset);
        assert!(comfortable.interaction.pressed_offset < touch.interaction.pressed_offset);
    }

    #[test]
    fn default_theme_initializes_hdr_tokens() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.hdr.mode, HdrThemeMode::Disabled);
        assert_eq!(theme.hdr.color_roles.surface.sdr, theme.colors.base_100);
        assert_eq!(theme.hdr.color_roles.accent.sdr, theme.colors.primary);
        assert_eq!(
            theme.hdr.color_roles.accent_text.sdr,
            theme.colors.primary_content
        );
    }

    #[test]
    fn light_and_dark_themes_derive_hdr_role_colors_from_semantics() {
        let light = DefaultTheme::light();
        let dark = DefaultTheme::dark();

        assert_eq!(light.hdr.color_roles.surface.sdr, light.colors.base_100);
        assert_eq!(light.hdr.color_roles.text.sdr, light.colors.base_content);
        assert_eq!(dark.hdr.color_roles.surface.sdr, dark.colors.base_100);
        assert_eq!(dark.hdr.color_roles.accent.sdr, dark.colors.primary);
    }

    #[test]
    fn sync_derived_fields_updates_semantic_palette_and_typography() {
        let mut theme = DefaultTheme::default();
        theme.colors.primary = Color::rgba(0.2, 0.3, 0.4, 1.0);
        theme.text.sm.size = 11.0;
        theme.text.sm.line_height = 15.0;
        theme.density = ThemeDensity::Touch;
        theme.sync_derived_fields();

        assert_eq!(theme.palette.accent, Color::rgba(0.2, 0.3, 0.4, 1.0));
        assert_eq!(theme.palette.caret, Color::rgba(0.2, 0.3, 0.4, 1.0));
        assert_eq!(theme.surfaces.accent, theme.palette.accent);
        assert_eq!(theme.surfaces.window, theme.palette.surface);
        assert_eq!(theme.surfaces.tooltip_text, theme.palette.surface);
        assert!(theme.surfaces.overlay_scrim.alpha > 0.0);
        assert_eq!(theme.typography.body_font_size, 11.0);
        assert_eq!(theme.typography.body_line_height, 15.0);
        assert_eq!(
            theme.metrics.min_height,
            DefaultTheme::touch().metrics.min_height
        );
        assert_eq!(
            theme.interaction.pressed_offset,
            DefaultTheme::touch().interaction.pressed_offset
        );
    }

    #[test]
    fn control_palette_exposes_semantic_status_colors() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.palette.info, theme.colors.info);
        assert_eq!(theme.palette.info_text, theme.colors.info_content);
        assert_eq!(theme.palette.success, theme.colors.success);
        assert_eq!(theme.palette.success_text, theme.colors.success_content);
        assert_eq!(theme.palette.warning, theme.colors.warning);
        assert_eq!(theme.palette.warning_text, theme.colors.warning_content);
        assert_eq!(theme.palette.danger, theme.colors.error);
        assert_eq!(theme.palette.danger_text, theme.colors.error_content);
        assert_eq!(
            theme.semantic_tone_colors(SemanticTone::Warning),
            (theme.palette.warning, theme.palette.warning_text)
        );
        assert_eq!(
            theme.semantic_tone_color(SemanticTone::Danger),
            theme.palette.danger
        );
        assert_eq!(
            theme.semantic_tone_text_color(SemanticTone::Success),
            theme.palette.success_text
        );
    }

    #[test]
    fn sync_derived_fields_updates_hdr_semantic_fallbacks() {
        let mut theme = DefaultTheme::default();
        let preserved_wide_gamut = Color::display_p3(0.9, 0.4, 0.2, 1.0);
        let preserved_hdr = Color::linear_display_p3(1.6, 0.5, 0.3, 1.0);

        theme.hdr.color_roles.accent.wide_gamut = Some(preserved_wide_gamut);
        theme.hdr.color_roles.accent.hdr = Some(preserved_hdr);
        theme.colors.primary = Color::rgba(0.2, 0.3, 0.4, 1.0);
        theme.colors.base_100 = Color::rgba(0.96, 0.97, 0.98, 1.0);
        theme.sync_derived_fields();

        assert_eq!(theme.hdr.color_roles.surface.sdr, theme.colors.base_100);
        assert_eq!(theme.hdr.color_roles.accent.sdr, theme.colors.primary);
        assert_eq!(
            theme.hdr.color_roles.accent.wide_gamut,
            Some(preserved_wide_gamut)
        );
        assert_eq!(theme.hdr.color_roles.accent.hdr, Some(preserved_hdr));
    }

    #[test]
    fn dark_theme_uses_professional_dark_tokens() {
        let theme = DefaultTheme::dark();

        assert_eq!(theme.colors.scheme, ThemeColorScheme::Dark);
        assert_eq!(theme.colors.name, "dark");
        assert_ne!(theme.colors.base_100, Color::BLACK);
        assert_eq!(theme.palette.surface, theme.colors.base_100);
        assert_ne!(theme.palette.surface_raised, Color::BLACK);
        assert_eq!(theme.palette.text, theme.colors.base_content);
        assert_ne!(theme.palette.text, Color::WHITE);
        assert_eq!(theme.palette.caret, theme.colors.primary);
        assert_eq!(theme.palette.accent, theme.colors.primary);
        assert_eq!(theme.palette.accent_text, theme.colors.primary_content);
        assert_eq!(theme.surfaces.window, theme.palette.surface);
        assert_eq!(theme.surfaces.panel, theme.palette.surface_raised);
        assert_eq!(theme.surfaces.border, theme.palette.border);
        assert_ne!(theme.surfaces.border, Color::WHITE);
        assert_ne!(theme.surfaces.text_faint, theme.palette.surface);
    }

    #[test]
    fn high_contrast_theme_uses_dedicated_scheme_and_metrics() {
        let theme = DefaultTheme::high_contrast();

        assert_eq!(theme.colors.scheme, ThemeColorScheme::HighContrast);
        assert_eq!(theme.colors.name, "high-contrast");
        assert_eq!(theme.palette.surface, theme.colors.base_100);
        assert_eq!(theme.palette.surface, Color::BLACK);
        assert_eq!(theme.surfaces.window, Color::BLACK);
        assert_ne!(theme.palette.surface_raised, Color::BLACK);
        assert_ne!(theme.palette.control, Color::BLACK);
        assert_ne!(theme.palette.control_hover, Color::BLACK);
        assert_ne!(theme.palette.control_active, Color::BLACK);
        assert_ne!(theme.palette.surface_focus, Color::BLACK);
        assert_eq!(theme.palette.text, theme.colors.base_content);
        assert!(theme.metrics.border_width > DefaultTheme::default().metrics.border_width);
        assert!(theme.metrics.focus_ring_width > DefaultTheme::default().metrics.focus_ring_width);

        let touch = DefaultTheme::high_contrast().with_density(ThemeDensity::Touch);
        assert_eq!(touch.density, ThemeDensity::Touch);
        assert!(touch.metrics.min_height > theme.metrics.min_height);
        assert!(touch.metrics.border_width > DefaultTheme::touch().metrics.border_width);
        assert!(touch.metrics.focus_ring_width > DefaultTheme::touch().metrics.focus_ring_width);
    }
}
