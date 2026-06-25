use std::sync::Arc;

use sui_core::{
    DragDropScope, DragEvent, DragEventKind, DragPayload, DragPreview, DragSessionId, DropEffect,
    Event, Point, PointerButton, PointerEventKind, Rect, Size, Vector,
};
use sui_layout::Constraints;
use sui_runtime::{
    ArrangeCtx, EventCtx, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx, SemanticsCtx,
    SingleChild, StackSurfaceOptions, Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{Border, LayerCompositionMode};

use crate::DefaultTheme;

const DEFAULT_DRAG_THRESHOLD: f32 = 4.0;

type PayloadFactory = Box<dyn FnMut() -> DragPayload>;
type DragStartCallback = Box<dyn FnMut(&mut EventCtx, &DragPreview)>;
type DragEndCallback = Box<dyn FnMut(&mut EventCtx, &DragEvent)>;
type DropAcceptCallback = Box<dyn FnMut(&DragEvent) -> DropEffect>;
type DropCallback = Box<dyn FnMut(&mut EventCtx, &DragEvent)>;
type HoverCallback = Box<dyn FnMut(bool)>;

pub struct DragDropHost {
    scope: DragDropScope,
    child: SingleChild,
    overlay: SingleChild,
}

impl DragDropHost {
    pub fn new<W>(scope: DragDropScope, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            overlay: SingleChild::new(DragPreviewOverlay::new(scope.clone())),
            scope,
            child: SingleChild::new(child),
        }
    }

    pub fn scope(&self) -> &DragDropScope {
        &self.scope
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }
}

impl Widget for DragDropHost {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = self.child.measure(ctx, constraints);
        self.overlay.measure(ctx, Constraints::tight(size));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
        self.overlay.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
        self.overlay.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
        self.overlay.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
        self.overlay.visit_children_mut(visitor);
    }
}

struct DragPreviewOverlay {
    scope: DragDropScope,
}

impl DragPreviewOverlay {
    fn new(scope: DragDropScope) -> Self {
        Self { scope }
    }
}

impl Widget for DragPreviewOverlay {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            constraints.max.width.max(0.0),
            constraints.max.height.max(0.0),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(active) = self.scope.active_drag() else {
            return;
        };

        let label = drag_preview_label(&active);
        let width = ((label.chars().count() as f32 * 7.0) + 24.0).clamp(48.0, 260.0);
        let height = 30.0;
        let offset = Vector::new(12.0, 12.0);
        let bounds = ctx.bounds();
        let x = (active.position.x + offset.x).min((bounds.max_x() - width).max(bounds.x()));
        let y = (active.position.y + offset.y).min((bounds.max_y() - height).max(bounds.y()));
        let rect = Rect::new(x.max(bounds.x()), y.max(bounds.y()), width, height);
        let theme = DefaultTheme::default();
        let mut text_style = theme.body_text_style();
        text_style.font_size = text_style.font_size.min(13.0);
        text_style.line_height = text_style.line_height.min(18.0);
        text_style.color = theme.palette.text;

        ctx.fill_rrect_bordered(
            rect,
            [7.0; 4],
            theme.palette.surface_raised.with_alpha(0.96),
            Border {
                width: 1.0,
                color: theme.palette.border.with_alpha(0.72),
            },
        );

        let text_rect = Rect::new(rect.x() + 12.0, rect.y() + 5.0, rect.width() - 24.0, 20.0);
        ctx.push_clip_rect(text_rect);
        ctx.draw_text(text_rect, label, text_style);
        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Overlay,
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        Some(StackSurfaceOptions {
            transient: true,
            hit_test: false,
            ..StackSurfaceOptions::default()
        })
    }
}

pub struct Draggable {
    scope: DragDropScope,
    child: SingleChild,
    payload: PayloadFactory,
    allowed_effect: DropEffect,
    preview_label: Option<String>,
    threshold: f32,
    press: Option<DragPress>,
    active_session: Option<DragSessionId>,
    on_drag_start: Option<DragStartCallback>,
    on_drag_end: Option<DragEndCallback>,
}

