use std::sync::OnceLock;

use sui_core::Color;
use sui_layout::Padding as Insets;
use sui_text::{FontFamilyStack, TextStyle};

use crate::animation::{AnimationSpec, Easing, Stagger};
use crate::hdr_theme::{
    HdrThemeTokens, WidgetColorRole, WidgetEffectRole, WidgetLuminanceRole, WidgetMaterialRole,
    resolve_widget_hdr_style,
};

/// Motion design tokens: a shared vocabulary of animation durations and easing
/// curves so widgets and applications animate consistently.
///
/// Durations are expressed in **seconds** (matching the `delta` supplied by
/// [`crate::animation::AnimatedValue::tick`] and the `time`/`delta` fields of
/// `WakeEvent::AnimationFrame`). Easing curves are built from the
/// [`Easing`] enum and are [`Copy`], keeping [`DefaultTheme`] `Copy`.
///
/// The duration ladder follows the Mesh design language: 70ms micro feedback,
/// 140ms small state changes, 220ms medium surfaces (popovers, dialogs) and
/// 340ms large transitions (drawers, sheets, page changes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeMotion {
    /// No animation: state changes apply immediately (0.0s).
    pub duration_instant: f32,
    /// Micro feedback such as hover tint / press state changes (0.07s).
    pub duration_fast: f32,
    /// Small state changes: toggles, fades, tooltips (0.14s).
    pub duration_normal: f32,
    /// Medium surfaces: popovers, dialogs, expanding panels (0.22s).
    pub duration_slow: f32,
    /// Large transitions: drawers, sheets, page changes (0.34s).
    pub duration_slower: f32,
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
            duration_fast: 0.07,
            duration_normal: 0.14,
            duration_slow: 0.22,
            duration_slower: 0.34,
            easing_standard: Easing::CubicBezier {
                x1: 0.2,
                y1: 0.0,
                x2: 0.0,
                y2: 1.0,
            },
            easing_emphasized: Easing::CubicBezier {
                x1: 0.45,
                y1: 0.0,
                x2: 0.15,
                y2: 1.0,
            },
            easing_decelerate: Easing::CubicBezier {
                x1: 0.2,
                y1: 0.0,
                x2: 0.0,
                y2: 1.0,
            },
            easing_accelerate: Easing::CubicBezier {
                x1: 0.45,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
        }
    }

    pub fn hover_duration(&self) -> f64 {
        f64::from(self.duration_fast)
    }

    pub fn press_duration(&self) -> f64 {
        f64::from(self.duration_fast)
    }

    pub fn focus_duration(&self) -> f64 {
        f64::from(self.duration_normal)
    }

    pub fn toggle_duration(&self) -> f64 {
        f64::from(self.duration_normal)
    }

    pub fn entrance_duration(&self) -> f64 {
        f64::from(self.duration_normal)
    }

    pub fn tab_switch_duration(&self) -> f64 {
        f64::from(self.duration_fast)
    }

    pub const fn hover_easing(&self) -> Easing {
        self.easing_standard
    }

    pub const fn press_easing(&self) -> Easing {
        self.easing_standard
    }

    pub const fn focus_easing(&self) -> Easing {
        self.easing_decelerate
    }

    pub const fn toggle_easing(&self) -> Easing {
        self.easing_emphasized
    }

    pub const fn entrance_easing(&self) -> Easing {
        self.easing_decelerate
    }

    pub const fn tab_switch_easing(&self) -> Easing {
        self.easing_standard
    }

    /// Hover feedback as an [`AnimationSpec`]. Like every spec below, widgets
    /// apply the app's motion policy when they start the transition.
    pub fn hover_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(self.hover_duration(), self.hover_easing())
    }

    pub fn press_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(self.press_duration(), self.press_easing())
    }

    pub fn focus_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(self.focus_duration(), self.focus_easing())
    }

    pub fn toggle_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(self.toggle_duration(), self.toggle_easing())
    }

    pub fn entrance_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(self.entrance_duration(), self.entrance_easing())
    }

    pub fn tab_switch_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(self.tab_switch_duration(), self.tab_switch_easing())
    }

    /// Content leaving: as quick as an entrance, accelerating away.
    pub fn exit_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(f64::from(self.duration_normal), self.easing_accelerate)
    }

    /// Content gliding to a new place after its layout changes, such as a
    /// reordered list item.
    pub fn layout_spec(&self) -> AnimationSpec {
        AnimationSpec::tween(f64::from(self.duration_slow), self.easing_standard)
    }

    /// Delays for a group of items entering together: a quick cascade that
    /// always finishes starting within a medium duration.
    pub fn stagger(&self) -> Stagger {
        let interval = f64::from(self.duration_fast) * 0.5;
        Stagger::new(interval).max_delay(f64::from(self.duration_slow))
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

impl From<ThemeFontStack> for FontFamilyStack {
    fn from(value: ThemeFontStack) -> Self {
        Self::new(value.primary, value.fallbacks)
    }
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

impl ThemeColorScheme {
    /// Whether content is drawn as light ink on dark surfaces.
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::HighContrast)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeDensity {
    Compact,
    #[default]
    Comfortable,
    Touch,
}

impl ThemeDensity {
    /// This density's value out of one for each density.
    fn pick<T>(self, compact: T, comfortable: T, touch: T) -> T {
        match self {
            Self::Compact => compact,
            Self::Comfortable => comfortable,
            Self::Touch => touch,
        }
    }
}

/// Contextual interface-control sizing.
///
/// Unlike [`ThemeDensity`], which is retained as the legacy global theme API,
/// control size is intended to be chosen for the interface being designed:
/// small for dense toolbars and configuration surfaces, medium for standard
/// controls, and large for hero actions or focused overlays. It changes
/// control geometry and typography while leaving the independent text-scale
/// ramp unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl ControlSize {
    const fn legacy_density(self) -> ThemeDensity {
        match self {
            Self::Small => ThemeDensity::Compact,
            Self::Medium => ThemeDensity::Comfortable,
            Self::Large => ThemeDensity::Touch,
        }
    }

    const fn control_height(self) -> f32 {
        match self {
            Self::Small => 28.0,
            Self::Medium => 32.0,
            Self::Large => 40.0,
        }
    }

    const fn row_height(self) -> f32 {
        match self {
            Self::Small => 24.0,
            Self::Medium => 28.0,
            Self::Large => 36.0,
        }
    }

    const fn icon_size(self) -> f32 {
        match self {
            Self::Small => 13.0,
            Self::Medium => 15.0,
            Self::Large => 17.0,
        }
    }
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

/// Every structural tier of a theme: surfaces, neutral fills, borders, and
/// ink. Built-in themes author these directly so each preset can choose pure
/// or subtly tinted neutrals; widgets never derive structure from the brand
/// color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeutralRamp {
    /// Application background behind panels.
    pub window: Color,
    /// Recessed chrome: sidebars, title bars, table headers.
    pub subtle: Color,
    /// Cards, panes, and other content surfaces.
    pub panel: Color,
    /// Floating chrome that lifts off the panel, such as tooltips. Menus,
    /// popovers, and dialogs paint the panel tier with a border and shadow.
    pub overlay: Color,
    /// Recessed neutral fill for tracks, tab strips, chips, and nested areas.
    pub control: Color,
    pub control_hover: Color,
    pub control_active: Color,
    /// Face of neutral buttons, select triggers, and raised segment thumbs.
    pub button: Color,
    pub button_hover: Color,
    pub button_active: Color,
    /// Text input well.
    pub field: Color,
    /// Hairlines inside components.
    pub border_subtle: Color,
    /// Default separators and container outlines.
    pub border: Color,
    /// Outlines of interactive controls such as buttons and fields.
    pub border_strong: Color,
    /// Outline of unselected checkboxes, radios, and switch tracks; at least
    /// 3:1 against the panel so the control boundary stays perceivable.
    pub border_control: Color,
    pub text: Color,
    pub text_secondary: Color,
    /// Placeholder and tertiary metadata; at least 4.5:1 on the panel.
    pub text_tertiary: Color,
    pub text_disabled: Color,
}

impl NeutralRamp {
    /// Pure achromatic light ramp shared by the SUI and neutral light presets.
    pub fn light() -> Self {
        Self {
            window: Color::WHITE,
            subtle: rgb8(250, 250, 250),
            panel: Color::WHITE,
            overlay: Color::WHITE,
            control: rgb8(245, 245, 245),
            control_hover: rgb8(235, 235, 235),
            control_active: rgb8(224, 224, 224),
            button: Color::WHITE,
            button_hover: rgb8(245, 245, 245),
            button_active: rgb8(235, 235, 235),
            field: Color::WHITE,
            border_subtle: rgb8(240, 240, 240),
            border: rgb8(229, 229, 229),
            border_strong: rgb8(212, 212, 212),
            border_control: rgb8(140, 140, 140),
            text: rgb8(23, 23, 23),
            text_secondary: rgb8(82, 82, 82),
            text_tertiary: rgb8(115, 115, 115),
            text_disabled: rgb8(163, 163, 163),
        }
    }

    /// SUI dark ramp: a constant, faint blue tint (OKLCH hue 258, chroma
    /// ~0.016) over evenly spaced lightness steps.
    pub fn dark() -> Self {
        Self {
            window: rgb8(9, 13, 20),
            subtle: rgb8(13, 18, 25),
            panel: rgb8(18, 23, 30),
            overlay: rgb8(25, 30, 37),
            control: rgb8(28, 33, 40),
            control_hover: rgb8(37, 42, 50),
            control_active: rgb8(43, 49, 57),
            button: rgb8(28, 33, 40),
            button_hover: rgb8(37, 42, 50),
            button_active: rgb8(43, 49, 57),
            field: rgb8(13, 18, 25),
            border_subtle: rgb8(31, 37, 45),
            border: rgb8(43, 50, 60),
            border_strong: rgb8(59, 66, 76),
            border_control: rgb8(108, 116, 127),
            text: rgb8(238, 240, 243),
            text_secondary: rgb8(185, 190, 198),
            text_tertiary: rgb8(142, 148, 158),
            text_disabled: rgb8(87, 93, 101),
        }
    }

    /// Achromatic twin of [`Self::dark`] with identical lightness steps.
    pub fn neutral_dark() -> Self {
        Self {
            window: rgb8(13, 13, 13),
            subtle: rgb8(18, 18, 18),
            panel: rgb8(23, 23, 23),
            overlay: rgb8(30, 30, 30),
            control: rgb8(33, 33, 33),
            control_hover: rgb8(42, 42, 42),
            control_active: rgb8(48, 48, 48),
            button: rgb8(33, 33, 33),
            button_hover: rgb8(42, 42, 42),
            button_active: rgb8(48, 48, 48),
            field: rgb8(18, 18, 18),
            border_subtle: rgb8(36, 36, 36),
            border: rgb8(49, 49, 49),
            border_strong: rgb8(65, 65, 65),
            border_control: rgb8(115, 115, 115),
            text: rgb8(240, 240, 240),
            text_secondary: rgb8(190, 190, 190),
            text_tertiary: rgb8(148, 148, 148),
            text_disabled: rgb8(92, 92, 92),
        }
    }

    /// True-black OLED ramp: window, chrome, and panels stay black and are
    /// separated by borders; only interactive fills lift off black, carrying
    /// the same faint tint as [`Self::dark`]. Text is dimmed below pure white.
    pub fn void() -> Self {
        Self {
            window: Color::BLACK,
            subtle: Color::BLACK,
            panel: Color::BLACK,
            overlay: rgb8(12, 17, 24),
            control: rgb8(15, 20, 27),
            control_hover: rgb8(25, 30, 38),
            control_active: rgb8(32, 38, 46),
            button: rgb8(15, 20, 27),
            button_hover: rgb8(25, 30, 38),
            button_active: rgb8(32, 38, 46),
            field: rgb8(7, 11, 18),
            border_subtle: rgb8(21, 26, 33),
            border: rgb8(36, 43, 52),
            border_strong: rgb8(52, 59, 69),
            border_control: rgb8(104, 111, 123),
            text: rgb8(228, 230, 234),
            text_secondary: rgb8(177, 182, 190),
            text_tertiary: rgb8(137, 144, 153),
            text_disabled: rgb8(82, 87, 95),
        }
    }
}

/// A decorative hue from [`DecorativeColors`]. Use decorative hues for
/// categorical emphasis — tags, avatars, node categories, chart series —
/// never for status: status keeps its semantic colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecorativeHue {
    Red,
    Orange,
    Amber,
    Green,
    Teal,
    Cyan,
    Blue,
    Violet,
    Magenta,
}

impl DecorativeHue {
    pub const ALL: [Self; 9] = [
        Self::Red,
        Self::Orange,
        Self::Amber,
        Self::Green,
        Self::Teal,
        Self::Cyan,
        Self::Blue,
        Self::Violet,
        Self::Magenta,
    ];
}

/// Vibrant categorical source colors with matched OKLCH lightness and
/// chroma, so no hue shouts over another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecorativeColors {
    pub red: Color,
    pub orange: Color,
    pub amber: Color,
    pub green: Color,
    pub teal: Color,
    pub cyan: Color,
    pub blue: Color,
    pub violet: Color,
    pub magenta: Color,
}

impl DecorativeColors {
    /// Light-surface decorative set. Every hue except amber reaches at least
    /// 3:1 against white; derived text roles cover the rest.
    pub fn light() -> Self {
        Self {
            red: rgb8(234, 60, 63),
            orange: rgb8(215, 105, 0),
            amber: rgb8(223, 156, 0),
            green: rgb8(0, 158, 72),
            teal: rgb8(0, 155, 139),
            cyan: rgb8(0, 150, 175),
            blue: rgb8(22, 122, 250),
            violet: rgb8(135, 93, 240),
            magenta: rgb8(196, 70, 189),
        }
    }

    /// Dark-surface decorative set, lifted so every hue reads at least 6.5:1
    /// on the dark panel.
    pub fn dark() -> Self {
        Self {
            red: rgb8(255, 113, 107),
            orange: rgb8(255, 138, 55),
            amber: rgb8(252, 177, 0),
            green: rgb8(55, 209, 108),
            teal: rgb8(0, 208, 187),
            cyan: rgb8(0, 201, 234),
            blue: rgb8(104, 165, 255),
            violet: rgb8(168, 143, 255),
            magenta: rgb8(228, 114, 220),
        }
    }

    pub fn get(&self, hue: DecorativeHue) -> Color {
        match hue {
            DecorativeHue::Red => self.red,
            DecorativeHue::Orange => self.orange,
            DecorativeHue::Amber => self.amber,
            DecorativeHue::Green => self.green,
            DecorativeHue::Teal => self.teal,
            DecorativeHue::Cyan => self.cyan,
            DecorativeHue::Blue => self.blue,
            DecorativeHue::Violet => self.violet,
            DecorativeHue::Magenta => self.magenta,
        }
    }

