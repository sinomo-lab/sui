//! The Themes page: built-in theme previews and the HDR theme lab.

#![forbid(unsafe_code)]

use std::{
    rc::Rc,
    sync::{OnceLock, RwLock},
};

use sui::prelude::*;
use sui::{
    HdrLuminanceTokens, HdrThemeMode, HdrThemeTokens, Rect, SemanticColorToken, SemanticsNode,
    SemanticsRole, WidgetColorRole, WidgetLuminanceRole, WidgetMaterialRole, WidgetPodMutVisitor,
    WidgetPodVisitor, resolve_semantic_color, resolve_widget_hdr_style,
};

use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};
use crate::demo_support::*;
use crate::live_performance::LivePerformanceRoot;

pub const THEME_DEMO_TITLE: &str = "Themes";
pub const THEME_DEMO_DESCRIPTION: &str =
    "Compare the SUI and neutral presets across standard, dark, OLED, and HDR UI styling.";

pub const THEME_DEMO_SCROLL_NAME: &str = "Theme demo gallery";
pub const THEME_PREVIEW_NAME: &str = "Theme preview showcase";
pub const LIGHT_THEME_PREVIEW_CARD_NAME: &str = "Light theme preview card";
pub const NEUTRAL_THEME_PREVIEW_CARD_NAME: &str = "Neutral theme preview card";
pub const DARK_THEME_PREVIEW_CARD_NAME: &str = "Dark theme preview card";
pub const NEUTRAL_DARK_THEME_PREVIEW_CARD_NAME: &str = "Neutral dark theme preview card";
pub const TRUE_BLACK_THEME_PREVIEW_CARD_NAME: &str = "True black theme preview card";
pub const HDR_THEME_LAB_NAME: &str = "HDR theme mode lab";
pub const HDR_THEME_LAB_ACTIVE_PREVIEW_NAME: &str = "Current HDR theme mode preview";
pub const LIGHT_PREVIEW_ACTION_LABEL: &str = "Light preview action";
pub const NEUTRAL_PREVIEW_ACTION_LABEL: &str = "Neutral preview action";
pub const DARK_PREVIEW_ACTION_LABEL: &str = "Dark preview action";
pub const NEUTRAL_DARK_PREVIEW_ACTION_LABEL: &str = "Neutral dark preview action";
pub const TRUE_BLACK_PREVIEW_ACTION_LABEL: &str = "True black preview action";
pub const LIGHT_PREVIEW_INPUT_LABEL: &str = "Light preview query";
pub const NEUTRAL_PREVIEW_INPUT_LABEL: &str = "Neutral preview query";
pub const DARK_PREVIEW_INPUT_LABEL: &str = "Dark preview query";
pub const NEUTRAL_DARK_PREVIEW_INPUT_LABEL: &str = "Neutral dark preview query";
pub const TRUE_BLACK_PREVIEW_INPUT_LABEL: &str = "True black preview query";

pub(crate) fn hdr_theme_lab_mode_store() -> &'static RwLock<HdrThemeMode> {
    static STORE: OnceLock<RwLock<HdrThemeMode>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HdrThemeMode::Disabled))
}

pub fn hdr_theme_lab_mode() -> HdrThemeMode {
    *hdr_theme_lab_mode_store()
        .read()
        .expect("widget-book HDR theme mode lock should not be poisoned")
}

pub fn set_hdr_theme_lab_mode(mode: HdrThemeMode) {
    *hdr_theme_lab_mode_store()
        .write()
        .expect("widget-book HDR theme mode lock should not be poisoned") = mode;
}

pub fn build_theme_demo_application() -> Application {
    set_hdr_theme_lab_mode(HdrThemeMode::Disabled);

    App::new()
        .window(Window::new(THEME_DEMO_TITLE).root(LivePerformanceRoot::new(
            THEME_DEMO_TITLE,
            THEME_DEMO_DESCRIPTION,
            build_theme_demo_surface(),
        )))
        .into_application()
}

pub(crate) struct ThemePreviewGrid {
    cards: WidgetChildren,
    /// Natural card heights from the last measure pass. Rows size to their
    /// tallest card so wrapped descriptions never squeeze card content.
    card_heights: Vec<f32>,
}