#[derive(Debug, Clone, Copy)]
struct DragPress {
    pointer_id: u64,
    start_position: Point,
}

impl Draggable {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            scope: DragDropScope::new(),
            child: SingleChild::new(child),
            payload: Box::new(|| DragPayload::text("")),
            allowed_effect: DropEffect::Move,
            preview_label: None,
            threshold: DEFAULT_DRAG_THRESHOLD,
            press: None,
            active_session: None,
            on_drag_start: None,
            on_drag_end: None,
        }
    }

    pub fn scope(mut self, scope: DragDropScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn payload<F>(mut self, payload: F) -> Self
    where
        F: FnMut() -> DragPayload + 'static,
    {
        self.payload = Box::new(payload);
        self
    }

    pub fn effect(mut self, effect: DropEffect) -> Self {
        self.allowed_effect = effect;
        self
    }

    pub fn preview_label(mut self, label: impl Into<String>) -> Self {
        self.preview_label = Some(label.into());
        self
    }

    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.max(0.0);
        self
    }

    pub fn on_drag_start<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, &DragPreview) + 'static,
    {
        self.on_drag_start = Some(Box::new(callback));
        self
    }

    pub fn on_drag_end<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, &DragEvent) + 'static,
    {
        self.on_drag_end = Some(Box::new(callback));
        self
    }

    pub fn scope_ref(&self) -> &DragDropScope {
        &self.scope
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }

    fn start_drag(&mut self, ctx: &mut EventCtx, press: DragPress, position: Point) {
        let payload = (self.payload)();
        let preview_label = self.preview_label.clone();
        let session_id = ctx.begin_drag(
            self.scope.id(),
            press.pointer_id,
            press.start_position,
            payload.clone(),
            self.allowed_effect,
            preview_label.clone(),
        );
        let preview = DragPreview {
            session_id,
            scope_id: self.scope.id(),
            pointer_id: press.pointer_id,
            source: ctx.widget_id(),
            position,
            start_position: press.start_position,
            payload,
            allowed_effect: self.allowed_effect,
            preview_label: preview_label.map(Arc::from),
        };
        self.scope.set_active_drag(preview.clone());
        self.active_session = Some(session_id);
        if let Some(callback) = &mut self.on_drag_start {
            callback(ctx, &preview);
        }
        ctx.request_paint();
    }
}