    pub fn set(&mut self, hue: DecorativeHue, color: Color) {
        match hue {
            DecorativeHue::Red => self.red = color,
            DecorativeHue::Orange => self.orange = color,
            DecorativeHue::Amber => self.amber = color,
            DecorativeHue::Green => self.green = color,
            DecorativeHue::Teal => self.teal = color,
            DecorativeHue::Cyan => self.cyan = color,
            DecorativeHue::Blue => self.blue = color,
            DecorativeHue::Violet => self.violet = color,
            DecorativeHue::Magenta => self.magenta = color,
        }
    }
}

/// SUI azure: OKLCH(0.549 0.230 262), 5.1:1 against white labels.
const SUI_AZURE: Color = Color::rgba(23.0 / 255.0, 98.0 / 255.0, 244.0 / 255.0, 1.0);
/// SUI violet: OKLCH(0.560 0.220 292), the secondary signal color.
const SUI_VIOLET: Color = Color::rgba(125.0 / 255.0, 77.0 / 255.0, 231.0 / 255.0, 1.0);

/// The source colors of a theme. Everything widgets paint — control states,
/// soft washes, legible tone text, focus, selection, and the HDR variants —
/// is derived from these values, so editing any field (for example
/// `primary`) keeps every dependent role consistent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    pub name: &'static str,
    pub scheme: ThemeColorScheme,
    pub neutrals: NeutralRamp,
    /// Brand color: primary actions, links, checked controls, focus rings,
    /// and thin selection indicators.
    pub primary: Color,
    pub on_primary: Color,
    /// Secondary signal color for live and decorative emphasis.
    pub secondary: Color,
    pub on_secondary: Color,
    pub info: Color,
    pub on_info: Color,
    pub success: Color,
    pub on_success: Color,
    pub warning: Color,
    pub on_warning: Color,
    pub danger: Color,
    pub on_danger: Color,
    pub decorative: DecorativeColors,
}

impl ThemeColors {
    /// SUI's branded light palette. This is an explicit alias for
    /// [`Self::light`] so applications can distinguish the SUI preset from
    /// other built-in light themes.
    pub fn sui() -> Self {
        Self::light()
    }

    /// Pure white and gray surfaces with the vibrant SUI azure accent.
    pub fn light() -> Self {
        Self {
            name: "light",
            scheme: ThemeColorScheme::Light,
            neutrals: NeutralRamp::light(),
            ..Self::branded_sources()
        }
    }

    /// Faintly blue-tinted dark surfaces with the same SUI azure accent as
    /// the light preset.
    pub fn dark() -> Self {
        Self {
            name: "dark",
            scheme: ThemeColorScheme::Dark,
            neutrals: NeutralRamp::dark(),
            decorative: DecorativeColors::dark(),
            ..Self::branded_sources()
        }
    }

    /// True-black OLED companion to [`Self::dark`].
    pub fn high_contrast() -> Self {
        Self {
            name: "void",
            scheme: ThemeColorScheme::HighContrast,
            neutrals: NeutralRamp::void(),
            decorative: DecorativeColors::dark(),
            ..Self::branded_sources()
        }
    }

    /// The SUI light ramp with an achromatic primary, for professional
    /// interfaces that have no product color preference. Status and
    /// decorative colors keep their meaning.
    pub fn neutral() -> Self {
        Self {
            name: "neutral",
            scheme: ThemeColorScheme::Light,
            neutrals: NeutralRamp::light(),
            primary: rgb8(23, 23, 23),
            on_primary: Color::WHITE,
            secondary: rgb8(82, 82, 82),
            on_secondary: Color::WHITE,
            ..Self::branded_sources()
        }
    }

    /// Dark companion to [`Self::neutral`] on the achromatic dark ramp.
    pub fn neutral_dark() -> Self {
        Self {
            name: "neutral-dark",
            scheme: ThemeColorScheme::Dark,
            neutrals: NeutralRamp::neutral_dark(),
            primary: rgb8(240, 240, 240),
            on_primary: rgb8(23, 23, 23),
            secondary: rgb8(190, 190, 190),
            on_secondary: rgb8(23, 23, 23),
            decorative: DecorativeColors::dark(),
            ..Self::branded_sources()
        }
    }

    pub fn with_scheme(scheme: ThemeColorScheme) -> Self {
        match scheme {
            ThemeColorScheme::Light => Self::light(),
            ThemeColorScheme::Dark => Self::dark(),
            ThemeColorScheme::HighContrast => Self::high_contrast(),
        }
    }

    /// Brand, status, and decorative sources shared by every preset. Status
    /// solids are identical across schemes; their soft washes and legible
    /// text are derived per scheme.
    fn branded_sources() -> Self {
        Self {
            name: "light",
            scheme: ThemeColorScheme::Light,
            neutrals: NeutralRamp::light(),
            primary: SUI_AZURE,
            on_primary: Color::WHITE,
            secondary: SUI_VIOLET,
            on_secondary: Color::WHITE,
            info: rgb8(0, 166, 222),
            on_info: rgb8(2, 42, 61),
            success: rgb8(6, 168, 78),
            on_success: rgb8(12, 46, 22),
            warning: rgb8(244, 165, 0),
            on_warning: rgb8(63, 37, 0),
            danger: rgb8(220, 38, 39),
            on_danger: Color::WHITE,
            decorative: DecorativeColors::light(),
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
                line_height: 17.0,
            },
            sm: ThemeTextToken {
                size: 13.0,
                line_height: 18.0,
            },
            base: ThemeTextToken {
                size: 15.0,
                line_height: 22.0,
            },
            lg: ThemeTextToken {
                size: 17.0,
                line_height: 25.0,
            },
            xl: ThemeTextToken {
                size: 19.0,
                line_height: 27.0,
            },
            _2xl: ThemeTextToken {
                size: 21.0,
                line_height: 29.0,
            },
            _3xl: ThemeTextToken {
                size: 25.0,
                line_height: 33.0,
            },
            _4xl: ThemeTextToken {
                size: 31.0,
                line_height: 39.0,
            },
            _5xl: ThemeTextToken {
                size: 37.0,
                line_height: 45.0,
            },
            _6xl: ThemeTextToken {
                size: 49.0,
                line_height: 55.0,
            },
            _7xl: ThemeTextToken {
                size: 61.0,
                line_height: 67.0,
            },
            _8xl: ThemeTextToken {
                size: 73.0,
                line_height: 79.0,
            },
            _9xl: ThemeTextToken {
                size: 97.0,
                line_height: 103.0,
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
            tighter: 0.0,
            tight: 0.0,
            normal: 0.0,
            wide: 0.02,
            wider: 0.04,
            widest: 0.06,
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
            tight: 1.3,
            snug: 1.4,
            normal: 1.5,
            relaxed: 1.6,
            loose: 1.65,
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
            xl: 10.0,
            _2xl: 14.0,
            _3xl: 18.0,
            _4xl: 999.0,
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
        let shadow = sui_scene::ShadowParams::new(
            self.offset_x,
            self.offset_y,
            self.blur,
            self.spread,
            self.color,
        );
        if self.inset {
            shadow.with_placement(sui_scene::ShadowPlacement::Inside)
        } else {
            shadow
        }
    }

    /// An outer (drop) shadow casts beyond the surface edge; an inset layer
    /// renders an inner shadow, with [`paint_theme_inset_shadow`].
    pub const fn is_outer(&self) -> bool {
        !self.inset
    }
}

impl ThemeShadow {
    /// The layers in painting order: `second` first, so `first` ends up on
    /// top, as CSS `box-shadow` stacks its first-listed shadow topmost.
    fn painting_order(&self) -> impl Iterator<Item = ThemeShadowLayer> {
        self.second.into_iter().chain(self.first)
    }
}

/// Paint the outer (drop) layers of a [`ThemeShadow`] behind a rounded-rect
/// surface. The tighter `second` layer is drawn first and the wider/more-diffuse
/// `first` layer on top — matching CSS `box-shadow`, where the first-listed
/// shadow is topmost.
///
/// Inset layers are left for [`paint_theme_inset_shadow`].
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
    for layer in shadow.painting_order().filter(ThemeShadowLayer::is_outer) {
        paint.draw_shadow(rect, radii, layer.to_shadow_params());
    }
}

/// Paint the inset layers of a [`ThemeShadow`] inside a rounded-rect surface.
/// Call it after filling the surface and before painting its border and
/// content, as CSS draws inset shadows. Pass the surface inside its border as
/// `rect`, as CSS does, or the border covers the thinnest layers. Outer layers
/// are left for [`paint_theme_shadow`].
pub fn paint_theme_inset_shadow(
    paint: &mut sui_runtime::PaintCtx,
    rect: sui_core::Rect,
    radii: [f32; 4],
    shadow: &ThemeShadow,
) {
    for layer in shadow.painting_order().filter(|layer| !layer.is_outer()) {
        paint.draw_shadow(rect, radii, layer.to_shadow_params());
    }
}

/// Paint a glow around a rounded rect: each layer of `glow` as a halo outside
/// the rect only, so the rect itself stays clear. It can be painted before the
/// rect's fill or after it; a translucent fill does not show the halo through
/// it. Resolve a theme glow for the output with
/// [`DefaultTheme::glow_for_output`].
pub fn paint_theme_glow(
    paint: &mut sui_runtime::PaintCtx,
    rect: sui_core::Rect,
    radii: [f32; 4],
    glow: &ThemeShadow,
) {
    for layer in glow.painting_order() {
        paint.draw_shadow(
            rect,
            radii,
            layer
                .to_shadow_params()
                .with_placement(sui_scene::ShadowPlacement::Outside),
        );
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

impl ThemeShadows {
    /// Scheme-aware elevation: Light casts faint shadows tinted with the
    /// theme's text ink, Dark casts deeper black shadows, and the true-black
    /// OLED theme casts none at all — elevation there is drawn with borders.
    pub fn for_colors(colors: &ThemeColors) -> Self {
        match colors.scheme {
            ThemeColorScheme::Light => Self::light_with_ink(colors.neutrals.text),
            ThemeColorScheme::Dark => Self::dark(),
            ThemeColorScheme::HighContrast => Self::none(),
        }
    }

    /// Light ladder: `0 1px 2px 6%`, `0 2px 10px 8%`, `0 16px 40px 16%`
    /// anchors interpolated across the scale, tinted with near-black ink.
    pub fn light() -> Self {
        Self::light_with_ink(rgb8(23, 23, 23))
    }

    /// The light ladder cast with a custom shadow ink.
    pub fn light_with_ink(shadow_ink: Color) -> Self {
        let ink = |alpha: f32| shadow_ink.with_alpha(alpha);

        Self {
            box_shadow: ThemeBoxShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, ink(0.04), false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, ink(0.06), false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 3.0, 0.0, ink(0.07), false),
                    shadow_layer(0.0, 1.0, 2.0, -1.0, ink(0.06), false),
                ),
                md: ThemeShadow::single(shadow_layer(0.0, 2.0, 10.0, 0.0, ink(0.08), false)),
                lg: ThemeShadow::single(shadow_layer(0.0, 8.0, 24.0, -2.0, ink(0.12), false)),
                xl: ThemeShadow::single(shadow_layer(0.0, 16.0, 40.0, -4.0, ink(0.16), false)),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 24.0, 56.0, -8.0, ink(0.20), false)),
            },
            inset: ThemeInsetShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, ink(0.05), true)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, ink(0.05), true)),
                sm: ThemeShadow::single(shadow_layer(0.0, 2.0, 4.0, 0.0, ink(0.05), true)),
            },
            drop: ThemeDropShadowScale {
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, ink(0.05), false)),
                sm: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, ink(0.12), false)),
                md: ThemeShadow::single(shadow_layer(0.0, 3.0, 3.0, 0.0, ink(0.10), false)),
                lg: ThemeShadow::single(shadow_layer(0.0, 4.0, 4.0, 0.0, ink(0.12), false)),
                xl: ThemeShadow::single(shadow_layer(0.0, 9.0, 7.0, 0.0, ink(0.09), false)),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 25.0, 25.0, 0.0, ink(0.12), false)),
            },
            text: ThemeTextShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, ink(0.12), false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, ink(0.16), false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 0.0, 0.0, ink(0.07), false),
                    shadow_layer(0.0, 1.0, 1.0, 0.0, ink(0.07), false),
                ),
                md: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 1.0, 0.0, ink(0.09), false),
                    shadow_layer(0.0, 2.0, 4.0, 0.0, ink(0.09), false),
                ),
                lg: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 2.0, 0.0, ink(0.09), false),
                    shadow_layer(0.0, 4.0, 8.0, 0.0, ink(0.09), false),
                ),
            },
        }
    }

    /// Mesh Dark ladder: `0 1px 2px 30%`, `0 4px 16px 40%`, `0 20px 48px 55%`
    /// anchors interpolated across the scale (pure black, deeper than Light).
    pub fn dark() -> Self {
        let black = |alpha: f32| Color::BLACK.with_alpha(alpha);

        Self {
            box_shadow: ThemeBoxShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, black(0.24), false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, black(0.30), false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 2.0, 6.0, 0.0, black(0.32), false),
                    shadow_layer(0.0, 1.0, 2.0, -1.0, black(0.28), false),
                ),
                md: ThemeShadow::single(shadow_layer(0.0, 4.0, 16.0, 0.0, black(0.40), false)),
                lg: ThemeShadow::single(shadow_layer(0.0, 10.0, 28.0, -2.0, black(0.46), false)),
                xl: ThemeShadow::single(shadow_layer(0.0, 20.0, 48.0, -4.0, black(0.55), false)),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 28.0, 64.0, -8.0, black(0.60), false)),
            },
            inset: ThemeInsetShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black(0.24), true)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black(0.24), true)),
                sm: ThemeShadow::single(shadow_layer(0.0, 2.0, 4.0, 0.0, black(0.24), true)),
            },
            drop: ThemeDropShadowScale {
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black(0.24), false)),
                sm: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, black(0.34), false)),
                md: ThemeShadow::single(shadow_layer(0.0, 3.0, 3.0, 0.0, black(0.32), false)),
                lg: ThemeShadow::single(shadow_layer(0.0, 4.0, 4.0, 0.0, black(0.34), false)),
                xl: ThemeShadow::single(shadow_layer(0.0, 9.0, 7.0, 0.0, black(0.30), false)),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 25.0, 25.0, 0.0, black(0.34), false)),
            },
            text: ThemeTextShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black(0.30), false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black(0.36), false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 0.0, 0.0, black(0.20), false),
                    shadow_layer(0.0, 1.0, 1.0, 0.0, black(0.20), false),
                ),
                md: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 1.0, 0.0, black(0.24), false),
                    shadow_layer(0.0, 2.0, 4.0, 0.0, black(0.24), false),
                ),
                lg: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 2.0, 0.0, black(0.24), false),
                    shadow_layer(0.0, 4.0, 8.0, 0.0, black(0.24), false),
                ),
            },
        }
    }

    /// No shadows anywhere: the Void/OLED elevation contract. Shadows are dead
    /// pixels on OLED; surfaces separate with borders instead.
    pub fn none() -> Self {
        let empty_box = ThemeBoxShadowScale {
            _2xs: ThemeShadow::empty(),
            xs: ThemeShadow::empty(),
            sm: ThemeShadow::empty(),
            md: ThemeShadow::empty(),
            lg: ThemeShadow::empty(),
            xl: ThemeShadow::empty(),
            _2xl: ThemeShadow::empty(),
        };

        Self {
            box_shadow: empty_box,
            inset: ThemeInsetShadowScale {
                _2xs: ThemeShadow::empty(),
                xs: ThemeShadow::empty(),
                sm: ThemeShadow::empty(),
            },
            drop: ThemeDropShadowScale {
                xs: ThemeShadow::empty(),
                sm: ThemeShadow::empty(),
                md: ThemeShadow::empty(),
                lg: ThemeShadow::empty(),
                xl: ThemeShadow::empty(),
                _2xl: ThemeShadow::empty(),
            },
            text: ThemeTextShadowScale {
                _2xs: ThemeShadow::empty(),
                xs: ThemeShadow::empty(),
                sm: ThemeShadow::empty(),
                md: ThemeShadow::empty(),
                lg: ThemeShadow::empty(),
            },
        }
    }
}

