use crate::ControlMetrics;
use crate::DefaultTheme;
use crate::IconGlyph;
use crate::Interpolate;
use crate::ResolvedEffectStyle;
use crate::SemanticTone;
use crate::ThemeTextToken;
use crate::composites::popups::{AnimatedScalar, TooltipPlacement};
use crate::composites::status::{StatusBadge, paint_status_badge};
use crate::controls::apply_hdr_policy_cap;
use crate::text_align::paint_aligned_text;
use crate::text_align::paint_single_line_aligned_text;
use sui_core::Color;
use sui_core::Path;
use sui_core::PathBuilder;
use sui_core::Point;
use sui_core::Rect;
use sui_core::SemanticsNode;
use sui_core::SemanticsRole;
use sui_core::SemanticsValue;
use sui_core::Size;
use sui_core::Vector;
use sui_layout::Constraints;
use sui_layout::Padding as Insets;
use sui_runtime::MeasureCtx;
use sui_runtime::PaintCtx;
use sui_runtime::SemanticsCtx;
use sui_runtime::Widget;
use sui_scene::StrokeStyle;
use sui_text::FontFeature;
use sui_text::FontWeight;
use sui_text::TextMeasurement;
use sui_text::TextStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverageDotsConfig {
    pub current: usize,
    pub target: usize,
    pub tone: SemanticTone,
    pub max_dots: usize,
    pub show_label: bool,
}

impl CoverageDotsConfig {
    pub fn new(current: usize, target: usize) -> Self {
        Self {
            current,
            target,
            tone: SemanticTone::Neutral,
            max_dots: 4,
            show_label: true,
        }
    }

    pub(super) fn normalized_target(self) -> usize {
        self.target.max(self.current)
    }

    pub(super) fn normalized_max_dots(self) -> usize {
        self.max_dots.max(1)
    }

    pub(super) fn label(self) -> String {
        format!("{}/{}", self.current, self.normalized_target())
    }
}

pub fn paint_coverage_dots(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    current: usize,
    target: usize,
    tone: SemanticTone,
) {
    let mut config = CoverageDotsConfig::new(current, target);
    config.tone = tone;
    paint_coverage_dots_with_config(ctx, theme, rect, config);
}

pub fn paint_coverage_dots_with_config(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    config: CoverageDotsConfig,
) {
    let target = config.normalized_target();
    if target == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (dot, gap, label_gap) = coverage_dot_metrics(theme);
    let label = config.label();
    let text_style = numeric_text_style(text_token_style(
        theme,
        theme.text.xs,
        theme.palette.text_muted,
    ));
    let label_width = if config.show_label {
        ctx.measure_text(label.clone(), text_style.clone())
            .map(|measurement| measurement.width)
            .unwrap_or(0.0)
    } else {
        0.0
    };
    // Keep the exact count readable first. Dots are redundant decoration and
    // disappear as a group when their complete representation does not fit.
    let shown = target.min(config.normalized_max_dots());
    let dots_width = shown as f32 * dot + shown.saturating_sub(1) as f32 * gap;
    let separation = if config.show_label { label_gap } else { 0.0 };
    let show_dots = dots_width + separation + label_width <= rect.width();
    let mut x = rect.x();
    ctx.push_clip_rect(rect);
    if show_dots {
        let y = rect.y() + (rect.height() - dot) * 0.5;
        let (tone_color, _) = theme.semantic_tone_colors(config.tone);
        for index in 0..shown {
            let dot_rect = Rect::new(x, y, dot, dot);
            if index < config.current.min(shown) {
                ctx.fill(rounded_rect_path(dot_rect, dot * 0.5), tone_color);
            } else {
                ctx.stroke(
                    rounded_rect_path(dot_rect, dot * 0.5),
                    theme.palette.border,
                    StrokeStyle::new(theme.metrics.border_width.max(1.0)),
                );
            }
            x += dot + gap;
        }
        x += separation - gap;
    }
    if config.show_label {
        let label_rect = Rect::new(x, rect.y(), (rect.max_x() - x).max(0.0), rect.height());
        paint_single_line_aligned_text(
            ctx,
            label_rect,
            &label,
            &text_style,
            text_style.line_height,
            0.0,
        );
    }
    ctx.pop_clip();
}