impl ThemePreviewGrid {
    const GAP: f32 = 16.0;
    const MIN_CARD_HEIGHT: f32 = 248.0;

    fn new() -> Self {
        let mut cards = WidgetChildren::with_capacity(5);
        for (name, theme, title, action_label, input_label) in [
            (
                LIGHT_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::sui(),
                "SUI light",
                LIGHT_PREVIEW_ACTION_LABEL,
                LIGHT_PREVIEW_INPUT_LABEL,
            ),
            (
                NEUTRAL_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::neutral(),
                "Neutral light",
                NEUTRAL_PREVIEW_ACTION_LABEL,
                NEUTRAL_PREVIEW_INPUT_LABEL,
            ),
            (
                DARK_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::dark(),
                "SUI dark",
                DARK_PREVIEW_ACTION_LABEL,
                DARK_PREVIEW_INPUT_LABEL,
            ),
            (
                NEUTRAL_DARK_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::neutral_dark(),
                "Neutral dark",
                NEUTRAL_DARK_PREVIEW_ACTION_LABEL,
                NEUTRAL_DARK_PREVIEW_INPUT_LABEL,
            ),
            (
                TRUE_BLACK_THEME_PREVIEW_CARD_NAME,
                DefaultTheme::high_contrast(),
                "SUI true black",
                TRUE_BLACK_PREVIEW_ACTION_LABEL,
                TRUE_BLACK_PREVIEW_INPUT_LABEL,
            ),
        ] {
            cards.push(NamedSection::new(
                name,
                theme_preview_card(theme, title, action_label, input_label),
            ));
        }
        Self {
            cards,
            card_heights: Vec::new(),
        }
    }

    fn columns_for_width(width: f32) -> usize {
        if width >= 1040.0 {
            3
        } else if width >= 680.0 {
            2
        } else {
            1
        }
    }

    fn column_width(width: f32, columns: usize) -> f32 {
        ((width - Self::GAP * columns.saturating_sub(1) as f32).max(0.0) / columns as f32).max(0.0)
    }

    /// Height of each row: the tallest card in it, never below the minimum.
    fn row_heights(&self, columns: usize) -> Vec<f32> {
        (0..self.cards.len())
            .step_by(columns.max(1))
            .map(|start| {
                self.card_heights
                    .iter()
                    .skip(start)
                    .take(columns.max(1))
                    .fold(Self::MIN_CARD_HEIGHT, |height, card| height.max(*card))
            })
            .collect()
    }

    fn content_height(&self, columns: usize) -> f32 {
        let rows = self.row_heights(columns);
        rows.iter().sum::<f32>() + rows.len().saturating_sub(1) as f32 * Self::GAP
    }
}

impl Widget for ThemePreviewGrid {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(1120.0)
        };
        let columns = Self::columns_for_width(width);
        let column_width = Self::column_width(width, columns);
        // Fix the width but let each card report its natural height.
        let card_constraints = Constraints::new(
            Size::new(column_width, Self::MIN_CARD_HEIGHT),
            Size::new(column_width, f32::INFINITY),
        );
        self.card_heights = (0..self.cards.len())
            .map(|index| {
                self.cards
                    .measure_child(index, ctx, card_constraints)
                    .height
            })
            .collect();
        constraints.clamp(Size::new(width, self.content_height(columns)))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let columns = Self::columns_for_width(bounds.width());
        let column_width = Self::column_width(bounds.width(), columns);
        let row_heights = self.row_heights(columns);
        let mut row_y = bounds.y();
        for (row, row_height) in row_heights.iter().copied().enumerate() {
            for column in 0..columns {
                let index = row * columns + column;
                if index >= self.cards.len() {
                    break;
                }
                self.cards.arrange_child(
                    index,
                    ctx,
                    Rect::new(
                        bounds.x() + column as f32 * (column_width + Self::GAP),
                        row_y,
                        column_width,
                        row_height,
                    ),
                );
            }
            row_y += row_height + Self::GAP;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.cards.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(THEME_PREVIEW_NAME.to_string());
        node.description =
            Some("Five built-in theme preview cards are visible in a responsive grid.".to_string());
        ctx.push(node);
        self.cards.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.cards.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.cards.visit_children_mut(visitor);
    }
}

