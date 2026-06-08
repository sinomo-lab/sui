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
pub struct ControlMetrics {
    pub min_height: f32,
    pub button_min_width: f32,
    pub button_padding: Insets,
    pub checkbox_padding: Insets,
    pub checkbox_indicator_size: f32,
    pub checkbox_gap: f32,
    pub separator_thickness: f32,
    pub icon_size: f32,
    pub icon_button_size: f32,
    pub switch_track_width: f32,
    pub switch_track_height: f32,
    pub slider_min_width: f32,
    pub slider_track_height: f32,
    pub slider_thumb_size: f32,
    pub number_input_stepper_width: f32,
    pub text_input_min_width: f32,
    pub text_input_padding: Insets,
    pub text_area_min_height: f32,
    pub select_menu_max_height: f32,
    pub corner_radius: f32,
    pub indicator_corner_radius: f32,
    pub border_width: f32,
    pub focus_ring_width: f32,
    pub focus_ring_outset: f32,
    pub caret_width: f32,
}

impl ControlMetrics {
    pub fn from_tokens(spacing: f32, radius: ThemeRadii) -> Self {
        let unit = spacing.max(1.0);
        Self {
            min_height: 24.0,
            button_min_width: 64.0,
            button_padding: Insets {
                left: unit * 2.0,
                top: unit * 1.25,
                right: unit * 2.0,
                bottom: unit * 1.25,
            },
            checkbox_padding: Insets {
                left: unit * 1.5,
                top: unit,
                right: unit * 1.5,
                bottom: unit,
            },
            checkbox_indicator_size: 14.0,
            checkbox_gap: 6.0,
            separator_thickness: 1.0,
            icon_size: 14.0,
            icon_button_size: 26.0,
            switch_track_width: 28.0,
            switch_track_height: 16.0,
            slider_min_width: 140.0,
            slider_track_height: 3.0,
            slider_thumb_size: 14.0,
            number_input_stepper_width: 24.0,
            text_input_min_width: 180.0,
            text_input_padding: Insets {
                left: unit * 2.0,
                top: unit * 1.25,
                right: unit * 2.0,
                bottom: unit * 1.25,
            },
            text_area_min_height: 80.0,
            select_menu_max_height: 200.0,
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
        Self::from_tokens(4.0, ThemeRadii::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultTheme {
    pub fonts: ThemeFontFamilies,
    pub colors: ThemeColors,
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
        let mut theme = Self::from_colors(ThemeColors::high_contrast());
        theme.metrics.border_width = 1.5;
        theme.metrics.focus_ring_width = 2.5;
        theme.metrics.focus_ring_outset = 2.0;
        theme
    }

    pub fn from_colors(colors: ThemeColors) -> Self {
        let text = ThemeTextScale::default();
        let radius = ThemeRadii::default();
        let spacing = 4.0;
        let hdr = HdrThemeTokens::from_colors(colors);
        let palette = ControlPalette::from_colors(&colors);
        let surfaces = SurfacePalette::from_theme_parts(&colors, &palette);

        Self {
            fonts: ThemeFontFamilies::default(),
            colors,
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
            metrics: ControlMetrics::from_tokens(spacing, radius),
        }
    }

    pub fn sync_derived_fields(&mut self) {
        self.hdr.sync_semantic_defaults(self.colors);
        self.palette = ControlPalette::from_colors(&self.colors);
        self.surfaces = SurfacePalette::from_theme_parts(&self.colors, &self.palette);
        self.typography = ControlTypography::from_text_scale(&self.text);
        self.metrics = ControlMetrics::from_tokens(self.spacing, self.radius);
    }

    pub fn text_style(&self, color: Color) -> TextStyle {
        TextStyle {
            font_size: self.typography.body_font_size.max(1.0),
            line_height: self.typography.body_line_height.max(1.0),
            color,
            ..TextStyle::default()
        }
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
    use super::{Color, DefaultTheme, ThemeColorScheme};
    use crate::hdr_theme::HdrThemeMode;

    #[test]
    fn default_theme_uses_body_text_scale_for_typography() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.typography.body_font_size, theme.text.sm.size);
        assert_eq!(theme.typography.body_line_height, theme.text.sm.line_height);
        assert_eq!(theme.typography.body_font_size, 14.0);
        assert_eq!(theme.typography.body_line_height, 20.0);
        assert_eq!(theme.metrics.min_height, 24.0);
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
        theme.sync_derived_fields();

        assert_eq!(theme.palette.accent, Color::rgba(0.2, 0.3, 0.4, 1.0));
        assert_eq!(theme.palette.caret, Color::rgba(0.2, 0.3, 0.4, 1.0));
        assert_eq!(theme.surfaces.accent, theme.palette.accent);
        assert_eq!(theme.surfaces.window, theme.palette.surface);
        assert_eq!(theme.typography.body_font_size, 11.0);
        assert_eq!(theme.typography.body_line_height, 15.0);
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
    }
}
