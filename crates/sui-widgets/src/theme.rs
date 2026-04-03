use palette::{FromColor, Oklch, Srgb};
use sui_core::Color;
use sui_layout::Padding as Insets;
use sui_text::TextStyle;

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
            base_100: oklch(100.0, 0.0, 0.0),
            base_200: oklch(98.0, 0.0, 0.0),
            base_300: oklch(95.0, 0.0, 0.0),
            base_content: oklch(21.0, 0.006, 285.885),
            primary: oklch(45.0, 0.24, 277.023),
            primary_content: oklch(93.0, 0.034, 272.788),
            secondary: oklch(65.0, 0.241, 354.308),
            secondary_content: oklch(94.0, 0.028, 342.258),
            accent: oklch(77.0, 0.152, 181.912),
            accent_content: oklch(38.0, 0.063, 188.416),
            neutral: oklch(14.0, 0.005, 285.823),
            neutral_content: oklch(92.0, 0.004, 286.32),
            info: oklch(74.0, 0.16, 232.661),
            info_content: oklch(29.0, 0.066, 243.157),
            success: oklch(76.0, 0.177, 163.223),
            success_content: oklch(37.0, 0.077, 168.94),
            warning: oklch(82.0, 0.189, 84.429),
            warning_content: oklch(41.0, 0.112, 45.904),
            error: oklch(71.0, 0.194, 13.428),
            error_content: oklch(27.0, 0.105, 12.094),
        }
    }

    pub fn dark() -> Self {
        Self {
            name: "dark",
            scheme: ThemeColorScheme::Dark,
            base_100: oklch(25.33, 0.016, 252.42),
            base_200: oklch(23.26, 0.014, 253.1),
            base_300: oklch(21.15, 0.012, 254.09),
            base_content: oklch(97.807, 0.029, 256.847),
            primary: oklch(58.0, 0.233, 277.117),
            primary_content: oklch(96.0, 0.018, 272.314),
            secondary: oklch(65.0, 0.241, 354.308),
            secondary_content: oklch(94.0, 0.028, 342.258),
            accent: oklch(77.0, 0.152, 181.912),
            accent_content: oklch(38.0, 0.063, 188.416),
            neutral: oklch(14.0, 0.005, 285.823),
            neutral_content: oklch(92.0, 0.004, 286.32),
            info: oklch(74.0, 0.16, 232.661),
            info_content: oklch(29.0, 0.066, 243.157),
            success: oklch(76.0, 0.177, 163.223),
            success_content: oklch(37.0, 0.077, 168.94),
            warning: oklch(82.0, 0.189, 84.429),
            warning_content: oklch(41.0, 0.112, 45.904),
            error: oklch(71.0, 0.194, 13.428),
            error_content: oklch(27.0, 0.105, 12.094),
        }
    }

    pub fn with_scheme(scheme: ThemeColorScheme) -> Self {
        match scheme {
            ThemeColorScheme::Light => Self::light(),
            ThemeColorScheme::Dark => Self::dark(),
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
            xs: 2.0,
            sm: 4.0,
            md: 6.0,
            lg: 8.0,
            xl: 12.0,
            _2xl: 16.0,
            _3xl: 24.0,
            _4xl: 32.0,
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
    pub placeholder: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub surface_pressed: Color,
    pub surface_focus: Color,
    pub border: Color,
    pub border_hover: Color,
    pub border_focus: Color,
    pub focus_ring: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub accent_border: Color,
    pub accent_border_hover: Color,
    pub accent_border_focus: Color,
    pub accent_text: Color,
}

impl ControlPalette {
    pub fn from_colors(colors: &ThemeColors) -> Self {
        let border = mix(colors.base_300, colors.base_content, 0.12);
        let border_hover = mix(colors.base_300, colors.base_content, 0.24);

        Self {
            text: colors.base_content,
            placeholder: colors.base_content.with_alpha(0.6),
            surface: colors.base_100,
            surface_hover: colors.base_200,
            surface_pressed: colors.base_300,
            surface_focus: mix(colors.base_200, colors.primary, 0.08),
            border,
            border_hover,
            border_focus: colors.primary,
            focus_ring: colors.primary.with_alpha(0.28),
            accent: colors.primary,
            accent_hover: interactive_variant(colors.primary, colors.scheme, 0.08),
            accent_pressed: interactive_variant(colors.primary, colors.scheme, 0.16),
            accent_border: interactive_variant(colors.primary, colors.scheme, 0.12),
            accent_border_hover: interactive_variant(colors.primary, colors.scheme, 0.2),
            accent_border_focus: colors.primary,
            accent_text: colors.primary_content,
        }
    }
}

impl Default for ControlPalette {
    fn default() -> Self {
        Self::from_colors(&ThemeColors::default())
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
            min_height: 40.0,
            button_min_width: 88.0,
            button_padding: Insets {
                left: unit * 3.5,
                top: unit * 2.5,
                right: unit * 3.5,
                bottom: unit * 2.5,
            },
            checkbox_padding: Insets {
                left: unit * 2.5,
                top: unit * 2.0,
                right: unit * 2.5,
                bottom: unit * 2.0,
            },
            checkbox_indicator_size: 18.0,
            checkbox_gap: 10.0,
            separator_thickness: 1.0,
            icon_size: 18.0,
            icon_button_size: 40.0,
            switch_track_width: 38.0,
            switch_track_height: 22.0,
            slider_min_width: 180.0,
            slider_track_height: 4.0,
            slider_thumb_size: 18.0,
            number_input_stepper_width: 32.0,
            text_input_min_width: 240.0,
            text_input_padding: Insets {
                left: unit * 3.0,
                top: unit * 2.5,
                right: unit * 3.0,
                bottom: unit * 2.5,
            },
            text_area_min_height: 120.0,
            select_menu_max_height: 200.0,
            corner_radius: radius.lg,
            indicator_corner_radius: 5.0,
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
    pub palette: ControlPalette,
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

    pub fn from_colors(colors: ThemeColors) -> Self {
        let text = ThemeTextScale::default();
        let radius = ThemeRadii::default();
        let spacing = 4.0;

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
            palette: ControlPalette::from_colors(&colors),
            typography: ControlTypography::from_text_scale(&text),
            metrics: ControlMetrics::from_tokens(spacing, radius),
        }
    }

    pub fn sync_derived_fields(&mut self) {
        self.palette = ControlPalette::from_colors(&self.colors);
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
        ThemeColorScheme::Dark => mix(color, Color::WHITE, amount),
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

fn oklch(lightness_percent: f32, chroma: f32, hue: f32) -> Color {
    let rgb: Srgb = Srgb::from_color(Oklch::new(lightness_percent / 100.0, chroma, hue));
    Color::srgba(
        rgb.red.clamp(0.0, 1.0),
        rgb.green.clamp(0.0, 1.0),
        rgb.blue.clamp(0.0, 1.0),
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::{Color, DefaultTheme, ThemeColorScheme};

    #[test]
    fn default_theme_uses_body_text_scale_for_typography() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.typography.body_font_size, theme.text.sm.size);
        assert_eq!(theme.typography.body_line_height, theme.text.sm.line_height);
    }

    #[test]
    fn sync_derived_fields_updates_semantic_palette_and_typography() {
        let mut theme = DefaultTheme::default();
        theme.colors.primary = Color::rgba(0.2, 0.3, 0.4, 1.0);
        theme.text.sm.size = 15.0;
        theme.text.sm.line_height = 22.0;
        theme.sync_derived_fields();

        assert_eq!(theme.palette.accent, Color::rgba(0.2, 0.3, 0.4, 1.0));
        assert_eq!(theme.typography.body_font_size, 15.0);
        assert_eq!(theme.typography.body_line_height, 22.0);
    }

    #[test]
    fn dark_theme_uses_dark_daisy_tokens() {
        let theme = DefaultTheme::dark();

        assert_eq!(theme.colors.scheme, ThemeColorScheme::Dark);
        assert_eq!(theme.colors.name, "dark");
        assert_eq!(theme.palette.surface, theme.colors.base_100);
        assert_eq!(theme.palette.text, theme.colors.base_content);
        assert_eq!(theme.palette.accent, theme.colors.primary);
        assert_eq!(theme.palette.accent_text, theme.colors.primary_content);
    }
}