pub(crate) fn hdr_theme_mode_title(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "SDR baseline",
        HdrThemeMode::WideGamutOnly => "Wide-gamut-only",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR",
        HdrThemeMode::FullHdr => "Full HDR",
    }
}

pub(crate) fn hdr_theme_mode_explanation(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => {
            "Uses the SDR fallback path only. Wide-gamut and HDR token branches stay available in the theme, but built-in widgets resolve to the existing SDR palette and luminance ceilings."
        }
        HdrThemeMode::WideGamutOnly => {
            "Prefers richer gamut variants while keeping luminance pinned to reference white. This validates color-volume differences without introducing above-white UI chrome."
        }
        HdrThemeMode::ConstrainedHdr => {
            "Allows a modest lift for accents, focused states, and emissive indicators while still treating reference white as the visual anchor."
        }
        HdrThemeMode::FullHdr => {
            "Allows the same semantic tokens to push farther into HDR headroom so popup arrivals and indicator energy can separate more clearly from the constrained path."
        }
    }
}

pub(crate) fn hdr_theme_lab_section_name(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "SDR baseline comparison",
        HdrThemeMode::WideGamutOnly => "Wide-gamut-only comparison",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR comparison",
        HdrThemeMode::FullHdr => "Full HDR comparison",
    }
}

pub(crate) fn hdr_theme_lab_theme(mode: HdrThemeMode) -> DefaultTheme {
    let mut theme = DefaultTheme::dark();
    theme.hdr = HdrThemeTokens::from_default_theme(theme);
    theme.hdr.mode = mode;
    theme.hdr.color_roles.surface = SemanticColorToken::from_sdr(theme.colors.neutrals.window)
        .with_wide_gamut(Color::display_p3(0.13, 0.16, 0.23, 1.0))
        .with_hdr(Color::linear_display_p3(0.18, 0.21, 0.30, 1.0));
    theme.hdr.color_roles.surface_elevated =
        SemanticColorToken::from_sdr(theme.colors.neutrals.panel)
            .with_wide_gamut(Color::display_p3(0.16, 0.19, 0.28, 1.0))
            .with_hdr(Color::linear_display_p3(0.24, 0.27, 0.38, 1.0));
    theme.hdr.color_roles.surface_outline =
        SemanticColorToken::from_sdr(theme.colors.neutrals.border)
            .with_wide_gamut(Color::display_p3(0.33, 0.39, 0.50, 1.0))
            .with_hdr(Color::linear_display_p3(0.42, 0.48, 0.62, 1.0));
    theme.hdr.color_roles.text = SemanticColorToken::from_sdr(theme.colors.neutrals.text)
        .with_wide_gamut(Color::display_p3(0.92, 0.95, 0.99, 1.0))
        .with_hdr(Color::linear_display_p3(1.02, 1.04, 1.10, 1.0));
    theme.hdr.color_roles.text_muted =
        SemanticColorToken::from_sdr(theme.colors.neutrals.text.with_alpha(0.74))
            .with_wide_gamut(Color::display_p3(0.75, 0.80, 0.89, 1.0))
            .with_hdr(Color::linear_display_p3(0.86, 0.90, 0.98, 1.0));
    theme.hdr.color_roles.accent = SemanticColorToken::from_sdr(theme.colors.primary)
        .with_wide_gamut(Color::display_p3(0.18, 0.74, 0.96, 1.0))
        .with_hdr(Color::linear_display_p3(0.78, 2.40, 3.20, 1.0));
    theme.hdr.color_roles.accent_text = SemanticColorToken::from_sdr(theme.colors.on_primary)
        .with_wide_gamut(Color::display_p3(0.03, 0.08, 0.12, 1.0))
        .with_hdr(Color::linear_display_p3(0.10, 0.14, 0.20, 1.0));
    theme.hdr.color_roles.secondary = SemanticColorToken::from_sdr(theme.colors.secondary)
        .with_wide_gamut(Color::display_p3(0.43, 0.66, 0.98, 1.0))
        .with_hdr(Color::linear_display_p3(0.96, 1.72, 2.42, 1.0));
    theme.hdr.color_roles.warning = SemanticColorToken::from_sdr(theme.colors.warning)
        .with_wide_gamut(Color::display_p3(0.98, 0.68, 0.18, 1.0))
        .with_hdr(Color::linear_display_p3(3.00, 1.50, 0.30, 1.0));
    theme.hdr.color_roles.info = SemanticColorToken::from_sdr(theme.colors.info)
        .with_wide_gamut(Color::display_p3(0.40, 0.78, 0.98, 1.0))
        .with_hdr(Color::linear_display_p3(0.88, 2.00, 2.90, 1.0));
    theme.hdr.luminance = HdrLuminanceTokens::constrained_defaults();
    theme.hdr.policy.max_large_area_lift = 1.18;
    theme.hdr.policy.max_constrained_lift = 1.32;
    theme.hdr.policy.max_emissive_lift = 1.75;
    theme.hdr.effects.pulse.speed = 1.1;
    theme.hdr.effects.pulse.color = Some(resolve_semantic_color(
        theme.hdr.color_roles.warning,
        HdrThemeMode::FullHdr,
    ));

    match mode {
        HdrThemeMode::Disabled | HdrThemeMode::WideGamutOnly => {}
        HdrThemeMode::ConstrainedHdr => {
            theme.hdr.luminance.focused = 1.08;
            theme.hdr.luminance.semantic_accent = 1.16;
            theme.hdr.luminance.emissive_indicator = 1.55;
            theme.hdr.luminance.alert_pulse = 1.42;
        }
        HdrThemeMode::FullHdr => {
            theme.hdr.luminance.focused = 1.18;
            theme.hdr.luminance.semantic_accent = 1.34;
            theme.hdr.luminance.emissive_indicator = 2.40;
            theme.hdr.luminance.alert_pulse = 2.05;
            theme.hdr.policy.max_large_area_lift = 1.36;
            theme.hdr.policy.max_emissive_lift = 2.60;
            theme.hdr.materials.raised.specular_strength = 0.18;
            theme.hdr.materials.raised.rim_light_strength = 0.14;
            theme.hdr.effects.glow.intensity = 0.32;
            theme.hdr.effects.pulse.intensity = 0.54;
        }
    }

    theme
}

