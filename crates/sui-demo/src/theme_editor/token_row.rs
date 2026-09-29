//! One color token in the panel: a row with its swatch, name, hex value,
//! contrast, and edit marker, or a compact chip for the decorative strip.
//! Pressing it opens the token's editor. Rows observe the editor's theme
//! and selection, so they repaint themselves as colors change.

use sui::prelude::*;
use sui::{
    Border, KeyState, PointerButton, PointerEventKind, SemanticsAction, SemanticsActionRequest,
    SemanticsNode, SemanticsRole, SemanticsValue, paint_text_line,
};

use super::contrast::{ContrastCheck, token_contrast};
use super::export::hex;
use super::state::ThemeEditorState;
use super::tokens::Token;
use crate::app::{DemoTextRole, DevThemeReader, demo_mono_text_style, demo_text_style};

const ROW_HEIGHT: f32 = 36.0;
const CHIP_SIZE: f32 = 34.0;
const SWATCH_SIZE: f32 = 24.0;
const HEX_WIDTH: f32 = 84.0;
const BADGE_WIDTH: f32 = 86.0;

pub(super) struct TokenRow {
    token: Token,
    state: ThemeEditorState,
    shell: DevThemeReader,
    compact: bool,
    hovered: bool,
    pressed: Option<u64>,
}

impl TokenRow {
    pub(super) fn new(token: Token, state: ThemeEditorState, shell: DevThemeReader) -> Self {
        Self {
            token,
            state,
            shell,
            compact: false,
            hovered: false,
            pressed: None,
        }
    }

    /// A swatch-only chip, for strips of related colors.
    pub(super) fn chip(token: Token, state: ThemeEditorState, shell: DevThemeReader) -> Self {
        Self {
            compact: true,
            ..Self::new(token, state, shell)
        }
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        self.state.toggle(self.token);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn contrast(&self) -> Option<ContrastCheck> {
        match self.token {
            Token::Source(source) => token_contrast(source, &self.state.recipe().colors),
            Token::Role(_) => None,
        }
    }

    fn set_hovered(&mut self, ctx: &mut EventCtx, hovered: bool) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
        }
    }

    fn paint_row(&self, ctx: &mut PaintCtx, shell: &DefaultTheme, selected: bool) {
        let bounds = ctx.bounds();
        let palette = shell.palette;
        let radius = shell.metrics.corner_radius;
        let background = if selected {
            Some(palette.selection)
        } else if self.hovered || self.pressed.is_some() {
            Some(palette.control_hover)
        } else {
            None
        };
        if let Some(background) = background {
            ctx.fill_rrect(bounds, [radius; 4], background);
        }

        let swatch = Rect::new(
            bounds.x() + 8.0,
            bounds.y() + (bounds.height() - SWATCH_SIZE) * 0.5,
            SWATCH_SIZE,
            SWATCH_SIZE,
        );
        self.paint_swatch(ctx, shell, swatch);

        let badge = self.contrast();
        let right = bounds.max_x() - 8.0;
        let badge_rect = Rect::new(
            right - BADGE_WIDTH,
            bounds.y() + (bounds.height() - 22.0) * 0.5,
            BADGE_WIDTH,
            22.0,
        );
        let hex_right = if badge.is_some() {
            badge_rect.x() - 8.0
        } else {
            right
        };
        let hex_rect = Rect::new(
            hex_right - HEX_WIDTH,
            bounds.y(),
            HEX_WIDTH,
            bounds.height(),
        );
        let label_rect = Rect::new(
            swatch.max_x() + 10.0,
            bounds.y(),
            (hex_rect.x() - swatch.max_x() - 26.0).max(0.0),
            bounds.height(),
        );

        let label_style = demo_text_style(*shell, DemoTextRole::Supporting, palette.text);
        ctx.push_clip_rect(label_rect);
        paint_text_line(
            ctx,
            label_rect,
            &self.label(),
            &label_style,
            TextAlign::Start,
        );
        ctx.pop_clip();

        if self.state.is_modified(self.token) {
            let center = Point::new(label_rect.max_x() + 8.0, bounds.y() + bounds.height() * 0.5);
            ctx.fill(Path::circle(center, 3.5), palette.accent);
        }

        let hex_style = demo_mono_text_style(*shell, DemoTextRole::Metadata, palette.text_muted);
        paint_text_line(
            ctx,
            hex_rect,
            &hex(self.state.color(self.token)),
            &hex_style,
            TextAlign::End,
        );

        if let Some(check) = badge {
            let (fill, ink) = if check.passes() {
                (palette.success_soft, palette.success_soft_text)
            } else {
                (palette.danger_soft, palette.danger_soft_text)
            };
            ctx.fill_rrect(badge_rect, [badge_rect.height() * 0.5; 4], fill);
            let badge_style = demo_text_style(*shell, DemoTextRole::Metadata, ink);
            paint_text_line(
                ctx,
                badge_rect,
                &check.badge(),
                &badge_style,
                TextAlign::Center,
            );
        }
    }

    fn label(&self) -> String {
        match self.token {
            Token::Source(source) => source.label().to_string(),
            Token::Role(role) => role.label().to_string(),
        }
    }

    /// A plain swatch, or for an "on" color a sample of text on its fill.
    fn paint_swatch(&self, ctx: &mut PaintCtx, shell: &DefaultTheme, rect: Rect) {
        let radius = 6.0;
        let color = self.state.color(self.token);
        let fill = match self.token {
            Token::Source(source) => source
                .fill()
                .map(|fill| fill.get(&self.state.recipe().colors)),
            Token::Role(_) => None,
        };
        ctx.fill_rrect(rect, [radius; 4], shell.palette.surface);
        ctx.fill_rrect_bordered(
            rect,
            [radius; 4],
            fill.unwrap_or(color),
            Border {
                width: 1.0,
                color: shell.palette.border_strong,
            },
        );
        if fill.is_some() {
            let style = demo_text_style(*shell, DemoTextRole::Metadata, color);
            paint_text_line(ctx, rect, "Aa", &style, TextAlign::Center);
        }
    }

    fn paint_chip(&self, ctx: &mut PaintCtx, shell: &DefaultTheme, selected: bool) {
        let bounds = ctx.bounds();
        let palette = shell.palette;
        let inner = bounds.inflate(-3.0, -3.0);
        if selected {
            ctx.fill_rrect(bounds, [9.0; 4], palette.accent);
        } else if self.hovered {
            ctx.fill_rrect(bounds, [9.0; 4], palette.border_strong);
        }
        ctx.fill_rrect(inner, [7.0; 4], palette.surface);
        ctx.fill_rrect_bordered(
            inner,
            [7.0; 4],
            self.state.color(self.token),
            Border {
                width: 1.0,
                color: palette.border,
            },
        );
        if self.state.is_modified(self.token) {
            let center = Point::new(inner.max_x() - 1.0, inner.y() + 1.0);
            ctx.fill(Path::circle(center, 4.5), palette.surface);
            ctx.fill(Path::circle(center, 3.0), palette.accent);
        }
    }
}