/// Which of the theme's glows to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GlowTone {
    /// The primary color's halo, for live and primary signals.
    #[default]
    Accent,
    /// The secondary color's halo, for voice and duplex signals.
    Secondary,
}

/// Glow tokens: soft zero-offset halos reserved for **live signals** (streaming,
/// voice, busy indicators, the primary action). Mesh keeps Light glow-free
/// (light does not glow on paper), gives Dark full glows, and damps Void to
/// protect OLED panels. Paint with [`paint_theme_glow`], after resolving them
/// for the output with [`DefaultTheme::glow_for_output`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeGlows {
    /// Accent-hued glow: live/primary signals (`--sm-glow-accent`).
    pub accent: ThemeShadow,
    /// Secondary-hued glow: voice/duplex signals (`--sm-glow-voice`).
    pub secondary: ThemeShadow,
}

impl ThemeGlows {
    /// Glows follow the theme's primary and secondary colors: none in Light,
    /// full halos in Dark, and damped halos in Void. Achromatic signal colors
    /// (the neutral presets) glow at half strength so white halos stay quiet.
    pub fn for_colors(colors: &ThemeColors) -> Self {
        let (blur, alpha) = match colors.scheme {
            ThemeColorScheme::Light => return Self::none(),
            ThemeColorScheme::Dark => (16.0, 0.24),
            ThemeColorScheme::HighContrast => (10.0, 0.14),
        };
        let halo = |color: Color, blur: f32| {
            let strength = if color.to_oklch().chroma < 0.04 {
                0.5
            } else {
                1.0
            };
            ThemeShadow::single(shadow_layer(
                0.0,
                0.0,
                blur,
                0.0,
                color.with_alpha(alpha * strength),
                false,
            ))
        };
        Self {
            accent: halo(colors.primary, blur),
            secondary: halo(colors.secondary, blur + 2.0),
        }
    }

    pub fn none() -> Self {
        Self {
            accent: ThemeShadow::empty(),
            secondary: ThemeShadow::empty(),
        }
    }

    pub fn get(&self, tone: GlowTone) -> ThemeShadow {
        match tone {
            GlowTone::Accent => self.accent,
            GlowTone::Secondary => self.secondary,
        }
    }
}

impl Default for ThemeGlows {
    fn default() -> Self {
        Self::for_colors(&ThemeColors::default())
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

/// The derived role set for one chromatic source color: the solid fill and
/// its interaction states, a soft wash, ink that stays legible on plain and
/// soft surfaces, and a translucent-looking outline. Every value is opaque,
/// flattened onto the theme panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneRoles {
    pub solid: Color,
    pub on_solid: Color,
    pub hover: Color,
    pub pressed: Color,
    pub soft: Color,
    pub text: Color,
    pub border: Color,
}

impl ToneRoles {
    /// Derive the role set in OKLCH so interaction states keep the source hue
    /// and chroma. Light themes darken on hover (near-black sources lighten
    /// instead); dark themes lighten on hover and darken when pressed.
    pub fn derive(solid: Color, on_solid: Color, scheme: ThemeColorScheme, panel: Color) -> Self {
        let dark = scheme.is_dark();
        let lightness = solid.to_oklch().lightness;
        let (hover, pressed) = if dark {
            (shift_lightness(solid, 0.03), shift_lightness(solid, -0.035))
        } else if lightness < 0.32 {
            (shift_lightness(solid, 0.07), shift_lightness(solid, 0.12))
        } else {
            (
                shift_lightness(solid, -0.045),
                shift_lightness(solid, -0.09),
            )
        };
        // Soft washes and outlines are authored as CSS-style translucency and
        // flattened in encoded space; the renderer's linear blending would
        // otherwise read them noticeably heavier.
        let soft = solid.with_alpha(if dark { 0.16 } else { 0.10 }).over(panel);
        let border = solid.with_alpha(if dark { 0.45 } else { 0.35 }).over(panel);
        let text = with_min_contrast(solid, soft, if dark { 6.0 } else { 4.5 });

        Self {
            solid,
            on_solid,
            hover,
            pressed,
            soft,
            text,
            border,
        }
    }
}

/// Derived [`ToneRoles`] for every [`DecorativeHue`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecorativePalette {
    pub red: ToneRoles,
    pub orange: ToneRoles,
    pub amber: ToneRoles,
    pub green: ToneRoles,
    pub teal: ToneRoles,
    pub cyan: ToneRoles,
    pub blue: ToneRoles,
    pub violet: ToneRoles,
    pub magenta: ToneRoles,
}

impl DecorativePalette {
    pub fn from_colors(colors: &ThemeColors) -> Self {
        let roles = |hue: DecorativeHue| {
            let solid = colors.decorative.get(hue);
            ToneRoles::derive(
                solid,
                readable_ink(solid),
                colors.scheme,
                colors.neutrals.panel,
            )
        };
        Self {
            red: roles(DecorativeHue::Red),
            orange: roles(DecorativeHue::Orange),
            amber: roles(DecorativeHue::Amber),
            green: roles(DecorativeHue::Green),
            teal: roles(DecorativeHue::Teal),
            cyan: roles(DecorativeHue::Cyan),
            blue: roles(DecorativeHue::Blue),
            violet: roles(DecorativeHue::Violet),
            magenta: roles(DecorativeHue::Magenta),
        }
    }

    pub fn get(&self, hue: DecorativeHue) -> ToneRoles {
        match hue {
            DecorativeHue::Red => self.red,
            DecorativeHue::Orange => self.orange,
            DecorativeHue::Amber => self.amber,
            DecorativeHue::Green => self.green,
            DecorativeHue::Teal => self.teal,
            DecorativeHue::Cyan => self.cyan,
            DecorativeHue::Blue => self.blue,
            DecorativeHue::Violet => self.violet,
            DecorativeHue::Magenta => self.magenta,
        }
    }

    /// Stable categorical assignment: the same index always maps to the same
    /// hue, cycling through [`DecorativeHue::ALL`].
    pub fn categorical(&self, index: usize) -> ToneRoles {
        self.get(DecorativeHue::ALL[index % DecorativeHue::ALL.len()])
    }
}