pub(crate) fn hdr_theme_lab_card(
    section_name: impl Into<String>,
    mode: HdrThemeMode,
    prefix: impl Into<String>,
    lead_text: impl Into<String>,
) -> impl Widget {
    let section_name = section_name.into();
    let prefix = prefix.into();
    let lead_text = lead_text.into();
    let theme = hdr_theme_lab_theme(mode);
    let indicator_style = resolve_widget_hdr_style(
        &theme.hdr,
        WidgetColorRole::Accent,
        WidgetLuminanceRole::EmissiveIndicator,
        WidgetMaterialRole::Flat,
        None,
    );
    let indicator_color = Color::new(
        indicator_style.color.space,
        indicator_style
            .color
            .red
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style
            .color
            .green
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style
            .color
            .blue
            .clamp(0.0, indicator_style.peak_lift),
        indicator_style.color.alpha,
    );
    let button_label = format!("{prefix} sample action");
    let switch_label = format!("{prefix} sample live indicator");
    let popover_name = format!("{prefix} attention popover");
    let popover_trigger_label = format!("{prefix} attention trigger");
    let swatch_name = format!("{prefix} emissive indicator");

    NamedSection::new(
        section_name,
        ThemePreviewCardFrame::new(
            theme,
            Stack::vertical()
                .spacing(12.0)
                .alignment(Alignment::Start)
                .with_child(
                    Label::new(hdr_theme_mode_title(mode)).style(demo_text_style(
                        theme,
                        DemoTextRole::Emphasis,
                        theme.palette.text,
                    )),
                )
                .with_child(MaximumWidth::new(
                    980.0,
                    Label::new(lead_text).style(demo_text_style(
                        theme,
                        DemoTextRole::Supporting,
                        theme.palette.placeholder,
                    )),
                ))
                .with_child(MaximumWidth::new(
                    980.0,
                    Label::new(format!(
                        "Token mode: {} · accent peak {:.2}× · indicator peak {:.2}× · alert peak {:.2}×",
                        hdr_theme_mode_title(mode),
                        theme.hdr.luminance.semantic_accent,
                        theme.hdr.luminance.emissive_indicator,
                        theme.hdr.luminance.alert_pulse,
                    ))
                    .style(demo_text_style(
                        theme,
                        DemoTextRole::Metadata,
                        theme.palette.placeholder,
                    )),
                ))
                .with_child(
                    Stack::horizontal()
                        .spacing(12.0)
                        .alignment(Alignment::Center)
                        .with_child(
                            SizedBox::new().width(300.0).with_child(
                                Button::primary(button_label)
                                    .min_width(280.0)
                                    .theme(theme),
                            ),
                        )
                        .with_child(
                            ColorSwatch::new(swatch_name, indicator_color)
                                .size(Size::new(64.0, 28.0))
                                .theme(theme),
                        )
                        .with_child(MaximumWidth::new(
                            520.0,
                            Label::new(
                                "The swatch mirrors the accent token resolved for the current gamut/HDR mode.",
                            )
                            .style(demo_text_style(
                                theme,
                                DemoTextRole::Metadata,
                                theme.palette.placeholder,
                            )),
                        )),
                )
                .with_child(
                    SizedBox::new().width(520.0).with_child(
                        Switch::new(switch_label)
                            .on(!matches!(mode, HdrThemeMode::Disabled))
                            .theme(theme),
                    ),
                )
                .with_child(
                    SizedBox::new().width(430.0).with_child(
                        Popover::new(
                            popover_name,
                            Button::new(popover_trigger_label)
                                .min_width(400.0)
                                .theme(theme),
                            MaximumWidth::new(
                                380.0,
                                Stack::vertical()
                                    .spacing(8.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(
                                        Label::new(
                                            "Small popup surfaces are where constrained vs full HDR arrival cues become easiest to validate.",
                                        )
                                        .style(demo_text_style(
                                            theme,
                                            DemoTextRole::Supporting,
                                            theme.palette.text,
                                        )),
                                    )
                                    .with_child(
                                        Label::new(
                                            "Use this trigger to compare popup chrome, border lift, and arrival emphasis against the matching button and switch.",
                                        )
                                        .style(demo_text_style(
                                            theme,
                                            DemoTextRole::Metadata,
                                            theme.palette.placeholder,
                                        )),
                                    )
                            ),
                        )
                        .theme(theme),
                    ),
                ),
        ),
    )
}

pub(crate) struct HdrThemeLabShowcase {
    active_mode: HdrThemeMode,
    active_preview: SingleChild,
    sdr_card: SingleChild,
    wide_gamut_card: SingleChild,
    constrained_card: SingleChild,
    full_hdr_card: SingleChild,
}

impl HdrThemeLabShowcase {
    const SECTION_GAP: f32 = 14.0;

    fn new() -> Self {
        let active_mode = hdr_theme_lab_mode();
        Self {
            active_mode,
            active_preview: SingleChild::new(Self::build_active_preview(active_mode)),
            sdr_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::Disabled),
                HdrThemeMode::Disabled,
                hdr_theme_mode_title(HdrThemeMode::Disabled),
                hdr_theme_mode_explanation(HdrThemeMode::Disabled),
            )),
            wide_gamut_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::WideGamutOnly),
                HdrThemeMode::WideGamutOnly,
                hdr_theme_mode_title(HdrThemeMode::WideGamutOnly),
                hdr_theme_mode_explanation(HdrThemeMode::WideGamutOnly),
            )),
            constrained_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::ConstrainedHdr),
                HdrThemeMode::ConstrainedHdr,
                hdr_theme_mode_title(HdrThemeMode::ConstrainedHdr),
                hdr_theme_mode_explanation(HdrThemeMode::ConstrainedHdr),
            )),
            full_hdr_card: SingleChild::new(hdr_theme_lab_card(
                hdr_theme_lab_section_name(HdrThemeMode::FullHdr),
                HdrThemeMode::FullHdr,
                hdr_theme_mode_title(HdrThemeMode::FullHdr),
                hdr_theme_mode_explanation(HdrThemeMode::FullHdr),
            )),
        }
    }

    fn build_active_preview(mode: HdrThemeMode) -> impl Widget {
        hdr_theme_lab_card(
            HDR_THEME_LAB_ACTIVE_PREVIEW_NAME,
            mode,
            format!("Current {} preview", hdr_theme_mode_title(mode)),
            format!(
                "This preview follows the shared HDR theme mode currently selected by the dev host: {}. Use it to compare the active styling path against the four fixed comparison cards below.",
                hdr_theme_mode_title(mode),
            ),
        )
    }

    fn sync_active_preview(&mut self) -> bool {
        let next_mode = hdr_theme_lab_mode();
        if next_mode == self.active_mode {
            return false;
        }

        self.active_mode = next_mode;
        self.active_preview = SingleChild::new(Self::build_active_preview(next_mode));
        true
    }
}

