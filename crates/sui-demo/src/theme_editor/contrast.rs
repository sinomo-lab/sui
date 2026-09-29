//! WCAG contrast checks over the pairs widgets actually paint. These are the
//! pairs the built-in presets' contrast tests cover.

use sui::ToneRoles;
use sui::prelude::*;

use super::tokens::SourceToken;

/// What a pair carries, which sets its WCAG threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContrastUse {
    /// Body-size text: AA at 4.5:1, AAA at 7:1.
    Text,
    /// Large labels only: AA at 3:1, full AA at 4.5:1.
    LargeText,
    /// Outlines and indicators: 3:1.
    Graphic,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ContrastCheck {
    pub(super) label: String,
    pub(super) foreground: Color,
    pub(super) background: Color,
    pub(super) usage: ContrastUse,
}

impl ContrastCheck {
    fn new(
        label: impl Into<String>,
        foreground: Color,
        background: Color,
        usage: ContrastUse,
    ) -> Self {
        Self {
            label: label.into(),
            foreground,
            background,
            usage,
        }
    }

    pub(super) fn ratio(&self) -> f32 {
        self.foreground.contrast_ratio(self.background)
    }

    pub(super) fn passes(&self) -> bool {
        self.ratio() >= self.minimum()
    }

    pub(super) fn minimum(&self) -> f32 {
        match self.usage {
            ContrastUse::Text => 4.5,
            ContrastUse::LargeText | ContrastUse::Graphic => 3.0,
        }
    }

    /// The WCAG level met, such as "AA" or "Fail".
    pub(super) fn rating(&self) -> &'static str {
        let ratio = self.ratio();
        match self.usage {
            ContrastUse::Text if ratio >= 7.0 => "AAA",
            ContrastUse::Text if ratio >= 4.5 => "AA",
            ContrastUse::LargeText if ratio >= 4.5 => "AA",
            ContrastUse::LargeText if ratio >= 3.0 => "AA large",
            ContrastUse::Graphic if ratio >= 3.0 => "Pass",
            _ => "Fail",
        }
    }

    /// "4.9:1 AA", as shown next to tokens and in the report.
    pub(super) fn badge(&self) -> String {
        format!("{:.1}:1 {}", self.ratio(), self.rating())
    }
}

/// Every check, in report order.
pub(super) fn contrast_checks(theme: &DefaultTheme) -> Vec<ContrastCheck> {
    let palette = theme.palette;
    let colors = theme.colors;
    let panel = palette.surface_raised;
    let mut checks = vec![
        ContrastCheck::new("Text on panel", palette.text, panel, ContrastUse::Text),
        ContrastCheck::new(
            "Muted text on panel",
            palette.text_muted,
            panel,
            ContrastUse::Text,
        ),
        ContrastCheck::new(
            "Placeholder on panel",
            palette.placeholder,
            panel,
            ContrastUse::Text,
        ),
        ContrastCheck::new(
            "Control outline on panel",
            palette.border_control,
            panel,
            ContrastUse::Graphic,
        ),
        ContrastCheck::new(
            "Focus ring on panel",
            palette.focus_ring,
            panel,
            ContrastUse::Graphic,
        ),
        ContrastCheck::new(
            "On primary",
            palette.accent_text,
            palette.accent,
            ContrastUse::Text,
        ),
        ContrastCheck::new(
            "On secondary",
            colors.on_secondary,
            colors.secondary,
            ContrastUse::Text,
        ),
        ContrastCheck::new(
            "On info",
            palette.info_text,
            palette.info,
            ContrastUse::Text,
        ),
        ContrastCheck::new(
            "On success",
            palette.success_text,
            palette.success,
            ContrastUse::Text,
        ),
        ContrastCheck::new(
            "On warning",
            palette.warning_text,
            palette.warning,
            ContrastUse::Text,
        ),
        ContrastCheck::new(
            "On danger",
            palette.danger_text,
            palette.danger,
            ContrastUse::Text,
        ),
    ];
    for (label, text, soft) in [
        (
            "Primary soft text",
            palette.accent_soft_text,
            palette.accent_soft,
        ),
        ("Info soft text", palette.info_soft_text, palette.info_soft),
        (
            "Success soft text",
            palette.success_soft_text,
            palette.success_soft,
        ),
        (
            "Warning soft text",
            palette.warning_soft_text,
            palette.warning_soft,
        ),
        (
            "Danger soft text",
            palette.danger_soft_text,
            palette.danger_soft,
        ),
    ] {
        checks.push(ContrastCheck::new(label, text, soft, ContrastUse::Text));
    }
    let worst = |pair: fn(ToneRoles) -> (Color, Color)| {
        DecorativeHue::ALL
            .into_iter()
            .map(|hue| (hue, pair(theme.decorative.get(hue))))
            .min_by(|(_, (fg_a, bg_a)), (_, (fg_b, bg_b))| {
                fg_a.contrast_ratio(*bg_a)
                    .total_cmp(&fg_b.contrast_ratio(*bg_b))
            })
            .expect("there are decorative hues")
    };
    let (hue, (text, soft)) = worst(|roles| (roles.text, roles.soft));
    checks.push(ContrastCheck::new(
        format!("Decorative tags (lowest: {})", hue_label(hue)),
        text,
        soft,
        ContrastUse::Text,
    ));
    let (hue, (on_solid, solid)) = worst(|roles| (roles.on_solid, roles.solid));
    checks.push(ContrastCheck::new(
        format!("Decorative fills (lowest: {})", hue_label(hue)),
        on_solid,
        solid,
        ContrastUse::LargeText,
    ));
    checks
}

fn hue_label(hue: DecorativeHue) -> &'static str {
    match hue {
        DecorativeHue::Red => "red",
        DecorativeHue::Orange => "orange",
        DecorativeHue::Amber => "amber",
        DecorativeHue::Green => "green",
        DecorativeHue::Teal => "teal",
        DecorativeHue::Cyan => "cyan",
        DecorativeHue::Blue => "blue",
        DecorativeHue::Violet => "violet",
        DecorativeHue::Magenta => "magenta",
    }
}

/// The check shown beside a source color's row, if it has one: "on" colors
/// against their fill, text levels and the control outline against panels.
pub(super) fn token_contrast(token: SourceToken, colors: &ThemeColors) -> Option<ContrastCheck> {
    if let Some(fill) = token.fill() {
        return Some(ContrastCheck::new(
            token.label(),
            token.get(colors),
            fill.get(colors),
            ContrastUse::Text,
        ));
    }
    let panel = colors.neutrals.panel;
    let usage = match token {
        SourceToken::Text | SourceToken::TextSecondary | SourceToken::TextTertiary => {
            ContrastUse::Text
        }
        SourceToken::ControlOutline => ContrastUse::Graphic,
        _ => return None,
    };
    Some(ContrastCheck::new(
        token.label(),
        token.get(colors),
        panel,
        usage,
    ))
}