pub(super) fn coverage_dot_metrics(theme: &DefaultTheme) -> (f32, f32, f32) {
    let dot = (theme.text.xs.size * 0.42).clamp(4.0, 6.0);
    let gap = (dot * 0.65).clamp(2.0, 4.0);
    let label_gap = (theme.metrics.icon_label_gap * 0.7).max(4.0);
    (dot, gap, label_gap)
}

pub struct CoverageDots {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) config: CoverageDotsConfig,
    pub(super) min_width: Option<f32>,
}

impl CoverageDots {
    pub fn new(name: impl Into<String>, current: usize, target: usize) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            config: CoverageDotsConfig::new(current, target),
            min_width: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.config.tone = tone;
        self
    }

    pub fn max_dots(mut self, max_dots: usize) -> Self {
        self.config.max_dots = max_dots;
        self
    }

    pub fn show_label(mut self, show_label: bool) -> Self {
        self.config.show_label = show_label;
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for CoverageDots {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let target = self.config.normalized_target();
        if target == 0 {
            return constraints.clamp(Size::ZERO);
        }
        let (dot, gap, label_gap) = coverage_dot_metrics(&theme);
        let shown = target.min(self.config.normalized_max_dots());
        let dots_width = shown as f32 * dot + shown.saturating_sub(1) as f32 * gap;
        let label_width = if self.config.show_label {
            measure_text(
                ctx,
                &self.config.label(),
                &numeric_text_style(text_token_style(
                    &theme,
                    theme.text.xs,
                    theme.palette.text_muted,
                )),
            )
            .width
                + label_gap
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            self.min_width.unwrap_or(0.0).max(dots_width + label_width),
            theme
                .text
                .xs
                .line_height
                .max(dot + 2.0)
                .max(theme.metrics.min_height * 0.55),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_coverage_dots_with_config(ctx, &self.resolved_theme(), ctx.bounds(), self.config);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let target = self.config.normalized_target();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(self.config.label()));
        node.description = Some(format!(
            "{} of {} covered",
            self.config.current.min(target),
            target
        ));
        ctx.push(node);
    }
}

pub struct PlacementBadge {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) label: String,
    pub(super) label_reader: Option<Box<dyn Fn() -> String>>,
    pub(super) icon: Option<IconGlyph>,
    pub(super) tone: SemanticTone,
    pub(super) tone_reader: Option<Box<dyn Fn() -> SemanticTone>>,
    pub(super) coverage: Option<(usize, usize)>,
    pub(super) coverage_reader: Option<Box<dyn Fn() -> Option<(usize, usize)>>>,
    pub(super) min_width: Option<f32>,
}