impl Widget for HdrThemeLabShowcase {
    fn event(&mut self, ctx: &mut EventCtx, _event: &Event) {
        if self.sync_active_preview() {
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(320.0)
        } else {
            760.0
        };
        let child_constraints = Constraints::new(Size::ZERO, Size::new(max_width, f32::INFINITY));
        let mut height = 0.0;
        let mut width: f32 = 0.0;

        for child in [
            &mut self.active_preview,
            &mut self.sdr_card,
            &mut self.wide_gamut_card,
            &mut self.constrained_card,
            &mut self.full_hdr_card,
        ] {
            let size = child.measure(ctx, child_constraints);
            width = width.max(size.width);
            height += size.height;
        }

        height += Self::SECTION_GAP * 4.0;
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let mut y = bounds.y();
        for child in [
            &mut self.active_preview,
            &mut self.sdr_card,
            &mut self.wide_gamut_card,
            &mut self.constrained_card,
            &mut self.full_hdr_card,
        ] {
            let size = child.child().measured_size();
            child.arrange(
                ctx,
                Rect::new(bounds.x(), y, bounds.width().min(size.width), size.height),
            );
            y += size.height + Self::SECTION_GAP;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.active_preview.paint(ctx);
        self.sdr_card.paint(ctx);
        self.wide_gamut_card.paint(ctx);
        self.constrained_card.paint(ctx);
        self.full_hdr_card.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(HDR_THEME_LAB_NAME.to_string());
        node.description = Some(format!(
            "Compares the same button, switch, emissive indicator, and popup trigger across SDR baseline, wide-gamut-only, constrained HDR, and full HDR. The shared preview currently uses {}.",
            hdr_theme_mode_title(self.active_mode),
        ));
        ctx.push(node);
        self.active_preview.semantics(ctx);
        self.sdr_card.semantics(ctx);
        self.wide_gamut_card.semantics(ctx);
        self.constrained_card.semantics(ctx);
        self.full_hdr_card.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.active_preview.visit_children(visitor);
        self.sdr_card.visit_children(visitor);
        self.wide_gamut_card.visit_children(visitor);
        self.constrained_card.visit_children(visitor);
        self.full_hdr_card.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.active_preview.visit_children_mut(visitor);
        self.sdr_card.visit_children_mut(visitor);
        self.wide_gamut_card.visit_children_mut(visitor);
        self.constrained_card.visit_children_mut(visitor);
        self.full_hdr_card.visit_children_mut(visitor);
    }
}

pub fn build_theme_demo_surface() -> impl Widget {
    build_theme_demo_surface_with_theme(default_theme_reader())
}

pub fn build_theme_demo_surface_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let scroll_state = ScrollState::new();

