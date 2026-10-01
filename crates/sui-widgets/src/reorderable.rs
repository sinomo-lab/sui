use std::sync::atomic::{AtomicU64, Ordering};

use sui_core::{
    DragDropScope, DragEventKind, DragOutcome, DragPayload, DragSessionId, DropEffect, Event,
    KeyState, KeyboardEvent, Path, Point, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsLiveRegion, SemanticsNode, SemanticsRole, SemanticsValue, Size, Transform, Vector,
    WidgetId,
};
use sui_layout::Constraints;
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, FrameClock, MeasureCtx, Motion, PaintCtx, SemanticsCtx,
    Widget, WidgetChildren, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};

use crate::frame::stroke_border;
use crate::{AnimationSpec, DefaultTheme, Easing};

const DEFAULT_DRAG_THRESHOLD: f32 = 4.0;
const REORDERABLE_LIST_PAYLOAD_KIND: &str = "sui-widgets.reorderable-list";
/// Tags the ids of the status nodes that announce keyboard moves.
const SYNTHETIC_REORDER_STATUS_TAG: u64 = 7_u64 << 60;

static NEXT_REORDER_STATUS_ID: AtomicU64 = AtomicU64::new(1);

type ReorderCallback = Box<dyn FnMut(&mut EventCtx, ReorderableListChange)>;
type ItemName = Box<dyn Fn(usize) -> String>;

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

/// Where a row is presented while rows slide into a new order. The runtime
/// animates it; rows sliding into place are movement, which reduced motion
/// skips.
#[derive(Debug, Clone, Copy)]
struct RowMotion {
    y: Motion<f32>,
}

impl RowMotion {
    fn new(y: f32) -> Self {
        Self {
            y: Motion::new(y).movement(),
        }
    }

    fn at(&self, clock: &impl FrameClock) -> f32 {
        self.y.get(clock)
    }

    fn jump_to(&mut self, y: f32) {
        self.y.jump_to(y);
    }

    fn set_target(&mut self, target_y: f32, duration: f64, easing: Easing, ctx: &mut EventCtx) {
        if (self.y.get(ctx) - target_y).abs() <= 0.5 && !self.y.is_animating(ctx) {
            self.jump_to(target_y);
            return;
        }
        ctx.animate(
            &mut self.y,
            target_y,
            AnimationSpec::tween(duration, easing),
        );
    }

