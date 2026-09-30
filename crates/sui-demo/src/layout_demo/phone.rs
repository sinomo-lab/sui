//! A phone drawn around content, with the system bars and keyboard a safe
//! area keeps the content clear of.

use std::rc::Rc;

use sui::{
    Rect, SafeAreaInsets, SemanticsNode, SemanticsRole, SemanticsValue, Signal,
    WidgetPodMutVisitor, WidgetPodVisitor, paint_text_line, prelude::*,
};

use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};

/// The status bar and the notch in it.
const STATUS_BAR: f32 = 44.0;
const HOME_INDICATOR: f32 = 34.0;
const KEYBOARD: f32 = 216.0;
const PHONE_SIZE: Size = Size::new(300.0, 560.0);
const BEZEL: f32 = 10.0;
const SCREEN_RADIUS: f32 = 30.0;

/// The insets the simulated phone reports: its status bar and home
/// indicator, and the keyboard while it is up.
pub(super) fn phone_insets(keyboard: bool) -> SafeAreaInsets {
    let bottom = if keyboard {
        HOME_INDICATOR + KEYBOARD
    } else {
        HOME_INDICATOR
    };
    SafeAreaInsets::new(0.0, STATUS_BAR, 0.0, bottom)
}

pub(super) fn insets_text(insets: SafeAreaInsets) -> String {
    format!(
        "Top {} · bottom {} · left {} · right {}",
        insets.top, insets.bottom, insets.left, insets.right
    )
}

/// Draws a phone around `child`, which fills its screen.
pub(super) struct PhoneMock {
    theme_reader: DevThemeReader,
    keyboard: Signal<bool>,
    child: SingleChild,
}

impl PhoneMock {
    pub(super) fn new<W>(theme_reader: &DevThemeReader, keyboard: &Signal<bool>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            keyboard: keyboard.clone(),
            child: SingleChild::new(child),
        }
    }
}

/// The phone's screen, inside its bezel, for a phone at `bounds`.
fn screen_rect(bounds: Rect) -> Rect {
    Rect::new(
        bounds.x() + BEZEL,
        bounds.y() + BEZEL,
        PHONE_SIZE.width - BEZEL * 2.0,
        PHONE_SIZE.height - BEZEL * 2.0,
    )
}

impl Widget for PhoneMock {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let screen = Size::new(
            PHONE_SIZE.width - BEZEL * 2.0,
            PHONE_SIZE.height - BEZEL * 2.0,
        );
        self.child.measure(ctx, Constraints::tight(screen));
        constraints.clamp(PHONE_SIZE)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, screen_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let keyboard = ctx.observe(&self.keyboard);
        let body = theme.palette.text;
        let screen = screen_rect(ctx.bounds());
        let outline = Rect::new(
            screen.x() - BEZEL,
            screen.y() - BEZEL,
            PHONE_SIZE.width,
            PHONE_SIZE.height,
        );
        ctx.fill(Path::rounded_rect(outline, SCREEN_RADIUS + BEZEL), body);
        ctx.fill(
            Path::rounded_rect(screen, SCREEN_RADIUS),
            theme.palette.surface,
        );
        // Where system UI sits, tinted so the insets show.
        let unsafe_tint = theme.decorative.get(DecorativeHue::Amber).soft;
        let insets = phone_insets(keyboard);
        ctx.push_clip(Path::rounded_rect(screen, SCREEN_RADIUS));
        ctx.fill(
            Path::rounded_rect(
                Rect::new(screen.x(), screen.y(), screen.width(), insets.top),
                0.0,
            ),
            unsafe_tint,
        );
        ctx.fill(
            Path::rounded_rect(
                Rect::new(
                    screen.x(),
                    screen.max_y() - insets.bottom,
                    screen.width(),
                    insets.bottom,
                ),
                0.0,
            ),
            unsafe_tint,
        );
        self.child.paint(ctx);

        let style = demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text);
        paint_text_line(
            ctx,
            Rect::new(
                screen.x() + 28.0,
                screen.y() + 14.0,
                60.0,
                style.line_height,
            ),
            "9:41",
            &style,
            TextAlign::Start,
        );
        ctx.fill(
            Path::rounded_rect(
                Rect::new(screen.max_x() - 52.0, screen.y() + 17.0, 24.0, 11.0),
                3.0,
            ),
            theme.palette.text,
        );
        ctx.fill(
            Path::rounded_rect(
                Rect::new(
                    screen.x() + (screen.width() - 96.0) * 0.5,
                    screen.y() + 8.0,
                    96.0,
                    26.0,
                ),
                13.0,
            ),
            body,
        );
        if keyboard {
            let panel = Rect::new(
                screen.x(),
                screen.max_y() - HOME_INDICATOR - KEYBOARD,
                screen.width(),
                KEYBOARD + HOME_INDICATOR,
            );
            ctx.fill(Path::rounded_rect(panel, 0.0), theme.palette.control);
            let rows = [10_usize, 9, 7];
            let key_height = 38.0;
            for (row, keys) in rows.iter().enumerate() {
                let gap = 5.0;
                let key_width = (screen.width() - 12.0 - gap * 9.0) / 10.0;
                let row_width = key_width * *keys as f32 + gap * (*keys as f32 - 1.0);
                let x = screen.x() + (screen.width() - row_width) * 0.5;
                let y = panel.y() + 12.0 + row as f32 * (key_height + 10.0);
                for key in 0..*keys {
                    ctx.fill(
                        Path::rounded_rect(
                            Rect::new(x + key as f32 * (key_width + gap), y, key_width, key_height),
                            5.0,
                        ),
                        theme.palette.surface_raised,
                    );
                }
            }
            ctx.fill(
                Path::rounded_rect(
                    Rect::new(
                        screen.x() + screen.width() * 0.25,
                        panel.y() + 12.0 + 3.0 * (key_height + 10.0),
                        screen.width() * 0.5,
                        key_height,
                    ),
                    5.0,
                ),
                theme.palette.surface_raised,
            );
        }
        ctx.fill(
            Path::rounded_rect(
                Rect::new(
                    screen.x() + (screen.width() - 110.0) * 0.5,
                    screen.max_y() - 12.0,
                    110.0,
                    5.0,
                ),
                2.5,
            ),
            theme.palette.text,
        );
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some("Phone preview".to_string());
        node.value = Some(SemanticsValue::Text(insets_text(phone_insets(
            self.keyboard.get(),
        ))));
        ctx.push(node);
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
    use super::*;

    #[test]
    fn the_keyboard_raises_the_bottom_inset() {
        assert_eq!(
            phone_insets(false),
            SafeAreaInsets::new(0.0, 44.0, 0.0, 34.0)
        );
        assert_eq!(phone_insets(true).bottom, 250.0);
        assert_eq!(
            insets_text(phone_insets(true)),
            "Top 44 · bottom 250 · left 0 · right 0"
        );
    }
}