impl Widget for TokenRow {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let inside = ctx.bounds().contains(pointer.position);
                self.set_hovered(ctx, inside);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(ctx, false);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.pressed = Some(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && self.pressed == Some(pointer.pointer_id) =>
            {
                self.pressed = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                if ctx.bounds().contains(pointer.position) {
                    self.activate(ctx);
                }
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                }
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.activate(ctx);
                ctx.set_handled();
            }
            Event::Semantics(semantics)
                if semantics.target == ctx.widget_id()
                    && matches!(semantics.action, SemanticsActionRequest::Activate) =>
            {
                self.activate(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = if self.compact {
            Size::new(CHIP_SIZE, CHIP_SIZE)
        } else {
            let width = if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                360.0
            };
            Size::new(width, ROW_HEIGHT)
        };
        constraints.clamp(size)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let _ = ctx.observe(&self.state.theme_signal());
        let selected = ctx.observe(&self.state.selected_signal()) == Some(self.token);
        let shell = (self.shell)();
        if self.compact {
            self.paint_chip(ctx, &shell, selected);
        } else {
            self.paint_row(ctx, &shell, selected);
        }
        if ctx.is_focused() {
            let radius = shell.metrics.corner_radius + 2.0;
            ctx.stroke(
                Path::rounded_rect(ctx.bounds().inflate(1.0, 1.0), radius),
                shell.palette.focus_ring,
                StrokeStyle::new(2.0),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let _ = ctx.observe(&self.state.theme_signal());
        let selected = ctx.observe(&self.state.selected_signal()) == Some(self.token);
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.token.row_name());
        node.value = Some(SemanticsValue::Text(hex(self.state.color(self.token))));
        node.description = self.contrast().map(|check| {
            format!(
                "Contrast {}{}",
                check.badge(),
                if self.state.is_modified(self.token) {
                    "; edited"
                } else {
                    ""
                }
            )
        });
        node.state.expanded = Some(selected);
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
    }
}
