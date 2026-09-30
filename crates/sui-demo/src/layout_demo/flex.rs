//! The flex playground: a row or column of five tiles laid out by the
//! chosen flex settings, and the builder code for them.

use std::rc::Rc;

use sui::{
    EventPhase, FlexStyle, KeyState, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsValue, Signal,
    WidgetPodMutVisitor, WidgetPodVisitor, paint_text_line, prelude::*,
};

use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};

/// How one tile takes its share of the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TileMode {
    Fixed,
    Grow,
    Capped,
}

impl TileMode {
    fn next(self) -> Self {
        match self {
            Self::Fixed => Self::Grow,
            Self::Grow => Self::Capped,
            Self::Capped => Self::Fixed,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Grow => "grows",
            Self::Capped => "capped",
        }
    }

    /// Main-axis size a tile starts from, and the most a capped tile grows
    /// to, along a row or a column.
    fn sizes(column: bool) -> (f32, f32) {
        if column { (36.0, 64.0) } else { (110.0, 160.0) }
    }

    fn item(self, column: bool) -> FlexItem {
        let (basis, cap) = Self::sizes(column);
        match self {
            Self::Fixed => FlexItem::fixed(basis),
            Self::Grow => FlexItem::new().grow(1.0).basis(basis),
            Self::Capped if column => FlexItem::new().grow(1.0).basis(basis).max_height(cap),
            Self::Capped => FlexItem::new().grow(1.0).basis(basis).max_width(cap),
        }
    }

    fn code(self, column: bool) -> String {
        let (basis, cap) = Self::sizes(column);
        match self {
            Self::Fixed => format!("FlexItem::fixed({basis:.1})"),
            Self::Grow => format!("FlexItem::new().grow(1.0).basis({basis:.1})"),
            Self::Capped => format!(
                "FlexItem::new().grow(1.0).basis({basis:.1}).{}({cap:.1})",
                if column { "max_height" } else { "max_width" }
            ),
        }
    }
}

pub(super) const TILE_COUNT: usize = 5;
const TILE_LETTERS: [&str; TILE_COUNT] = ["A", "B", "C", "D", "E"];
const TILE_HUES: [DecorativeHue; TILE_COUNT] = [
    DecorativeHue::Blue,
    DecorativeHue::Teal,
    DecorativeHue::Violet,
    DecorativeHue::Orange,
    DecorativeHue::Magenta,
];
/// Natural heights, different so cross-axis alignment shows.
const TILE_HEIGHTS: [f32; TILE_COUNT] = [52.0, 76.0, 60.0, 68.0, 44.0];
/// How tall the playground is laid out as a column.
pub(super) const COLUMN_HEIGHT: f32 = 280.0;

pub(super) const JUSTIFY: [(FlexJustify, &str, &str); 6] = [
    (FlexJustify::Start, "Start", "Start"),
    (FlexJustify::Center, "Center", "Center"),
    (FlexJustify::End, "End", "End"),
    (FlexJustify::SpaceBetween, "Space between", "SpaceBetween"),
    (FlexJustify::SpaceAround, "Space around", "SpaceAround"),
    (FlexJustify::SpaceEvenly, "Space evenly", "SpaceEvenly"),
];

pub(super) const ALIGN: [(Alignment, &str, &str); 4] = [
    (Alignment::Start, "Start", "Start"),
    (Alignment::Center, "Center", "Center"),
    (Alignment::End, "End", "End"),
    (Alignment::Stretch, "Stretch", "Stretch"),
];

/// What the playground's controls chose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FlexSettings {
    pub(super) column: bool,
    /// Index into [`JUSTIFY`].
    pub(super) justify: usize,
    /// Index into [`ALIGN`].
    pub(super) align: usize,
    pub(super) gap: f32,
    pub(super) wrap: bool,
    pub(super) modes: [TileMode; TILE_COUNT],
}

