use crate::DefaultTheme;
use crate::IconGlyph;
use crate::SemanticTone;
use crate::ThemeTextToken;
use crate::composites::indicators::{
    measure_text, mix_color, numeric_text_style_if_numeric, physical_pixels, rounded_rect_path,
    text_token_style,
};
use crate::composites::status::{StatusBadge, paint_status_badge};
use crate::controls::draw_icon_glyph;
use crate::text_align::paint_aligned_text;
use crate::text_align::paint_single_line_aligned_text;
use sui_core::Color;
use sui_core::Path;
use sui_core::Rect;
use sui_core::SemanticsNode;
use sui_core::SemanticsRole;
use sui_core::SemanticsValue;
use sui_core::Size;
use sui_layout::Constraints;
use sui_layout::Padding as Insets;
use sui_runtime::MeasureCtx;
use sui_runtime::PaintCtx;
use sui_runtime::SemanticsCtx;
use sui_runtime::Widget;
use sui_scene::StrokeStyle;
use sui_text::FontWeight;
use sui_text::TextAlign;
use sui_text::TextDocument;
use sui_text::TextLayoutRequest;
use sui_text::TextStyle;
use sui_text::TextWrap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HairlineEdge {
    Top,
    Right,
    Bottom,
    Left,
}

pub fn paint_rounded_rect(ctx: &mut PaintCtx, rect: Rect, color: Color, radius: f32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let radius = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    if radius <= 0.5 {
        ctx.fill_rect(rect, color);
    } else {
        ctx.fill(Path::rounded_rect(rect, radius), color);
    }
}

pub fn paint_rounded_panel(
    ctx: &mut PaintCtx,
    rect: Rect,
    fill: Color,
    border: Color,
    radius: f32,
) {
    paint_rounded_rect(ctx, rect, border, radius);
    paint_rounded_rect(ctx, rect.inflate(-1.0, -1.0), fill, (radius - 1.0).max(0.0));
}

pub fn paint_hairline(ctx: &mut PaintCtx, rect: Rect, edge: HairlineEdge, color: Color) {
    let line = match edge {
        HairlineEdge::Top => Rect::new(rect.x(), rect.y(), rect.width(), 1.0),
        HairlineEdge::Right => Rect::new(rect.max_x() - 1.0, rect.y(), 1.0, rect.height()),
        HairlineEdge::Bottom => Rect::new(rect.x(), rect.max_y() - 1.0, rect.width(), 1.0),
        HairlineEdge::Left => Rect::new(rect.x(), rect.y(), 1.0, rect.height()),
    };
    ctx.fill_rect(line, color);
}

pub fn paint_border(ctx: &mut PaintCtx, rect: Rect, color: Color) {
    paint_hairline(ctx, rect, HairlineEdge::Top, color);
    paint_hairline(ctx, rect, HairlineEdge::Right, color);
    paint_hairline(ctx, rect, HairlineEdge::Bottom, color);
    paint_hairline(ctx, rect, HairlineEdge::Left, color);
}

pub(super) fn centered_text_slot(bounds: Rect, center_y: f32, line_height: f32) -> Rect {
    let height = line_height.max(1.0) * 2.0;
    Rect::new(bounds.x(), center_y - height * 0.5, bounds.width(), height)
}

#[derive(Clone, Copy)]
pub struct EmptyStatePaint<'a> {
    pub(super) icon: Option<IconGlyph>,
    pub(super) title: &'a str,
    pub(super) description: &'a str,
    pub(super) detail: Option<&'a str>,
    pub(super) background: Option<Color>,
    pub(super) center_offset_y: f32,
    pub(super) reserve_action_space: bool,
}

impl<'a> EmptyStatePaint<'a> {
    pub const fn new(title: &'a str, description: &'a str) -> Self {
        Self {
            icon: None,
            title,
            description,
            detail: None,
            background: None,
            center_offset_y: 0.0,
            reserve_action_space: false,
        }
    }

    pub const fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    pub const fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub const fn center_offset_y(mut self, offset: f32) -> Self {
        self.center_offset_y = offset;
        self
    }

    pub const fn reserve_action_space(mut self, reserve: bool) -> Self {
        self.reserve_action_space = reserve;
        self
    }
}