impl Widget for Draggable {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position)
                    && ctx.phase() != sui_runtime::EventPhase::Capture =>
            {
                self.press = Some(DragPress {
                    pointer_id: pointer.pointer_id,
                    start_position: pointer.position,
                });
                self.active_session = None;
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self
                        .press
                        .is_some_and(|press| press.pointer_id == pointer.pointer_id) =>
            {
                if let Some(session_id) = self.active_session {
                    self.scope
                        .update_drag_position(session_id, pointer.position);
                    ctx.request_paint();
                    ctx.set_handled();
                    return;
                }

                let press = self.press.unwrap();
                let delta = pointer.position - press.start_position;
                let distance_sq = (delta.x * delta.x) + (delta.y * delta.y);
                if distance_sq >= self.threshold * self.threshold {
                    self.start_drag(ctx, press, pointer.position);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Up | PointerEventKind::Cancel
                ) && self
                    .press
                    .is_some_and(|press| press.pointer_id == pointer.pointer_id) =>
            {
                if self.active_session.is_none() {
                    self.press = None;
                }
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Drag(drag)
                if drag.kind == DragEventKind::End
                    && self.active_session == Some(drag.session_id) =>
            {
                self.scope.finish_drag(drag.session_id);
                self.press = None;
                self.active_session = None;
                if let Some(callback) = &mut self.on_drag_end {
                    callback(ctx, drag);
                }
                ctx.request_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
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

pub struct DropTarget {
    scope: DragDropScope,
    child: SingleChild,
    accept: DropAcceptCallback,
    hovered: bool,
    on_drop: Option<DropCallback>,
    on_hover_change: Option<HoverCallback>,
}

impl DropTarget {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            scope: DragDropScope::new(),
            child: SingleChild::new(child),
            accept: Box::new(|_| DropEffect::Copy),
            hovered: false,
            on_drop: None,
            on_hover_change: None,
        }
    }

    pub fn scope(mut self, scope: DragDropScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn accept<F>(mut self, accept: F) -> Self
    where
        F: FnMut(&DragEvent) -> DropEffect + 'static,
    {
        self.accept = Box::new(accept);
        self
    }

    pub fn on_drop<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, &DragEvent) + 'static,
    {
        self.on_drop = Some(Box::new(callback));
        self
    }

    pub fn on_hover_change<F>(mut self, callback: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_hover_change = Some(Box::new(callback));
        self
    }

    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    pub fn scope_ref(&self) -> &DragDropScope {
        &self.scope
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }

    fn set_hovered(&mut self, ctx: &mut EventCtx, hovered: bool) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        if let Some(callback) = &mut self.on_hover_change {
            callback(hovered);
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

impl Widget for DropTarget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Drag(drag) = event else {
            return;
        };
        if drag.scope_id != self.scope.id() {
            return;
        }

        match drag.kind {
            DragEventKind::Enter | DragEventKind::Over => {
                let effect = (self.accept)(drag);
                if effect.is_some() {
                    ctx.accept_drop(effect);
                    self.set_hovered(ctx, true);
                } else {
                    self.set_hovered(ctx, false);
                }
            }
            DragEventKind::Leave => {
                self.set_hovered(ctx, false);
            }
            DragEventKind::Drop if drag.target == Some(ctx.widget_id()) => {
                if let Some(callback) = &mut self.on_drop {
                    callback(ctx, drag);
                }
                self.set_hovered(ctx, false);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
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

fn drag_preview_label(active: &DragPreview) -> String {
    if let Some(label) = &active.preview_label {
        return label.to_string();
    }

    match &active.payload {
        DragPayload::Text(text) if !text.is_empty() => text.clone(),
        DragPayload::Image { .. } => "Image".to_string(),
        DragPayload::Custom { kind, .. } => kind.to_string(),
        DragPayload::Text(_) => "Dragging".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{SizedBox, Stack};
    use sui_core::{
        DragOutcome, Modifiers, PointerButtons, PointerEvent, PointerKind, Result, WindowId,
    };
    use sui_runtime::{Application, Runtime, WindowBuilder};

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut buttons = PointerButtons::NONE;
        if pressed {
            buttons.insert(PointerButton::Primary);
        }

        Event::Pointer(PointerEvent {
            pointer_id: 1,
            kind,
            position,
            delta: Vector::ZERO,
            scroll_delta: None,
            button: Some(PointerButton::Primary),
            buttons,
            modifiers: Modifiers::NONE,
            pointer_kind: PointerKind::Mouse,
            is_primary: true,
        })
    }

    fn build_runtime<W>(root: W) -> (Runtime, WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Drag Drop").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    #[test]
    fn draggable_starts_after_threshold_and_drops_typed_payload() -> Result<()> {
        let scope = DragDropScope::new();
        let starts = Rc::new(RefCell::new(0));
        let ends = Rc::new(RefCell::new(Vec::new()));
        let drops = Rc::new(RefCell::new(Vec::new()));
        let hover_changes = Rc::new(RefCell::new(Vec::new()));

        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::custom("asset", 42_u32))
            .effect(DropEffect::Move)
            .preview_label("Asset")
            .on_drag_start({
                let starts = Rc::clone(&starts);
                move |_, _| *starts.borrow_mut() += 1
            })
            .on_drag_end({
                let ends = Rc::clone(&ends);
                move |_, drag| ends.borrow_mut().push(drag.outcome)
            });

        let target = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .accept(|drag| {
                if drag.payload.custom_data::<u32>() == Some(&42) {
                    DropEffect::Move
                } else {
                    DropEffect::None
                }
            })
            .on_drop({
                let drops = Rc::clone(&drops);
                move |_, drag| {
                    drops
                        .borrow_mut()
                        .push(*drag.payload.custom_data::<u32>().unwrap());
                }
            })
            .on_hover_change({
                let hover_changes = Rc::clone(&hover_changes);
                move |hovered| hover_changes.borrow_mut().push(hovered)
            });

        let root = DragDropHost::new(
            scope.clone(),
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 20.0), true),
        )?;

        assert_eq!(*starts.borrow(), 0);
        assert!(scope.active_drag().is_none());

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;
        assert_eq!(*starts.borrow(), 1);
        assert!(scope.active_drag().is_some());

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
        )?;

        assert_eq!(&*drops.borrow(), &[42]);
        assert!(matches!(
            ends.borrow().as_slice(),
            [Some(DragOutcome::Dropped {
                effect: DropEffect::Move,
                ..
            })]
        ));
        assert_eq!(&*hover_changes.borrow(), &[true, false]);
        assert!(scope.active_drag().is_none());
        Ok(())
    }

    #[test]
    fn nearest_accepting_drop_target_receives_drop() -> Result<()> {
        let scope = DragDropScope::new();
        let inner_drops = Rc::new(RefCell::new(0));
        let outer_drops = Rc::new(RefCell::new(0));

        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("item"));

        let inner = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .accept(|_| DropEffect::Copy)
            .on_drop({
                let inner_drops = Rc::clone(&inner_drops);
                move |_, _| *inner_drops.borrow_mut() += 1
            });
        let outer = DropTarget::new(inner)
            .scope(scope.clone())
            .accept(|_| DropEffect::Copy)
            .on_drop({
                let outer_drops = Rc::clone(&outer_drops);
                move |_, _| *outer_drops.borrow_mut() += 1
            });

        let root = DragDropHost::new(
            scope,
            Stack::horizontal().with_child(source).with_child(outer),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
        )?;

        assert_eq!(*inner_drops.borrow(), 1);
        assert_eq!(*outer_drops.borrow(), 0);
        Ok(())
    }

    #[test]
    fn cross_scope_target_ignores_drag_session() -> Result<()> {
        let source_scope = DragDropScope::new();
        let target_scope = DragDropScope::new();
        let ends = Rc::new(RefCell::new(Vec::new()));
        let drops = Rc::new(RefCell::new(0));

        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(source_scope.clone())
            .payload(|| DragPayload::text("item"))
            .on_drag_end({
                let ends = Rc::clone(&ends);
                move |_, drag| ends.borrow_mut().push(drag.outcome)
            });
        let target = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(target_scope)
            .accept(|_| DropEffect::Copy)
            .on_drop({
                let drops = Rc::clone(&drops);
                move |_, _| *drops.borrow_mut() += 1
            });

        let root = DragDropHost::new(
            source_scope,
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
        )?;

        assert_eq!(*drops.borrow(), 0);
        assert_eq!(&*ends.borrow(), &[Some(DragOutcome::Cancelled)]);
        Ok(())
    }

    #[test]
    fn drag_drop_host_preview_layer_is_not_hit_tested() -> Result<()> {
        let scope = DragDropScope::new();
        let starts = Rc::new(RefCell::new(0));
        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("item"))
            .on_drag_start({
                let starts = Rc::clone(&starts);
                move |_, _| *starts.borrow_mut() += 1
            });
        let root = DragDropHost::new(scope, source);
        let (mut runtime, window_id) = build_runtime(root);

        let output = runtime.render(window_id)?;
        let mut preview_layer = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
                preview_layer = Some((layer.widget_id(), layer.descriptor.hit_test));
            }
        });

        assert!(matches!(preview_layer, Some((_, false))));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(24.0, 20.0), true),
        )?;

        assert_eq!(*starts.borrow(), 1);
        Ok(())
    }

    #[test]
    fn drag_payload_custom_equality_uses_stable_kind() {
        let left = DragPayload::custom("asset", 1_u32);
        let right = DragPayload::custom("asset", 2_u32);
        let other = DragPayload::custom("other", 1_u32);

        assert_eq!(left, right);
        assert_ne!(left, other);
        assert_eq!(left.custom_data::<u32>(), Some(&1));
        assert_eq!(right.custom_data::<u32>(), Some(&2));
    }
}