impl PlacementBadge {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            icon: None,
            tone: SemanticTone::Neutral,
            tone_reader: None,
            coverage: None,
            coverage_reader: None,
            min_width: None,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        let mut badge = Self::new(fallback);
        badge.label_reader = Some(Box::new(reader));
        badge
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self.tone_reader = None;
        self
    }

    pub fn tone_when<F>(mut self, tone: F) -> Self
    where
        F: Fn() -> SemanticTone + 'static,
    {
        self.tone_reader = Some(Box::new(tone));
        self
    }

    pub fn coverage(mut self, current: usize, target: usize) -> Self {
        self.coverage = Some((current, target));
        self.coverage_reader = None;
        self
    }

    pub fn coverage_when<F>(mut self, coverage: F) -> Self
    where
        F: Fn() -> Option<(usize, usize)> + 'static,
    {
        self.coverage_reader = Some(Box::new(coverage));
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn label(&self) -> String {
        self.label_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.label.clone())
    }

    pub(super) fn resolved_tone(&self) -> SemanticTone {
        self.tone_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.tone)
    }

    pub(super) fn resolved_coverage(&self) -> Option<(usize, usize)> {
        self.coverage_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.coverage)
            .filter(|(_, target)| *target > 0)
    }

    pub(super) fn metrics(theme: &DefaultTheme) -> (f32, f32, f32) {
        let height = (theme.metrics.min_height - 2.0).max(22.0);
        let coverage_width = 50.0;
        let gap = theme.metrics.icon_label_gap.max(6.0);
        (height, coverage_width, gap)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacementBadgePaint {
    pub padding: Insets,
}

impl PlacementBadgePaint {
    pub const fn new() -> Self {
        Self {
            padding: Insets::ZERO,
        }
    }

    pub const fn padding(mut self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        self.padding = Insets {
            left,
            top,
            right,
            bottom,
        };
        self
    }

    pub(super) fn content_rect(self, rect: Rect) -> Rect {
        Rect::new(
            rect.x() + self.padding.left,
            rect.y() + self.padding.top,
            (rect.width() - self.padding.left - self.padding.right).max(0.0),
            (rect.height() - self.padding.top - self.padding.bottom).max(0.0),
        )
    }
}

impl Default for PlacementBadgePaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_placement_badge(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
    coverage: Option<(usize, usize)>,
) {
    paint_placement_badge_with(
        ctx,
        theme,
        rect,
        label,
        icon,
        tone,
        coverage,
        PlacementBadgePaint::new(),
    );
}

pub fn paint_placement_badge_with(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
    coverage: Option<(usize, usize)>,
    paint: PlacementBadgePaint,
) {
    let rect = paint.content_rect(rect);
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (_, coverage_width, gap) = PlacementBadge::metrics(theme);
    let show_coverage = coverage.is_some() && rect.width() >= 118.0;
    let coverage_slot = if show_coverage { coverage_width } else { 0.0 };
    let slot_gap = if show_coverage { gap } else { 0.0 };
    let badge_rect = Rect::new(
        rect.x(),
        rect.y(),
        (rect.width() - coverage_slot - slot_gap).clamp(48.0, 86.0),
        rect.height(),
    );
    let label_width = ctx
        .measure_text(
            label.to_string(),
            semibold_control_text_style(theme, theme.palette.text),
        )
        .map(|measurement| measurement.width)
        .unwrap_or(0.0);
    let icon_width =
        (rect.height() - 13.0).clamp(11.0, 15.0) + theme.metrics.icon_label_gap.max(4.0);
    let padding = theme.metrics.button_padding.left.max(6.0) * 1.5;
    let icon = icon.filter(|_| label_width + icon_width + padding <= badge_rect.width());
    paint_status_badge(ctx, badge_rect, theme, label, icon, tone);

    if show_coverage && let Some((current, target)) = coverage {
        let dots_rect = Rect::new(
            badge_rect.max_x() + slot_gap,
            rect.y(),
            (rect.max_x() - badge_rect.max_x() - slot_gap).max(0.0),
            rect.height(),
        );
        paint_coverage_dots(ctx, theme, dots_rect, current, target, tone);
    }
}

impl Widget for PlacementBadge {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let (height, coverage_width, gap) = Self::metrics(&theme);
        let label = self.label();
        let tone = self.resolved_tone();
        let icon_w = self
            .icon
            .map(|_| (height - 13.0).clamp(11.0, 15.0) + theme.metrics.icon_label_gap.max(4.0))
            .unwrap_or(0.0);
        let text = measure_text(
            ctx,
            &label,
            &StatusBadge::new(&label).text_style(&theme, &label, tone),
        );
        let badge_width =
            (text.width.ceil() + icon_w + theme.metrics.button_padding.left.max(8.0) * 2.0)
                .clamp(48.0, 86.0);
        let coverage_width = if self.resolved_coverage().is_some() {
            coverage_width + gap
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            self.min_width
                .unwrap_or(0.0)
                .max(badge_width + coverage_width),
            height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        paint_placement_badge(
            ctx,
            &theme,
            ctx.bounds(),
            &self.label(),
            self.icon,
            self.resolved_tone(),
            self.resolved_coverage(),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let label = self.label();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(label.clone());
        node.value = Some(SemanticsValue::Text(label.clone()));
        if let Some((current, target)) = self.resolved_coverage() {
            let target = target.max(current);
            node.description = Some(format!("{current} of {target} replicas available"));
        }
        ctx.push(node);
    }
}

pub struct ProgressBar {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) value: f64,
    pub(super) tone: SemanticTone,
    pub(super) min_width: Option<f32>,
    pub(super) height: Option<f32>,
    pub(super) show_value: bool,
}

impl ProgressBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            min: 0.0,
            max: 1.0,
            value: 0.0,
            tone: SemanticTone::Accent,
            min_width: None,
            height: None,
            show_value: false,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self.value = self.value.clamp(self.min, self.max);
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(1.0));
        self
    }

    pub fn show_value(mut self, show_value: bool) -> Self {
        self.show_value = show_value;
        self
    }

    pub(super) fn fraction(&self) -> f32 {
        if (self.max - self.min).abs() <= f64::EPSILON {
            0.0
        } else {
            ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
        }
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

pub fn paint_progress_bar(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    fraction: f32,
    tone: SemanticTone,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let metrics = theme.metrics;
    let palette = theme.palette;
    let (tone, _) = theme.semantic_tone_colors(tone);
    draw_control_shape(
        ctx,
        rect,
        metrics.corner_radius,
        physical_pixels(ctx, metrics.border_width).min(rect.height() * 0.5),
        palette.control,
        palette.border,
    );

    let fill = Rect::new(
        rect.x(),
        rect.y(),
        rect.width() * fraction.clamp(0.0, 1.0),
        rect.height(),
    );
    if fill.width() > 0.0 {
        ctx.fill(rounded_rect_path(fill, metrics.corner_radius), tone);
    }
}

impl Widget for ProgressBar {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let min_height = if let Some(height) = self.height {
            height
        } else if self.show_value {
            metrics
                .progress_bar_value_height
                .max(text_token_style(&theme, theme.text.sm, theme.palette.text).line_height)
        } else {
            metrics.progress_bar_height
        };
        constraints.clamp(Size::new(
            self.min_width.unwrap_or(metrics.progress_bar_min_width),
            min_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let (_, tone_text) = theme.semantic_tone_colors(self.tone);
        paint_progress_bar(ctx, ctx.bounds(), &theme, self.fraction(), self.tone);
        if self.show_value {
            let label = format!("{:.0}%", self.fraction() * 100.0);
            let text_style = numeric_text_style(text_token_style(&theme, theme.text.sm, tone_text));
            let label_padding = Insets {
                top: 0.0,
                bottom: 0.0,
                ..metrics.progress_bar_label_padding
            };
            let label_slot = inset_rect(ctx.bounds(), label_padding);
            ctx.push_clip_rect(label_slot);
            paint_aligned_text(
                ctx,
                label_slot,
                &label,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ProgressBar, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Range {
            value: self.value,
            min: self.min,
            max: self.max,
        });
        ctx.push(node);
    }
}

pub struct Spinner {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) size: f32,
    pub(super) label: Option<String>,
}

impl Spinner {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            size: 20.0,
            label: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(8.0);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub(super) fn indicator_rect(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y() + ((bounds.height() - self.size) * 0.5),
            self.size,
            self.size,
        )
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }
}

impl Widget for Spinner {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let text_style = text_token_style(&theme, theme.text.sm, theme.palette.text);
        let label_measurement = self
            .label
            .as_ref()
            .map(|label| measure_text(ctx, label, &text_style));
        let label_width = label_measurement
            .map(|measurement| measurement.width + 12.0)
            .unwrap_or(0.0);
        let label_height = label_measurement
            .map(|measurement| measurement.height.max(text_style.line_height))
            .unwrap_or(0.0);
        constraints.clamp(Size::new(
            self.size + label_width,
            self.size.max(20.0).max(label_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let indicator = self.indicator_rect(ctx.bounds());
        let center = rect_center(indicator);
        let radius = indicator.width().min(indicator.height()) * 0.4;
        let dot_radius = (indicator.width() * 0.09).max(1.5);
        for index in 0..10 {
            let angle = (index as f32 / 10.0) * std::f32::consts::TAU;
            let alpha = 0.22 + ((index as f32) / 10.0) * 0.72;
            let color = Color::rgba(
                palette.accent.red,
                palette.accent.green,
                palette.accent.blue,
                alpha,
            );
            let dot = Point::new(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            );
            ctx.fill(Path::circle(dot, dot_radius), color);
        }

        if let Some(label) = &self.label {
            let text_style = text_token_style(&theme, theme.text.sm, palette.text);
            let text_slot = Rect::new(
                indicator.max_x() + 12.0,
                ctx.bounds().y(),
                ctx.bounds().width() - indicator.width() - 12.0,
                ctx.bounds().height(),
            );
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot,
                label,
                &text_style,
                text_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::BusyIndicator, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = self.label.clone();
        node.state.busy = true;
        ctx.push(node);
    }
}

pub type BusyIndicator = Spinner;

pub(super) fn measure_text(ctx: &mut MeasureCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.layout()
        .measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
        })
}

pub(super) fn text_token_style(
    theme: &DefaultTheme,
    token: ThemeTextToken,
    color: Color,
) -> TextStyle {
    TextStyle {
        font_size: token.size.max(1.0),
        line_height: token.line_height.max(1.0),
        color,
        ..theme.body_text_style()
    }
}

pub(super) fn semibold_control_text_style(theme: &DefaultTheme, color: Color) -> TextStyle {
    let mut style = theme.text_style(color);
    style.weight = FontWeight::SEMIBOLD;
    style
}

pub(super) fn numeric_text_style(mut style: TextStyle) -> TextStyle {
    style.features.enable(FontFeature::TABULAR_FIGURES);
    style
}

pub(super) fn numeric_text_style_if_numeric(text: &str, style: TextStyle) -> TextStyle {
    if text_contains_ascii_digit(text) {
        numeric_text_style(style)
    } else {
        style
    }
}

pub(super) fn text_contains_ascii_digit(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
}

pub(super) fn rect_center(rect: Rect) -> Point {
    Point::new(
        rect.x() + (rect.width() * 0.5),
        rect.y() + (rect.height() * 0.5),
    )
}

pub(super) fn inset_rect(rect: Rect, padding: Insets) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.y() + padding.top,
        (rect.width() - padding.left - padding.right).max(0.0),
        (rect.height() - padding.top - padding.bottom).max(0.0),
    )
}

