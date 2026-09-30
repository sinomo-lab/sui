//! Widgets the board is drawn with: cards that move with the keyboard, the
//! slot above each card that shows where a dragged card will land, and
//! drop zones that show whether they accept what is over them.

use std::{cell::Cell, rc::Rc};

use sui::{
    DropHover, EventPhase, KeyState, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsNode, SemanticsRole, SemanticsValue, Signal, WakeEvent, WidgetPodMutVisitor,
    WidgetPodVisitor, prelude::*,
};

use super::board::{Card, Column, Nudge};
use crate::app::DevThemeReader;

/// The gap above a card, where a card dropped on it lands.
pub(super) const CARD_GAP: f32 = 10.0;

/// What a drag over a card would do to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum CardHint {
    #[default]
    None,
    /// A dragged card would land just above this one.
    InsertBefore,
    /// A shelf asset would be attached.
    Attach,
    /// A text snippet would be added to the note.
    Note,
}

type NudgeHandler = Rc<dyn Fn(&mut EventCtx, u32, Nudge)>;
type DeleteHandler = Rc<dyn Fn(&mut EventCtx, u32)>;

/// Holds a card below a gap, and draws a bar in the gap while a dragged
/// card would land there.
pub(super) struct CardSlot {
    theme_reader: DevThemeReader,
    hint: Signal<CardHint>,
    child: SingleChild,
}

impl CardSlot {
    pub(super) fn new<W>(theme_reader: &DevThemeReader, hint: &Signal<CardHint>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            hint: hint.clone(),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for CardSlot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let inner = Constraints::new(
            Size::new(
                constraints.min.width,
                (constraints.min.height - CARD_GAP).max(0.0),
            ),
            Size::new(
                constraints.max.width,
                (constraints.max.height - CARD_GAP).max(0.0),
            ),
        );
        let child = self.child.measure(ctx, inner);
        constraints.clamp(Size::new(child.width, child.height + CARD_GAP))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.y() + CARD_GAP,
                bounds.width(),
                (bounds.height() - CARD_GAP).max(0.0),
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if ctx.observe(&self.hint) == CardHint::InsertBefore {
            let theme = (self.theme_reader)();
            let bounds = ctx.bounds();
            ctx.fill(
                Path::rounded_rect(
                    Rect::new(
                        bounds.x(),
                        bounds.y() + (CARD_GAP - 3.0) * 0.5,
                        bounds.width(),
                        3.0,
                    ),
                    1.5,
                ),
                theme.palette.accent,
            );
        }
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

/// A card: its surface, its focus ring, and the keys that move it. Alt with
/// an arrow moves it a step, and Delete puts it in the trash.
pub(super) struct CardFrame {
    theme_reader: DevThemeReader,
    card: Signal<Card>,
    /// The column this card is built in.
    column: Column,
    hint: Signal<CardHint>,
    /// The card that should take focus once it is laid out in a column,
    /// after a move built it there. The card leaving the old column stays
    /// out of it.
    focus_request: Rc<Cell<Option<(u32, Column)>>>,
    on_nudge: NudgeHandler,
    on_delete: DeleteHandler,
    child: SingleChild,
}

impl CardFrame {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new<W>(
        theme_reader: &DevThemeReader,
        card: &Signal<Card>,
        column: Column,
        hint: &Signal<CardHint>,
        focus_request: &Rc<Cell<Option<(u32, Column)>>>,
        on_nudge: NudgeHandler,
        on_delete: DeleteHandler,
        child: W,
    ) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            card: card.clone(),
            column,
            hint: hint.clone(),
            focus_request: Rc::clone(focus_request),
            on_nudge,
            on_delete,
            child: SingleChild::new(child),
        }
    }

    fn id(&self) -> u32 {
        self.card.get().id
    }

    fn wants_focus(&self) -> bool {
        self.focus_request.get() == Some((self.id(), self.column))
    }
}