pub fn paint_empty_state(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    bounds: Rect,
    paint: EmptyStatePaint<'_>,
) {
    if let Some(background) = paint.background {
        ctx.fill_rect(bounds, background);
    }

    let cy = bounds.y() + bounds.height() * 0.5
        - if paint.reserve_action_space {
            18.0
        } else {
            0.0
        }
        + paint.center_offset_y;
    let icon_color = theme.surfaces.text_faint;
    if let Some(icon) = paint.icon {
        let side = 40.0;
        let cx = bounds.x() + bounds.width() * 0.5;
        draw_icon_glyph(
            ctx,
            icon,
            Rect::new(cx - side * 0.5, cy - 46.0 - side * 0.5, side, side),
            icon_color,
        );
    }

    let mut title_style = text_token_style(theme, theme.text.lg, theme.surfaces.text_muted);
    title_style.weight = FontWeight::SEMIBOLD;
    paint_single_line_aligned_text(
        ctx,
        centered_text_slot(bounds, cy + 4.0, title_style.line_height),
        paint.title,
        &title_style,
        title_style.line_height,
        0.5,
    );

    let description_style = text_token_style(theme, theme.text.sm, theme.surfaces.text_faint);
    paint_single_line_aligned_text(
        ctx,
        centered_text_slot(bounds, cy + 30.0, description_style.line_height),
        paint.description,
        &description_style,
        description_style.line_height,
        0.5,
    );

    if let Some(detail) = paint.detail {
        let mut detail_style = text_token_style(theme, theme.text.xs, theme.surfaces.text_muted);
        detail_style.weight = FontWeight::MEDIUM;
        paint_single_line_aligned_text(
            ctx,
            centered_text_slot(bounds, cy + 48.0, detail_style.line_height),
            detail,
            &detail_style,
            detail_style.line_height,
            0.5,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandButtonFill {
    Surface,
    Filled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandButtonPaint {
    pub tone: SemanticTone,
    pub icon_tone: Option<SemanticTone>,
    pub fill: CommandButtonFill,
    pub hovered: bool,
    pub pressed: bool,
}

impl CommandButtonPaint {
    pub const fn neutral() -> Self {
        Self {
            tone: SemanticTone::Neutral,
            icon_tone: None,
            fill: CommandButtonFill::Surface,
            hovered: false,
            pressed: false,
        }
    }

    pub const fn tonal(tone: SemanticTone) -> Self {
        Self {
            tone,
            icon_tone: None,
            fill: CommandButtonFill::Surface,
            hovered: false,
            pressed: false,
        }
    }

    pub const fn filled(tone: SemanticTone) -> Self {
        Self {
            tone,
            icon_tone: None,
            fill: CommandButtonFill::Filled,
            hovered: false,
            pressed: false,
        }
    }

    pub const fn icon_tone(mut self, tone: SemanticTone) -> Self {
        self.icon_tone = Some(tone);
        self
    }

    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }
}

impl Default for CommandButtonPaint {
    fn default() -> Self {
        Self::neutral()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisclosureButtonPaint {
    pub command: CommandButtonPaint,
}

impl DisclosureButtonPaint {
    pub const fn new() -> Self {
        Self {
            command: CommandButtonPaint::tonal(SemanticTone::Accent)
                .icon_tone(SemanticTone::Accent),
        }
    }

    pub const fn command(mut self, command: CommandButtonPaint) -> Self {
        self.command = command;
        self
    }

    pub const fn tone(mut self, tone: SemanticTone) -> Self {
        self.command = CommandButtonPaint::tonal(tone).icon_tone(tone);
        self
    }

    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.command = self.command.hovered(hovered);
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.command = self.command.pressed(pressed);
        self
    }
}

impl Default for DisclosureButtonPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_command_button(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    icon: Option<IconGlyph>,
    style: CommandButtonPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (tone_color, tone_text) = theme.semantic_tone_colors(style.tone);
    let (base_fill, border, label_color) = match style.fill {
        CommandButtonFill::Surface => {
            let label_color = if style.tone == SemanticTone::Neutral {
                theme.palette.text
            } else {
                tone_color
            };
            let border = if style.tone == SemanticTone::Neutral {
                theme.palette.border
            } else {
                tone_color.with_alpha(0.72)
            };
            (theme.surfaces.field, border, label_color)
        }
        CommandButtonFill::Filled => (tone_color, tone_color, tone_text),
    };
    let fill = match style.fill {
        CommandButtonFill::Surface if style.pressed => theme.palette.control_active,
        CommandButtonFill::Surface if style.hovered => theme.palette.control_hover,
        CommandButtonFill::Filled
            if style.pressed && matches!(style.tone, SemanticTone::Accent) =>
        {
            theme.palette.accent_pressed
        }
        CommandButtonFill::Filled
            if style.hovered && matches!(style.tone, SemanticTone::Accent) =>
        {
            theme.palette.accent_hover
        }
        _ => base_fill,
    };
    let icon_color = style
        .icon_tone
        .map(|tone| {
            if style.fill == CommandButtonFill::Filled && tone == style.tone {
                theme.semantic_tone_text_color(tone)
            } else if tone == SemanticTone::Neutral {
                theme.palette.text_muted
            } else {
                theme.semantic_tone_color(tone)
            }
        })
        .unwrap_or_else(|| match style.fill {
            CommandButtonFill::Surface => {
                if style.tone == SemanticTone::Neutral {
                    theme.palette.text_muted
                } else {
                    tone_color
                }
            }
            CommandButtonFill::Filled => tone_text,
        });

    let radius = theme
        .metrics
        .corner_radius
        .min(rect.height() * 0.35)
        .max(0.0);
    ctx.fill(rounded_rect_path(rect, radius), fill);
    ctx.stroke(
        rounded_rect_path(rect, radius),
        border,
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );

    let icon_size = (rect.height() - 14.0).clamp(12.0, 16.0);
    let padding = theme
        .metrics
        .button_padding
        .left
        .max(8.0)
        .min(rect.width() * 0.4);
    let gap = theme.metrics.icon_label_gap.max(5.0);
    let mut text_x = rect.x() + padding;
    if let Some(icon) = icon {
        let icon_rect = Rect::new(
            rect.x() + padding,
            rect.y() + (rect.height() - icon_size) * 0.5,
            icon_size,
            icon_size,
        );
        draw_icon_glyph(ctx, icon, icon_rect, icon_color);
        text_x = icon_rect.max_x() + gap;
    }

    let label_rect = Rect::new(
        text_x,
        rect.y(),
        (rect.max_x() - text_x - padding * 0.75).max(0.0),
        rect.height(),
    );
    if label_rect.width() <= 0.0 {
        return;
    }

    let mut text_style = text_token_style(theme, theme.text.sm, label_color);
    text_style.weight = FontWeight::SEMIBOLD;
    let text_style = numeric_text_style_if_numeric(label, text_style);
    ctx.push_clip_rect(label_rect);
    paint_single_line_aligned_text(
        ctx,
        label_rect,
        label,
        &text_style,
        text_style.line_height,
        0.0,
    );
    ctx.pop_clip();
}

pub fn paint_disclosure_button(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    expanded: bool,
    paint: DisclosureButtonPaint,
) {
    paint_command_button(
        ctx,
        theme,
        rect,
        label,
        Some(if expanded {
            IconGlyph::ChevronUp
        } else {
            IconGlyph::ChevronDown
        }),
        paint.command,
    );
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionTilePaint {
    pub tone: SemanticTone,
    pub highlighted: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub enabled: bool,
    pub background: Option<Color>,
    pub border: Option<Color>,
    pub title_color: Option<Color>,
    pub subtitle_color: Option<Color>,
    pub icon_color: Option<Color>,
    pub leading_tone_dot: Option<SemanticTone>,
    pub radius: Option<f32>,
    pub padding_x: Option<f32>,
    pub leading_width: f32,
    pub trailing_width: f32,
}

impl ActionTilePaint {
    pub const fn neutral() -> Self {
        Self {
            tone: SemanticTone::Neutral,
            highlighted: false,
            hovered: false,
            pressed: false,
            enabled: true,
            background: None,
            border: None,
            title_color: None,
            subtitle_color: None,
            icon_color: None,
            leading_tone_dot: None,
            radius: None,
            padding_x: None,
            leading_width: 0.0,
            trailing_width: 0.0,
        }
    }

    pub const fn tonal(tone: SemanticTone) -> Self {
        Self {
            tone,
            highlighted: true,
            hovered: false,
            pressed: false,
            enabled: true,
            background: None,
            border: None,
            title_color: None,
            subtitle_color: None,
            icon_color: None,
            leading_tone_dot: None,
            radius: None,
            padding_x: None,
            leading_width: 0.0,
            trailing_width: 0.0,
        }
    }

    pub const fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub const fn background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn title_color(mut self, title_color: Color) -> Self {
        self.title_color = Some(title_color);
        self
    }

    pub const fn subtitle_color(mut self, subtitle_color: Color) -> Self {
        self.subtitle_color = Some(subtitle_color);
        self
    }

    pub const fn icon_color(mut self, icon_color: Color) -> Self {
        self.icon_color = Some(icon_color);
        self
    }

    pub const fn leading_tone_dot(mut self, tone: SemanticTone) -> Self {
        self.leading_tone_dot = Some(tone);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn padding_x(mut self, padding_x: f32) -> Self {
        self.padding_x = Some(padding_x);
        self
    }

    pub const fn leading_width(mut self, leading_width: f32) -> Self {
        self.leading_width = leading_width;
        self
    }

    pub const fn trailing_width(mut self, trailing_width: f32) -> Self {
        self.trailing_width = trailing_width;
        self
    }
}

impl Default for ActionTilePaint {
    fn default() -> Self {
        Self::neutral()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalloutPaint {
    pub tone: SemanticTone,
    pub fill: Option<Color>,
    pub border: Option<Color>,
    pub rail_color: Option<Color>,
    pub icon_color: Option<Color>,
    pub title_color: Option<Color>,
    pub body_color: Option<Color>,
    pub radius: Option<f32>,
    pub padding: Insets,
    pub icon_size: f32,
    pub icon_gap: f32,
    pub rail_width: f32,
    pub reserved_bottom: f32,
}

impl CalloutPaint {
    pub const fn new(tone: SemanticTone) -> Self {
        Self {
            tone,
            fill: None,
            border: None,
            rail_color: None,
            icon_color: None,
            title_color: None,
            body_color: None,
            radius: None,
            padding: Insets {
                left: 12.0,
                top: 10.0,
                right: 12.0,
                bottom: 10.0,
            },
            icon_size: 15.0,
            icon_gap: 9.0,
            rail_width: 2.0,
            reserved_bottom: 0.0,
        }
    }

    pub const fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn rail_color(mut self, rail_color: Color) -> Self {
        self.rail_color = Some(rail_color);
        self
    }

    pub const fn icon_color(mut self, icon_color: Color) -> Self {
        self.icon_color = Some(icon_color);
        self
    }

    pub const fn title_color(mut self, title_color: Color) -> Self {
        self.title_color = Some(title_color);
        self
    }

    pub const fn body_color(mut self, body_color: Color) -> Self {
        self.body_color = Some(body_color);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub const fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = icon_size;
        self
    }

    pub const fn icon_gap(mut self, icon_gap: f32) -> Self {
        self.icon_gap = icon_gap;
        self
    }

    pub const fn rail_width(mut self, rail_width: f32) -> Self {
        self.rail_width = rail_width;
        self
    }

    pub const fn reserved_bottom(mut self, reserved_bottom: f32) -> Self {
        self.reserved_bottom = reserved_bottom;
        self
    }
}

impl Default for CalloutPaint {
    fn default() -> Self {
        Self::new(SemanticTone::Info)
    }
}

pub fn paint_callout(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    icon: Option<IconGlyph>,
    title: Option<&str>,
    body: &str,
    style: CalloutPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let palette = theme.palette;
    let (tone_color, _) = theme.semantic_tone_colors(style.tone);
    // Mesh callout: quiet soft wash, hairline border, a 2px status rail, and
    // the status-hued ink (not the on-solid content color) for the icon.
    let (tone_soft, tone_ink) = theme.semantic_tone_soft_colors(style.tone);
    let fill = style.fill.unwrap_or(tone_soft);
    let border = style.border.unwrap_or(palette.border);
    let rail = style.rail_color.unwrap_or(tone_color);
    let radius = style.radius.unwrap_or(theme.radius.md).max(0.0);

    ctx.fill(rounded_rect_path(rect, radius), fill);
    ctx.stroke(
        rounded_rect_path(rect, radius),
        border,
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );

    let rail_width = style.rail_width.max(0.0).min(rect.width());
    if rail_width > 0.0 {
        let rail_rect = Rect::new(rect.x(), rect.y(), rail_width, rect.height());
        ctx.fill(rounded_rect_path(rail_rect, rail_width * 0.5), rail);
    }

    let padding = style.padding;
    let content_bottom = (rect.max_y() - padding.bottom.max(0.0) - style.reserved_bottom.max(0.0))
        .max(rect.y() + padding.top.max(0.0));
    let content = Rect::new(
        rect.x() + padding.left.max(0.0),
        rect.y() + padding.top.max(0.0),
        (rect.width() - padding.left.max(0.0) - padding.right.max(0.0)).max(0.0),
        (content_bottom - rect.y() - padding.top.max(0.0)).max(0.0),
    );
    if content.width() <= 0.0 || content.height() <= 0.0 {
        return;
    }

    let icon_size = style
        .icon_size
        .max(0.0)
        .min(content.height())
        .min(content.width());
    let mut text_x = content.x();
    if let Some(icon) = icon.filter(|_| icon_size > 0.0) {
        let icon_rect = Rect::new(
            content.x(),
            content.y() + 2.0_f32.min((content.height() - icon_size).max(0.0)),
            icon_size,
            icon_size,
        );
        draw_icon_glyph(ctx, icon, icon_rect, style.icon_color.unwrap_or(tone_ink));
        text_x = icon_rect.max_x() + style.icon_gap.max(0.0);
    }

    let text_rect = Rect::new(
        text_x,
        content.y(),
        (content.max_x() - text_x).max(0.0),
        content.height(),
    );
    if text_rect.width() <= 0.0 || text_rect.height() <= 0.0 {
        return;
    }

    let title_line = if title.is_some() {
        theme.text.sm.line_height
    } else {
        0.0
    };
    if let Some(title) = title {
        let mut title_style = text_token_style(
            theme,
            theme.text.sm,
            style.title_color.unwrap_or(palette.text),
        );
        title_style.weight = FontWeight::SEMIBOLD;
        let title_rect = Rect::new(text_rect.x(), text_rect.y(), text_rect.width(), title_line);
        ctx.push_clip_rect(title_rect);
        paint_single_line_aligned_text(
            ctx,
            title_rect,
            title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }

    if body.trim().is_empty() {
        return;
    }
    let body_y = text_rect.y() + title_line + if title.is_some() { 4.0 } else { 0.0 };
    let body_rect = Rect::new(
        text_rect.x(),
        body_y,
        text_rect.width(),
        (text_rect.max_y() - body_y).max(0.0),
    );
    if body_rect.width() <= 0.0 || body_rect.height() <= 0.0 {
        return;
    }

    let color = style.body_color.unwrap_or(palette.text_muted);
    let mut layout_style = text_token_style(theme, theme.text.sm, color);
    layout_style.color = Color::WHITE;
    let mut document = TextDocument::from_plain_text(body.to_string(), layout_style);
    for paragraph in &mut document.paragraphs {
        paragraph.style.align = TextAlign::Start;
        paragraph.style.wrap = TextWrap::Word;
    }

    ctx.push_clip_rect(body_rect);
    if let Ok(layout) = ctx.layout_text_document(TextLayoutRequest::new(document).with_box_size(
        Size::new(body_rect.width().max(1.0), body_rect.height().max(1.0)),
    )) {
        ctx.draw_text_layout_with_color(body_rect.origin, &layout, color);
    } else {
        let fallback_style = text_token_style(theme, theme.text.sm, color);
        ctx.draw_text(body_rect, body.to_string(), fallback_style);
    }
    ctx.pop_clip();
}

pub fn paint_action_tile(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    title: &str,
    subtitle: Option<&str>,
    icon: Option<IconGlyph>,
    style: ActionTilePaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let palette = theme.palette;
    let tone_color = theme.semantic_tone_color(style.tone);
    let effective_tone = if style.tone == SemanticTone::Neutral {
        palette.text_muted
    } else {
        tone_color
    };
    let base_background = if !style.enabled {
        mix_color(palette.control, palette.surface, 0.68).with_alpha(0.82)
    } else if style.pressed {
        palette.control_active
    } else if style.hovered {
        palette.control_hover
    } else {
        palette.control
    };
    let background = style.background.unwrap_or(base_background);
    let base_border = if !style.enabled {
        palette.border.with_alpha(0.55)
    } else if style.highlighted {
        effective_tone.with_alpha(0.84)
    } else if style.hovered {
        palette.border_hover
    } else {
        palette.border
    };
    let border = style.border.unwrap_or(base_border);
    let radius = theme
        .metrics
        .corner_radius
        .min(rect.height() * 0.28)
        .max(0.0);
    let radius = style.radius.unwrap_or(radius).max(0.0);
    ctx.fill(rounded_rect_path(rect, radius), background);
    ctx.stroke(
        rounded_rect_path(rect, radius),
        border,
        StrokeStyle::new(theme.metrics.border_width.max(1.0)),
    );

    let padding_x = style
        .padding_x
        .unwrap_or_else(|| theme.metrics.button_padding.left.max(10.0))
        .max(0.0)
        .min(rect.width() * 0.45);
    let compact = rect.height() <= 46.0 || subtitle.is_none();
    let base_icon_side: f32 = if compact { 14.0 } else { 17.0 };
    let icon_side = base_icon_side
        .min((rect.height() - 14.0).max(10.0))
        .max(0.0);
    let icon_y = if compact {
        rect.y() + (rect.height() - icon_side) * 0.5
    } else {
        rect.y() + 12.0
    };
    let mut text_x = rect.x() + padding_x;
    if let Some(icon) = icon {
        let icon_rect = Rect::new(rect.x() + padding_x, icon_y, icon_side, icon_side);
        let icon_color = style.icon_color.unwrap_or_else(|| {
            if style.enabled {
                if style.highlighted || style.hovered {
                    effective_tone
                } else {
                    palette.text_muted
                }
            } else {
                palette.text.with_alpha(0.34)
            }
        });
        draw_icon_glyph(ctx, icon, icon_rect, icon_color);
        text_x = icon_rect.max_x() + theme.metrics.icon_label_gap.max(7.0);
    } else if let Some(dot_tone) = style.leading_tone_dot {
        let dot_side = 8.0_f32.min((rect.height() - 12.0).max(4.0)).max(4.0);
        let leading_width = style
            .leading_width
            .max(dot_side + theme.metrics.icon_label_gap.max(7.0));
        let dot_rect = Rect::new(
            rect.x() + padding_x,
            if compact {
                rect.y() + (rect.height() - dot_side) * 0.5
            } else {
                rect.y() + 15.0_f32.min((rect.height() - dot_side).max(0.0))
            },
            dot_side,
            dot_side,
        );
        ctx.fill(
            rounded_rect_path(dot_rect, dot_side * 0.5),
            theme.semantic_tone_color(dot_tone),
        );
        text_x += leading_width;
    } else if style.leading_width > 0.0 {
        text_x += style.leading_width;
    }

    let text_width = (rect.max_x() - text_x - padding_x - style.trailing_width.max(0.0)).max(0.0);
    if text_width <= 0.0 {
        return;
    }

    let title_color = style.title_color.unwrap_or_else(|| {
        if !style.enabled {
            palette.text.with_alpha(0.42)
        } else if style.highlighted {
            palette.text
        } else {
            palette.text_muted
        }
    });
    let subtitle_color = style.subtitle_color.unwrap_or_else(|| {
        if !style.enabled {
            palette.text.with_alpha(0.32)
        } else {
            palette.placeholder
        }
    });
    let mut title_style = text_token_style(theme, theme.text.sm, title_color);
    title_style.weight = FontWeight::SEMIBOLD;
    let title_style = numeric_text_style_if_numeric(title, title_style);
    let subtitle_style = text_token_style(theme, theme.text.xs, subtitle_color);

    if compact {
        let title_rect = Rect::new(text_x, rect.y(), text_width, rect.height());
        ctx.push_clip_rect(title_rect);
        paint_aligned_text(
            ctx,
            title_rect,
            title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
        return;
    }

    let title_rect = Rect::new(text_x, rect.y() + 8.0, text_width, title_style.line_height);
    ctx.push_clip_rect(title_rect);
    paint_single_line_aligned_text(
        ctx,
        title_rect,
        title,
        &title_style,
        title_style.line_height,
        0.0,
    );
    ctx.pop_clip();

    if let Some(subtitle) = subtitle {
        let subtitle_rect = Rect::new(
            text_x,
            title_rect.max_y() + theme.metrics.action_card_text_gap.min(3.0),
            text_width,
            subtitle_style.line_height,
        );
        ctx.push_clip_rect(subtitle_rect);
        paint_single_line_aligned_text(
            ctx,
            subtitle_rect,
            subtitle,
            &subtitle_style,
            subtitle_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodePanelPaint {
    pub fill: Option<Color>,
    pub border: Option<Color>,
    pub header_fill: Option<Color>,
    pub label_color: Option<Color>,
    pub radius: Option<f32>,
    pub header_height: f32,
    pub content_padding: Insets,
    pub label_inset_x: f32,
}

impl CodePanelPaint {
    pub const fn new() -> Self {
        Self {
            fill: None,
            border: None,
            header_fill: None,
            label_color: None,
            radius: None,
            header_height: 24.0,
            content_padding: Insets {
                left: 8.0,
                top: 6.0,
                right: 8.0,
                bottom: 4.0,
            },
            label_inset_x: 10.0,
        }
    }

    pub const fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn header_fill(mut self, header_fill: Color) -> Self {
        self.header_fill = Some(header_fill);
        self
    }

    pub const fn label_color(mut self, label_color: Color) -> Self {
        self.label_color = Some(label_color);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn header_height(mut self, header_height: f32) -> Self {
        self.header_height = header_height;
        self
    }

    pub const fn content_padding(mut self, content_padding: Insets) -> Self {
        self.content_padding = content_padding;
        self
    }

    pub const fn label_inset_x(mut self, label_inset_x: f32) -> Self {
        self.label_inset_x = label_inset_x;
        self
    }
}

impl Default for CodePanelPaint {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeTextSpan<'a> {
    pub text: &'a str,
    pub color: Option<Color>,
}

impl<'a> CodeTextSpan<'a> {
    pub const fn new(text: &'a str) -> Self {
        Self { text, color: None }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeTextLine<'a> {
    pub spans: &'a [CodeTextSpan<'a>],
    pub background: Option<Color>,
    pub fallback_color: Option<Color>,
}

impl<'a> CodeTextLine<'a> {
    pub const fn new(spans: &'a [CodeTextSpan<'a>]) -> Self {
        Self {
            spans,
            background: None,
            fallback_color: None,
        }
    }

    pub const fn background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    pub const fn fallback_color(mut self, color: Color) -> Self {
        self.fallback_color = Some(color);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeTextPaint {
    pub color: Option<Color>,
    /// Font size override. Non-positive values resolve to the active theme's
    /// `xs` text token when painted.
    pub font_size: f32,
    /// Line-height override. Non-positive values resolve to the active theme's
    /// `xs` text token when painted.
    pub line_height: f32,
    pub x_padding: f32,
    pub weight: FontWeight,
}

impl CodeTextPaint {
    pub const fn new() -> Self {
        Self {
            color: None,
            font_size: 0.0,
            line_height: 0.0,
            x_padding: 2.0,
            weight: FontWeight::NORMAL,
        }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub const fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    pub const fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    pub const fn x_padding(mut self, x_padding: f32) -> Self {
        self.x_padding = x_padding;
        self
    }

    pub const fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }
}

impl Default for CodeTextPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_code_lines(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    lines: &[CodeTextLine<'_>],
    style: CodeTextPaint,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || lines.is_empty() {
        return;
    }

    let token = theme.text.xs;
    let font_size = if style.font_size > 0.0 {
        style.font_size
    } else {
        token.size
    };
    let line_height = if style.line_height > 0.0 {
        style.line_height
    } else {
        token.line_height
    };
    let mut base_style = TextStyle {
        font_size: font_size.max(1.0),
        line_height: line_height.max(1.0),
        color: style.color.unwrap_or(theme.palette.text),
        ..theme.mono_text_style(theme.palette.text)
    };
    base_style.weight = style.weight;

    let line_height = base_style.line_height;
    let mut y = rect.y();
    ctx.push_clip_rect(rect);
    for line in lines {
        if y + line_height > rect.max_y() {
            break;
        }
        if let Some(background) = line.background {
            ctx.fill_rect(
                Rect::new(rect.x(), y, rect.width(), line_height),
                background,
            );
        }

        let mut x = rect.x() + style.x_padding.max(0.0);
        for span in line.spans {
            if span.text.is_empty() || x > rect.max_x() {
                continue;
            }
            let mut span_style = base_style.clone();
            span_style.color = span
                .color
                .or(line.fallback_color)
                .unwrap_or(base_style.color);
            let width = ctx
                .measure_text(span.text.to_string(), span_style.clone())
                .ok()
                .map(|measurement| measurement.width)
                .unwrap_or(0.0);
            ctx.draw_text(
                Rect::new(x, y, (rect.max_x() - x).max(0.0), line_height),
                span.text,
                span_style,
            );
            x += width;
        }
        y += line_height;
    }
    ctx.pop_clip();
}

pub fn paint_code_panel(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    label: &str,
    style: CodePanelPaint,
) -> Rect {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Rect::ZERO;
    }

    let fill = style.fill.unwrap_or(theme.surfaces.field);
    let border = style.border.unwrap_or(theme.surfaces.border);
    let header_fill = style.header_fill.unwrap_or(theme.surfaces.titlebar);
    let label_color = style.label_color.unwrap_or(theme.surfaces.text_faint);
    let radius = style
        .radius
        .unwrap_or(theme.radius.xl)
        .min(rect.width().min(rect.height()) * 0.5)
        .max(0.0);
    let border_width = physical_pixels(ctx, theme.metrics.border_width.max(1.0));

    let panel_shape = rounded_rect_path(rect, radius);
    ctx.fill(panel_shape.clone(), fill);

    let header_height = style.header_height.clamp(0.0, rect.height());
    if header_height > 0.0 {
        let header_rect = Rect::new(rect.x(), rect.y(), rect.width(), header_height);
        let header_radius = radius.min(header_height * 0.5);
        ctx.fill(rounded_rect_path(header_rect, header_radius), header_fill);
        if header_height > header_radius {
            ctx.fill_rect(
                Rect::new(
                    header_rect.x(),
                    (header_rect.max_y() - header_radius).max(header_rect.y()),
                    header_rect.width(),
                    header_radius,
                ),
                header_fill,
            );
        }

        let mut label_style = text_token_style(theme, theme.text.xs, label_color);
        label_style.weight = FontWeight::SEMIBOLD;
        let label_x = rect.x() + style.label_inset_x.max(0.0);
        let label_rect = Rect::new(
            label_x,
            rect.y() + ((header_height - label_style.line_height) * 0.5).max(0.0),
            (rect.max_x() - label_x - style.label_inset_x.max(0.0)).max(0.0),
            label_style.line_height,
        );
        if label_rect.width() > 0.0 {
            ctx.push_clip_rect(label_rect);
            paint_single_line_aligned_text(
                ctx,
                label_rect,
                label,
                &label_style,
                label_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }
    }

    ctx.stroke(panel_shape, border, StrokeStyle::new(border_width));

    Rect::new(
        rect.x() + style.content_padding.left,
        rect.y() + header_height + style.content_padding.top,
        (rect.width() - style.content_padding.left - style.content_padding.right).max(0.0),
        (rect.height() - header_height - style.content_padding.top - style.content_padding.bottom)
            .max(0.0),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionPanelPaint {
    pub fill: Option<Color>,
    pub border: Option<Color>,
    pub title_color: Option<Color>,
    pub radius: Option<f32>,
    pub header_height: f32,
    pub content_padding: Insets,
    pub title_inset_x: f32,
    pub trailing_width: f32,
    pub title_token: Option<ThemeTextToken>,
    pub title_weight: FontWeight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionPanelGeometry {
    pub panel_rect: Rect,
    pub header_rect: Rect,
    pub title_rect: Rect,
    pub content_rect: Rect,
}

impl SectionPanelPaint {
    pub const fn new() -> Self {
        Self {
            fill: None,
            border: None,
            title_color: None,
            radius: None,
            header_height: 34.0,
            content_padding: Insets {
                left: 12.0,
                top: 0.0,
                right: 12.0,
                bottom: 8.0,
            },
            title_inset_x: 12.0,
            trailing_width: 0.0,
            title_token: None,
            title_weight: FontWeight::SEMIBOLD,
        }
    }

    pub const fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    pub const fn border(mut self, border: Color) -> Self {
        self.border = Some(border);
        self
    }

    pub const fn title_color(mut self, title_color: Color) -> Self {
        self.title_color = Some(title_color);
        self
    }

    pub const fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    pub const fn header_height(mut self, header_height: f32) -> Self {
        self.header_height = header_height;
        self
    }

    pub const fn content_padding(mut self, content_padding: Insets) -> Self {
        self.content_padding = content_padding;
        self
    }

    pub const fn title_inset_x(mut self, title_inset_x: f32) -> Self {
        self.title_inset_x = title_inset_x;
        self
    }

    pub const fn trailing_width(mut self, trailing_width: f32) -> Self {
        self.trailing_width = trailing_width;
        self
    }

    pub const fn title_token(mut self, title_token: ThemeTextToken) -> Self {
        self.title_token = Some(title_token);
        self
    }

    pub const fn title_weight(mut self, title_weight: FontWeight) -> Self {
        self.title_weight = title_weight;
        self
    }
}

impl Default for SectionPanelPaint {
    fn default() -> Self {
        Self::new()
    }
}

pub fn paint_section_panel(
    ctx: &mut PaintCtx,
    theme: &DefaultTheme,
    rect: Rect,
    title: &str,
    style: SectionPanelPaint,
) -> SectionPanelGeometry {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return SectionPanelGeometry {
            panel_rect: Rect::ZERO,
            header_rect: Rect::ZERO,
            title_rect: Rect::ZERO,
            content_rect: Rect::ZERO,
        };
    }

    let fill = style.fill.unwrap_or(theme.surfaces.panel);
    let border = style.border.unwrap_or(theme.surfaces.border);
    let title_color = style.title_color.unwrap_or(theme.surfaces.text);
    let radius = style
        .radius
        .unwrap_or(theme.radius.lg)
        .min(rect.width().min(rect.height()) * 0.5)
        .max(0.0);
    let header_height = style.header_height.clamp(0.0, rect.height());
    let shape = rounded_rect_path(rect, radius);
    ctx.fill(shape.clone(), fill);
    ctx.stroke(
        shape,
        border,
        StrokeStyle::new(physical_pixels(ctx, theme.metrics.border_width.max(1.0))),
    );

    let header_rect = Rect::new(rect.x(), rect.y(), rect.width(), header_height);
    let mut title_style = text_token_style(
        theme,
        style.title_token.unwrap_or(theme.text.sm),
        title_color,
    );
    title_style.weight = style.title_weight;
    let title_x = rect.x() + style.title_inset_x.max(0.0);
    let title_rect = Rect::new(
        title_x,
        rect.y() + ((header_height - title_style.line_height) * 0.5).max(0.0),
        (rect.max_x() - title_x - style.title_inset_x.max(0.0) - style.trailing_width.max(0.0))
            .max(0.0),
        title_style.line_height,
    );
    if title_rect.width() > 0.0 && !title.is_empty() {
        ctx.push_clip_rect(title_rect);
        paint_single_line_aligned_text(
            ctx,
            title_rect,
            title,
            &title_style,
            title_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }

    let content_rect = Rect::new(
        rect.x() + style.content_padding.left,
        rect.y() + header_height + style.content_padding.top,
        (rect.width() - style.content_padding.left - style.content_padding.right).max(0.0),
        (rect.height() - header_height - style.content_padding.top - style.content_padding.bottom)
            .max(0.0),
    );

    SectionPanelGeometry {
        panel_rect: rect,
        header_rect,
        title_rect,
        content_rect,
    }
}

impl Widget for StatusBadge {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let (height, icon_size, gap, padding) = self.metrics(&theme);
        let label = self.label();
        let tone = self.resolved_tone();
        let text = measure_text(ctx, &label, &self.text_style(&theme, &label, tone));
        let icon_w = self.icon.map(|_| icon_size + gap).unwrap_or(0.0);
        let natural_w = text.width.ceil() + icon_w + padding * 2.0;
        constraints.clamp(Size::new(
            self.min_width.unwrap_or(0.0).max(natural_w),
            height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let label = self.label();
        let tone = self.resolved_tone();
        paint_status_badge(ctx, ctx.bounds(), &theme, &label, self.icon, tone);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let label = self.label();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(label.clone());
        node.value = Some(SemanticsValue::Text(label));
        ctx.push(node);
    }
}