    VirtualScrollView::new()
        .name(THEME_DEMO_SCROLL_NAME)
        .state(scroll_state)
        .padding(ROOT_GALLERY_PADDING)
        .spacing(18.0)
        .with_child(
            Stack::vertical()
                .spacing(6.0)
                .alignment(Alignment::Stretch)
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    demo_label(
                        &theme_reader,
                        THEME_DEMO_TITLE,
                        DemoTextRole::PageTitle,
                        DemoTextColor::Text,
                    ),
                ))
                .with_child(MaximumWidth::new(
                    GALLERY_TEXT_MAX_WIDTH,
                    demo_label(
                        &theme_reader,
                        THEME_DEMO_DESCRIPTION,
                        DemoTextRole::Body,
                        DemoTextColor::Muted,
                    ),
                )),
        )
        .with_child(panel_with_theme(
            Rc::clone(&theme_reader),
            "Built-in themes",
            "Compare every built-in theme with the same compact set of controls and source color swatches.",
            ThemePreviewGrid::new(),
        ))
        .with_child(panel_with_theme(
            Rc::clone(&theme_reader),
            "HDR theme lab",
            "Compare the same tokenized theme across SDR baseline, wide-gamut-only, constrained HDR, and full HDR. The first card follows the shared mode currently selected by the dev host.",
            HdrThemeLabShowcase::new(),
        ))
}