impl Default for DecorativePalette {
    fn default() -> Self {
        DefaultTheme::default().decorative
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPalette {
    pub text: Color,
    pub text_muted: Color,
    pub placeholder: Color,
    pub text_disabled: Color,
    pub surface: Color,
    pub surface_raised: Color,
    /// Recessed neutral fill: tracks, tab strips, chips, nested areas.
    pub control: Color,
    pub control_hover: Color,
    pub control_active: Color,
    /// Face of neutral buttons, select triggers, and raised segment thumbs.
    pub button: Color,
    pub button_hover: Color,
    pub button_pressed: Color,
    /// Resting outline of neutral buttons and framed fields.
    pub button_border: Color,
    /// Inset field background for text inputs and editable surfaces.
    pub field: Color,
    pub surface_hover: Color,
    pub surface_pressed: Color,
    pub surface_focus: Color,
    pub border: Color,
    pub border_strong: Color,
    pub border_hover: Color,
    /// Neutral border a focused control settles on beneath the accent ring.
    pub border_focus: Color,
    /// Outline of unselected checkboxes, radios, and switch tracks (3:1).
    pub border_control: Color,
    /// Keyboard focus color: the primary, adjusted to stay visible on the
    /// panel. Drawn as a ring at `focus_ring_width` with `focus_ring_outset`.
    pub focus: Color,
    pub focus_ring: Color,
    pub caret: Color,
    /// Neutral selection fill for rows, tiles, and ranges.
    pub selection: Color,
    /// Accent-colored outline for selected tiles, segmented thumbs, and
    /// swatches. Selection fill remains neutral; this narrow border is the
    /// decorative brand cue.
    pub selection_border: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub accent_border: Color,
    pub accent_border_hover: Color,
    pub accent_border_focus: Color,
    /// Content color on a solid accent fill.
    pub accent_text: Color,
    /// Soft accent wash for badges, callouts, and live signals.
    pub accent_soft: Color,
    /// Accent-hued ink legible on plain and soft surfaces: links and sparse
    /// decorative emphasis.
    pub accent_soft_text: Color,
    pub info: Color,
    pub info_text: Color,
    pub info_hover: Color,
    pub info_pressed: Color,
    pub info_border: Color,
    pub info_soft: Color,
    pub info_soft_text: Color,
    pub success: Color,
    pub success_text: Color,
    pub success_hover: Color,
    pub success_pressed: Color,
    pub success_border: Color,
    pub success_soft: Color,
    pub success_soft_text: Color,
    pub warning: Color,
    pub warning_text: Color,
    pub warning_hover: Color,
    pub warning_pressed: Color,
    pub warning_border: Color,
    pub warning_soft: Color,
    pub warning_soft_text: Color,
    pub danger: Color,
    pub danger_text: Color,
    pub danger_hover: Color,
    pub danger_pressed: Color,
    pub danger_border: Color,
    pub danger_soft: Color,
    pub danger_soft_text: Color,
}

impl ControlPalette {
    pub fn from_colors(colors: &ThemeColors) -> Self {
        let dark = colors.scheme.is_dark();
        let neutrals = colors.neutrals;
        let panel = neutrals.panel;
        let tone = |solid: Color, on_solid: Color| {
            ToneRoles::derive(solid, on_solid, colors.scheme, panel)
        };
        let accent = tone(colors.primary, colors.on_primary);
        let info = tone(colors.info, colors.on_info);
        let success = tone(colors.success, colors.on_success);
        let warning = tone(colors.warning, colors.on_warning);
        let danger = tone(colors.danger, colors.on_danger);
        let focus = with_min_contrast(colors.primary, panel, if dark { 4.5 } else { 3.0 });

        Self {
            text: neutrals.text,
            text_muted: neutrals.text_secondary,
            placeholder: neutrals.text_tertiary,
            text_disabled: neutrals.text_disabled,
            surface: neutrals.window,
            surface_raised: panel,
            control: neutrals.control,
            control_hover: neutrals.control_hover,
            control_active: neutrals.control_active,
            button: neutrals.button,
            button_hover: neutrals.button_hover,
            button_pressed: neutrals.button_active,
            button_border: neutrals.border_strong,
            field: neutrals.field,
            surface_hover: neutrals.control_hover,
            surface_pressed: neutrals.control_active,
            // Focused fields keep their well; the accent ring carries focus.
            surface_focus: neutrals.field,
            border: neutrals.border,
            border_strong: neutrals.border_strong,
            border_hover: mix(neutrals.border_strong, neutrals.border_control, 0.45),
            border_focus: mix(neutrals.border_strong, neutrals.border_control, 0.65),
            border_control: neutrals.border_control,
            focus,
            focus_ring: focus,
            caret: neutrals.text,
            selection: neutrals
                .text
                .with_alpha(if dark { 0.10 } else { 0.08 })
                .over(panel),
            selection_border: accent.border,
            accent: accent.solid,
            accent_hover: accent.hover,
            accent_pressed: accent.pressed,
            accent_border: accent.border,
            accent_border_hover: mix(accent.border, accent.solid, 0.35),
            accent_border_focus: accent.solid,
            accent_text: accent.on_solid,
            accent_soft: accent.soft,
            accent_soft_text: accent.text,
            info: info.solid,
            info_text: info.on_solid,
            info_hover: info.hover,
            info_pressed: info.pressed,
            info_border: info.border,
            info_soft: info.soft,
            info_soft_text: info.text,
            success: success.solid,
            success_text: success.on_solid,
            success_hover: success.hover,
            success_pressed: success.pressed,
            success_border: success.border,
            success_soft: success.soft,
            success_soft_text: success.text,
            warning: warning.solid,
            warning_text: warning.on_solid,
            warning_hover: warning.hover,
            warning_pressed: warning.pressed,
            warning_border: warning.border,
            warning_soft: warning.soft,
            warning_soft_text: warning.text,
            danger: danger.solid,
            danger_text: danger.on_solid,
            danger_hover: danger.hover,
            danger_pressed: danger.pressed,
            danger_border: danger.border,
            danger_soft: danger.soft,
            danger_soft_text: danger.text,
        }
    }
}

impl Default for ControlPalette {
    fn default() -> Self {
        DefaultTheme::default().palette
    }
}

/// Shift OKLCH lightness while keeping hue and chroma (gamut-mapped to sRGB).
fn shift_lightness(color: Color, delta: f32) -> Color {
    let oklch = color.to_oklch();
    oklch
        .with_lightness((oklch.lightness + delta).clamp(0.0, 1.0))
        .to_srgb()
        .with_alpha(color.alpha)
}

/// Move `color` along OKLCH lightness, away from `background`, until it
/// reaches `target` contrast or the lightness range ends. Hue and chroma are
/// preserved as far as the sRGB gamut allows.
fn with_min_contrast(color: Color, background: Color, target: f32) -> Color {
    if color.contrast_ratio(background) >= target {
        return color;
    }
    let oklch = color.to_oklch();
    let step = if background.relative_luminance() < 0.18 {
        0.01
    } else {
        -0.01
    };
    let mut lightness = oklch.lightness;
    let mut candidate = color;
    while (0.0..=1.0).contains(&(lightness + step)) {
        lightness += step;
        candidate = oklch.with_lightness(lightness).to_srgb();
        if candidate.contrast_ratio(background) >= target {
            break;
        }
    }
    candidate.with_alpha(color.alpha)
}

/// White or a deep same-hue ink, whichever reads better on `solid`.
fn readable_ink(solid: Color) -> Color {
    let hue = solid.to_oklch().hue;
    let deep = Color::oklch(0.26, 0.06, hue);
    if Color::WHITE.contrast_ratio(solid) >= deep.contrast_ratio(solid) {
        Color::WHITE
    } else {
        deep
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePalette {
    pub dark: bool,
    pub window: Color,
    /// Slightly recessed window background (`--sm-bg-subtle`): sidebars,
    /// table headers, code block chrome.
    pub window_subtle: Color,
    pub sidebar: Color,
    pub panel: Color,
    /// Nested/raised fill one step above `panel` (`--sm-surface-2`).
    pub surface_2: Color,
    /// The strongest neutral fill tier (`--sm-surface-3`): hover wells,
    /// track backgrounds, avatar fills.
    pub surface_3: Color,
    /// Floating surface for menus, popovers, dialogs, toasts (`--sm-overlay`).
    pub overlay: Color,
    pub titlebar: Color,
    pub field: Color,
    pub border: Color,
    pub border_strong: Color,
    /// Hairline separators inside components (`--sm-border-subtle`).
    pub border_subtle: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_faint: Color,
    /// Disabled content (`--sm-text-disabled`).
    pub text_disabled: Color,
    /// Content drawn on inverted surfaces (`--sm-text-invert`).
    pub text_invert: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub on_accent: Color,
    /// Accent-hued text legible on plain surfaces (`--sm-accent-text`). Use
    /// for links and sparse decorative emphasis, not selected content.
    pub accent_text: Color,
    /// Translucent accent wash (`--sm-accent-soft`) for explicit decorative
    /// emphasis and live signals.
    pub accent_soft: Color,
    /// Translucent accent border (`--sm-accent-border`).
    pub accent_border: Color,
    /// The dedicated neutral keyboard-focus ring color (`--sm-focus`).
    pub focus: Color,
    pub hover: Color,
    pub selected: Color,
    /// Thin decorative selection outline; pair with neutral `selected` fill.
    pub selected_border: Color,
    pub overlay_scrim: Color,
    pub tooltip: Color,
    pub tooltip_border: Color,
    pub tooltip_text: Color,
    // Internal compatibility caches for widget-owned appearance resolvers.
    // External code customizes these through Canvas*/ColorPicker appearance types.
    pub(crate) canvas: Color,
    pub(crate) canvas_grid: Color,
    pub(crate) canvas_axis_x: Color,
    pub(crate) canvas_axis_y: Color,
    pub(crate) pixel_canvas_paper: Color,
    pub(crate) pixel_canvas_document_edge: Color,
    pub(crate) pixel_canvas_shadow_near: Color,
    pub(crate) pixel_canvas_shadow_far: Color,
    pub(crate) pixel_canvas_grid: Color,
    pub(crate) canvas_ruler: Color,
    pub(crate) canvas_ruler_border: Color,
    pub(crate) canvas_ruler_tick: Color,
    pub(crate) canvas_ruler_text: Color,
    pub(crate) checkerboard_light: Color,
    pub(crate) checkerboard_dark: Color,
    pub(crate) color_picker_chrome_border: Color,
    pub(crate) color_picker_plane_border: Color,
    pub(crate) color_picker_bar_border: Color,
    pub(crate) color_picker_marker_outer: Color,
    pub(crate) color_picker_marker_dark: Color,
    pub(crate) color_picker_marker_light: Color,
    pub(crate) color_picker_sdr_marker: Color,
    pub(crate) color_picker_hdr_divider: Color,
    pub good: Color,
    /// Positive-status text legible on plain/soft surfaces (`--sm-ok-text`).
    pub good_text: Color,
    /// Translucent positive wash (`--sm-ok-soft`).
    pub good_soft: Color,
    pub warn: Color,
    /// Warning text legible on plain/soft surfaces (`--sm-warn-text`).
    pub warn_text: Color,
    /// Translucent warning wash (`--sm-warn-soft`).
    pub warn_soft: Color,
    pub bad: Color,
    /// Danger text legible on plain/soft surfaces (`--sm-danger-text`).
    pub bad_text: Color,
    /// Translucent danger wash (`--sm-danger-soft`).
    pub bad_soft: Color,
    /// Solid informational status (`--sm-info`).
    pub info: Color,
    /// Informational text legible on plain/soft surfaces (`--sm-info-text`).
    pub info_text: Color,
    /// Translucent informational wash (`--sm-info-soft`).
    pub info_soft: Color,
}

impl SurfacePalette {
    pub fn from_theme_parts(colors: &ThemeColors, controls: &ControlPalette) -> Self {
        let dark = colors.scheme.is_dark();
        let neutrals = colors.neutrals;
        let text_muted = controls.text_muted;
        let text_faint = controls.placeholder;
        let window_subtle = neutrals.subtle;
        let overlay = neutrals.overlay;
        let shadow_ink = if dark { Color::BLACK } else { neutrals.text };

        Self {
            dark,
            window: controls.surface,
            window_subtle,
            // The workspace chrome (sidebars, title bars) sits on the subtle
            // background tier so content panes read as the brighter surface.
            sidebar: window_subtle,
            panel: controls.surface_raised,
            surface_2: controls.control,
            surface_3: controls.control_hover,
            overlay,
            titlebar: window_subtle,
            field: controls.field,
            border: controls.border,
            border_strong: controls.border_strong,
            border_subtle: neutrals.border_subtle,
            text: controls.text,
            text_muted,
            text_faint,
            text_disabled: neutrals.text_disabled,
            text_invert: if dark { neutrals.window } else { Color::WHITE },
            accent: controls.accent,
            accent_hover: controls.accent_hover,
            on_accent: controls.accent_text,
            accent_text: controls.accent_soft_text,
            accent_soft: controls.accent_soft,
            accent_border: controls.accent_border,
            focus: controls.focus,
            hover: controls.text.with_alpha(if dark { 0.06 } else { 0.045 }),
            selected: controls.selection,
            selected_border: controls.selection_border,
            overlay_scrim: match colors.scheme {
                ThemeColorScheme::Light => neutrals.text.with_alpha(0.40),
                ThemeColorScheme::Dark => mix(neutrals.window, Color::BLACK, 0.5).with_alpha(0.64),
                ThemeColorScheme::HighContrast => Color::BLACK.with_alpha(0.72),
            },
            // Tooltips are quiet floating surfaces, not inverted bubbles:
            // overlay fill, strong border, secondary ink.
            tooltip: overlay,
            tooltip_border: controls.border_strong,
            tooltip_text: text_muted,
            canvas: controls.surface,
            canvas_grid: controls.border.with_alpha(if dark { 0.30 } else { 0.18 }),
            canvas_axis_x: colors.danger.with_alpha(if dark { 0.72 } else { 0.55 }),
            canvas_axis_y: colors.success.with_alpha(if dark { 0.72 } else { 0.55 }),
            pixel_canvas_paper: if dark {
                mix(controls.surface_raised, controls.text, 0.10)
            } else {
                window_subtle
            },
            pixel_canvas_document_edge: controls.text.with_alpha(if dark { 0.82 } else { 0.72 }),
            pixel_canvas_shadow_near: shadow_ink.with_alpha(if dark { 0.30 } else { 0.16 }),
            pixel_canvas_shadow_far: shadow_ink.with_alpha(if dark { 0.18 } else { 0.08 }),
            pixel_canvas_grid: controls.text.with_alpha(if dark { 0.32 } else { 0.28 }),
            canvas_ruler: controls.surface_raised,
            canvas_ruler_border: controls.border.with_alpha(0.78),
            canvas_ruler_tick: controls.text_muted.with_alpha(0.72),
            canvas_ruler_text: controls.text.with_alpha(0.76),
            checkerboard_light: if dark {
                mix(controls.surface_raised, controls.text, 0.18)
            } else {
                window_subtle
            },
            checkerboard_dark: if dark {
                mix(controls.surface_raised, controls.text, 0.10)
            } else {
                neutrals.border
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
            good_text: controls.success_soft_text,
            good_soft: controls.success_soft,
            warn: colors.warning,
            warn_text: controls.warning_soft_text,
            warn_soft: controls.warning_soft,
            bad: colors.danger,
            bad_text: controls.danger_soft_text,
            bad_soft: controls.danger_soft,
            info: colors.info,
            info_text: controls.info_soft_text,
            info_soft: controls.info_soft,
        }
    }
}

impl Default for SurfacePalette {
    fn default() -> Self {
        DefaultTheme::default().surfaces
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlTypography {
    pub body_font_size: f32,
    pub body_line_height: f32,
}

impl ControlTypography {
    pub fn from_text_scale(text: &ThemeTextScale) -> Self {
        Self::for_density(text, ThemeDensity::default())
    }

    /// Legacy density typography. Authored interfaces should prefer
    /// [`Self::for_size`]; the legacy Compact/Comfortable/Touch tiers mirror
    /// Small/Medium/Large so both paths resolve the same text-ramp tokens.
    pub fn for_density(text: &ThemeTextScale, density: ThemeDensity) -> Self {
        let token = match density {
            ThemeDensity::Compact => text.sm,
            ThemeDensity::Comfortable => text.base,
            ThemeDensity::Touch => text.lg,
        };
        Self {
            body_font_size: token.size,
            body_line_height: token.line_height,
        }
    }

    /// Resolve authored interface-size typography from the current text scale.
    /// Text scaling remains independent: each size selects a token from the
    /// already-scaled ramp instead of applying another multiplier.
    pub fn for_size(text: &ThemeTextScale, size: ControlSize) -> Self {
        let token = match size {
            ControlSize::Small => text.sm,
            ControlSize::Medium => text.base,
            ControlSize::Large => text.lg,
        };
        Self {
            body_font_size: token.size,
            body_line_height: token.line_height,
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
                pressed_offset: 0.0,
                active_indicator_thickness: 2.0,
            },
            ThemeDensity::Comfortable => Self {
                hover_blend: 0.86,
                pressed_blend: 1.0,
                selected_blend: 0.22,
                tab_selected_blend: 0.08,
                disabled_opacity: 0.74,
                disabled_content_opacity: 0.50,
                pressed_offset: 0.0,
                active_indicator_thickness: 3.0,
            },
            ThemeDensity::Touch => Self {
                hover_blend: 0.94,
                pressed_blend: 1.0,
                selected_blend: 0.24,
                tab_selected_blend: 0.09,
                disabled_opacity: 0.78,
                disabled_content_opacity: 0.54,
                pressed_offset: 0.0,
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
    /// The narrowest a menu panel gets, however short its labels.
    pub menu_min_width: f32,
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

/// Insets of `horizontal` on the left and right and `vertical` on the top and
/// bottom.
const fn symmetric_insets(horizontal: f32, vertical: f32) -> Insets {
    Insets {
        left: horizontal,
        top: vertical,
        right: horizontal,
        bottom: vertical,
    }
}

impl ControlMetrics {
    pub fn from_tokens(spacing: f32, radius: ThemeRadii, density: ThemeDensity) -> Self {
        let unit = spacing.max(1.0);
        // A metric that varies with density picks its compact, comfortable,
        // and touch values, in that order.
        Self {
            min_height: density.pick(28.0, 32.0, 36.0),
            touch_target_size: density.pick(28.0, 36.0, 44.0),
            button_min_width: 64.0,
            button_padding: density.pick(
                symmetric_insets(unit * 1.5, unit * 0.75),
                symmetric_insets(unit * 2.0, unit * 1.25),
                symmetric_insets(unit * 2.5, unit * 1.5),
            ),
            checkbox_padding: density.pick(
                symmetric_insets(unit, unit * 0.5),
                symmetric_insets(unit * 1.5, unit),
                symmetric_insets(unit * 2.0, unit * 1.5),
            ),
            checkbox_indicator_size: 15.0,
            checkbox_gap: 6.0,
            icon_label_gap: 6.0,
            separator_thickness: 1.0,
            icon_size: density.pick(14.0, 16.0, 18.0),
            icon_button_size: density.pick(28.0, 32.0, 36.0),
            switch_track_width: 32.0,
            switch_track_height: 19.0,
            switch_thumb_inset: 3.0,
            slider_min_width: density.pick(120.0, 140.0, 160.0),
            slider_padding: density.pick(
                symmetric_insets(unit * 1.5, unit * 0.5),
                symmetric_insets(unit * 2.0, unit),
                symmetric_insets(unit * 2.5, unit * 1.5),
            ),
            slider_track_height: 4.0,
            slider_thumb_size: 14.0,
            number_input_stepper_width: density.pick(22.0, 24.0, 30.0),
            text_input_min_width: density.pick(150.0, 180.0, 200.0),
            text_input_padding: density.pick(
                symmetric_insets(unit * 2.0, unit * 0.75),
                symmetric_insets(unit * 2.0, unit * 1.25),
                symmetric_insets(unit * 2.0, unit * 1.5),
            ),
            text_area_min_height: density.pick(56.0, 64.0, 72.0),
            select_menu_max_height: density.pick(176.0, 200.0, 230.0),
            select_menu_gap: 6.0,
            select_menu_edge_padding: 8.0,
            tab_height: density.pick(32.0, 36.0, 40.0),
            tab_min_width: density.pick(84.0, 96.0, 104.0),
            tab_gap: 6.0,
            tab_padding: density.pick(
                symmetric_insets(unit * 2.5, unit * 0.75),
                symmetric_insets(unit * 2.5, unit),
                symmetric_insets(unit * 2.5, unit * 1.5),
            ),
            tab_panel_padding: density.pick(
                Insets::all(unit * 3.0),
                Insets::all(unit * 4.0),
                Insets::all(unit * 4.5),
            ),
            tab_panel_gap: density.pick(unit * 2.0, unit * 3.0, unit * 3.5),
            menu_row_height: density.pick(24.0, 28.0, 36.0),
            menu_padding: density.pick(
                Insets::all(unit * 0.75),
                Insets::all(unit),
                Insets::all(unit * 1.5),
            ),
            menu_item_padding: density.pick(
                symmetric_insets(unit * 2.0, unit * 0.5),
                symmetric_insets(unit * 2.5, unit),
                symmetric_insets(unit * 3.0, unit * 1.5),
            ),
            menu_shortcut_width: density.pick(84.0, 96.0, 112.0),
            menu_min_width: density.pick(144.0, 160.0, 200.0),
            popover_padding: density.pick(
                Insets::all(unit * 2.5),
                Insets::all(unit * 3.5),
                Insets::all(unit * 4.0),
            ),
            popover_gap: unit * 2.0,
            popover_reveal_offset: density.pick(8.0, 10.0, 12.0),
            tooltip_padding: density.pick(
                symmetric_insets(unit * 2.0, unit * 1.5),
                Insets::all(unit * 2.25),
                symmetric_insets(unit * 3.0, unit * 2.5),
            ),
            tooltip_min_width: density.pick(80.0, 96.0, 112.0),
            tooltip_gap: density.pick(unit * 2.0, unit * 2.5, unit * 3.0),
            tooltip_reveal_offset: density.pick(6.0, 8.0, 10.0),
            dialog_min_width: density.pick(240.0, 280.0, 320.0),
            dialog_max_width: density.pick(440.0, 520.0, 600.0),
            dialog_outer_margin: density.pick(unit * 4.0, unit * 6.0, unit * 8.0),
            dialog_padding: density.pick(
                Insets::all(unit * 3.5),
                Insets::all(18.0),
                Insets::all(unit * 6.0),
            ),
            dialog_title_font_size: density.pick(16.0, 16.0, 18.0),
            dialog_title_line_height: density.pick(24.0, 24.0, 26.0),
            dialog_description_gap: density.pick(unit * 1.5, unit * 2.0, unit * 2.5),
            dialog_body_gap: density.pick(unit * 3.0, 14.0, unit * 4.5),
            dialog_footer_gap: density.pick(unit * 3.5, 18.0, unit * 5.0),
            dialog_action_gap: density.pick(unit * 2.0, unit * 2.5, unit * 3.0),
            dialog_action_min_width: density.pick(92.0, 110.0, 128.0),
            toolbar_extent: density.pick(40.0, 52.0, 56.0),
            toolbar_padding: density.pick(
                Insets::all(unit * 1.5),
                Insets::all(unit * 2.0),
                Insets::all(unit * 2.25),
            ),
            toolbar_spacing: density.pick(unit * 1.5, unit * 2.0, unit * 2.25),
            command_group_padding: density.pick(
                Insets::all(unit * 0.25),
                Insets::all(unit * 0.5),
                Insets::all(unit * 0.75),
            ),
            command_group_spacing: density.pick(unit * 0.5, unit * 0.75, unit * 0.875),
            command_group_radius: density.pick(radius.md, radius.lg, radius.xl),
            tool_palette_item_size: density.pick(30.0, 40.0, 44.0),
            tool_palette_icon_size: density.pick(16.0, 20.0, 22.0),
            preset_strip_item_height: density.pick(24.0, 28.0, 36.0),
            preset_strip_item_min_width: density.pick(36.0, 44.0, 50.0),
            preset_strip_item_padding: density.pick(
                symmetric_insets(unit * 2.0, unit),
                symmetric_insets(unit * 3.0, unit),
                symmetric_insets(unit * 3.5, unit * 1.5),
            ),
            preset_strip_gap: density.pick(unit, unit * 1.5, unit * 1.75),
            preset_strip_label_padding: density.pick(
                Insets::all(unit * 0.75),
                Insets::all(unit),
                Insets::all(unit * 1.5),
            ),
            action_card_min_width: density.pick(252.0, 280.0, 300.0),
            action_card_min_height: density.pick(84.0, 104.0, 112.0),
            action_card_padding: density.pick(
                Insets {
                    left: unit * 3.0,
                    top: unit * 2.5,
                    right: unit * 2.5,
                    bottom: unit * 2.5,
                },
                Insets {
                    left: unit * 4.0,
                    top: unit * 3.5,
                    right: unit * 3.5,
                    bottom: unit * 3.5,
                },
                Insets {
                    left: unit * 4.5,
                    top: unit * 4.0,
                    right: unit * 4.0,
                    bottom: unit * 4.0,
                },
            ),
            action_card_icon_box_size: density.pick(32.0, 38.0, 42.0),
            action_card_icon_size: density.pick(16.0, 20.0, 22.0),
            action_card_icon_gap: density.pick(unit * 2.5, unit * 3.0, unit * 3.25),
            action_card_text_gap: density.pick(unit, unit * 1.25, unit * 1.5),
            action_card_trailing_gap: density.pick(18.0, 22.0, 26.0),
            action_card_chevron_size: density.pick(14.0, 16.0, 18.0),
            action_card_accent_width: density.pick(2.0, 3.0, 3.5),
            action_card_accent_inset: density.pick(unit * 2.0, unit * 2.5, unit * 2.75),
            status_bar_height: density.pick(34.0, 40.0, 46.0),
            status_bar_segment_padding: density.pick(unit * 2.0, unit * 2.5, unit * 2.75),
            status_bar_segment_min_width: density.pick(72.0, 86.0, 96.0),
            status_bar_separator_inset: density.pick(unit * 1.25, unit * 1.5, unit * 1.75),
            progress_bar_min_width: density.pick(180.0, 240.0, 260.0),
            progress_bar_height: density.pick(14.0, 18.0, 22.0),
            progress_bar_value_height: density.pick(22.0, 18.0, 22.0),
            progress_bar_label_padding: density.pick(
                Insets::all(unit * 0.5),
                Insets::all(unit * 0.5),
                Insets::all(unit * 0.75),
            ),
            property_row_label_width: density.pick(96.0, 112.0, 136.0),
            property_row_inline_gap: density.pick(unit * 1.5, unit * 2.0, unit * 3.0),
            property_row_stacked_gap: density.pick(unit, unit * 1.5, unit * 2.0),
            form_row_label_width: density.pick(112.0, 128.0, 144.0),
            form_row_control_width: density.pick(300.0, 340.0, 380.0),
            form_row_gap: density.pick(unit * 2.0, unit * 3.0, unit * 4.0),
            field_group_spacing: density.pick(unit * 1.5, unit * 2.0, unit * 3.0),
            form_section_padding: density.pick(
                Insets {
                    left: unit * 2.5,
                    top: unit * 2.0,
                    right: unit * 2.5,
                    bottom: unit * 2.5,
                },
                Insets {
                    left: 14.0,
                    top: unit * 3.0,
                    right: 14.0,
                    bottom: 14.0,
                },
                Insets {
                    left: unit * 4.5,
                    top: unit * 4.0,
                    right: unit * 4.5,
                    bottom: unit * 4.5,
                },
            ),
            form_section_body_gap: density.pick(unit * 2.0, unit * 3.0, unit * 4.0),
            form_section_header_gap: density.pick(unit * 2.0, unit * 2.5, unit * 3.0),
            form_section_description_gap: density.pick(unit * 0.5, unit * 0.75, unit),
            form_section_max_width: density.pick(600.0, 640.0, 720.0),
            form_section_radius: density.pick(radius.md, radius.lg, radius.xl),
            panel_section_gap: density.pick(unit * 1.5, unit * 2.0, unit * 3.0),
            panel_section_action_gap: density.pick(unit, unit * 1.5, unit * 2.0),
            panel_section_disclosure_size: density.pick(14.0, 16.0, 20.0),
            dock_panel_header_height: density.pick(28.0, 34.0, 44.0),
            dock_panel_padding: density.pick(
                symmetric_insets(unit * 2.0, unit * 1.5),
                symmetric_insets(unit * 2.5, unit * 2.0),
                symmetric_insets(unit * 3.5, unit * 3.0),
            ),
            data_viewport_padding: density.pick(
                Insets::all(unit * 0.75),
                Insets::all(unit),
                Insets::all(unit * 1.5),
            ),
            data_row_padding: density.pick(
                Insets {
                    left: unit * 2.0,
                    top: unit * 0.5,
                    right: unit * 1.5,
                    bottom: unit * 0.5,
                },
                Insets {
                    left: unit * 2.5,
                    top: unit * 0.75,
                    right: unit * 2.0,
                    bottom: unit * 0.75,
                },
                Insets {
                    left: unit * 3.0,
                    top: unit * 1.25,
                    right: unit * 2.5,
                    bottom: unit * 1.25,
                },
            ),
            data_row_icon_size: density.pick(12.0, 14.0, 16.0),
            data_row_icon_gap: density.pick(unit * 1.5, unit * 2.0, unit * 2.25),
            data_row_trailing_gap: density.pick(unit * 2.0, unit * 3.0, unit * 3.5),
            data_scroll_thumb_width: density.pick(3.0, 4.0, 6.0),
            data_scroll_thumb_inset: density.pick(5.0, 6.0, 8.0),
            data_scroll_thumb_radius: density.pick(radius.sm, radius.sm, radius.md),
            data_scroll_thumb_min_length: density.pick(24.0, 28.0, 44.0),
            data_scroll_thumb_opacity: density.pick(0.68, 0.75, 0.78),
            list_row_height: density.pick(24.0, 28.0, 36.0),
            layer_row_height: density.pick(28.0, 32.0, 40.0),
            layer_action_size: density.pick(20.0, 24.0, 28.0),
            layer_action_icon_inset: density.pick(4.5, 5.0, 7.0),
            layer_lock_icon_inset: density.pick(3.5, 4.0, 6.0),
            layer_visibility_stroke_width: density.pick(1.25, 1.4, 1.8),
            layer_visibility_slash_stroke_width: density.pick(1.45, 1.6, 2.0),
            layer_thumbnail_size: density.pick(22.0, 28.0, 32.0),
            layer_thumbnail_inset: density.pick(1.5, 2.0, 3.0),
            layer_thumbnail_radius: density.pick(radius.md, radius.md, radius.lg),
            layer_thumbnail_disabled_opacity: density.pick(0.34, 0.36, 0.40),
            layer_thumbnail_disabled_border_opacity: density.pick(0.52, 0.55, 0.60),
            tree_row_height: density.pick(24.0, 28.0, 36.0),
            tree_indent: density.pick(unit * 3.5, 16.0, 20.0),
            tree_disclosure_size: density.pick(10.0, 12.0, 14.0),
            tree_disclosure_gap: density.pick(unit, 6.0, unit * 1.5),
            table_row_height: density.pick(24.0, 28.0, 36.0),
            table_header_height: density.pick(24.0, 28.0, 36.0),
            table_cell_padding: unit * 2.5,
            table_header_separator_inset: density.pick(3.0, 4.0, 8.0),
            table_separator_width: density.pick(1.0, 1.0, 1.5),
            table_row_border_opacity: density.pick(0.50, 0.55, 0.60),
            breadcrumb_height: density.pick(28.0, 36.0, 40.0),
            breadcrumb_item_padding: density.pick(
                symmetric_insets(unit * 2.0, unit * 0.75),
                symmetric_insets(unit * 2.0, unit),
                symmetric_insets(unit * 2.5, unit * 1.5),
            ),
            breadcrumb_gap: density.pick(unit * 4.0, unit * 5.0, unit * 5.5),
            breadcrumb_separator_size: density.pick(9.0, 10.0, 11.0),
            image_corner_radius: density.pick(radius.md, radius.lg, radius.xl),
            color_swatch_width: density.pick(48.0, 56.0, 72.0),
            color_swatch_height: density.pick(28.0, 32.0, 44.0),
            color_swatch_inner_inset: density.pick(1.0, 1.0, 1.5),
            color_swatch_checker_size: density.pick(5.0, 6.0, 8.0),
            color_palette_swatch_size: density.pick(24.0, 28.0, 40.0),
            color_palette_gap: density.pick(unit * 1.25, unit * 1.5, unit * 2.0),
            color_palette_swatch_inset: density.pick(2.0, 2.0, 3.0),
            color_palette_selected_swatch_inset: density.pick(3.0, 3.0, 4.0),
            color_palette_checker_size: density.pick(5.0, 5.0, 7.0),
            brush_preview_min_width: density.pick(220.0, 260.0, 320.0),
            brush_preview_min_height: density.pick(58.0, 70.0, 88.0),
            brush_preview_padding: density.pick(
                Insets::all(unit * 1.5),
                Insets::all(unit * 2.0),
                Insets::all(unit * 3.0),
            ),
            brush_preview_swatch_width: density.pick(46.0, 54.0, 72.0),
            brush_preview_swatch_gap: density.pick(unit * 2.0, unit * 2.5, unit * 3.5),
            brush_preview_checker_size: density.pick(5.0, 6.0, 8.0),
            brush_preview_text_height: density.pick(15.0, 16.0, 18.0),
            brush_preview_text_font_size: density.pick(10.0, 11.0, 12.0),
            brush_preview_text_line_height: density.pick(13.0, 14.0, 16.0),
            color_picker_content_inset: density.pick(unit * 3.0, 14.0, unit * 4.5),
            color_picker_panel_gap: density.pick(unit * 2.5, 14.0, unit * 4.5),
            color_picker_top_bar_height: density.pick(40.0, 52.0, 64.0),
            color_picker_swatch_width: density.pick(64.0, 96.0, 112.0),
            color_picker_swatch_gap: density.pick(unit * 2.0, unit * 2.5, unit * 3.0),
            color_picker_section_gap: density.pick(14.0, 14.0, 18.0),
            color_picker_wheel_size: density.pick(128.0, 166.0, 210.0),
            color_picker_map_size: density.pick(132.0, 210.0, 240.0),
            color_picker_row_height: density.pick(24.0, 24.0, 44.0),
            color_picker_row_gap: density.pick(unit * 2.0, unit * 2.0, unit * 3.0),
            color_picker_right_panel_width: density.pick(150.0, 226.0, 280.0),
            color_picker_field_height: density.pick(28.0, 30.0, 44.0),
            color_picker_field_gap: density.pick(12.0, 12.0, 16.0),
            color_picker_dropdown_gap: density.pick(unit, unit, unit * 2.0),
            color_picker_encoding_menu_row_height: density.pick(28.0, 28.0, 44.0),
            scroll_bar_thickness: density.pick(10.0, 12.0, 18.0),
            scroll_bar_min_thumb_length: density.pick(24.0, 28.0, 44.0),
            split_view_divider_thickness: density.pick(1.0, 1.0, 2.0),
            split_view_drag_target_thickness: density.pick(10.0, 12.0, 44.0),
            floating_workspace_margin: density.pick(unit * 2.0, unit * 3.0, unit * 4.5),
            floating_view_title_bar_height: density.pick(30.0, 32.0, 52.0),
            floating_view_title_padding: density.pick(
                symmetric_insets(unit * 2.5, unit * 1.5),
                symmetric_insets(14.0, unit * 2.0),
                symmetric_insets(unit * 4.5, unit * 3.5),
            ),
            floating_view_resize_handle_size: density.pick(16.0, 18.0, 28.0),
            canvas_ruler_extent: density.pick(20.0, 22.0, 32.0),
            canvas_ruler_major_tick: density.pick(9.0, 10.0, 16.0),
            canvas_ruler_minor_tick: density.pick(4.0, 5.0, 8.0),
            canvas_ruler_target_major_spacing: density.pick(84.0, 96.0, 120.0),
            canvas_ruler_label_padding: density.pick(
                Insets::all(unit * 0.5),
                symmetric_insets(unit * 0.75, unit * 0.5),
                Insets::all(unit),
            ),
            canvas_ruler_label_max_width: density.pick(48.0, 54.0, 72.0),
            canvas_grid_step: density.pick(32.0, 40.0, 48.0),
            canvas_axis_overscan: density.pick(72.0, 80.0, 96.0),
            pixel_canvas_fit_padding: density.pick(20.0, 24.0, 32.0),
            pixel_canvas_grid_zoom: 6.0,
            pixel_canvas_nearest_sampling_zoom: 1.0,
            pixel_canvas_zoom_step: 1.1,
            corner_radius: density.pick(radius.md, radius.md, radius.lg),
            indicator_corner_radius: radius.sm + 1.0,
            border_width: 1.0,
            focus_ring_width: 2.0,
            // Keep the 2px ring wholly clear of the control border.
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
    /// Contextual authored size, when this theme copy is scoped to an interface surface.
    pub control_size: Option<ControlSize>,
    pub spacing: f32,
    pub breakpoints: ThemeBreakpoints,
    pub containers: ThemeContainers,
    pub text: ThemeTextScale,
    pub font_weights: ThemeFontWeights,
    pub tracking: ThemeTracking,
    pub leading: ThemeLeading,
    pub radius: ThemeRadii,
    pub shadows: ThemeShadows,
    /// Live-signal glow halos (`--sm-glow-*`): empty in Light, damped in the
    /// true-black theme. Paint with [`paint_theme_shadow`].
    pub glows: ThemeGlows,
    pub blur: ThemeBlurScale,
    pub perspective: ThemePerspective,
    pub aspect: ThemeAspectRatios,
    pub motion: ThemeMotion,
    pub hdr: HdrThemeTokens,
    pub palette: ControlPalette,
    pub surfaces: SurfacePalette,
    /// Derived roles for the categorical decorative hues.
    pub decorative: DecorativePalette,
    pub typography: ControlTypography,
    pub interaction: ControlStateMetrics,
    pub metrics: ControlMetrics,
}

impl DefaultTheme {
    pub fn new() -> Self {
        Self::default()
    }

    /// SUI's branded default theme.
    pub fn sui() -> Self {
        Self::light()
    }

    /// Pure white and gray surfaces with the vibrant SUI azure accent.
    pub fn light() -> Self {
        static THEME: OnceLock<DefaultTheme> = OnceLock::new();
        *THEME.get_or_init(|| Self::from_colors(ThemeColors::light()))
    }

    /// A neutral light theme for serious, professional interfaces without an
    /// application-specific color preference.
    pub fn neutral() -> Self {
        static THEME: OnceLock<DefaultTheme> = OnceLock::new();
        *THEME.get_or_init(|| Self::from_colors(ThemeColors::neutral()))
    }

    /// Dark companion to [`Self::neutral`].
    pub fn neutral_dark() -> Self {
        static THEME: OnceLock<DefaultTheme> = OnceLock::new();
        *THEME.get_or_init(|| Self::from_colors(ThemeColors::neutral_dark()))
    }

    /// Faintly blue-tinted dark surfaces with the SUI azure accent.
    pub fn dark() -> Self {
        static THEME: OnceLock<DefaultTheme> = OnceLock::new();
        *THEME.get_or_init(|| Self::from_colors(ThemeColors::dark()))
    }

    pub fn high_contrast() -> Self {
        static THEME: OnceLock<DefaultTheme> = OnceLock::new();
        *THEME.get_or_init(|| Self::from_colors(ThemeColors::high_contrast()))
    }

    /// The true-black OLED theme ("Void"): borders instead of shadows, dimmed
    /// whites, damped glows. Alias for [`Self::high_contrast`].
    pub fn void() -> Self {
        Self::high_contrast()
    }

    pub fn compact() -> Self {
        Self::default().with_size(ControlSize::Small)
    }

    pub fn comfortable() -> Self {
        Self::default().with_size(ControlSize::Medium)
    }

    pub fn touch() -> Self {
        Self::default().with_size(ControlSize::Large)
    }

    /// Build a theme from source colors, deriving every palette role. The
    /// built-in presets cache their result; call this for custom palettes.
    pub fn from_colors(colors: ThemeColors) -> Self {
        let text = ThemeTextScale::default();
        let radius = ThemeRadii::default();
        let spacing = 4.0;
        let density = ThemeDensity::default();
        let hdr = HdrThemeTokens::from_colors(colors);
        let palette = ControlPalette::from_colors(&colors);
        let surfaces = SurfacePalette::from_theme_parts(&colors, &palette);
        let decorative = DecorativePalette::from_colors(&colors);

        let mut theme = Self {
            fonts: ThemeFontFamilies::default(),
            colors,
            density,
            control_size: Some(ControlSize::Medium),
            spacing,
            breakpoints: ThemeBreakpoints::default(),
            containers: ThemeContainers::default(),
            text,
            font_weights: ThemeFontWeights::default(),
            tracking: ThemeTracking::default(),
            leading: ThemeLeading::default(),
            radius,
            shadows: ThemeShadows::for_colors(&colors),
            glows: ThemeGlows::for_colors(&colors),
            blur: ThemeBlurScale::default(),
            perspective: ThemePerspective::default(),
            aspect: ThemeAspectRatios::default(),
            motion: ThemeMotion::default(),
            hdr,
            palette,
            surfaces,
            decorative,
            typography: ControlTypography::for_density(&text, density),
            interaction: ControlStateMetrics::for_density(density),
            metrics: ControlMetrics::from_tokens(spacing, radius, density),
        };
        theme.sync_size_fields(ControlSize::Medium);
        theme
    }

    pub fn with_density(mut self, density: ThemeDensity) -> Self {
        self.density = density;
        self.control_size = None;
        self.sync_density_fields();
        self
    }

    /// Apply contextual interface sizing without changing the text-scale ramp.
    ///
    /// The existing density tiers provide the mature detailed metric ladders
    /// behind `Small`, `Medium`, and `Large`. The authored size contract then
    /// normalizes the primary control, row, icon, and control-text tokens while
    /// leaving the caller's text ramp intact.
    pub fn with_size(mut self, size: ControlSize) -> Self {
        self.density = size.legacy_density();
        self.control_size = Some(size);
        self.sync_size_fields(size);
        self
    }

    fn sync_size_fields(&mut self, size: ControlSize) {
        self.interaction = ControlStateMetrics::for_density(size.legacy_density());
        self.metrics =
            ControlMetrics::from_tokens(self.spacing, self.radius, size.legacy_density());
        self.metrics.min_height = size.control_height();
        self.metrics.touch_target_size = 44.0;
        self.metrics.icon_size = size.icon_size();
        self.metrics.icon_button_size = size.control_height();
        self.metrics.list_row_height = size.row_height();
        self.metrics.tree_row_height = size.row_height();
        self.metrics.table_row_height = size.row_height();
        self.metrics.menu_row_height = size.row_height();
        self.typography = ControlTypography::for_size(&self.text, size);
    }

    fn sync_density_fields(&mut self) {
        if let Some(size) = self
            .control_size
            .filter(|size| size.legacy_density() == self.density)
        {
            self.sync_size_fields(size);
            return;
        }
        self.control_size = None;
        self.typography = ControlTypography::for_density(&self.text, self.density);
        self.interaction = ControlStateMetrics::for_density(self.density);
        self.metrics = ControlMetrics::from_tokens(self.spacing, self.radius, self.density);
    }

    pub fn sync_derived_fields(&mut self) {
        self.hdr.sync_semantic_defaults(self.colors);
        self.palette = ControlPalette::from_colors(&self.colors);
        self.surfaces = SurfacePalette::from_theme_parts(&self.colors, &self.palette);
        self.decorative = DecorativePalette::from_colors(&self.colors);
        self.shadows = ThemeShadows::for_colors(&self.colors);
        self.glows = ThemeGlows::for_colors(&self.colors);
        self.sync_density_fields();
    }

    pub fn text_style(&self, color: Color) -> TextStyle {
        self.text_style_with_font_stack(color, self.fonts.sans)
    }

    /// Build a text style using the theme's serif family preference stack.
    pub fn serif_text_style(&self, color: Color) -> TextStyle {
        self.text_style_with_font_stack(color, self.fonts.serif)
    }

    /// Build a text style using the theme's monospace family preference stack.
    pub fn mono_text_style(&self, color: Color) -> TextStyle {
        self.text_style_with_font_stack(color, self.fonts.mono)
    }

    fn text_style_with_font_stack(&self, color: Color, fonts: ThemeFontStack) -> TextStyle {
        TextStyle {
            font_families: Some(fonts.into()),
            font_size: self.typography.body_font_size.max(1.0),
            line_height: self.typography.body_line_height.max(1.0),
            color,
            ..TextStyle::default()
        }
    }

    pub fn semantic_tone_colors(&self, tone: SemanticTone) -> (Color, Color) {
        match tone {
            SemanticTone::Neutral => (self.palette.button, self.palette.text),
            SemanticTone::Accent => (self.palette.accent, self.palette.accent_text),
            SemanticTone::Info => (self.palette.info, self.palette.info_text),
            SemanticTone::Success => (self.palette.success, self.palette.success_text),
            SemanticTone::Warning => (self.palette.warning, self.palette.warning_text),
            SemanticTone::Danger => (self.palette.danger, self.palette.danger_text),
        }
    }

    /// The theme's `tone` glow as it should look on `output`.
    ///
    /// Where the theme's HDR mode and the output allow it, each halo takes the
    /// HDR variant of the tone's color, as bright as an emissive indicator may
    /// be, or its wide-gamut variant on a wide-gamut output, keeping the halo's
    /// own alpha. Elsewhere, and whenever the theme turns HDR effects off, the
    /// glow is the theme's token. Light themes have no glow.
    pub fn glow_for_output(
        &self,
        tone: GlowTone,
        output: Option<sui_runtime::OutputColorRange>,
    ) -> ThemeShadow {
        let glow = self.glows.get(tone);
        let role = match tone {
            GlowTone::Accent => WidgetColorRole::Accent,
            GlowTone::Secondary => WidgetColorRole::Secondary,
        };
        let style = resolve_widget_hdr_style(
            &self.hdr.limited_to(output),
            role,
            WidgetLuminanceRole::EmissiveIndicator,
            WidgetMaterialRole::Flat,
            Some(WidgetEffectRole::Glow),
        );
        if style.effect.is_none() {
            return glow;
        }
        let color = crate::controls::apply_hdr_policy_cap(style.color, style.peak_lift);
        let lit = |layer: Option<ThemeShadowLayer>| {
            layer.map(|layer| ThemeShadowLayer {
                color: color.with_alpha(layer.color.alpha),
                ..layer
            })
        };
        ThemeShadow {
            first: lit(glow.first),
            second: lit(glow.second),
        }
    }

    /// The complete role set for a semantic tone. `Neutral` resolves to the
    /// neutral button face: white with an outline in light themes, a raised
    /// fill in dark themes, always with full-strength ink.
    pub fn tone_roles(&self, tone: SemanticTone) -> ToneRoles {
        let palette = &self.palette;
        match tone {
            SemanticTone::Neutral => ToneRoles {
                solid: palette.button,
                on_solid: palette.text,
                hover: palette.button_hover,
                pressed: palette.button_pressed,
                soft: palette.control,
                text: palette.text,
                border: palette.button_border,
            },
            SemanticTone::Accent => ToneRoles {
                solid: palette.accent,
                on_solid: palette.accent_text,
                hover: palette.accent_hover,
                pressed: palette.accent_pressed,
                soft: palette.accent_soft,
                text: palette.accent_soft_text,
                border: palette.accent_border,
            },
            SemanticTone::Info => ToneRoles {
                solid: palette.info,
                on_solid: palette.info_text,
                hover: palette.info_hover,
                pressed: palette.info_pressed,
                soft: palette.info_soft,
                text: palette.info_soft_text,
                border: palette.info_border,
            },
            SemanticTone::Success => ToneRoles {
                solid: palette.success,
                on_solid: palette.success_text,
                hover: palette.success_hover,
                pressed: palette.success_pressed,
                soft: palette.success_soft,
                text: palette.success_soft_text,
                border: palette.success_border,
            },
            SemanticTone::Warning => ToneRoles {
                solid: palette.warning,
                on_solid: palette.warning_text,
                hover: palette.warning_hover,
                pressed: palette.warning_pressed,
                soft: palette.warning_soft,
                text: palette.warning_soft_text,
                border: palette.warning_border,
            },
            SemanticTone::Danger => ToneRoles {
                solid: palette.danger,
                on_solid: palette.danger_text,
                hover: palette.danger_hover,
                pressed: palette.danger_pressed,
                soft: palette.danger_soft,
                text: palette.danger_soft_text,
                border: palette.danger_border,
            },
        }
    }

    pub fn semantic_tone_color(&self, tone: SemanticTone) -> Color {
        self.semantic_tone_colors(tone).0
    }

    pub fn semantic_tone_text_color(&self, tone: SemanticTone) -> Color {
        self.semantic_tone_colors(tone).1
    }

    /// Soft pair for an explicitly semantic tone: a restrained wash to fill
    /// with and tone-hued ink that stays legible on it. Use for badges and
    /// callouts; selected rows use the neutral `selection` role. The solid
    /// pair from [`Self::semantic_tone_colors`] is for filled controls.
    pub fn semantic_tone_soft_colors(&self, tone: SemanticTone) -> (Color, Color) {
        match tone {
            SemanticTone::Neutral => (self.palette.control, self.palette.text_muted),
            SemanticTone::Accent => (self.palette.accent_soft, self.palette.accent_soft_text),
            SemanticTone::Info => (self.palette.info_soft, self.palette.info_soft_text),
            SemanticTone::Success => (self.palette.success_soft, self.palette.success_soft_text),
            SemanticTone::Warning => (self.palette.warning_soft, self.palette.warning_soft_text),
            SemanticTone::Danger => (self.palette.danger_soft, self.palette.danger_soft_text),
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
        Self::sui()
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

fn rgb8(red: u8, green: u8, blue: u8) -> Color {
    Color::rgba(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        1.0,
    )
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
    use super::{
        Color, ControlSize, ControlTypography, DecorativeHue, DefaultTheme, GlowTone, SemanticTone,
        ThemeColorScheme, ThemeColors, ThemeDensity, ThemeShadow, ThemeTextScale, rgb8,
    };
    use crate::hdr_theme::HdrThemeMode;
    use sui_core::ColorSpace;
    use sui_runtime::OutputColorRange;

    #[test]
    fn glows_brighten_to_the_accents_hdr_color_where_the_output_and_theme_allow() {
        assert_eq!(
            DefaultTheme::light().glow_for_output(GlowTone::Accent, None),
            ThemeShadow::empty(),
            "light themes have no glow"
        );
        let dark = DefaultTheme::dark();
        assert_eq!(
            dark.glow_for_output(GlowTone::Accent, Some(OutputColorRange::HighDynamicRange)),
            dark.glows.accent,
            "the theme's token while its HDR mode is off"
        );

        let mut hdr = dark;
        hdr.hdr.mode = HdrThemeMode::FullHdr;
        let token = dark.glows.accent.first.unwrap();
        let bright = hdr
            .glow_for_output(GlowTone::Accent, Some(OutputColorRange::HighDynamicRange))
            .first
            .unwrap();
        assert_eq!(bright.color.space, ColorSpace::LinearDisplayP3);
        assert_eq!(bright.color.alpha, token.color.alpha);
        assert_eq!(bright.blur, token.blur);
        let wide = hdr
            .glow_for_output(GlowTone::Accent, Some(OutputColorRange::WideGamut))
            .first
            .unwrap();
        assert_eq!(wide.color.space, ColorSpace::DisplayP3);
        assert_eq!(
            hdr.glow_for_output(GlowTone::Accent, Some(OutputColorRange::Standard)),
            dark.glows.accent,
            "a standard output gets the token"
        );
        assert_ne!(
            hdr.glow_for_output(
                GlowTone::Secondary,
                Some(OutputColorRange::HighDynamicRange)
            )
            .first
            .unwrap()
            .color,
            bright.color,
            "the secondary glow takes the secondary color"
        );
    }

    #[test]
    fn default_theme_uses_body_text_scale_for_typography() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.typography.body_font_size, theme.text.base.size);
        assert_eq!(
            theme.typography.body_line_height,
            theme.text.base.line_height
        );
        assert_eq!(theme.typography.body_font_size, 15.0);
        assert_eq!(theme.typography.body_line_height, 22.0);
        assert_eq!(theme.control_size, Some(ControlSize::Medium));
        assert_eq!(theme.density, ThemeDensity::Comfortable);
        assert_eq!(theme.metrics.min_height, 32.0);
        assert_eq!(theme.metrics.touch_target_size, 44.0);
        assert_eq!(theme.metrics.icon_button_size, 32.0);
        assert_eq!(
            theme.body_text_style().font_families,
            Some(theme.fonts.sans.into())
        );
        assert_eq!(
            theme.serif_text_style(theme.palette.text).font_families,
            Some(theme.fonts.serif.into())
        );
        assert_eq!(
            theme.mono_text_style(theme.palette.text).font_families,
            Some(theme.fonts.mono.into())
        );
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
        assert_eq!(compact.interaction.pressed_offset, 0.0);
        assert_eq!(comfortable.interaction.pressed_offset, 0.0);
        assert_eq!(touch.interaction.pressed_offset, 0.0);
    }

    #[test]
    fn contextual_control_sizes_match_authored_geometry_and_typography() {
        assert_eq!(ControlSize::default(), ControlSize::Medium);

        let base = DefaultTheme::default();
        let small = base.with_size(ControlSize::Small);
        let medium = base.with_size(ControlSize::Medium);
        let large = base.with_size(ControlSize::Large);

        assert_eq!(small.density, ThemeDensity::Compact);
        assert_eq!(medium.density, ThemeDensity::Comfortable);
        assert_eq!(large.density, ThemeDensity::Touch);
        assert_eq!(small.metrics.min_height, 28.0);
        assert_eq!(medium.metrics.min_height, 32.0);
        assert_eq!(large.metrics.min_height, 40.0);
        assert_eq!(small.metrics.list_row_height, 24.0);
        assert_eq!(medium.metrics.list_row_height, 28.0);
        assert_eq!(large.metrics.list_row_height, 36.0);
        assert_eq!(small.metrics.touch_target_size, 44.0);
        assert_eq!(medium.metrics.touch_target_size, 44.0);
        assert_eq!(large.metrics.touch_target_size, 44.0);
        assert_eq!(small.typography.body_font_size, base.text.sm.size);
        assert_eq!(medium.typography.body_font_size, base.text.base.size);
        assert_eq!(large.typography.body_font_size, base.text.lg.size);
        assert_eq!(large.text, base.text);
        assert_eq!(small.metrics.menu_row_height, 24.0);
        assert_eq!(medium.metrics.menu_row_height, 28.0);
        assert_eq!(large.metrics.menu_row_height, 36.0);
    }

    #[test]
    fn contextual_control_size_uses_the_callers_scaled_text_ramp() {
        let mut theme = DefaultTheme::default();
        theme.text.sm.size = 15.0;
        theme.text.base.size = 17.0;
        theme.text.lg.size = 19.0;

        let small = theme.with_size(ControlSize::Small);
        let medium = theme.with_size(ControlSize::Medium);
        let large = theme.with_size(ControlSize::Large);

        assert_eq!(small.typography.body_font_size, 15.0);
        assert_eq!(medium.typography.body_font_size, 17.0);
        assert_eq!(large.typography.body_font_size, 19.0);
    }

    #[test]
    fn contextual_control_size_survives_derived_theme_refresh() {
        let mut theme = DefaultTheme::dark().with_size(ControlSize::Small);
        theme.text.sm.size = 14.5;

        theme.sync_derived_fields();

        assert_eq!(theme.control_size, Some(ControlSize::Small));
        assert_eq!(theme.metrics.min_height, 28.0);
        assert_eq!(theme.metrics.touch_target_size, 44.0);
        assert_eq!(theme.typography.body_font_size, 14.5);
    }

    #[test]
    fn default_theme_initializes_hdr_tokens() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.hdr.mode, HdrThemeMode::Disabled);
        assert_eq!(
            theme.hdr.color_roles.surface.sdr,
            theme.colors.neutrals.window
        );
        assert_eq!(theme.hdr.color_roles.accent.sdr, theme.colors.primary);
        assert_eq!(
            theme.hdr.color_roles.accent_text.sdr,
            theme.colors.on_primary
        );
    }

    fn built_in_presets() -> [DefaultTheme; 5] {
        [
            DefaultTheme::light(),
            DefaultTheme::dark(),
            DefaultTheme::void(),
            DefaultTheme::neutral(),
            DefaultTheme::neutral_dark(),
        ]
    }

    fn assert_contrast(label: &str, foreground: Color, background: Color, minimum: f32) {
        let ratio = foreground.contrast_ratio(background);
        assert!(
            ratio >= minimum,
            "{label}: contrast {ratio:.2} is below {minimum} ({foreground:?} on {background:?})"
        );
    }

    fn hue_distance(first: f32, second: f32) -> f32 {
        let distance = (first - second).rem_euclid(360.0);
        distance.min(360.0 - distance)
    }

    #[test]
    fn sui_presets_share_the_azure_brand_across_schemes() {
        let light = ThemeColors::light();
        let dark = ThemeColors::dark();
        let void = ThemeColors::high_contrast();

        assert_eq!(light.primary, rgb8(23, 98, 244));
        assert_eq!(light.on_primary, Color::WHITE);
        assert_eq!(light.secondary, rgb8(125, 77, 231));
        for colors in [dark, void] {
            assert_eq!(colors.primary, light.primary);
            assert_eq!(colors.secondary, light.secondary);
            assert_eq!(colors.danger, light.danger);
            assert_eq!(colors.success, light.success);
            assert_eq!(colors.warning, light.warning);
            assert_eq!(colors.info, light.info);
        }
        assert_eq!(void.neutrals.window, Color::BLACK);
        assert_eq!(void.neutrals.panel, Color::BLACK);
    }

    #[test]
    fn light_surfaces_are_pure_and_dark_surfaces_carry_a_constant_faint_tint() {
        let light = ThemeColors::light().neutrals;
        for color in [
            light.window,
            light.subtle,
            light.panel,
            light.control,
            light.control_hover,
            light.border,
            light.border_strong,
            light.border_control,
            light.text,
            light.text_secondary,
            light.text_tertiary,
        ] {
            assert!(
                color.to_oklch().chroma < 1.0e-3,
                "light neutrals must be achromatic: {color:?}"
            );
        }

        let dark = ThemeColors::dark().neutrals;
        for color in [
            dark.window,
            dark.subtle,
            dark.panel,
            dark.overlay,
            dark.control,
            dark.control_hover,
            dark.border,
        ] {
            let oklch = color.to_oklch();
            assert!(
                (0.012..=0.022).contains(&oklch.chroma),
                "dark surface tint drifted: {oklch:?}"
            );
            assert!(
                (245.0..=270.0).contains(&oklch.hue),
                "dark surfaces lean blue: {oklch:?}"
            );
        }
        // Text reads white, not blue: the tint lives on the surfaces.
        assert!(dark.text.to_oklch().chroma < 0.01);

        let neutral_dark = ThemeColors::neutral_dark().neutrals;
        assert!(neutral_dark.panel.to_oklch().chroma < 1.0e-3);
        assert!(
            (neutral_dark.panel.to_oklch().lightness - dark.panel.to_oklch().lightness).abs()
                < 0.01,
            "the neutral dark ramp is the untinted twin of the SUI dark ramp"
        );
    }

    #[test]
    fn built_in_presets_meet_contrast_floors() {
        for theme in built_in_presets() {
            let name = theme.colors.name;
            let palette = theme.palette;
            let panel = palette.surface_raised;

            assert_contrast(name, palette.text, panel, 12.0);
            assert_contrast(name, palette.text_muted, panel, 7.0);
            assert_contrast(name, palette.placeholder, panel, 4.5);
            assert_contrast(name, palette.placeholder, palette.surface, 4.5);
            assert_contrast(name, palette.border_control, panel, 3.0);
            assert_contrast(name, palette.focus_ring, panel, 3.0);
            assert_contrast(name, palette.accent_text, palette.accent, 4.5);
            assert_contrast(name, palette.danger_text, palette.danger, 4.5);
            assert_contrast(name, palette.success_text, palette.success, 4.5);
            assert_contrast(name, palette.warning_text, palette.warning, 4.5);
            assert_contrast(name, palette.info_text, palette.info, 4.5);
            for (soft, text) in [
                (palette.accent_soft, palette.accent_soft_text),
                (palette.info_soft, palette.info_soft_text),
                (palette.success_soft, palette.success_soft_text),
                (palette.warning_soft, palette.warning_soft_text),
                (palette.danger_soft, palette.danger_soft_text),
            ] {
                assert_contrast(name, text, soft, 4.5);
                assert_contrast(name, text, panel, 4.5);
            }
            for hue in DecorativeHue::ALL {
                let roles = theme.decorative.get(hue);
                assert_contrast(name, roles.text, roles.soft, 4.5);
                // Mid-lightness decorative fills carry large labels only
                // (avatars, counters); body-size labels use soft + text.
                assert_contrast(name, roles.on_solid, roles.solid, 3.0);
            }
        }
    }

    #[test]
    fn interaction_roles_are_neutral_while_focus_follows_the_brand() {
        for theme in built_in_presets() {
            let palette = theme.palette;
            assert_eq!(palette.caret, palette.text);
            assert!(palette.selection.to_oklch().chroma < 0.02);
            assert_eq!(palette.selection_border, palette.accent_border);
            assert_eq!(palette.surface_focus, palette.field);
            assert_ne!(palette.border_focus, palette.border_strong);
            assert_eq!(palette.focus, palette.focus_ring);
            assert_eq!(theme.surfaces.selected, palette.selection);
            assert_eq!(theme.surfaces.selected_border, palette.selection_border);
            let focus_hue = palette.focus.to_oklch().hue;
            let primary = theme.colors.primary.to_oklch();
            if primary.chroma > 0.04 {
                assert!(hue_distance(focus_hue, primary.hue) < 2.0);
            }
        }
    }

    #[test]
    fn changing_primary_rederives_every_accent_role() {
        let mut colors = ThemeColors::light();
        colors.primary = Color::rgba(0.86, 0.12, 0.47, 1.0);
        let theme = DefaultTheme::from_colors(colors);
        let hue = colors.primary.to_oklch().hue;

        assert_eq!(theme.palette.accent, colors.primary);
        // Washes flattened in encoded space drift slightly in hue as they
        // approach white, so they get a looser tolerance than solid roles.
        for (role, tolerance) in [
            (theme.palette.accent_hover, 3.0),
            (theme.palette.accent_pressed, 3.0),
            (theme.palette.accent_soft_text, 3.0),
            (theme.palette.focus_ring, 3.0),
            (theme.palette.accent_soft, 20.0),
            (theme.palette.accent_border, 20.0),
            (theme.palette.selection_border, 20.0),
        ] {
            let role_hue = role.to_oklch().hue;
            assert!(
                hue_distance(role_hue, hue) < tolerance,
                "role {role:?} must follow the edited primary hue {hue}, got {role_hue}"
            );
        }
        assert!(
            theme.palette.accent_hover.to_oklch().lightness < colors.primary.to_oklch().lightness,
            "light themes darken on hover"
        );
        let glow = DefaultTheme::from_colors(ThemeColors {
            primary: colors.primary,
            ..ThemeColors::dark()
        })
        .glows
        .accent
        .first
        .expect("dark themes glow");
        assert!(hue_distance(glow.color.to_oklch().hue, hue) < 3.0);
    }

    #[test]
    fn neutral_presets_have_an_achromatic_brand_and_keep_semantic_colors() {
        let light = DefaultTheme::neutral();
        let dark = DefaultTheme::neutral_dark();

        assert_eq!(light.colors.name, "neutral");
        assert_eq!(light.colors.neutrals, ThemeColors::light().neutrals);
        assert_eq!(light.colors.primary, rgb8(23, 23, 23));
        assert_eq!(dark.colors.name, "neutral-dark");
        assert_eq!(dark.colors.primary, rgb8(240, 240, 240));
        for theme in [light, dark] {
            for color in [
                theme.colors.primary,
                theme.colors.secondary,
                theme.palette.accent,
                theme.palette.accent_hover,
                theme.palette.focus,
                theme.palette.selection,
            ] {
                assert!(color.to_oklch().chroma < 1.0e-3, "{color:?}");
            }
            assert_eq!(theme.colors.danger, ThemeColors::light().danger);
            assert_eq!(theme.hdr.color_roles.accent.wide_gamut, None);
            assert_eq!(theme.hdr.color_roles.accent.hdr, None);
            assert!(theme.hdr.color_roles.danger.wide_gamut.is_some());
        }
        // Light themes lighten a near-black primary on hover.
        assert!(
            light.palette.accent_hover.to_oklch().lightness
                > light.palette.accent.to_oklch().lightness
        );
        let glow = dark.glows.accent.first.expect("neutral dark glows quietly");
        assert!(glow.color.alpha < DefaultTheme::dark().glows.accent.first.unwrap().color.alpha);
    }

    #[test]
    fn neutral_buttons_use_the_raised_face_and_full_ink() {
        let light = DefaultTheme::light();
        assert_eq!(
            light.semantic_tone_colors(SemanticTone::Neutral),
            (light.palette.button, light.palette.text)
        );
        assert_eq!(light.palette.button, Color::WHITE);
        assert_eq!(light.palette.button_border, light.palette.border_strong);
        assert!(
            light.palette.button_hover.relative_luminance()
                < light.palette.button.relative_luminance()
        );
        assert!(
            light.palette.button_pressed.relative_luminance()
                < light.palette.button_hover.relative_luminance()
        );

        let dark = DefaultTheme::dark();
        assert!(
            dark.palette.button_hover.relative_luminance()
                > dark.palette.button.relative_luminance()
        );
    }

    #[test]
    fn decorative_palette_derives_roles_for_every_hue() {
        let theme = DefaultTheme::light();
        let mut hues = Vec::new();
        for hue in DecorativeHue::ALL {
            let roles = theme.decorative.get(hue);
            assert_eq!(roles.solid, theme.colors.decorative.get(hue));
            assert!(
                roles.solid.to_oklch().chroma > 0.10,
                "{hue:?} must be vibrant"
            );
            hues.push(roles.solid.to_oklch().hue);
        }
        for (index, first) in hues.iter().enumerate() {
            for second in &hues[index + 1..] {
                assert!(
                    hue_distance(*first, *second) > 15.0,
                    "decorative hues must stay distinct"
                );
            }
        }
        assert_eq!(theme.decorative.categorical(0), theme.decorative.red);
        assert_eq!(theme.decorative.categorical(10), theme.decorative.orange);
    }

    #[test]
    fn elevation_follows_the_mesh_ladder_per_scheme() {
        let light = DefaultTheme::light();
        let dark = DefaultTheme::dark();
        let void = DefaultTheme::void();

        // Light casts faint ink shadows.
        let light_sm = light.shadows.box_shadow.xs.first.expect("light xs shadow");
        assert!(light_sm.color.alpha > 0.0 && light_sm.color.alpha < 0.1);
        // Dark casts deeper black shadows.
        let dark_sm = dark.shadows.box_shadow.xs.first.expect("dark xs shadow");
        assert!(dark_sm.color.alpha > light_sm.color.alpha);
        // Void casts none: elevation is drawn with borders.
        assert_eq!(void.shadows.box_shadow.xs, ThemeShadow::empty());
        assert_eq!(void.shadows.box_shadow._2xl, ThemeShadow::empty());
        assert_eq!(void.shadows.drop.sm, ThemeShadow::empty());

        // Glows: absent in Light, present in Dark, damped in Void.
        assert_eq!(light.glows.accent, ThemeShadow::empty());
        let dark_glow = dark.glows.accent.first.expect("dark accent glow");
        let void_glow = void.glows.accent.first.expect("void accent glow");
        assert!(dark_glow.blur > void_glow.blur);
        assert!(dark_glow.color.alpha > void_glow.color.alpha);
        assert_eq!(dark_glow.offset_x, 0.0);
        assert_eq!(dark_glow.offset_y, 0.0);
    }

    #[test]
    fn density_tiers_match_the_mesh_contract() {
        let text = ThemeTextScale::default();
        assert_eq!(
            ControlTypography::for_density(&text, ThemeDensity::Compact),
            ControlTypography::for_size(&text, ControlSize::Small)
        );
        assert_eq!(
            ControlTypography::for_density(&text, ThemeDensity::Comfortable),
            ControlTypography::for_size(&text, ControlSize::Medium)
        );
        assert_eq!(
            ControlTypography::for_density(&text, ThemeDensity::Touch),
            ControlTypography::for_size(&text, ControlSize::Large)
        );
        let legacy_compact = DefaultTheme::default().with_density(ThemeDensity::Compact);
        let legacy_comfortable = DefaultTheme::default().with_density(ThemeDensity::Comfortable);
        let legacy_touch = DefaultTheme::default().with_density(ThemeDensity::Touch);
        assert_eq!(legacy_compact.typography.body_font_size, text.sm.size);
        assert_eq!(legacy_comfortable.typography.body_font_size, text.base.size);
        assert_eq!(legacy_touch.typography.body_font_size, text.lg.size);

        let compact = DefaultTheme::compact();
        let comfortable = DefaultTheme::comfortable();
        let touch = DefaultTheme::touch();

        // Authored control heights: 28 / 32 / 40, with a separate 44px target.
        assert_eq!(compact.metrics.min_height, 28.0);
        assert_eq!(comfortable.metrics.min_height, 32.0);
        assert_eq!(touch.metrics.min_height, 40.0);
        assert_eq!(compact.metrics.touch_target_size, 44.0);
        assert_eq!(comfortable.metrics.touch_target_size, 44.0);
        assert_eq!(touch.metrics.touch_target_size, 44.0);

        // Rows: 24 / 28 / 36, shared by lists, trees, tables, and menus.
        assert_eq!(compact.metrics.list_row_height, 24.0);
        assert_eq!(comfortable.metrics.list_row_height, 28.0);
        assert_eq!(touch.metrics.list_row_height, 36.0);
        for theme in [&compact, &comfortable, &touch] {
            assert_eq!(theme.metrics.tree_row_height, theme.metrics.list_row_height);
            assert_eq!(
                theme.metrics.table_row_height,
                theme.metrics.list_row_height
            );
            assert_eq!(theme.metrics.menu_row_height, theme.metrics.list_row_height);
        }

        // Control type follows Small / Medium / Large on the shared text ramp.
        assert_eq!(compact.typography.body_font_size, 13.0);
        assert_eq!(comfortable.typography.body_font_size, 15.0);
        assert_eq!(touch.typography.body_font_size, 17.0);

        // Control radius: 6 compact/comfortable, 8 touch.
        assert_eq!(compact.metrics.corner_radius, 6.0);
        assert_eq!(comfortable.metrics.corner_radius, 6.0);
        assert_eq!(touch.metrics.corner_radius, 8.0);

        // The switch is a fixed 32x19 control at every density.
        for theme in [&compact, &comfortable, &touch] {
            assert_eq!(theme.metrics.switch_track_width, 32.0);
            assert_eq!(theme.metrics.switch_track_height, 19.0);
            assert_eq!(theme.metrics.checkbox_indicator_size, 15.0);
        }
    }

    #[test]
    fn mesh_motion_ladder_is_70_140_220_340() {
        let motion = super::ThemeMotion::standard();
        assert_eq!(motion.duration_fast, 0.07);
        assert_eq!(motion.duration_normal, 0.14);
        assert_eq!(motion.duration_slow, 0.22);
        assert_eq!(motion.duration_slower, 0.34);
    }

    #[test]
    fn light_and_dark_themes_derive_hdr_role_colors_from_semantics() {
        let light = DefaultTheme::light();
        let dark = DefaultTheme::dark();

        assert_eq!(
            light.hdr.color_roles.surface.sdr,
            light.colors.neutrals.window
        );
        assert_eq!(light.hdr.color_roles.text.sdr, light.colors.neutrals.text);
        assert_eq!(
            dark.hdr.color_roles.surface.sdr,
            dark.colors.neutrals.window
        );
        assert_eq!(dark.hdr.color_roles.accent.sdr, dark.colors.primary);
    }

    #[test]
    fn sync_derived_fields_updates_semantic_palette_and_typography() {
        let mut theme = DefaultTheme::default();
        theme.colors.primary = Color::rgba(0.2, 0.3, 0.4, 1.0);
        theme.text.base.size = 11.0;
        theme.text.base.line_height = 15.0;
        theme.text.lg.size = 15.0;
        theme.text.lg.line_height = 22.0;
        theme.density = ThemeDensity::Touch;
        theme.sync_derived_fields();

        assert_eq!(theme.palette.accent, Color::rgba(0.2, 0.3, 0.4, 1.0));
        assert_eq!(theme.palette.caret, theme.colors.neutrals.text);
        assert_eq!(theme.surfaces.accent, theme.palette.accent);
        assert_eq!(theme.surfaces.window, theme.palette.surface);
        // Mesh tooltips are quiet floating surfaces: overlay fill, secondary ink.
        assert_eq!(theme.surfaces.tooltip, theme.surfaces.overlay);
        assert_eq!(theme.surfaces.tooltip_text, theme.palette.text_muted);
        assert!(theme.surfaces.overlay_scrim.alpha > 0.0);
        // Touch density reads at the lg body size per the Mesh density contract.
        assert_eq!(theme.typography.body_font_size, 15.0);
        assert_eq!(theme.typography.body_line_height, 22.0);
        assert_eq!(
            theme.metrics.min_height,
            DefaultTheme::default()
                .with_density(ThemeDensity::Touch)
                .metrics
                .min_height
        );
        assert_eq!(
            theme.interaction.pressed_offset,
            DefaultTheme::default()
                .with_density(ThemeDensity::Touch)
                .interaction
                .pressed_offset
        );
    }

    #[test]
    fn control_palette_exposes_semantic_status_colors() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.palette.info, theme.colors.info);
        assert_eq!(theme.palette.info_text, theme.colors.on_info);
        assert_eq!(theme.palette.success, theme.colors.success);
        assert_eq!(theme.palette.success_text, theme.colors.on_success);
        assert_eq!(theme.palette.warning, theme.colors.warning);
        assert_eq!(theme.palette.warning_text, theme.colors.on_warning);
        assert_eq!(theme.palette.danger, theme.colors.danger);
        assert_eq!(theme.palette.danger_text, theme.colors.on_danger);
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
        let stale_wide_gamut = Color::display_p3(0.9, 0.4, 0.2, 1.0);
        let stale_hdr = Color::linear_display_p3(1.6, 0.5, 0.3, 1.0);

        theme.hdr.color_roles.accent.wide_gamut = Some(stale_wide_gamut);
        theme.hdr.color_roles.accent.hdr = Some(stale_hdr);
        theme.colors = ThemeColors::dark();
        theme.colors.neutrals.window = Color::rgba(0.96, 0.97, 0.98, 1.0);
        theme.sync_derived_fields();

        let accent = theme.hdr.color_roles.accent;
        assert_eq!(
            theme.hdr.color_roles.surface.sdr,
            theme.colors.neutrals.window
        );
        assert_eq!(accent.sdr, theme.colors.primary);
        let wide = accent
            .wide_gamut
            .expect("chromatic accent has a P3 variant");
        assert_ne!(wide, stale_wide_gamut);
        assert!(wide.to_oklch().chroma > theme.colors.primary.to_oklch().chroma);
        assert!(hue_distance(wide.to_oklch().hue, theme.colors.primary.to_oklch().hue) < 2.0);
        let hdr = accent.hdr.expect("the accent is a live-signal role");
        assert!(hdr.to_linear_srgb().blue > wide.to_linear_srgb().blue);
    }

    #[test]
    fn dark_theme_uses_professional_dark_tokens() {
        let theme = DefaultTheme::dark();

        assert_eq!(theme.colors.scheme, ThemeColorScheme::Dark);
        assert_eq!(theme.colors.name, "dark");
        assert_ne!(theme.colors.neutrals.window, Color::BLACK);
        assert_eq!(theme.palette.surface, theme.colors.neutrals.window);
        assert_ne!(theme.palette.surface_raised, Color::BLACK);
        assert_eq!(theme.palette.text, theme.colors.neutrals.text);
        assert_ne!(theme.palette.text, Color::WHITE);
        assert_eq!(theme.palette.caret, theme.colors.neutrals.text);
        assert_eq!(theme.palette.accent, theme.colors.primary);
        assert_eq!(theme.palette.accent_text, theme.colors.on_primary);
        assert_eq!(theme.surfaces.window, theme.palette.surface);
        assert_eq!(theme.surfaces.panel, theme.palette.surface_raised);
        assert_eq!(theme.surfaces.border, theme.palette.border);
        assert_ne!(theme.surfaces.border, Color::WHITE);
        assert_ne!(theme.surfaces.text_faint, theme.palette.surface);
    }

    #[test]
    fn high_contrast_scheme_uses_true_black_oled_palette() {
        let theme = DefaultTheme::high_contrast();

        assert_eq!(theme.colors.scheme, ThemeColorScheme::HighContrast);
        assert_eq!(theme.colors.name, "void");
        assert_eq!(theme.palette.surface, theme.colors.neutrals.window);
        assert_eq!(theme.palette.surface, Color::BLACK);
        assert_eq!(theme.surfaces.window, Color::BLACK);
        // The Void OLED contract keeps cards true black — the border is the
        // card. Only input wells and hover fills lift off black.
        assert_eq!(theme.palette.surface_raised, Color::BLACK);
        assert!(theme.palette.border.alpha > 0.0);
        assert_ne!(theme.palette.control, Color::BLACK);
        assert_ne!(theme.palette.field, Color::BLACK);
        assert_ne!(theme.palette.text, Color::WHITE);
        assert_ne!(theme.palette.control_hover, Color::BLACK);
        assert_ne!(theme.palette.control_active, Color::BLACK);
        assert_ne!(theme.palette.surface_focus, Color::BLACK);
        assert_eq!(theme.palette.text, theme.colors.neutrals.text);
        assert_eq!(
            theme.metrics.border_width,
            DefaultTheme::default().metrics.border_width
        );
        assert_eq!(
            theme.metrics.focus_ring_width,
            DefaultTheme::default().metrics.focus_ring_width
        );

        let touch = DefaultTheme::high_contrast().with_density(ThemeDensity::Touch);
        assert_eq!(touch.density, ThemeDensity::Touch);
        assert!(touch.metrics.min_height > theme.metrics.min_height);
        assert_eq!(
            touch.metrics.border_width,
            DefaultTheme::touch().metrics.border_width
        );
        assert_eq!(
            touch.metrics.focus_ring_width,
            DefaultTheme::touch().metrics.focus_ring_width
        );
    }
}