impl Default for FlexSettings {
    fn default() -> Self {
        Self {
            column: false,
            justify: 0,
            align: 1,
            gap: 12.0,
            wrap: false,
            modes: [
                TileMode::Fixed,
                TileMode::Grow,
                TileMode::Capped,
                TileMode::Fixed,
                TileMode::Grow,
            ],
        }
    }
}

impl FlexSettings {
    fn style(&self) -> FlexStyle {
        FlexStyle::new(if self.column {
            Axis::Vertical
        } else {
            Axis::Horizontal
        })
        .justify(JUSTIFY[self.justify].0)
        .align_items(ALIGN[self.align].0)
        .gap(self.gap)
        .wrap(if self.wrap {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        })
    }
}

/// The builder code that lays the tiles out as `settings` does.
pub(super) fn flex_code(settings: &FlexSettings) -> String {
    let mut code = format!(
        "Flex::{}()\n    .justify(FlexJustify::{})\n    .align_items(Alignment::{})\n    .gap({:.1})\n    .wrap(FlexWrap::{})",
        if settings.column {
            "vertical"
        } else {
            "horizontal"
        },
        JUSTIFY[settings.justify].2,
        ALIGN[settings.align].2,
        settings.gap,
        if settings.wrap { "Wrap" } else { "NoWrap" },
    );
    for (letter, mode) in TILE_LETTERS.iter().zip(settings.modes) {
        code.push_str(&format!(
            "\n    .with_item(tile(\"{letter}\"), {})",
            mode.code(settings.column)
        ));
    }
    code
}

/// The five tiles in a [`Flex`] that follows the settings. The flex is
/// restyled in place, so the tiles, and keyboard focus on them, stay put.
pub(super) struct FlexPlayground {
    settings: Signal<FlexSettings>,
    applied: Option<FlexSettings>,
    flex: Flex,
}

impl FlexPlayground {
    pub(super) fn new(
        theme_reader: &DevThemeReader,
        settings: &Signal<FlexSettings>,
        show_sizes: &Signal<bool>,
    ) -> Self {
        let mut flex = Flex::horizontal();
        for index in 0..TILE_COUNT {
            flex.push_item(
                FlexTile {
                    index,
                    settings: settings.clone(),
                    show_sizes: show_sizes.clone(),
                    theme_reader: Rc::clone(theme_reader),
                    pressed: false,
                },
                FlexItem::new(),
            );
        }
        Self {
            settings: settings.clone(),
            applied: None,
            flex,
        }
    }

    fn apply(&mut self, settings: FlexSettings) {
        if self.applied == Some(settings) {
            return;
        }
        self.applied = Some(settings);
        self.flex =
            std::mem::replace(&mut self.flex, Flex::horizontal()).with_style(settings.style());
        for (index, mode) in settings.modes.iter().enumerate() {
            self.flex.set_item(index, mode.item(settings.column));
        }
    }
}

impl Widget for FlexPlayground {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let settings = ctx.observe(&self.settings);
        self.apply(settings);
        let constraints = if settings.column {
            Constraints::new(
                Size::new(constraints.min.width, COLUMN_HEIGHT),
                Size::new(constraints.max.width, COLUMN_HEIGHT),
            )
        } else {
            constraints
        };
        self.flex.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.flex.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.flex.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.flex.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.flex.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.flex.visit_children_mut(visitor);
    }
}

/// One tile; pressing it switches it between fixed, grows and capped.
struct FlexTile {
    index: usize,
    settings: Signal<FlexSettings>,
    show_sizes: Signal<bool>,
    theme_reader: DevThemeReader,
    pressed: bool,
}

impl FlexTile {
    fn name(&self) -> String {
        format!("Tile {}", TILE_LETTERS[self.index])
    }

    fn cycle(&self, ctx: &mut EventCtx) {
        let index = self.index;
        self.settings
            .update(|settings| settings.modes[index] = settings.modes[index].next());
        ctx.request_paint();
        ctx.request_semantics();
        ctx.set_handled();
    }
}