pub(super) fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(rect, radius.min(rect.width().min(rect.height()) * 0.5))
}

pub(super) fn tab_indicator_rect<F>(
    mut tab_rect: F,
    from_index: usize,
    selected_index: usize,
    progress: f32,
    padding: Insets,
    thickness: f32,
) -> Option<Rect>
where
    F: FnMut(usize) -> Option<Rect>,
{
    let to = tab_indicator_from_tab_rect(tab_rect(selected_index)?, padding, thickness);
    let from = tab_rect(from_index)
        .map(|rect| tab_indicator_from_tab_rect(rect, padding, thickness))
        .unwrap_or(to);
    Some(lerp_rect(from, to, progress))
}

pub(super) fn tab_indicator_from_tab_rect(rect: Rect, padding: Insets, thickness: f32) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.max_y() - thickness,
        (rect.width() - padding.left - padding.right).max(0.0),
        thickness,
    )
}

pub(super) fn sliding_inset_rect<F>(
    mut item_rect: F,
    from_index: usize,
    selected_index: usize,
    progress: f32,
    insets: Insets,
) -> Option<Rect>
where
    F: FnMut(usize) -> Option<Rect>,
{
    let to = inset_rect(item_rect(selected_index)?, insets);
    let from = item_rect(from_index)
        .map(|rect| inset_rect(rect, insets))
        .unwrap_or(to);
    Some(lerp_rect(from, to, progress))
}

