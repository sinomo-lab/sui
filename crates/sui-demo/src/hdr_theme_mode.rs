//! The HDR theme mode the demo's widgets preview, which Settings and the HDR
//! validation page choose, and the pieces the HDR validation page draws each
//! mode with.

#![forbid(unsafe_code)]

use std::sync::OnceLock;

use sui::prelude::*;
use sui::{
    HdrLuminanceTokens, HdrThemeMode, HdrThemeTokens, Rect, SemanticColorToken, SemanticsNode,
    SemanticsRole, WidgetColorRole, WidgetLuminanceRole, WidgetMaterialRole, WidgetPodMutVisitor,
    WidgetPodVisitor, resolve_semantic_color, resolve_widget_hdr_style,
};

/// The HDR theme mode the demo's widgets preview, as a signal to observe.
pub(crate) fn hdr_theme_mode_signal() -> Signal<HdrThemeMode> {
    static MODE: OnceLock<Signal<HdrThemeMode>> = OnceLock::new();
    MODE.get_or_init(|| Signal::named("HDR theme mode", HdrThemeMode::Disabled))
        .clone()
}

pub fn hdr_theme_mode() -> HdrThemeMode {
    hdr_theme_mode_signal().get()
}

pub fn set_hdr_theme_mode(mode: HdrThemeMode) {
    hdr_theme_mode_signal().set(mode);
}

pub(crate) fn hdr_theme_mode_title(mode: HdrThemeMode) -> &'static str {
    match mode {
        HdrThemeMode::Disabled => "SDR baseline",
        HdrThemeMode::WideGamutOnly => "Wide-gamut-only",
        HdrThemeMode::ConstrainedHdr => "Constrained HDR",
        HdrThemeMode::FullHdr => "Full HDR",
    }
}

/// A dark theme with wide-gamut and HDR variants for every role, set to
/// `mode`, so the same controls can be compared across modes.
pub(crate) fn hdr_mode_preview_theme(mode: HdrThemeMode) -> DefaultTheme {
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

/// The accent an emissive indicator resolves to in `theme`, lifted as far
/// as its HDR mode allows.
fn emissive_indicator_color(theme: &DefaultTheme) -> Color {
    let indicator_style = resolve_widget_hdr_style(
        &theme.hdr,
        WidgetColorRole::Accent,
        WidgetLuminanceRole::EmissiveIndicator,
        WidgetMaterialRole::Flat,
        None,
    );
    let lift = indicator_style.peak_lift;
    Color::new(
        indicator_style.color.space,
        indicator_style.color.red.clamp(0.0, lift),
        indicator_style.color.green.clamp(0.0, lift),
        indicator_style.color.blue.clamp(0.0, lift),
        indicator_style.color.alpha,
    )
}

/// A swatch of the accent an emissive indicator shows in `theme`, as far as
/// the window's output lets the theme's HDR mode go.
pub(crate) struct EmissiveIndicator {
    name: String,
    theme: DefaultTheme,
    size: Size,
}

impl EmissiveIndicator {
    pub(crate) fn new(name: impl Into<String>, theme: DefaultTheme) -> Self {
        Self {
            name: name.into(),
            theme,
            size: Size::new(64.0, 28.0),
        }
    }
}

impl Widget for EmissiveIndicator {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let mut theme = self.theme;
        theme.hdr = theme.hdr.limited_to(ctx.output_color_range());
        let bounds = ctx.bounds();
        let radius = theme.metrics.corner_radius;
        ctx.fill_rrect(bounds, [radius; 4], emissive_indicator_color(&theme));
        let mut outline = PathBuilder::new();
        outline.push_rounded_rect(bounds, radius);
        ctx.stroke(outline.build(), theme.palette.border, StrokeStyle::new(1.0));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ColorSwatch, ctx.bounds());
        node.name = Some(self.name.clone());
        ctx.push(node);
    }
}

/// A padded card drawn in `theme`, whatever the demo's own theme is.
pub(crate) struct ThemedCardFrame {
    theme: DefaultTheme,
    padding: Insets,
    child: SingleChild,
}

impl ThemedCardFrame {
    pub(crate) fn new<W>(theme: DefaultTheme, child: W) -> Self
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

impl Widget for ThemedCardFrame {
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
mod tests {
    use sui::{Application, Brush, SceneCommand, WindowBuilder};

    use super::*;

    /// The brightest channel an emissive indicator paints in `mode`.
    fn indicator_peak(mode: HdrThemeMode) -> f32 {
        let mut runtime =
            Application::new()
                .window(WindowBuilder::new().title("Emissive indicator").root(
                    EmissiveIndicator::new("Indicator", hdr_mode_preview_theme(mode)),
                ))
                .build()
                .expect("the runtime should build");
        let window = runtime.window_ids()[0];
        let output = runtime.render(window).expect("the indicator should render");
        let mut peak = 0.0_f32;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::FillRoundedRect {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                peak = peak.max(color.red.max(color.green).max(color.blue));
            }
        });
        peak
    }

    #[test]
    fn full_hdr_lifts_indicators_further_than_constrained_hdr() {
        let sdr = indicator_peak(HdrThemeMode::Disabled);
        let constrained = indicator_peak(HdrThemeMode::ConstrainedHdr);
        let full = indicator_peak(HdrThemeMode::FullHdr);

        assert!(sdr <= 1.0, "SDR stays at reference white, got {sdr}");
        assert!(
            constrained > 1.0,
            "constrained HDR rises above reference white, got {constrained}"
        );
        assert!(
            full > constrained,
            "full HDR rises further, got full={full} constrained={constrained}"
        );
    }
}