impl Widget for FlexTile {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() == EventPhase::Capture {
            return;
        }
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.pressed = true;
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Up && self.pressed => {
                self.pressed = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                if ctx.bounds().contains(pointer.position) {
                    self.cycle(ctx);
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel && self.pressed => {
                self.pressed = false;
                ctx.release_pointer_capture(pointer.pointer_id);
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && !key.repeat
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.cycle(ctx);
            }
            Event::Semantics(semantics)
                if semantics.target == ctx.widget_id()
                    && matches!(semantics.action, SemanticsActionRequest::Activate) =>
            {
                self.cycle(ctx);
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, TILE_HEIGHTS[self.index]))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let settings = ctx.observe(&self.settings);
        let show_sizes = ctx.observe(&self.show_sizes);
        let roles = theme.decorative.get(TILE_HUES[self.index]);
        let bounds = ctx.bounds();
        ctx.fill(Path::rounded_rect(bounds, 8.0), roles.solid);
        if ctx.is_focused() {
            ctx.stroke(
                Path::rounded_rect(bounds.inflate(-1.5, -1.5), 7.0),
                roles.on_solid,
                StrokeStyle::new(2.0),
            );
        }

        let style = demo_text_style(theme, DemoTextRole::Metadata, roles.on_solid);
        let mut lines = vec![format!(
            "{} · {}",
            TILE_LETTERS[self.index],
            settings.modes[self.index].name()
        )];
        if show_sizes {
            lines.push(format!(
                "{} × {}",
                bounds.width().round(),
                bounds.height().round()
            ));
        }
        let height = style.line_height * lines.len() as f32;
        let mut y = bounds.y() + (bounds.height() - height) * 0.5;
        ctx.push_clip_rect(bounds);
        for line in &lines {
            paint_text_line(
                ctx,
                Rect::new(bounds.x() + 4.0, y, bounds.width() - 8.0, style.line_height),
                line,
                &style,
                TextAlign::Center,
            );
            y += style.line_height;
        }
        ctx.pop_clip();
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mode = self.settings.get().modes[self.index];
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.name());
        node.value = Some(SemanticsValue::Text(mode.name().to_string()));
        node.description = Some("Switches between fixed, grows, and capped".to_string());
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        node.state.focused = ctx.is_focused();
        ctx.push(node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_follows_the_settings() {
        let settings = FlexSettings::default();
        assert_eq!(
            flex_code(&settings),
            "Flex::horizontal()\n    .justify(FlexJustify::Start)\n    .align_items(Alignment::Center)\n    .gap(12.0)\n    .wrap(FlexWrap::NoWrap)\n    .with_item(tile(\"A\"), FlexItem::fixed(110.0))\n    .with_item(tile(\"B\"), FlexItem::new().grow(1.0).basis(110.0))\n    .with_item(tile(\"C\"), FlexItem::new().grow(1.0).basis(110.0).max_width(160.0))\n    .with_item(tile(\"D\"), FlexItem::fixed(110.0))\n    .with_item(tile(\"E\"), FlexItem::new().grow(1.0).basis(110.0))"
        );

        let column = FlexSettings {
            column: true,
            justify: 3,
            align: 3,
            gap: 4.0,
            wrap: true,
            modes: [TileMode::Capped; TILE_COUNT],
        };
        let code = flex_code(&column);
        assert!(code.starts_with("Flex::vertical()\n    .justify(FlexJustify::SpaceBetween)"));
        assert!(code.contains(".align_items(Alignment::Stretch)"));
        assert!(code.contains(".wrap(FlexWrap::Wrap)"));
        assert!(code.contains("FlexItem::new().grow(1.0).basis(36.0).max_height(64.0)"));
    }

    #[test]
    fn tiles_cycle_through_every_mode() {
        let mut mode = TileMode::Fixed;
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(mode);
            mode = mode.next();
        }
        assert_eq!(seen, [TileMode::Fixed, TileMode::Grow, TileMode::Capped]);
        assert_eq!(mode, TileMode::Fixed);
    }
}