pub(super) fn tab_panel_transition_translation(
    from_index: usize,
    selected_index: usize,
    progress: f32,
    metrics: ControlMetrics,
) -> Vector {
    if from_index == selected_index {
        return Vector::ZERO;
    }

    let remaining = 1.0 - progress.clamp(0.0, 1.0);
    if remaining <= AnimatedScalar::EPSILON {
        return Vector::ZERO;
    }

    let direction = if selected_index > from_index {
        1.0
    } else {
        -1.0
    };
    Vector::new(direction * metrics.tab_panel_gap * remaining, 0.0)
}

pub(super) fn lerp_rect(from: Rect, to: Rect, progress: f32) -> Rect {
    let progress = progress.clamp(0.0, 1.0);
    Rect::new(
        f32::interpolate(from.x(), to.x(), progress),
        f32::interpolate(from.y(), to.y(), progress),
        f32::interpolate(from.width(), to.width(), progress),
        f32::interpolate(from.height(), to.height(), progress),
    )
}

pub(super) fn tab_state_visuals(
    theme: &DefaultTheme,
    selected: bool,
    hovered: bool,
    pressed: bool,
    hover_amount: f32,
    press_amount: f32,
) -> Option<(Color, Color)> {
    let palette = theme.palette;
    let interaction = theme.interaction;
    if selected {
        Some((palette.selection, palette.selection_border))
    } else if pressed || press_amount > 0.0 {
        Some((
            mix_color(
                if hover_amount > 0.0 {
                    mix_color(
                        palette.control,
                        palette.control_hover,
                        interaction.hover_blend * hover_amount,
                    )
                } else {
                    palette.control
                },
                palette.control_active,
                interaction.pressed_blend * press_amount,
            ),
            palette.border_hover,
        ))
    } else if hovered || hover_amount > 0.0 {
        Some((
            mix_color(
                palette.control,
                palette.control_hover,
                interaction.hover_blend * hover_amount,
            ),
            palette.border_hover,
        ))
    } else {
        None
    }
}