impl Widget for CardFrame {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Wake(WakeEvent::AnimationFrame { .. }) if self.wants_focus() => {
                self.focus_request.set(None);
                ctx.request_focus();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != EventPhase::Capture =>
            {
                // Pressing a card focuses it, so its keys work next.
                ctx.request_focus();
            }
            Event::Keyboard(key)
                if ctx.is_focused() && key.state == KeyState::Pressed && !key.is_composing =>
            {
                let modifiers = key.modifiers;
                let only_alt =
                    modifiers.alt && !modifiers.control && !modifiers.shift && !modifiers.meta;
                let nudge = match key.key.as_str() {
                    "ArrowUp" if only_alt => Some(Nudge::Up),
                    "ArrowDown" if only_alt => Some(Nudge::Down),
                    "ArrowLeft" if only_alt => Some(Nudge::Left),
                    "ArrowRight" if only_alt => Some(Nudge::Right),
                    _ => None,
                };
                if let Some(nudge) = nudge {
                    (self.on_nudge)(ctx, self.id(), nudge);
                    ctx.set_handled();
                } else if key.key == "Delete" && !modifiers.any() {
                    (self.on_delete)(ctx, self.id());
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.wants_focus() {
            ctx.request_animation_frame();
        }
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let hint = ctx.observe(&self.hint);
        let bounds = ctx.bounds();
        let shape = Path::rounded_rect(bounds, 10.0);
        let highlighted = matches!(hint, CardHint::Attach | CardHint::Note);
        ctx.fill(
            shape,
            if highlighted {
                theme.palette.accent_soft
            } else {
                theme.palette.surface_raised
            },
        );
        let (border, width) = if highlighted {
            (theme.palette.accent, 2.0)
        } else if ctx.is_focused() {
            (theme.palette.focus_ring, 2.0)
        } else {
            (theme.palette.border, 1.0)
        };
        ctx.stroke(
            Path::rounded_rect(
                bounds.inflate(-width * 0.5, -width * 0.5),
                10.0 - width * 0.5,
            ),
            border,
            StrokeStyle::new(width),
        );
        self.child.paint(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let card = self.card.get();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ListItem, ctx.bounds());
        node.name = Some(card.title.clone());
        node.value = (!card.note.is_empty()).then(|| SemanticsValue::Text(card.note.clone()));
        node.description =
            Some("Alt and an arrow key move the card; Delete puts it in the trash".to_string());
        node.actions = vec![SemanticsAction::Focus];
        node.state.focused = ctx.is_focused();
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

/// A drop zone's panel, tinted by whether it accepts or refuses the drag
/// over it.
pub(super) struct ZoneFrame {
    theme_reader: DevThemeReader,
    name: String,
    hover: Signal<DropHover>,
    min_height: f32,
    child: SingleChild,
}

impl ZoneFrame {
    pub(super) fn new<W>(
        theme_reader: &DevThemeReader,
        name: impl Into<String>,
        hover: &Signal<DropHover>,
        child: W,
    ) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            name: name.into(),
            hover: hover.clone(),
            min_height: 0.0,
            child: SingleChild::new(child),
        }
    }

    pub(super) fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }
}

impl Widget for ZoneFrame {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child = self.child.measure(ctx, constraints);
        constraints.clamp(Size::new(child.width, child.height.max(self.min_height)))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let hover = ctx.observe(&self.hover);
        let bounds = ctx.bounds();
        let (fill, border, width) = match hover {
            DropHover::Idle => (theme.palette.control, theme.palette.border, 1.0),
            DropHover::Accepting(_) => (theme.palette.accent_soft, theme.palette.accent, 2.0),
            DropHover::Refusing => (
                theme.palette.danger.with_alpha(0.12),
                theme.palette.danger,
                2.0,
            ),
        };
        ctx.fill(Path::rounded_rect(bounds, 10.0), fill);
        ctx.stroke(
            Path::rounded_rect(
                bounds.inflate(-width * 0.5, -width * 0.5),
                10.0 - width * 0.5,
            ),
            border,
            StrokeStyle::new(width),
        );
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(
            match self.hover.get() {
                DropHover::Idle => "Idle",
                DropHover::Accepting(_) => "Accepting",
                DropHover::Refusing => "Refusing",
            }
            .to_string(),
        ));
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