pub(crate) fn theme_preview_card(
    theme: DefaultTheme,
    title: &'static str,
    action_label: &'static str,
    input_label: &'static str,
) -> impl Widget {
    let theme_name = theme.colors.name.replace('-', " ");
    let body = Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(Label::new(format!("{title} theme")).style(demo_text_style(
            theme,
            DemoTextRole::Emphasis,
            theme.palette.text,
        )))
        .with_child(MaximumWidth::new(
            520.0,
            Label::new(format!(
                "{} base surface with {} accent for primary actions.",
                theme_name, theme_name
            ))
            .style(demo_text_style(
                theme,
                DemoTextRole::Supporting,
                theme.palette.placeholder,
            )),
        ))
        .with_child(
            TextInput::new(input_label)
                .placeholder("Find layer, panel, or asset")
                .theme(theme),
        )
        .with_child(Button::primary(action_label).theme(theme))
        .with_child(
            Switch::new(format!("{title} preview live updates"))
                .on(true)
                .theme(theme),
        )
        .with_child(
            Stack::horizontal()
                .spacing(10.0)
                .alignment(Alignment::Center)
                .with_child(
                    ColorSwatch::new(format!("{title} base swatch"), theme.colors.neutrals.panel)
                        .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(format!("{title} primary swatch"), theme.colors.primary)
                        .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(format!("{title} secondary swatch"), theme.colors.secondary)
                        .size(Size::new(58.0, 28.0)),
                ),
        );

    ThemePreviewCardFrame::new(theme, body)
}

pub(crate) struct ThemePreviewCardFrame {
    theme: DefaultTheme,
    padding: Insets,
    child: SingleChild,
}

impl ThemePreviewCardFrame {
    fn new<W>(theme: DefaultTheme, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme,
            padding: Insets::all(16.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for ThemePreviewCardFrame {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.min.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
            Size::new(
                (constraints.max.width - self.padding.left - self.padding.right).max(0.0),
                (constraints.max.height - self.padding.top - self.padding.bottom).max(0.0),
            ),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        constraints.clamp(Size::new(
            child_size.width + self.padding.left + self.padding.right,
            child_size.height + self.padding.top + self.padding.bottom,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured = self.child.child().measured_size();
        let child_bounds = Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - self.padding.left - self.padding.right)
                .max(0.0)
                .min(measured.width),
            (bounds.height() - self.padding.top - self.padding.bottom)
                .max(0.0)
                .min(measured.height),
        );
        self.child.arrange(ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let border = self.theme.palette.border.with_alpha(0.92);
        let background = self.theme.palette.surface_raised;
        ctx.fill(Path::rounded_rect(bounds, 10.0), background);
        ctx.stroke(
            Path::rounded_rect(bounds, 10.0),
            border,
            StrokeStyle::new(1.0),
        );
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

#[cfg(test)]
mod tests;