pub(super) fn draw_control_frame(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    background: Color,
    border: Color,
    focus_ring: Option<Color>,
) {
    draw_control_shape(
        ctx,
        bounds,
        radius,
        physical_pixels(ctx, metrics.border_width),
        background,
        border,
    );

    if let Some(focus_ring) = focus_ring {
        draw_focus_ring_frame(ctx, bounds, radius, metrics, focus_ring);
    }
}

pub(super) fn draw_focus_ring_frame(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    focus_ring: Color,
) {
    let focus_ring_outset = physical_pixels(ctx, metrics.focus_ring_outset);
    ctx.stroke(
        rounded_rect_path(
            bounds.inflate(focus_ring_outset, focus_ring_outset),
            radius + focus_ring_outset,
        ),
        focus_ring,
        StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
    );
}

pub(super) fn draw_control_shape(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    border_width: f32,
    background: Color,
    border: Color,
) {
    let shape = rounded_rect_path(bounds, radius);
    ctx.fill(shape.clone(), background);
    ctx.stroke(shape, border, StrokeStyle::new(border_width));
}

pub(super) fn mix_color(left: Color, right: Color, amount: f32) -> Color {
    crate::animation::Interpolate::interpolate(left, right, amount)
}

pub(super) fn draw_popover_arrival_overlay(
    ctx: &mut PaintCtx,
    rect: Rect,
    metrics: ControlMetrics,
    background: Color,
    border: Color,
    arrival_effect: ResolvedEffectStyle,
) {
    let overlay_inset = physical_pixels(ctx, 1.0);
    let overlay_rect = rect.inflate(-overlay_inset, -overlay_inset);
    let overlay_radius = (metrics.corner_radius + 2.0 - overlay_inset).max(0.0);
    let overlay_fill = mix_color(background, arrival_effect.color, 0.35)
        .with_alpha((0.10 + (arrival_effect.intensity * 0.12)).clamp(0.0, 0.22));
    let stroke_color = apply_hdr_policy_cap(
        mix_color(border, arrival_effect.color, 0.55),
        arrival_effect
            .color
            .red
            .max(arrival_effect.color.green.max(arrival_effect.color.blue)),
    )
    .with_alpha((0.16 + (arrival_effect.intensity * 0.12)).clamp(0.0, 0.30));

    ctx.fill(
        rounded_rect_path(overlay_rect, overlay_radius),
        overlay_fill,
    );
    ctx.stroke(
        rounded_rect_path(
            overlay_rect.inflate(-overlay_inset * 0.5, -overlay_inset * 0.5),
            (overlay_radius - (overlay_inset * 0.5)).max(0.0),
        ),
        stroke_color,
        StrokeStyle::new(physical_pixels(ctx, 1.0)),
    );
}

pub(super) fn tooltip_tail(trigger: Rect, bubble: Rect, placement: TooltipPlacement) -> Path {
    let center_x = rect_center(trigger)
        .x
        .clamp(bubble.x() + 12.0, bubble.max_x() - 12.0);
    let mut builder = PathBuilder::new();
    match placement {
        TooltipPlacement::Above => {
            builder
                .move_to(Point::new(center_x - 6.0, bubble.max_y() - 1.0))
                .line_to(Point::new(center_x + 6.0, bubble.max_y() - 1.0))
                .line_to(Point::new(center_x, bubble.max_y() + 8.0));
        }
        TooltipPlacement::Below => {
            builder
                .move_to(Point::new(center_x - 6.0, bubble.y() + 1.0))
                .line_to(Point::new(center_x + 6.0, bubble.y() + 1.0))
                .line_to(Point::new(center_x, bubble.y() - 8.0));
        }
    }
    builder.build()
}

pub(super) fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    ctx.dpi().physical_pixels_to_logical(value)
}