    fn is_animating(&self, clock: &impl FrameClock) -> bool {
        self.y.is_animating(clock)
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
    item_name: Option<ItemName>,
    /// The row, by position, that the keyboard acts on while the list has
    /// focus.
    current: usize,
    /// What the last keyboard move did, for assistive technology.
    announcement: Option<String>,
    status_id: WidgetId,
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
            item_name: None,
            current: 0,
            announcement: None,
            status_id: WidgetId::new(
                SYNTHETIC_REORDER_STATUS_TAG
                    | NEXT_REORDER_STATUS_ID.fetch_add(1, Ordering::Relaxed),
            ),
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

    pub fn gap(mut self, gap: f32) -> Self {
        self.spacing = gap.max(0.0);
        self
    }

    #[deprecated(note = "use `gap`")]
    pub fn spacing(self, spacing: f32) -> Self {
        self.gap(spacing)
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

    /// Names item `index` (in the order items were added) when announcing a
    /// keyboard move, such as "Moved Draft to position 2 of 4". Items are
    /// "Item 1", "Item 2"… otherwise.
    pub fn item_name<F>(mut self, name: F) -> Self
    where
        F: Fn(usize) -> String + 'static,
    {
        self.item_name = Some(Box::new(name));
        self
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    fn name_of(&self, item: usize) -> String {
        self.item_name
            .as_ref()
            .map_or_else(|| format!("Item {}", item + 1), |name| name(item))
    }

    /// The row, by position, whose widget holds `focused`.
    fn row_holding(&self, focused: WidgetId) -> Option<usize> {
        let item = self
            .children
            .as_slice()
            .iter()
            .position(|row| pod_contains(row, focused))?;
        self.visual_index_of(item)
    }

    /// Keys that move rows: Alt+Up and Alt+Down move the row with focus one
    /// place, Alt+Home and Alt+End to either end. While the list itself has
    /// focus, Up, Down, Home and End choose the row the keys act on.
    fn handle_key(&mut self, ctx: &mut EventCtx, key: &KeyboardEvent) {
        if key.state != KeyState::Pressed
            || key.is_composing
            || ctx.phase() == EventPhase::Capture
            || self.order.is_empty()
            || self.active_drag.is_some()
        {
            return;
        }
        let list_focused = ctx.is_focused();
        let row = if list_focused {
            self.current.min(self.order.len() - 1)
        } else if let Some(row) = ctx
            .focused_widget_id()
            .and_then(|focused| self.row_holding(focused))
        {
            row
        } else {
            return;
        };
        let last = self.order.len() - 1;
        let modifiers = key.modifiers;
        let plain = !modifiers.any();
        let only_alt = modifiers.alt && !modifiers.control && !modifiers.shift && !modifiers.meta;
        let destination = |key: &str| match key {
            "ArrowUp" => Some(row.saturating_sub(1)),
            "ArrowDown" => Some((row + 1).min(last)),
            "Home" => Some(0),
            "End" => Some(last),
            _ => None,
        };
        let Some(to) = destination(key.key.as_str()) else {
            return;
        };
        if only_alt {
            self.move_row(ctx, row, to);
            ctx.set_handled();
        } else if plain && list_focused {
            if to != self.current {
                self.current = to;
                ctx.request_paint();
                ctx.request_semantics();
            }
            ctx.set_handled();
        }
    }

    /// Move the row at position `from` to position `to`, as a keyboard move.
    fn move_row(&mut self, ctx: &mut EventCtx, from: usize, to: usize) {
        self.current = to;
        let item = self.order[from];
        let count = self.order.len();
        self.announcement = Some(if from == to {
            format!(
                "{} is already at position {} of {count}",
                self.name_of(item),
                to + 1
            )
        } else {
            format!(
                "Moved {} to position {} of {count}",
                self.name_of(item),
                to + 1
            )
        });
        ctx.request_semantics();
        ctx.request_paint();
        if from == to {
            return;
        }
        self.order.remove(from);
        self.order.insert(to, item);
        // Rows slide from where they were drawn to their new places.
        self.retarget_row_motions(ctx, true);
        ctx.request_arrange();
        if let Some(callback) = &mut self.on_reorder {
            callback(ctx, ReorderableListChange { item, from, to });
        }
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
        for item in 0..self.row_motions.len() {
            if self.active_drag.is_some_and(|drag| drag.item == item) {
                continue;
            }
            let target_y = base_y + offsets.get(item).copied().unwrap_or(0.0);
            let motion = &mut self.row_motions[item];
            if animate {
                motion.set_target(target_y, duration, easing, ctx);
            } else {
                motion.jump_to(target_y);
            }
        }

        ctx.request_paint();
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
            self.current = to;
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
            Event::Keyboard(key) => self.handle_key(ctx, key),
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != sui_runtime::EventPhase::Capture =>
            {
                if let Some(press) = self.press_at(pointer.pointer_id, pointer.position) {
                    // Keys act on the row last pressed.
                    self.current = press.from;
                    self.press = Some(press);
                    ctx.request_focus();
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

        for (item, &offset) in offsets.iter().enumerate().take(self.children.len()) {
            let size = Size::new(bounds.width(), self.row_sizes[item].height);
            let rect = Rect::new(bounds.x(), bounds.y() + offset, size.width, size.height);
            self.row_bounds[item] = rect;
            self.children.arrange_child(item, ctx, rect);
            if !self.row_motions[item].is_animating(ctx) && self.active_drag.is_none() {
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
                .map(|motion| motion.at(ctx))
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
        } else if ctx.is_focused()
            && let Some(item) = self.order.get(self.current).copied()
        {
            // The row the keyboard acts on, drawn inside the list's clip.
            let theme = self.resolved_theme();
            let width = ctx
                .dpi()
                .physical_pixels_to_logical(theme.metrics.focus_ring_width.max(1.0));
            let rect = self.row_bounds.get(item).copied().unwrap_or(Rect::ZERO);
            let y = self
                .row_motions
                .get(item)
                .map_or(rect.y(), |motion| motion.at(ctx));
            stroke_border(
                ctx,
                Rect::new(rect.x(), y, rect.width(), rect.height()),
                theme.metrics.corner_radius,
                width,
                theme.palette.focus_ring,
            );
        }

        ctx.pop_clip();
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = Some(
            "Alt+Up and Alt+Down move the focused row; Alt+Home and Alt+End move it to either end"
                .to_string(),
        );
        if let Some(item) = self.order.get(self.current).copied() {
            node.value = Some(SemanticsValue::Text(format!(
                "{}, position {} of {}",
                self.name_of(item),
                self.current + 1,
                self.order.len()
            )));
        }
        node.actions = vec![SemanticsAction::Focus];
        ctx.push(node);
        if let Some(announcement) = &self.announcement {
            let mut status = SemanticsNode::new(self.status_id, SemanticsRole::Status, Rect::ZERO);
            status.parent = Some(ctx.widget_id());
            status.name = Some(announcement.clone());
            status.live_region = Some(SemanticsLiveRegion::Polite);
            ctx.push(status);
        }
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

/// Whether `target` is `root` or one of its descendants.
fn pod_contains(root: &WidgetPod, target: WidgetId) -> bool {
    struct Finder {
        target: WidgetId,
        found: bool,
    }

    impl WidgetPodVisitor for Finder {
        fn visit(&mut self, child: &WidgetPod) {
            if self.found {
                return;
            }
            if child.id() == self.target {
                self.found = true;
            } else {
                child.visit_children(self);
            }
        }
    }

    if root.id() == target {
        return true;
    }
    let mut finder = Finder {
        target,
        found: false,
    };
    root.visit_children(&mut finder);
    finder.found
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{Button, SizedBox, Stack};
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
            .gap(0.0)
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

    fn key(name: &str, alt: bool) -> Event {
        let mut event = KeyboardEvent::new(name, KeyState::Pressed);
        event.modifiers.alt = alt;
        Event::Keyboard(event)
    }

    const NAMES: [&str; 3] = ["Draft", "Review", "Ship"];

    fn status(runtime: &Runtime, window_id: WindowId) -> Option<String> {
        runtime
            .semantics(window_id)
            .ok()?
            .iter()
            .find(|node| node.role == SemanticsRole::Status)
            .and_then(|node| node.name.clone())
    }

    #[test]
    fn alt_arrows_move_the_row_the_keyboard_is_on() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let list = ReorderableList::new("Tasks")
            .gap(0.0)
            .item(SizedBox::new().width(120.0).height(30.0))
            .item(SizedBox::new().width(120.0).height(30.0))
            .item(SizedBox::new().width(120.0).height(30.0))
            .item_name(|item| NAMES[item].to_string())
            .on_reorder({
                let changes = Rc::clone(&changes);
                move |change| changes.borrow_mut().push(change)
            });
        let (mut runtime, window_id) = build_runtime(list);
        let _ = runtime.render(window_id)?;

        // Pressing a row focuses the list with the keyboard on that row.
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 15.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(10.0, 15.0), false),
        )?;
        let list_node = |runtime: &Runtime| {
            runtime
                .semantics(window_id)
                .unwrap()
                .iter()
                .find(|node| node.role == SemanticsRole::List)
                .cloned()
                .unwrap()
        };
        let _ = runtime.render(window_id)?;
        assert_eq!(
            runtime.focused_widget(window_id)?,
            Some(list_node(&runtime).id)
        );

        runtime.handle_event(window_id, key("ArrowDown", false))?;
        let _ = runtime.render(window_id)?;
        assert_eq!(
            list_node(&runtime).value,
            Some(SemanticsValue::Text("Review, position 2 of 3".to_string()))
        );
        assert!(changes.borrow().is_empty(), "plain arrows only choose");

        runtime.handle_event(window_id, key("ArrowDown", true))?;
        let _ = runtime.render(window_id)?;
        assert_eq!(
            status(&runtime, window_id).as_deref(),
            Some("Moved Review to position 3 of 3")
        );
        runtime.handle_event(window_id, key("Home", true))?;
        let _ = runtime.render(window_id)?;
        assert_eq!(
            status(&runtime, window_id).as_deref(),
            Some("Moved Review to position 1 of 3")
        );
        runtime.handle_event(window_id, key("ArrowUp", true))?;
        let _ = runtime.render(window_id)?;
        assert_eq!(
            status(&runtime, window_id).as_deref(),
            Some("Review is already at position 1 of 3")
        );

        assert_eq!(
            &*changes.borrow(),
            &[
                ReorderableListChange {
                    item: 1,
                    from: 1,
                    to: 2
                },
                ReorderableListChange {
                    item: 1,
                    from: 2,
                    to: 0
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn alt_arrows_move_the_row_holding_focus() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let row = |name: &'static str| Stack::horizontal().with_child(Button::new(name));
        let list = ReorderableList::new("Tasks")
            .item(row("Draft"))
            .item(row("Review"))
            .item(row("Ship"))
            .on_reorder({
                let changes = Rc::clone(&changes);
                move |change| changes.borrow_mut().push(change)
            });
        let (mut runtime, window_id) = build_runtime(list);
        let _ = runtime.render(window_id)?;
        let ship = runtime
            .semantics(window_id)?
            .iter()
            .find(|node| node.name.as_deref() == Some("Ship"))
            .map(|node| node.id)
            .unwrap();
        runtime.handle_semantics_action(
            window_id,
            ship,
            sui_core::SemanticsActionRequest::Focus,
        )?;
        assert_eq!(runtime.focused_widget(window_id)?, Some(ship));

        runtime.handle_event(window_id, key("ArrowUp", true))?;
        assert_eq!(
            &*changes.borrow(),
            &[ReorderableListChange {
                item: 2,
                from: 2,
                to: 1
            }]
        );
        assert_eq!(
            runtime.focused_widget(window_id)?,
            Some(ship),
            "focus stays"
        );
        Ok(())
    }
}
