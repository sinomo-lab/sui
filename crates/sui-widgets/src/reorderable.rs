use sui_core::{
    DragDropScope, DragEventKind, DragOutcome, DragPayload, DragSessionId, DropEffect, Event, Path,
    Point, PointerButton, PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole,
    Size, Transform, Vector, WakeEvent,
};
use sui_layout::Constraints;
use sui_runtime::{
    ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget, WidgetChildren,
    WidgetPodMutVisitor, WidgetPodVisitor,
};

use crate::{DefaultTheme, Easing, Transition};

const DEFAULT_DRAG_THRESHOLD: f32 = 4.0;
const REORDERABLE_LIST_PAYLOAD_KIND: &str = "sui-widgets.reorderable-list";

type ReorderCallback = Box<dyn FnMut(&mut EventCtx, ReorderableListChange)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReorderableListChange {
    pub item: usize,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReorderPayload {
    list: u64,
    item: usize,
}

#[derive(Debug, Clone, Copy)]
struct ReorderPress {
    pointer_id: u64,
    start_position: Point,
    item: usize,
    from: usize,
    drag_offset_y: f32,
}

#[derive(Debug, Clone, Copy)]
struct ActiveReorderDrag {
    pointer_id: u64,
    session_id: DragSessionId,
    item: usize,
    from: usize,
    target: usize,
    drag_offset_y: f32,
    position: Point,
}

#[derive(Debug, Clone, Copy)]
struct RowMotion {
    y: f32,
    target_y: f32,
    transition: Option<Transition<f32>>,
}

impl RowMotion {
    fn new(y: f32) -> Self {
        Self {
            y,
            target_y: y,
            transition: None,
        }
    }

    fn current_at(self, time: f64) -> f32 {
        self.transition
            .map(|transition| transition.sample(time))
            .unwrap_or(self.y)
    }

    fn jump_to(&mut self, y: f32) {
        self.y = y;
        self.target_y = y;
        self.transition = None;
    }

    fn set_target(&mut self, target_y: f32, time: f64, duration: f64, easing: Easing) -> bool {
        let current = self.current_at(time);
        if (current - target_y).abs() <= 0.5 {
            self.jump_to(target_y);
            return false;
        }

        self.y = current;
        self.target_y = target_y;
        self.transition = Some(Transition::new(current, target_y, time, duration, easing));
        true
    }

    fn advance(&mut self, time: f64) -> bool {
        let Some(transition) = self.transition else {
            return false;
        };

        self.y = transition.sample(time);
        if transition.is_complete(time) {
            self.jump_to(self.target_y);
            false
        } else {
            true
        }
    }

    fn is_animating(&self) -> bool {
        self.transition.is_some()
    }
}

pub struct ReorderableList {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    scope: DragDropScope,
    name: String,
    children: WidgetChildren,
    order: Vec<usize>,
    spacing: f32,
    drag_threshold: f32,
    animation_duration: Option<f64>,
    animation_easing: Option<Easing>,
    preview_label: Option<String>,
    press: Option<ReorderPress>,
    active_drag: Option<ActiveReorderDrag>,
    row_sizes: Vec<Size>,
    row_offsets: Vec<f32>,
    row_bounds: Vec<Rect>,
    row_motions: Vec<RowMotion>,
    content_y: f32,
    content_height: f32,
    on_reorder: Option<ReorderCallback>,
}

impl ReorderableList {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            scope: DragDropScope::new(),
            name: name.into(),
            children: WidgetChildren::new(),
            order: Vec::new(),
            spacing: 8.0,
            drag_threshold: DEFAULT_DRAG_THRESHOLD,
            animation_duration: None,
            animation_easing: None,
            preview_label: None,
            press: None,
            active_drag: None,
            row_sizes: Vec::new(),
            row_offsets: Vec::new(),
            row_bounds: Vec::new(),
            row_motions: Vec::new(),
            content_y: 0.0,
            content_height: 0.0,
            on_reorder: None,
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

    pub fn scope(mut self, scope: DragDropScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn drag_threshold(mut self, threshold: f32) -> Self {
        self.drag_threshold = threshold.max(0.0);
        self
    }

    pub fn animation_duration(mut self, duration: f64) -> Self {
        self.animation_duration = Some(duration.max(0.0));
        self
    }

    pub fn animation_easing(mut self, easing: Easing) -> Self {
        self.animation_easing = Some(easing);
        self
    }

    pub fn preview_label(mut self, label: impl Into<String>) -> Self {
        self.preview_label = Some(label.into());
        self
    }

    pub fn item<W>(mut self, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.children.push(child);
        self
    }

    pub fn on_reorder<F>(mut self, callback: F) -> Self
    where
        F: FnMut(ReorderableListChange) + 'static,
    {
        let mut callback = callback;
        self.on_reorder = Some(Box::new(move |_, change| callback(change)));
        self
    }

    pub fn on_reorder_with_ctx<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, ReorderableListChange) + 'static,
    {
        self.on_reorder = Some(Box::new(callback));
        self
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn scope_ref(&self) -> &DragDropScope {
        &self.scope
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(*self.theme)
    }

    fn sync_storage(&mut self) {
        let len = self.children.len();
        if self.order.len() != len {
            self.order = (0..len).collect();
        }
        self.row_sizes.resize(len, Size::ZERO);
        self.row_offsets.resize(len, 0.0);
        self.row_bounds.resize(len, Rect::ZERO);
        if self.row_motions.len() != len {
            self.row_motions = (0..len).map(|_| RowMotion::new(0.0)).collect();
        }
        if self
            .active_drag
            .is_some_and(|drag| drag.item >= len || drag.from >= len || drag.target >= len)
        {
            self.active_drag = None;
            self.press = None;
        }
    }

    fn compute_offsets_for_order(&self, order: &[usize]) -> (Vec<f32>, f32) {
        let mut offsets = vec![0.0; self.children.len()];
        let mut y: f32 = 0.0;
        for (visual, item) in order.iter().copied().enumerate() {
            if visual > 0 {
                y += self.spacing;
            }
            offsets[item] = y;
            y += self
                .row_sizes
                .get(item)
                .copied()
                .unwrap_or(Size::ZERO)
                .height;
        }
        (offsets, y)
    }

    fn desired_order(&self) -> Vec<usize> {
        let Some(drag) = self.active_drag else {
            return self.order.clone();
        };

        let mut order = self.order.clone();
        let Some(from) = order.iter().position(|item| *item == drag.item) else {
            return order;
        };
        let item = order.remove(from);
        let target = drag.target.min(order.len());
        order.insert(target, item);
        order
    }

    fn visual_index_of(&self, item: usize) -> Option<usize> {
        self.order.iter().position(|candidate| *candidate == item)
    }

    fn insertion_index_at(&self, position: Point) -> usize {
        let local_y = position.y - self.content_y;
        let mut y = 0.0;
        for visual in 0..self.order.len() {
            if visual > 0 {
                y += self.spacing;
            }
            let item = self.order[visual];
            let height = self
                .row_sizes
                .get(item)
                .copied()
                .unwrap_or(Size::ZERO)
                .height;
            if local_y < y + (height * 0.5) {
                return visual;
            }
            y += height;
        }
        self.order.len()
    }

    fn target_index_at(&self, item: usize, position: Point) -> usize {
        if self.order.is_empty() {
            return 0;
        }

        let insertion = self.insertion_index_at(position);
        let from = self.visual_index_of(item).unwrap_or(0);
        let target = if insertion > from {
            insertion.saturating_sub(1)
        } else {
            insertion
        };
        target.min(self.order.len().saturating_sub(1))
    }

    fn press_at(&self, pointer_id: u64, position: Point) -> Option<ReorderPress> {
        for (visual, item) in self.order.iter().copied().enumerate() {
            let rect = self.row_bounds.get(item).copied().unwrap_or(Rect::ZERO);
            if rect.contains(position) {
                return Some(ReorderPress {
                    pointer_id,
                    start_position: position,
                    item,
                    from: visual,
                    drag_offset_y: position.y - rect.y(),
                });
            }
        }
        None
    }

    fn start_drag(&mut self, ctx: &mut EventCtx, press: ReorderPress, position: Point) {
        let session_id = ctx.begin_drag(
            self.scope.id(),
            press.pointer_id,
            press.start_position,
            DragPayload::custom(
                REORDERABLE_LIST_PAYLOAD_KIND,
                ReorderPayload {
                    list: ctx.widget_id().get(),
                    item: press.item,
                },
            ),
            DropEffect::Move,
            self.preview_label.clone(),
        );
        let target = self.target_index_at(press.item, position);
        self.active_drag = Some(ActiveReorderDrag {
            pointer_id: press.pointer_id,
            session_id,
            item: press.item,
            from: press.from,
            target,
            drag_offset_y: press.drag_offset_y,
            position,
        });
        if let Some(motion) = self.row_motions.get_mut(press.item) {
            motion.jump_to(position.y - press.drag_offset_y);
        }
        self.retarget_row_motions(ctx, true);
    }

    fn set_drag_target(&mut self, ctx: &mut EventCtx, target: usize, position: Point) {
        let Some(drag) = &mut self.active_drag else {
            return;
        };
        drag.position = position;
        if drag.target != target {
            drag.target = target;
            self.retarget_row_motions(ctx, true);
        } else {
            ctx.request_paint();
        }
    }

    fn retarget_row_motions(&mut self, ctx: &mut EventCtx, animate: bool) {
        let order = self.desired_order();
        let (offsets, _) = self.compute_offsets_for_order(&order);
        let base_y = self.content_y;
        let theme = self.resolved_theme();
        let duration = self
            .animation_duration
            .unwrap_or_else(|| theme.motion.duration_fast.into());
        let easing = self
            .animation_easing
            .unwrap_or(theme.motion.easing_standard);
        let mut animating = false;

        for item in 0..self.row_motions.len() {
            if self.active_drag.is_some_and(|drag| drag.item == item) {
                continue;
            }
            let target_y = base_y + offsets.get(item).copied().unwrap_or(0.0);
            let motion = &mut self.row_motions[item];
            if animate {
                animating |= motion.set_target(target_y, ctx.current_time(), duration, easing);
            } else {
                motion.jump_to(target_y);
            }
        }

        ctx.request_paint();
        if animating {
            ctx.request_animation_frame();
        }
    }

    fn advance_motions(&mut self, ctx: &mut EventCtx, time: f64) {
        let mut animating = false;
        for motion in &mut self.row_motions {
            animating |= motion.advance(time);
        }
        ctx.request_paint();
        if animating {
            ctx.request_animation_frame();
        }
    }

    fn reset_drag(&mut self, ctx: &mut EventCtx, animate: bool) {
        self.press = None;
        self.active_drag = None;
        self.retarget_row_motions(ctx, animate);
    }

    fn finish_reorder(&mut self, ctx: &mut EventCtx) {
        let Some(drag) = self.active_drag else {
            self.reset_drag(ctx, true);
            return;
        };
        let Some(from) = self.order.iter().position(|item| *item == drag.item) else {
            self.reset_drag(ctx, true);
            return;
        };
        let mut to = drag.target.min(self.order.len().saturating_sub(1));

        if let Some(motion) = self.row_motions.get_mut(drag.item) {
            motion.jump_to(drag.position.y - drag.drag_offset_y);
        }

        if from != to {
            let item = self.order.remove(from);
            if to > self.order.len() {
                to = self.order.len();
            }
            self.order.insert(to, item);
            if let Some(callback) = &mut self.on_reorder {
                callback(ctx, ReorderableListChange { item, from, to });
            }
            ctx.request_semantics();
        }

        self.reset_drag(ctx, true);
    }

    fn marker_y(&self) -> Option<f32> {
        let drag = self.active_drag?;
        let order = self.desired_order();
        let (offsets, _) = self.compute_offsets_for_order(&order);
        let base_y = self.content_y;
        Some(base_y + offsets.get(drag.item).copied().unwrap_or(0.0))
    }
}

impl Widget for ReorderableList {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != sui_runtime::EventPhase::Capture =>
            {
                if let Some(press) = self.press_at(pointer.pointer_id, pointer.position) {
                    self.press = Some(press);
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self
                        .press
                        .is_some_and(|press| press.pointer_id == pointer.pointer_id) =>
            {
                if let Some(drag) = self.active_drag {
                    if drag.pointer_id == pointer.pointer_id {
                        let target = self.target_index_at(drag.item, pointer.position);
                        self.set_drag_target(ctx, target, pointer.position);
                        ctx.set_handled();
                    }
                    return;
                }

                let press = self.press.unwrap();
                let delta = pointer.position - press.start_position;
                let distance_sq = (delta.x * delta.x) + (delta.y * delta.y);
                if distance_sq >= self.drag_threshold * self.drag_threshold {
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
                if self.active_drag.is_none() {
                    self.press = None;
                }
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Drag(drag) if drag.scope_id == self.scope.id() => {
                if drag.payload.custom_kind() != Some(REORDERABLE_LIST_PAYLOAD_KIND) {
                    return;
                }
                let Some(payload) = drag.payload.custom_data::<ReorderPayload>() else {
                    return;
                };
                if payload.list != ctx.widget_id().get() {
                    return;
                }

                match drag.kind {
                    DragEventKind::Enter | DragEventKind::Over => {
                        let target = self.target_index_at(payload.item, drag.position);
                        self.set_drag_target(ctx, target, drag.position);
                        ctx.accept_drop(DropEffect::Move);
                    }
                    DragEventKind::Leave => {
                        if self
                            .active_drag
                            .is_some_and(|active| active.session_id == drag.session_id)
                        {
                            let from = self.active_drag.map(|active| active.from).unwrap_or(0);
                            self.set_drag_target(ctx, from, drag.position);
                        }
                    }
                    DragEventKind::Drop if drag.target == Some(ctx.widget_id()) => {
                        self.finish_reorder(ctx);
                        ctx.set_handled();
                    }
                    DragEventKind::End => {
                        if matches!(drag.outcome, Some(DragOutcome::Cancelled)) {
                            self.reset_drag(ctx, true);
                        } else {
                            self.press = None;
                            self.active_drag = None;
                        }
                    }
                    _ => {}
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.advance_motions(ctx, *time);
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_storage();

        let child_constraints = Constraints::new(
            Size::ZERO,
            Size::new(constraints.max.width.max(0.0), f32::INFINITY),
        );
        let mut width: f32 = 0.0;
        for index in 0..self.children.len() {
            let size = self.children.measure_child(index, ctx, child_constraints);
            self.row_sizes[index] = size;
            width = width.max(size.width);
        }

        let (_, height) = self.compute_offsets_for_order(&self.order);
        self.content_height = height;
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.sync_storage();
        let (offsets, height) = self.compute_offsets_for_order(&self.order);
        self.content_y = bounds.y();
        self.row_offsets = offsets.clone();
        self.content_height = height;

        for item in 0..self.children.len() {
            let size = Size::new(bounds.width(), self.row_sizes[item].height);
            let rect = Rect::new(
                bounds.x(),
                bounds.y() + offsets[item],
                size.width,
                size.height,
            );
            self.row_bounds[item] = rect;
            self.children.arrange_child(item, ctx, rect);
            if !self.row_motions[item].is_animating() && self.active_drag.is_none() {
                self.row_motions[item].jump_to(rect.y());
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.push_clip_rect(bounds);

        let active_item = self.active_drag.map(|drag| drag.item);
        for item in self.desired_order() {
            if active_item == Some(item) {
                continue;
            }
            let rect = self.row_bounds.get(item).copied().unwrap_or(Rect::ZERO);
            let y = self
                .row_motions
                .get(item)
                .map(|motion| motion.y)
                .unwrap_or(rect.y());
            ctx.translate(Vector::new(0.0, y - rect.y()));
            self.children.as_slice()[item].paint(ctx);
            ctx.pop_transform();
        }

        if let Some(marker_y) = self.marker_y() {
            let theme = self.resolved_theme();
            let marker = Rect::new(
                bounds.x() + 4.0,
                (marker_y - 1.0).max(bounds.y()),
                (bounds.width() - 8.0).max(0.0),
                2.0,
            );
            ctx.fill(Path::rounded_rect(marker, 1.0), theme.palette.border_focus);
        }

        if let Some(drag) = self.active_drag {
            let item = drag.item;
            let rect = self.row_bounds.get(item).copied().unwrap_or(Rect::ZERO);
            let y = drag.position.y - drag.drag_offset_y;
            ctx.push_transform(Transform::translation(0.0, y - rect.y()));
            self.children.as_slice()[item].paint(ctx);
            ctx.pop_transform();
        }

        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        node.name = Some(self.name.clone());
        node.actions = vec![SemanticsAction::Focus];
        ctx.push(node);
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::SizedBox;
    use sui_core::{Modifiers, PointerButtons, PointerEvent, PointerKind, Result, WindowId};
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
            .window(WindowBuilder::new().title("Reorder").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    #[test]
    fn reorderable_list_reports_reorder_after_pointer_drag() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let list = ReorderableList::new("Tasks")
            .spacing(0.0)
            .item(SizedBox::new().width(120.0).height(30.0))
            .item(SizedBox::new().width(120.0).height(30.0))
            .item(SizedBox::new().width(120.0).height(30.0))
            .on_reorder({
                let changes = Rc::clone(&changes);
                move |change| changes.borrow_mut().push(change)
            });
        let (mut runtime, window_id) = build_runtime(list);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 15.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(10.0, 48.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(10.0, 78.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(10.0, 78.0), false),
        )?;

        assert_eq!(
            &*changes.borrow(),
            &[ReorderableListChange {
                item: 0,
                from: 0,
                to: 2
            }]
        );
        Ok(())
    }
}
