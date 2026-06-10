use sui_core::{
    Color, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, Point,
    PointerButton, PointerEventKind, Rect, SemanticsAction, SemanticsNode, SemanticsRole,
    SemanticsValue, Size, WakeEvent, WidgetId,
};
use sui_layout::{Axis, Constraints};
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx,
    SemanticsCtx, SingleChild, StackHostOptions, StackOrderPolicy, Widget, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{LayerCompositionMode, StrokeStyle};

use crate::{
    DefaultTheme, MotionScalar,
    containers::{ScrollBar, ScrollState, ScrollView},
    text_align::aligned_text_rect_for_text,
};

type AnimatedScalar = MotionScalar;

fn set_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    duration: f64,
    easing: crate::Easing,
    ctx: &mut EventCtx,
) -> bool {
    animation.set_target_event(target, duration, easing, ctx)
}

fn set_hover_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.hover_duration(),
        theme.motion.hover_easing(),
        ctx,
    )
}

fn set_press_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.press_duration(),
        theme.motion.press_easing(),
        ctx,
    )
}

fn set_focus_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) -> bool {
    set_animation_target(
        animation,
        target,
        theme.motion.focus_duration(),
        theme.motion.focus_easing(),
        ctx,
    )
}

fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    Color::new(
        from.space,
        from.red + (to.red - from.red) * t,
        from.green + (to.green - from.green) * t,
        from.blue + (to.blue - from.blue) * t,
        from.alpha + (to.alpha - from.alpha) * t,
    )
}

pub type ResizablePane = SplitView;

#[derive(Debug, Clone, PartialEq)]
pub struct FloatingViewConfig {
    pub title: String,
    pub bounds: Rect,
    pub min_size: Size,
    pub visible: bool,
}

impl FloatingViewConfig {
    pub fn new(title: impl Into<String>, bounds: Rect) -> Self {
        Self {
            title: title.into(),
            bounds,
            min_size: Size::new(220.0, 160.0),
            visible: true,
        }
    }

    pub fn min_size(mut self, min_size: Size) -> Self {
        self.min_size = Size::new(min_size.width.max(120.0), min_size.height.max(120.0));
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatingViewSnapshot {
    pub id: u64,
    pub title: String,
    pub bounds: Rect,
    pub min_size: Size,
    pub visible: bool,
    pub maximized: bool,
    pub surface_widget_id: Option<WidgetId>,
}

#[derive(Debug, Clone, PartialEq)]
struct FloatingViewState {
    id: u64,
    title: String,
    bounds: Rect,
    min_size: Size,
    visible: bool,
    surface_widget_id: Option<WidgetId>,
}

#[derive(Debug, Default)]
struct FloatingWorkspaceStateInner {
    next_id: u64,
    views: Vec<FloatingViewState>,
    z_order: Vec<u64>,
    maximized_view: Option<u64>,
    active_resize_view: Option<u64>,
}

#[derive(Clone, Default)]
pub struct FloatingWorkspaceState {
    inner: std::rc::Rc<std::cell::RefCell<FloatingWorkspaceStateInner>>,
}

impl FloatingWorkspaceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_view(&self, config: FloatingViewConfig) -> u64 {
        let mut inner = self.inner.borrow_mut();
        let id = inner.next_id.max(1);
        inner.next_id = id + 1;
        inner.views.push(FloatingViewState {
            id,
            title: config.title,
            bounds: config.bounds,
            min_size: Size::new(
                config.min_size.width.max(120.0),
                config.min_size.height.max(120.0),
            ),
            visible: config.visible,
            surface_widget_id: None,
        });
        inner.z_order.push(id);
        id
    }

    pub fn snapshots(&self) -> Vec<FloatingViewSnapshot> {
        let inner = self.inner.borrow();
        inner
            .views
            .iter()
            .map(|view| FloatingViewSnapshot {
                id: view.id,
                title: view.title.clone(),
                bounds: view.bounds,
                min_size: view.min_size,
                visible: view.visible,
                maximized: inner.maximized_view == Some(view.id),
                surface_widget_id: view.surface_widget_id,
            })
            .collect()
    }

    pub fn snapshot(&self, view_id: u64) -> Option<FloatingViewSnapshot> {
        let inner = self.inner.borrow();
        inner
            .views
            .iter()
            .find(|view| view.id == view_id)
            .map(|view| FloatingViewSnapshot {
                id: view.id,
                title: view.title.clone(),
                bounds: view.bounds,
                min_size: view.min_size,
                visible: view.visible,
                maximized: inner.maximized_view == Some(view.id),
                surface_widget_id: view.surface_widget_id,
            })
    }

    pub fn set_view_surface_widget(&self, view_id: u64, widget_id: WidgetId) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(view) = inner.views.iter_mut().find(|view| view.id == view_id) else {
            return false;
        };
        if view.surface_widget_id == Some(widget_id) {
            return false;
        }
        view.surface_widget_id = Some(widget_id);
        true
    }

    pub fn view_surface_widget_id(&self, view_id: u64) -> Option<WidgetId> {
        self.inner
            .borrow()
            .views
            .iter()
            .find(|view| view.id == view_id)
            .and_then(|view| view.surface_widget_id)
    }

    pub fn set_view_visible(&self, view_id: u64, visible: bool) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(view) = inner.views.iter_mut().find(|view| view.id == view_id) else {
            return false;
        };
        if view.visible == visible {
            return false;
        }
        view.visible = visible;
        if !visible && inner.maximized_view == Some(view_id) {
            inner.maximized_view = None;
        }
        true
    }

    pub fn toggle_view_visible(&self, view_id: u64) -> Option<bool> {
        let next = self.snapshot(view_id).map(|view| !view.visible)?;
        self.set_view_visible(view_id, next);
        Some(next)
    }

    pub fn set_view_bounds(&self, view_id: u64, bounds: Rect) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(view) = inner.views.iter_mut().find(|view| view.id == view_id) else {
            return false;
        };
        if view.bounds == bounds {
            return false;
        }
        view.bounds = bounds;
        true
    }

    pub fn bring_to_front(&self, view_id: u64) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(index) = inner.z_order.iter().position(|id| *id == view_id) else {
            return false;
        };
        if index + 1 == inner.z_order.len() {
            return false;
        }
        let id = inner.z_order.remove(index);
        inner.z_order.push(id);
        true
    }

    pub fn set_view_maximized(&self, view_id: u64, maximized: bool) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Some(index) = inner.views.iter().position(|view| view.id == view_id) else {
            return false;
        };
        if maximized {
            inner.views[index].visible = true;
            let ordering_changed =
                if let Some(order_index) = inner.z_order.iter().position(|id| *id == view_id) {
                    if order_index + 1 == inner.z_order.len() {
                        false
                    } else {
                        let id = inner.z_order.remove(order_index);
                        inner.z_order.push(id);
                        true
                    }
                } else {
                    inner.z_order.push(view_id);
                    true
                };
            if inner.maximized_view == Some(view_id) {
                return ordering_changed;
            }
            inner.maximized_view = Some(view_id);
            return true;
        }

        if inner.maximized_view != Some(view_id) {
            return false;
        }
        inner.maximized_view = None;
        true
    }

    pub fn active_view_ids(&self) -> Vec<u64> {
        let inner = self.inner.borrow();
        if let Some(maximized) = inner.maximized_view {
            return inner
                .views
                .iter()
                .find(|view| view.id == maximized && view.visible)
                .map(|view| vec![view.id])
                .unwrap_or_default();
        }

        inner
            .z_order
            .iter()
            .copied()
            .filter(|id| {
                inner
                    .views
                    .iter()
                    .any(|view| view.id == *id && view.visible)
            })
            .collect()
    }

    pub fn active_resize_view(&self) -> Option<u64> {
        self.inner.borrow().active_resize_view
    }

    pub fn set_active_resize_view(&self, view_id: Option<u64>) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.active_resize_view == view_id {
            return false;
        }
        inner.active_resize_view = view_id;
        true
    }
}

struct FloatingWorkspaceEntry {
    view_id: u64,
    child: WidgetPod,
}

enum FloatingWorkspaceGestureKind {
    Move,
    Resize,
}

struct FloatingWorkspaceGesture {
    view_id: u64,
    pointer_id: u64,
    kind: FloatingWorkspaceGestureKind,
    pointer_origin: Point,
    initial_bounds: Rect,
}

pub struct FloatingWorkspace {
    theme: Box<DefaultTheme>,
    name: Option<String>,
    state: FloatingWorkspaceState,
    views: Vec<FloatingWorkspaceEntry>,
    gesture: Option<FloatingWorkspaceGesture>,
}

impl FloatingWorkspace {
    pub fn new(state: FloatingWorkspaceState) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: None,
            state,
            views: Vec::new(),
            gesture: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn state(&self) -> FloatingWorkspaceState {
        self.state.clone()
    }

    pub fn with_view<W>(mut self, config: FloatingViewConfig, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.push_view(config, child);
        self
    }

    pub fn push_view<W>(&mut self, config: FloatingViewConfig, child: W) -> u64
    where
        W: Widget + 'static,
    {
        let view_id = self.state.add_view(config);
        let child = WidgetPod::new(FloatingViewSurface::new(
            (*self.theme).clone(),
            self.state.clone(),
            view_id,
            child,
        ));
        self.state.set_view_surface_widget(view_id, child.id());
        self.views.push(FloatingWorkspaceEntry { view_id, child });
        view_id
    }

    fn entry(&self, view_id: u64) -> Option<&FloatingWorkspaceEntry> {
        self.views.iter().find(|entry| entry.view_id == view_id)
    }

    fn entry_mut(&mut self, view_id: u64) -> Option<&mut FloatingWorkspaceEntry> {
        self.views.iter_mut().find(|entry| entry.view_id == view_id)
    }

    fn active_view_ids(&self) -> Vec<u64> {
        self.state.active_view_ids()
    }

    fn frontmost_hit(&self, host_bounds: Rect, position: Point) -> Option<FloatingWorkspaceHit> {
        self.active_view_ids()
            .into_iter()
            .rev()
            .filter_map(|view_id| self.state.snapshot(view_id))
            .find_map(|view| {
                let bounds = resolved_floating_view_bounds(&self.theme, host_bounds, &view);
                if !bounds.contains(position) {
                    return None;
                }

                if !view.maximized
                    && floating_view_resize_handle_rect(&self.theme, bounds).contains(position)
                {
                    return Some(FloatingWorkspaceHit {
                        view_id: view.id,
                        region: FloatingWorkspaceHitRegion::ResizeHandle,
                    });
                }

                if !view.maximized
                    && floating_view_title_bar_rect(&self.theme, bounds).contains(position)
                {
                    return Some(FloatingWorkspaceHit {
                        view_id: view.id,
                        region: FloatingWorkspaceHitRegion::TitleBar,
                    });
                }

                Some(FloatingWorkspaceHit {
                    view_id: view.id,
                    region: FloatingWorkspaceHitRegion::Body,
                })
            })
    }

    fn update_drag(&mut self, host_bounds: Rect, position: Point) -> Option<Rect> {
        let Some(gesture) = &self.gesture else {
            return None;
        };
        let Some(view) = self.state.snapshot(gesture.view_id) else {
            return None;
        };

        let delta = position - gesture.pointer_origin;
        let previous_bounds = view.bounds;
        let next_bounds = match gesture.kind {
            FloatingWorkspaceGestureKind::Move => Rect::new(
                gesture.initial_bounds.x() + delta.x,
                gesture.initial_bounds.y() + delta.y,
                gesture.initial_bounds.width(),
                gesture.initial_bounds.height(),
            ),
            FloatingWorkspaceGestureKind::Resize => Rect::new(
                gesture.initial_bounds.x(),
                gesture.initial_bounds.y(),
                gesture.initial_bounds.width() + delta.x,
                gesture.initial_bounds.height() + delta.y,
            ),
        };
        let clamped =
            clamp_floating_view_bounds(&self.theme, host_bounds, next_bounds, view.min_size);
        if !self.state.set_view_bounds(view.id, clamped) {
            return None;
        }

        Some(previous_bounds.union(clamped))
    }
}

impl Widget for FloatingWorkspace {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self
                        .gesture
                        .as_ref()
                        .is_some_and(|gesture| gesture.pointer_id == pointer.pointer_id) =>
            {
                let resizing = self.gesture.as_ref().is_some_and(|gesture| {
                    matches!(gesture.kind, FloatingWorkspaceGestureKind::Resize)
                });
                let refresh_target = if resizing {
                    self.gesture
                        .as_ref()
                        .and_then(|gesture| self.state.view_surface_widget_id(gesture.view_id))
                        .unwrap_or(ctx.widget_id())
                } else {
                    ctx.widget_id()
                };
                if let Some(dirty_region) = self.update_drag(ctx.bounds(), pointer.position) {
                    if resizing {
                        request_widget_refresh(ctx, refresh_target, true, false, dirty_region);
                    } else {
                        request_widget_refresh(ctx, refresh_target, false, false, dirty_region);
                    }
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self
                        .gesture
                        .as_ref()
                        .is_some_and(|gesture| gesture.pointer_id == pointer.pointer_id) =>
            {
                let active_view = self.gesture.as_ref().and_then(|gesture| {
                    matches!(gesture.kind, FloatingWorkspaceGestureKind::Resize)
                        .then_some(gesture.view_id)
                });
                if self.state.set_active_resize_view(None) {
                    let refresh_target = active_view
                        .and_then(|view_id| self.state.view_surface_widget_id(view_id))
                        .unwrap_or(ctx.widget_id());
                    let refresh_region = active_view
                        .and_then(|view_id| self.state.snapshot(view_id).map(|view| view.bounds))
                        .unwrap_or(ctx.bounds());
                    request_widget_refresh(ctx, refresh_target, true, false, refresh_region);
                }
                self.gesture = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self
                        .gesture
                        .as_ref()
                        .is_some_and(|gesture| gesture.pointer_id == pointer.pointer_id) =>
            {
                let active_view = self.gesture.as_ref().and_then(|gesture| {
                    matches!(gesture.kind, FloatingWorkspaceGestureKind::Resize)
                        .then_some(gesture.view_id)
                });
                if self.state.set_active_resize_view(None) {
                    let refresh_target = active_view
                        .and_then(|view_id| self.state.view_surface_widget_id(view_id))
                        .unwrap_or(ctx.widget_id());
                    let refresh_region = active_view
                        .and_then(|view_id| self.state.snapshot(view_id).map(|view| view.bounds))
                        .unwrap_or(ctx.bounds());
                    request_widget_refresh(ctx, refresh_target, true, false, refresh_region);
                }
                self.gesture = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let Some(hit) = self.frontmost_hit(ctx.bounds(), pointer.position) else {
                    return;
                };

                let ordering_changed = self.state.bring_to_front(hit.view_id);
                if ordering_changed {
                    ctx.request_ordering();
                    ctx.request_paint();
                    ctx.request_hit_test();
                }

                let Some(view) = self.state.snapshot(hit.view_id) else {
                    return;
                };
                let bounds = resolved_floating_view_bounds(&self.theme, ctx.bounds(), &view);
                match hit.region {
                    FloatingWorkspaceHitRegion::TitleBar => {
                        self.gesture = Some(FloatingWorkspaceGesture {
                            view_id: hit.view_id,
                            pointer_id: pointer.pointer_id,
                            kind: FloatingWorkspaceGestureKind::Move,
                            pointer_origin: pointer.position,
                            initial_bounds: bounds,
                        });
                        ctx.request_pointer_capture(pointer.pointer_id);
                        ctx.set_handled();
                    }
                    FloatingWorkspaceHitRegion::ResizeHandle => {
                        self.state.set_active_resize_view(Some(hit.view_id));
                        self.gesture = Some(FloatingWorkspaceGesture {
                            view_id: hit.view_id,
                            pointer_id: pointer.pointer_id,
                            kind: FloatingWorkspaceGestureKind::Resize,
                            pointer_origin: pointer.position,
                            initial_bounds: bounds,
                        });
                        ctx.request_pointer_capture(pointer.pointer_id);
                        ctx.set_handled();
                    }
                    FloatingWorkspaceHitRegion::Body => {}
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let snapshots = self.state.snapshots();
        let fallback = snapshots.iter().fold(Size::ZERO, |size, view| {
            Size::new(
                size.width.max(view.bounds.max_x().max(0.0)),
                size.height.max(view.bounds.max_y().max(0.0)),
            )
        });
        let size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                fallback.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                fallback.height
            },
        ));

        let host_bounds = Rect::from_origin_size(Point::ZERO, size);
        for view_id in self.active_view_ids() {
            let Some(snapshot) = self.state.snapshot(view_id) else {
                continue;
            };
            let bounds = resolved_floating_view_bounds(&self.theme, host_bounds, &snapshot);
            if let Some(entry) = self.entry_mut(view_id) {
                entry.child.measure(ctx, Constraints::tight(bounds.size));
            }
        }

        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        for view_id in self.active_view_ids() {
            let Some(snapshot) = self.state.snapshot(view_id) else {
                continue;
            };
            let resolved = resolved_floating_view_bounds(&self.theme, bounds, &snapshot);
            if !snapshot.maximized && resolved != snapshot.bounds {
                self.state.set_view_bounds(view_id, resolved);
            }
            if let Some(entry) = self.entry_mut(view_id) {
                entry.child.arrange(ctx, resolved);
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let dirty_region = widget_invalidation_region(ctx.invalidations(), ctx.widget_id())
            .and_then(|region| region.intersection(ctx.bounds()));

        if let Some(region) = dirty_region {
            ctx.push_clip_rect(region);
        }

        let palette = self.theme.palette;
        ctx.fill_bounds(palette.control);
        ctx.fill_rect(
            ctx.bounds().inflate(
                -self.theme.metrics.floating_workspace_margin,
                -self.theme.metrics.floating_workspace_margin,
            ),
            palette.surface_raised.with_alpha(0.72),
        );

        for view_id in self.active_view_ids() {
            if let Some(entry) = self.entry(view_id) {
                if dirty_region
                    .is_some_and(|region| entry.child.bounds().intersection(region).is_none())
                {
                    continue;
                }
                entry.child.paint(ctx);
            }
        }

        if dirty_region.is_some() {
            ctx.pop_clip();
        }
    }

    fn stack_host_options(&self) -> Option<StackHostOptions> {
        Some(StackHostOptions {
            order_policy: StackOrderPolicy::FocusFronted,
        })
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = self.name.clone();
        node.state.focused = ctx.is_focused();
        ctx.push(node);

        for view_id in self.active_view_ids() {
            if let Some(entry) = self.entry(view_id) {
                entry.child.semantics(ctx);
            }
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for view_id in self.active_view_ids() {
            if let Some(entry) = self.entry(view_id) {
                visitor.visit(&entry.child);
            }
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for view_id in self.active_view_ids() {
            if let Some(entry) = self.entry_mut(view_id) {
                visitor.visit(&mut entry.child);
            }
        }
    }
}

struct FloatingWorkspaceHit {
    view_id: u64,
    region: FloatingWorkspaceHitRegion,
}

enum FloatingWorkspaceHitRegion {
    TitleBar,
    ResizeHandle,
    Body,
}

struct FloatingViewSurface {
    theme: Box<DefaultTheme>,
    state: FloatingWorkspaceState,
    view_id: u64,
    host: SingleChild,
}

impl FloatingViewSurface {
    fn new<W>(theme: DefaultTheme, state: FloatingWorkspaceState, view_id: u64, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: Box::new(theme.clone()),
            state: state.clone(),
            view_id,
            host: SingleChild::new(FloatingViewHost::new(theme, state, view_id, child)),
        }
    }
}

impl Widget for FloatingViewSurface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.host.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.host.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(view) = self.state.snapshot(self.view_id) else {
            return;
        };

        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let border_width = metrics.border_width.max(1.0);
        ctx.push_clip_rect(ctx.bounds());
        ctx.fill_rect(ctx.bounds(), palette.surface_raised);
        if !view.maximized {
            let title_bar = floating_view_title_bar_rect(&self.theme, ctx.bounds());
            ctx.fill_rect(title_bar, palette.control_active);
            let title_padding = metrics.floating_view_title_padding;
            let text_slot = Rect::new(
                title_bar.x() + title_padding.left,
                title_bar.y() + title_padding.top,
                (title_bar.width() - title_padding.left - title_padding.right).max(0.0),
                (title_bar.height() - title_padding.top - title_padding.bottom).max(0.0),
            );
            let title_style = self.theme.text_style(palette.text);
            let title_rect = aligned_text_rect_for_text(
                ctx,
                text_slot,
                &view.title,
                &title_style,
                title_style.line_height,
                0.0,
            );
            ctx.push_clip_rect(text_slot);
            ctx.draw_text(title_rect, view.title, title_style);
            ctx.pop_clip();
        }
        self.host.paint(ctx);
        if !view.maximized {
            ctx.stroke_rect(ctx.bounds(), palette.border, StrokeStyle::new(border_width));
            let handle = floating_view_resize_handle_rect(&self.theme, ctx.bounds());
            let accent = palette.border.with_alpha(0.95);
            let first_length = (metrics.floating_view_resize_handle_size * 0.56).max(6.0);
            let second_length = (metrics.floating_view_resize_handle_size * 0.34).max(4.0);
            let handle_inset = metrics.border_width.max(1.0);
            let handle_stroke = metrics.border_width.max(1.0) + 0.5;
            ctx.stroke(
                diagonal_handle_path(handle, first_length, handle_inset),
                accent,
                StrokeStyle::new(handle_stroke),
            );
            ctx.stroke(
                diagonal_handle_path(handle, second_length, handle_inset + self.theme.spacing),
                accent.with_alpha(0.72),
                StrokeStyle::new(handle_stroke),
            );
        }
        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if let Some(view) = self.state.snapshot(self.view_id) {
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
            node.name = Some(view.title);
            ctx.push(node);
        }
        self.host.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.host.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.host.visit_children_mut(visitor);
    }
}

struct FloatingViewHost {
    theme: Box<DefaultTheme>,
    state: FloatingWorkspaceState,
    view_id: u64,
    content: SingleChild,
    vertical_scroll_bar: SingleChild,
    horizontal_scroll_bar: SingleChild,
}

impl FloatingViewHost {
    fn new<W>(theme: DefaultTheme, state: FloatingWorkspaceState, view_id: u64, child: W) -> Self
    where
        W: Widget + 'static,
    {
        let scroll_state = ScrollState::new();
        let scroll_view = ScrollView::both(child)
            .state(scroll_state.clone())
            .viewport_size_hint(true)
            .name("Floating view content");
        Self {
            theme: Box::new(theme.clone()),
            state,
            view_id,
            vertical_scroll_bar: SingleChild::new(
                ScrollBar::vertical(scroll_state.clone())
                    .theme(theme.clone())
                    .name("Vertical scroll bar"),
            ),
            horizontal_scroll_bar: SingleChild::new(
                ScrollBar::horizontal(scroll_state.clone())
                    .theme(theme)
                    .name("Horizontal scroll bar"),
            ),
            content: SingleChild::new(scroll_view),
        }
    }

    fn resizing(&self) -> bool {
        self.state.active_resize_view() == Some(self.view_id)
    }
}

impl Widget for FloatingViewHost {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let Some(view) = self.state.snapshot(self.view_id) else {
            return constraints.max;
        };
        let outer = constraints.max;
        let probe = Rect::from_origin_size(Point::ZERO, outer);
        let content = floating_view_content_rect(&self.theme, probe, view.maximized);
        self.content.measure(ctx, Constraints::tight(content.size));
        let thickness = self.theme.metrics.scroll_bar_thickness;
        self.vertical_scroll_bar.measure(
            ctx,
            Constraints::tight(Size::new(thickness, content.height())),
        );
        self.horizontal_scroll_bar.measure(
            ctx,
            Constraints::tight(Size::new(content.width(), thickness)),
        );
        constraints.clamp(outer)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let Some(view) = self.state.snapshot(self.view_id) else {
            return;
        };
        let content = floating_view_content_rect(&self.theme, bounds, view.maximized);
        self.content.arrange(ctx, content);
        let thickness = self
            .theme
            .metrics
            .scroll_bar_thickness
            .min(content.width().max(0.0))
            .min(content.height().max(0.0));
        let vertical_height = (content.height() - thickness).max(0.0);
        let horizontal_width = (content.width() - thickness).max(0.0);
        self.vertical_scroll_bar.arrange(
            ctx,
            Rect::new(
                content.max_x() - thickness,
                content.y(),
                thickness,
                vertical_height,
            ),
        );
        self.horizontal_scroll_bar.arrange(
            ctx,
            Rect::new(
                content.x(),
                content.max_y() - thickness,
                horizontal_width,
                thickness,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.push_clip_rect(ctx.bounds());
        self.content.paint(ctx);
        if !self.resizing() {
            self.vertical_scroll_bar.paint(ctx);
            self.horizontal_scroll_bar.paint(ctx);
        }
        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if self.resizing() {
                LayerCompositionMode::Normal
            } else {
                LayerCompositionMode::Scroll
            },
        }
    }

    fn stack_host_options(&self) -> Option<StackHostOptions> {
        Some(StackHostOptions::default())
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        if !self.resizing() {
            self.vertical_scroll_bar.semantics(ctx);
            self.horizontal_scroll_bar.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.vertical_scroll_bar.visit_children(visitor);
        self.horizontal_scroll_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.vertical_scroll_bar.visit_children_mut(visitor);
        self.horizontal_scroll_bar.visit_children_mut(visitor);
    }
}

pub struct SplitView {
    theme: Box<DefaultTheme>,
    name: Option<String>,
    axis: Axis,
    ratio: f32,
    min_first: f32,
    min_second: f32,
    divider_thickness: Option<f32>,
    first: SingleChild,
    second: SingleChild,
    hovered: bool,
    hover_animation: AnimatedScalar,
    drag_animation: AnimatedScalar,
    focus_animation: AnimatedScalar,
    drag_pointer: Option<u64>,
    divider_bounds: Rect,
    on_change: Option<Box<dyn FnMut(f32)>>,
}

impl SplitView {
    pub fn new<W1, W2>(axis: Axis, first: W1, second: W2) -> Self
    where
        W1: Widget + 'static,
        W2: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: None,
            axis,
            ratio: 0.5,
            min_first: 120.0,
            min_second: 120.0,
            divider_thickness: None,
            first: SingleChild::new(first),
            second: SingleChild::new(second),
            hovered: false,
            hover_animation: AnimatedScalar::new(0.0),
            drag_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            drag_pointer: None,
            divider_bounds: Rect::ZERO,
            on_change: None,
        }
    }

    pub fn horizontal<W1, W2>(first: W1, second: W2) -> Self
    where
        W1: Widget + 'static,
        W2: Widget + 'static,
    {
        Self::new(Axis::Horizontal, first, second)
    }

    pub fn vertical<W1, W2>(first: W1, second: W2) -> Self
    where
        W1: Widget + 'static,
        W2: Widget + 'static,
    {
        Self::new(Axis::Vertical, first, second)
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio.clamp(0.0, 1.0);
        self
    }

    pub fn min_first(mut self, min_first: f32) -> Self {
        self.min_first = min_first.max(0.0);
        self
    }

    pub fn min_second(mut self, min_second: f32) -> Self {
        self.min_second = min_second.max(0.0);
        self
    }

    pub fn divider_thickness(mut self, divider_thickness: f32) -> Self {
        self.divider_thickness = Some(divider_thickness.max(0.0));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(f32) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn first(&self) -> &sui_runtime::WidgetPod {
        self.first.child()
    }

    pub fn first_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.first.child_mut()
    }

    pub fn second(&self) -> &sui_runtime::WidgetPod {
        self.second.child()
    }

    pub fn second_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.second.child_mut()
    }

    pub fn current_ratio(&self) -> f32 {
        self.ratio
    }

    fn resolved_divider_thickness(&self) -> f32 {
        self.divider_thickness
            .unwrap_or(self.theme.metrics.split_view_divider_thickness)
            .max(self.theme.metrics.border_width.max(1.0))
    }

    fn divider_rect(&self, bounds: Rect) -> Rect {
        self.divider_bounds.translate(bounds.origin.to_vector())
    }

    fn divider_hit_rect(&self, bounds: Rect) -> Rect {
        let divider = self.divider_rect(bounds);
        let target = self
            .theme
            .metrics
            .split_view_drag_target_thickness
            .max(self.resolved_divider_thickness());
        let extra = ((target - axis_main(self.axis, divider.size)).max(0.0)) * 0.5;
        let hit = match self.axis {
            Axis::Horizontal => divider.inflate(extra, 0.0),
            Axis::Vertical => divider.inflate(0.0, extra),
        };

        hit.intersection(bounds).unwrap_or(divider)
    }

    fn allowed_first_main_range(&self, available: f32) -> (f32, f32) {
        let available = available.max(0.0);
        let lower = self.min_first.min(available);
        let upper = (available - self.min_second).max(0.0);

        if lower <= upper {
            (lower, upper)
        } else {
            (0.0, available)
        }
    }

    fn divider_main_offset(&self, bounds: Rect) -> f32 {
        let divider = self.resolved_divider_thickness();
        let available = (axis_main(self.axis, bounds.size) - divider).max(0.0);
        let (lower, upper) = self.allowed_first_main_range(available);
        let first = (available * self.ratio).clamp(lower, upper);
        first
    }

    fn update_hover(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            let theme = *self.theme;
            self.hovered = hovered;
            set_hover_animation_target(
                &mut self.hover_animation,
                hovered as u8 as f32,
                &theme,
                ctx,
            );
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn set_dragging(&mut self, dragging: bool, ctx: &mut EventCtx) {
        let theme = *self.theme;
        set_press_animation_target(&mut self.drag_animation, dragging as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn advance_animations(&mut self, time: f64) -> bool {
        self.hover_animation.advance(time)
            | self.drag_animation.advance(time)
            | self.focus_animation.advance(time)
    }

    fn set_ratio_from_position(&mut self, bounds: Rect, position: Point) {
        let divider = self.resolved_divider_thickness();
        let total = (axis_main(self.axis, bounds.size) - divider).max(0.0);
        if total <= 0.0 {
            return;
        }

        let pointer_main = axis_position(self.axis, position) - axis_origin(self.axis, bounds);
        let (lower, upper) = self.allowed_first_main_range(total);
        let clamped = pointer_main.clamp(lower, upper);
        let ratio = (clamped / total).clamp(0.0, 1.0);
        if (ratio - self.ratio).abs() > f32::EPSILON {
            self.ratio = ratio;
            if let Some(on_change) = &mut self.on_change {
                on_change(self.ratio);
            }
        }
    }

    fn nudge_ratio(&mut self, delta: f32) {
        let next = (self.ratio + delta).clamp(0.0, 1.0);
        if (next - self.ratio).abs() > f32::EPSILON {
            self.ratio = next;
            if let Some(on_change) = &mut self.on_change {
                on_change(self.ratio);
            }
        }
    }
}

impl Widget for SplitView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                if self.drag_pointer == Some(pointer.pointer_id) {
                    self.set_ratio_from_position(ctx.bounds(), pointer.position);
                    ctx.request_arrange();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if ctx.phase() != EventPhase::Capture {
                    self.update_hover(
                        self.divider_hit_rect(ctx.bounds())
                            .contains(pointer.position),
                        ctx,
                    );
                } else {
                    self.update_hover(false, ctx);
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.phase() != EventPhase::Capture
                    && pointer.button == Some(PointerButton::Primary)
                    && self
                        .divider_hit_rect(ctx.bounds())
                        .contains(pointer.position) =>
            {
                self.drag_pointer = Some(pointer.pointer_id);
                self.update_hover(true, ctx);
                self.set_dragging(true, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                self.set_ratio_from_position(ctx.bounds(), pointer.position);
                ctx.request_arrange();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.drag_pointer == Some(pointer.pointer_id) =>
            {
                self.drag_pointer = None;
                self.set_dragging(false, ctx);
                self.update_hover(
                    self.divider_hit_rect(ctx.bounds())
                        .contains(pointer.position),
                    ctx,
                );
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.drag_pointer == Some(pointer.pointer_id) =>
            {
                self.drag_pointer = None;
                self.set_dragging(false, ctx);
                self.update_hover(false, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.drag_pointer.is_none() {
                    self.update_hover(false, ctx);
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let step = 0.05;
                let delta = match (self.axis, key.key.as_str()) {
                    (Axis::Horizontal, "ArrowLeft") | (Axis::Vertical, "ArrowUp") => -step,
                    (Axis::Horizontal, "ArrowRight") | (Axis::Vertical, "ArrowDown") => step,
                    (Axis::Horizontal, "Home") | (Axis::Vertical, "Home") => {
                        self.ratio = 0.0;
                        ctx.request_arrange();
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                        return;
                    }
                    (Axis::Horizontal, "End") | (Axis::Vertical, "End") => {
                        self.ratio = 1.0;
                        ctx.request_arrange();
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                        return;
                    }
                    _ => return,
                };
                self.nudge_ratio(delta);
                ctx.request_arrange();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let divider = self.resolved_divider_thickness();

        let probe_constraints = constraints.loosen();
        let first_probe = self.first.measure(ctx, probe_constraints);
        let second_probe = self.second.measure(ctx, probe_constraints);
        let natural = axis_size(
            self.axis,
            axis_main(self.axis, first_probe) + divider + axis_main(self.axis, second_probe),
            axis_cross(self.axis, first_probe).max(axis_cross(self.axis, second_probe)),
        );

        let size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                natural.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                natural.height
            },
        ));

        let total_main = axis_main(self.axis, size);
        let cross = axis_cross(self.axis, size);
        let divider_offset = self.divider_main_offset(Rect::from_origin_size(Point::ZERO, size));
        let first_main = divider_offset.max(0.0);
        let second_main = (total_main - divider - first_main).max(0.0);
        self.first
            .measure(ctx, split_child_constraints(self.axis, first_main, cross));
        self.second
            .measure(ctx, split_child_constraints(self.axis, second_main, cross));

        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let divider = self.resolved_divider_thickness();
        let size = bounds.size;

        let total_main = axis_main(self.axis, size);
        let cross = axis_cross(self.axis, size);
        let divider_offset = self.divider_main_offset(Rect::from_origin_size(Point::ZERO, size));
        let first_main = divider_offset.max(0.0);
        let second_main = (total_main - divider - first_main).max(0.0);
        let first_constraints = split_child_constraints(self.axis, first_main, cross);
        let second_constraints = split_child_constraints(self.axis, second_main, cross);
        let first_size = first_constraints.clamp(self.first.child().measured_size());
        let second_size = second_constraints.clamp(self.second.child().measured_size());

        self.first
            .arrange(ctx, Rect::from_origin_size(bounds.origin, first_size));
        let second_origin =
            bounds.origin + axis_point(self.axis, first_main + divider, 0.0).to_vector();
        self.second
            .arrange(ctx, Rect::from_origin_size(second_origin, second_size));

        self.divider_bounds = Rect::from_origin_size(
            axis_point(self.axis, first_size_main(self.axis, first_size), 0.0),
            axis_size(self.axis, divider, cross),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.first.paint(ctx);
        self.second.paint(ctx);

        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let divider_bounds = self.divider_rect(ctx.bounds());
        let hover_color = mix_color(
            palette.border,
            palette.border_hover,
            self.hover_animation.value,
        );
        let focus_color = mix_color(
            hover_color,
            palette.border_focus,
            self.focus_animation.value,
        );
        let divider_color = mix_color(focus_color, palette.border_focus, self.drag_animation.value);

        ctx.fill_rect(divider_bounds, divider_color);
        if self.resolved_divider_thickness() > metrics.border_width.max(1.0) {
            ctx.stroke_rect(
                divider_bounds,
                divider_color.with_alpha(0.72),
                StrokeStyle::new(metrics.border_width.max(1.0)),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Splitter, ctx.bounds());
        node.name = self.name.clone();
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Number(self.ratio as f64));
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
        self.first.semantics(ctx);
        self.second.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = *self.theme;
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.first.visit_children(visitor);
        self.second.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.first.visit_children_mut(visitor);
        self.second.visit_children_mut(visitor);
    }
}

struct FloatingWindowEntry {
    bounds: Rect,
    child: WidgetPod,
}

struct FloatingStackSurface {
    host: SingleChild,
}

impl FloatingStackSurface {
    fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            host: SingleChild::new(child),
        }
    }
}

impl Widget for FloatingStackSurface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.host.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.host.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.host.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.host.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.host.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.host.visit_children_mut(visitor);
    }
}

pub struct FloatingStack {
    theme: Box<DefaultTheme>,
    name: Option<String>,
    windows: Vec<FloatingWindowEntry>,
}

impl FloatingStack {
    pub fn new() -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: None,
            windows: Vec::new(),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_window<W>(mut self, bounds: Rect, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.push_window(bounds, child);
        self
    }

    pub fn push_window<W>(&mut self, bounds: Rect, child: W)
    where
        W: Widget + 'static,
    {
        self.windows.push(FloatingWindowEntry {
            bounds,
            child: WidgetPod::new(FloatingStackSurface::new(child)),
        });
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    fn frontmost_window_at(&self, host_bounds: Rect, position: Point) -> Option<usize> {
        self.windows
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, entry)| {
                entry
                    .bounds
                    .translate(host_bounds.origin.to_vector())
                    .contains(position)
                    .then_some(index)
            })
    }

    fn bring_to_front(&mut self, index: usize) -> bool {
        if index >= self.windows.len() || index + 1 == self.windows.len() {
            return false;
        }

        let entry = self.windows.remove(index);
        self.windows.push(entry);
        true
    }

    fn content_rect(&self) -> Rect {
        self.windows
            .iter()
            .fold(Rect::ZERO, |current, entry| current.union(entry.bounds))
    }
}

impl Default for FloatingStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for FloatingStack {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event {
            if pointer.kind == PointerEventKind::Down
                && pointer.button == Some(PointerButton::Primary)
                && let Some(index) = self.frontmost_window_at(ctx.bounds(), pointer.position)
                && self.bring_to_front(index)
            {
                ctx.request_ordering();
                ctx.request_paint();
                ctx.request_hit_test();
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        for entry in self.windows.iter_mut() {
            entry
                .child
                .measure(ctx, Constraints::tight(entry.bounds.size));
        }

        let content = self.content_rect();
        constraints.clamp(Size::new(
            content.max_x().max(0.0),
            content.max_y().max(0.0),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        for entry in self.windows.iter_mut() {
            entry
                .child
                .arrange(ctx, entry.bounds.translate(bounds.origin.to_vector()));
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        for entry in &self.windows {
            entry.child.paint(ctx);
        }
    }

    fn stack_host_options(&self) -> Option<StackHostOptions> {
        Some(StackHostOptions {
            order_policy: StackOrderPolicy::FocusFronted,
        })
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            self.content_rect()
                .translate(ctx.bounds().origin.to_vector()),
        );
        node.name = self.name.clone();
        node.state.focused = ctx.is_focused();
        ctx.push(node);

        for entry in &self.windows {
            entry.child.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for entry in &self.windows {
            visitor.visit(&entry.child);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for entry in &mut self.windows {
            visitor.visit(&mut entry.child);
        }
    }
}

fn split_child_constraints(axis: Axis, main: f32, cross: f32) -> Constraints {
    match axis {
        Axis::Horizontal => Constraints::tight(Size::new(main.max(0.0), cross.max(0.0))),
        Axis::Vertical => Constraints::tight(Size::new(cross.max(0.0), main.max(0.0))),
    }
}

fn axis_main(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn axis_cross(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

fn axis_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

fn axis_point(axis: Axis, main: f32, cross: f32) -> Point {
    match axis {
        Axis::Horizontal => Point::new(main, cross),
        Axis::Vertical => Point::new(cross, main),
    }
}

fn axis_position(axis: Axis, point: Point) -> f32 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

fn axis_origin(axis: Axis, rect: Rect) -> f32 {
    match axis {
        Axis::Horizontal => rect.x(),
        Axis::Vertical => rect.y(),
    }
}

fn first_size_main(axis: Axis, size: Size) -> f32 {
    axis_main(axis, size)
}

fn floating_view_title_bar_height(theme: &DefaultTheme) -> f32 {
    theme.metrics.floating_view_title_bar_height
}

fn floating_view_title_bar_rect(theme: &DefaultTheme, bounds: Rect) -> Rect {
    Rect::new(
        bounds.x(),
        bounds.y(),
        bounds.width(),
        floating_view_title_bar_height(theme).min(bounds.height()),
    )
}

fn floating_view_content_rect(theme: &DefaultTheme, bounds: Rect, maximized: bool) -> Rect {
    if maximized {
        return bounds;
    }

    let border = theme.metrics.border_width.max(1.0);
    let title_height = floating_view_title_bar_height(theme);
    Rect::new(
        bounds.x() + border,
        bounds.y() + title_height,
        (bounds.width() - (border * 2.0)).max(0.0),
        (bounds.height() - title_height - border).max(0.0),
    )
}

fn floating_view_resize_handle_rect(theme: &DefaultTheme, bounds: Rect) -> Rect {
    let size = theme.metrics.floating_view_resize_handle_size;
    Rect::new(bounds.max_x() - size, bounds.max_y() - size, size, size)
}

fn clamp_floating_view_bounds(
    theme: &DefaultTheme,
    host_bounds: Rect,
    bounds: Rect,
    min_size: Size,
) -> Rect {
    let title_height = floating_view_title_bar_height(theme);
    let max_width = host_bounds.width().max(0.0);
    let max_height = host_bounds.height().max(0.0);
    let width = bounds.width().clamp(
        min_size.width.min(max_width),
        max_width.max(min_size.width.min(max_width)),
    );
    let height = bounds.height().clamp(
        min_size.height.min(max_height),
        max_height.max(min_size.height.min(max_height)),
    );
    let min_visible_width = width.min(56.0);
    let min_visible_height = height.min(title_height.max(32.0));
    let max_x = (host_bounds.max_x() - min_visible_width).max(host_bounds.x());
    let max_y = (host_bounds.max_y() - min_visible_height).max(host_bounds.y());
    Rect::new(
        bounds.x().clamp(host_bounds.x(), max_x),
        bounds.y().clamp(host_bounds.y(), max_y),
        width,
        height,
    )
}

fn resolved_floating_view_bounds(
    theme: &DefaultTheme,
    host_bounds: Rect,
    view: &FloatingViewSnapshot,
) -> Rect {
    if view.maximized {
        host_bounds
    } else {
        clamp_floating_view_bounds(theme, host_bounds, view.bounds, view.min_size)
    }
}

fn diagonal_handle_path(bounds: Rect, inset: f32, offset: f32) -> sui_core::Path {
    let start = Point::new(bounds.max_x() - inset, bounds.max_y() - offset);
    let end = Point::new(bounds.max_x() - offset, bounds.max_y() - inset);
    let mut builder = sui_core::PathBuilder::new();
    builder.move_to(start);
    builder.line_to(end);
    builder.build()
}

fn request_widget_refresh(
    ctx: &mut EventCtx,
    widget_id: WidgetId,
    include_measure: bool,
    include_ordering: bool,
    region: Rect,
) {
    let target = InvalidationTarget::Widget(widget_id);
    if include_measure {
        ctx.request(
            InvalidationRequest::new(target, InvalidationKind::Measure).with_region(region),
        );
    } else {
        ctx.request(
            InvalidationRequest::new(target, InvalidationKind::Arrange).with_region(region),
        );
    }
    if include_ordering {
        ctx.request(
            InvalidationRequest::new(target, InvalidationKind::Ordering).with_region(region),
        );
    }
    ctx.request(InvalidationRequest::new(target, InvalidationKind::Paint).with_region(region));
    ctx.request(InvalidationRequest::new(target, InvalidationKind::HitTest).with_region(region));
    ctx.request(InvalidationRequest::new(target, InvalidationKind::Semantics).with_region(region));
}

fn widget_invalidation_region(
    invalidations: &[InvalidationRequest],
    widget_id: WidgetId,
) -> Option<Rect> {
    invalidations
        .iter()
        .filter(|request| matches!(request.target, InvalidationTarget::Widget(target) if target == widget_id))
        .filter(|request| {
            matches!(
                request.kind,
                InvalidationKind::Measure
                    | InvalidationKind::Arrange
                    | InvalidationKind::Ordering
                    | InvalidationKind::Transform
                    | InvalidationKind::Clip
                    | InvalidationKind::Effect
                    | InvalidationKind::Visibility
                    | InvalidationKind::Paint
            )
        })
        .filter_map(|request| request.region)
        .reduce(|current, next| current.union(next))
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{
        FloatingStack, FloatingViewConfig, FloatingWorkspace, FloatingWorkspaceState, SplitView,
    };
    use crate::DefaultTheme;
    use crate::containers::SizedBox;
    use sui_core::{
        Color, Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect,
        Result, ScrollDelta, SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector, WidgetId,
        WindowEvent,
    };
    use sui_layout::{Axis, Constraints};
    use sui_render_wgpu::{RgbaImage, WgpuRenderer};
    use sui_runtime::{
        Application, EventCtx, MeasureCtx, PaintCtx, RenderOutput, Runtime, SemanticsCtx,
        StackOrderPolicy, Widget, WindowBuilder,
    };
    use sui_scene::{Brush, SceneCommand, SceneLayerUpdateKind};
    use sui_text::{FontRegistry, TextSystem};

    struct ColorFill {
        color: Color,
    }

    impl ColorFill {
        fn new(color: Color) -> Self {
            Self { color }
        }
    }

    impl Widget for ColorFill {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: sui_layout::Constraints) -> Size {
            constraints.max
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(self.color);
        }
    }

    struct ConstraintProbe {
        name: String,
        desired: Size,
        color: Color,
        seen: Rc<RefCell<Vec<Constraints>>>,
    }

    impl ConstraintProbe {
        fn new(
            name: impl Into<String>,
            desired: Size,
            color: Color,
            seen: Rc<RefCell<Vec<Constraints>>>,
        ) -> Self {
            Self {
                name: name.into(),
                desired,
                color,
                seen,
            }
        }
    }

    impl Widget for ConstraintProbe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            self.seen.borrow_mut().push(constraints);
            self.desired
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(self.color);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(self.name.clone());
            ctx.push(node);
        }
    }

    struct PointerDownProbe {
        name: String,
        desired: Size,
        seen: Rc<RefCell<Vec<String>>>,
    }

    impl PointerDownProbe {
        fn new(name: impl Into<String>, seen: Rc<RefCell<Vec<String>>>) -> Self {
            Self {
                name: name.into(),
                desired: Size::new(100.0, 40.0),
                seen,
            }
        }
    }

    impl Widget for PointerDownProbe {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if matches!(
                event,
                Event::Pointer(pointer)
                    if pointer.kind == PointerEventKind::Down
                        && pointer.button == Some(PointerButton::Primary)
            ) {
                self.seen.borrow_mut().push("down".to_string());
                ctx.set_handled();
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(self.desired)
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::rgba(0.16, 0.36, 0.72, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(self.name.clone());
            ctx.push(node);
        }
    }

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("SplitView").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

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
            modifiers: sui_core::Modifiers::NONE,
            pointer_kind: sui_core::PointerKind::Mouse,
            is_primary: true,
        })
    }

    fn handle_ready_events(runtime: &mut Runtime) -> Result<usize> {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (window_id, event) in ready {
            runtime.handle_event(window_id, event)?;
        }
        Ok(count)
    }

    fn solid_fill_colors(output: &RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::FillRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::FillPath {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                colors.push(*color);
            }
        });
        colors
    }

    fn render_rgba(
        runtime: &mut Runtime,
        renderer: &mut WgpuRenderer,
        window_id: sui_core::WindowId,
    ) -> Result<(RenderOutput, RgbaImage)> {
        let output = runtime.render(window_id)?;
        renderer.render(&output.frame)?;
        let image = renderer.capture_last_frame_rgba(window_id)?;
        Ok((output, image))
    }

    fn rgba_pixel(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
        let index = ((y as usize * image.width() as usize) + x as usize) * 4;
        let pixels = image.pixels();
        [
            pixels[index],
            pixels[index + 1],
            pixels[index + 2],
            pixels[index + 3],
        ]
    }

    fn text_run_for(output: &RenderOutput, text: &str) -> sui_text::TextRun {
        let mut found = None;
        output.frame.scene.visit_commands(&mut |command| {
            if found.is_some() {
                return;
            }
            found = match command {
                SceneCommand::DrawText(run) if run.text == text => Some(run.clone()),
                SceneCommand::DrawShapedText(run) => run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == text)
                    .map(|layout| sui_text::TextRun {
                        rect: Rect::new(
                            run.origin.x,
                            run.origin.y,
                            layout.box_size().width,
                            layout.box_size().height,
                        ),
                        text: layout.text().to_string(),
                        style: layout.style().clone(),
                    }),
                _ => None,
            };
        });
        found.expect("text draw command present")
    }

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        let top = -measurement.cap_height.unwrap_or(measurement.ascent);
        let bottom = measurement.descent * 0.5;
        (top + bottom) * 0.5
    }

    fn scene_layer_widget_ids(scene: &sui_scene::Scene) -> Vec<WidgetId> {
        scene
            .commands()
            .iter()
            .filter_map(|command| match command {
                SceneCommand::Layer(layer) => Some(layer.widget_id()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn split_view_drag_updates_ratio_and_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            SplitView::new(
                Axis::Horizontal,
                SizedBox::new().width(100.0).height(40.0),
                SizedBox::new().width(100.0).height(40.0),
            )
            .name("Editor split")
            .min_first(40.0)
            .min_second(40.0)
            .on_change(move |ratio| on_change.borrow_mut().push(ratio)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(105.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(145.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(145.0, 20.0), false),
        )?;

        assert!(changes.borrow().last().is_some_and(|ratio| *ratio > 0.65));

        let output = runtime.render(window_id)?;
        let splitter = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Splitter)
            .expect("splitter semantics present");
        assert_eq!(splitter.name.as_deref(), Some("Editor split"));
        assert!(matches!(
            splitter.value,
            Some(SemanticsValue::Number(value)) if value > 0.65
        ));
        Ok(())
    }

    #[test]
    fn split_view_divider_hover_and_drag_use_theme_motion() -> Result<()> {
        let theme = DefaultTheme::default();
        let hover_duration = theme.motion.hover_duration();
        let press_duration = theme.motion.press_duration();
        let expected_hover = theme.palette.border_hover;
        let expected_drag = theme.palette.border_focus;
        let divider_x = 100.0 + theme.metrics.split_view_divider_thickness * 0.5;
        let divider_point = Point::new(divider_x, 20.0);
        let (mut runtime, window_id) = build_runtime(
            SplitView::new(
                Axis::Horizontal,
                SizedBox::new().width(100.0).height(40.0),
                SizedBox::new().width(100.0).height(40.0),
            )
            .theme(theme)
            .min_first(40.0)
            .min_second(40.0),
        );

        runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, divider_point, false),
        )?;
        runtime.tick(hover_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_hover = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_hover).contains(&expected_hover),
            "split divider hover should not snap to the settled hover color"
        );

        runtime.tick(hover_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_hover = runtime.render(window_id)?;
        assert!(solid_fill_colors(&settled_hover).contains(&expected_hover));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, divider_point, true),
        )?;
        runtime.tick(hover_duration + press_duration * 0.5);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let mid_drag = runtime.render(window_id)?;
        assert!(
            !solid_fill_colors(&mid_drag).contains(&expected_drag),
            "split divider drag should not snap to the settled drag color"
        );

        runtime.tick(hover_duration + press_duration);
        assert_eq!(handle_ready_events(&mut runtime)?, 1);
        let settled_drag = runtime.render(window_id)?;
        assert!(solid_fill_colors(&settled_drag).contains(&expected_drag));

        Ok(())
    }

    #[test]
    fn split_view_allows_resize_below_combined_minimums() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            SplitView::new(
                Axis::Horizontal,
                SizedBox::new().width(320.0).height(180.0),
                SizedBox::new().width(320.0).height(180.0),
            )
            .min_first(236.0)
            .min_second(420.0)
            .divider_thickness(12.0),
        );

        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(664.0, 360.0))),
        )?;

        let output = runtime.render(window_id)?;

        assert_eq!(output.frame.viewport, Size::new(664.0, 360.0));
        Ok(())
    }

    #[test]
    fn split_view_default_divider_uses_thin_layout_gap() -> Result<()> {
        let first_constraints = Rc::new(RefCell::new(Vec::new()));
        let second_constraints = Rc::new(RefCell::new(Vec::new()));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(240.0).height(100.0).with_child(
                SplitView::new(
                    Axis::Horizontal,
                    ConstraintProbe::new(
                        "First pane",
                        Size::new(400.0, 100.0),
                        Color::rgba(0.22, 0.48, 0.72, 1.0),
                        Rc::clone(&first_constraints),
                    ),
                    ConstraintProbe::new(
                        "Second pane",
                        Size::new(400.0, 100.0),
                        Color::rgba(0.72, 0.48, 0.22, 1.0),
                        Rc::clone(&second_constraints),
                    ),
                )
                .min_first(40.0)
                .min_second(40.0),
            ),
        );

        let _ = runtime.render(window_id)?;

        let pane_width =
            (240.0 - DefaultTheme::default().metrics.split_view_divider_thickness) * 0.5;
        let expected = Constraints::tight(Size::new(pane_width, 100.0));
        assert_eq!(first_constraints.borrow().last(), Some(&expected));
        assert_eq!(second_constraints.borrow().last(), Some(&expected));
        Ok(())
    }

    #[test]
    fn pane_chrome_defaults_follow_theme_density() -> Result<()> {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        let (mut compact_runtime, compact_window) = build_runtime(
            SplitView::new(
                Axis::Horizontal,
                SizedBox::new().width(80.0).height(40.0),
                SizedBox::new().width(80.0).height(40.0),
            )
            .theme(compact)
            .min_first(0.0)
            .min_second(0.0),
        );
        let compact_size = compact_runtime.render(compact_window)?.frame.viewport;

        let (mut touch_runtime, touch_window) = build_runtime(
            SplitView::new(
                Axis::Horizontal,
                SizedBox::new().width(80.0).height(40.0),
                SizedBox::new().width(80.0).height(40.0),
            )
            .theme(touch)
            .min_first(0.0)
            .min_second(0.0),
        );
        let touch_size = touch_runtime.render(touch_window)?.frame.viewport;

        assert_eq!(
            compact_size.width,
            160.0 + compact.metrics.split_view_divider_thickness
        );
        assert_eq!(
            touch_size.width,
            160.0 + touch.metrics.split_view_divider_thickness
        );
        assert!(touch_size.width > compact_size.width);

        let bounds = Rect::new(0.0, 0.0, 240.0, 180.0);
        assert_eq!(
            super::floating_view_title_bar_rect(&compact, bounds).height(),
            compact.metrics.floating_view_title_bar_height
        );
        assert_eq!(
            super::floating_view_title_bar_rect(&touch, bounds).height(),
            touch.metrics.floating_view_title_bar_height
        );
        assert!(
            super::floating_view_content_rect(&touch, bounds, false).y()
                > super::floating_view_content_rect(&compact, bounds, false).y()
        );

        let compact_handle = super::floating_view_resize_handle_rect(&compact, bounds);
        let touch_handle = super::floating_view_resize_handle_rect(&touch, bounds);
        assert_eq!(
            compact_handle.size,
            Size::new(
                compact.metrics.floating_view_resize_handle_size,
                compact.metrics.floating_view_resize_handle_size,
            )
        );
        assert_eq!(
            touch_handle.size,
            Size::new(
                touch.metrics.floating_view_resize_handle_size,
                touch.metrics.floating_view_resize_handle_size,
            )
        );
        assert!(touch_handle.width() > compact_handle.width());
        Ok(())
    }

    #[test]
    fn split_view_measures_children_with_resolved_pane_constraints() -> Result<()> {
        let first_constraints = Rc::new(RefCell::new(Vec::new()));
        let second_constraints = Rc::new(RefCell::new(Vec::new()));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(240.0).height(100.0).with_child(
                SplitView::new(
                    Axis::Horizontal,
                    ConstraintProbe::new(
                        "First pane",
                        Size::new(400.0, 100.0),
                        Color::rgba(0.22, 0.48, 0.72, 1.0),
                        Rc::clone(&first_constraints),
                    ),
                    ConstraintProbe::new(
                        "Second pane",
                        Size::new(400.0, 100.0),
                        Color::rgba(0.72, 0.48, 0.22, 1.0),
                        Rc::clone(&second_constraints),
                    ),
                )
                .min_first(40.0)
                .min_second(40.0)
                .divider_thickness(8.0),
            ),
        );

        let _ = runtime.render(window_id)?;

        let expected = Constraints::tight(Size::new(116.0, 100.0));
        assert_eq!(
            first_constraints.borrow().last(),
            Some(&expected),
            "first pane should be remeasured with the resolved pane width"
        );
        assert_eq!(
            second_constraints.borrow().last(),
            Some(&expected),
            "second pane should be remeasured with the resolved pane width"
        );
        Ok(())
    }

    #[test]
    fn split_view_extra_drag_target_defers_to_child_hit() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let child_events = Rc::new(RefCell::new(Vec::new()));
        let (mut runtime, window_id) = build_runtime(
            SplitView::new(
                Axis::Horizontal,
                SizedBox::new().width(100.0).height(40.0),
                PointerDownProbe::new("Second pane target", Rc::clone(&child_events)),
            )
            .min_first(40.0)
            .min_second(40.0)
            .on_change(move |ratio| on_change.borrow_mut().push(ratio)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(105.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(145.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(145.0, 20.0), false),
        )?;

        assert_eq!(child_events.borrow().as_slice(), ["down"]);
        assert!(
            changes.borrow().is_empty(),
            "splitter should not drag when a child handles the enlarged hit area"
        );
        Ok(())
    }

    #[test]
    fn floating_stack_reorders_host_surfaces_on_pointer_focus() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            FloatingStack::new()
                .name("floating workspace")
                .with_window(
                    Rect::new(0.0, 0.0, 120.0, 80.0),
                    SizedBox::new().width(120.0).height(80.0),
                )
                .with_window(
                    Rect::new(48.0, 0.0, 120.0, 80.0),
                    SizedBox::new().width(120.0).height(80.0),
                ),
        );

        let before = runtime.render(window_id)?;
        let before_graph = runtime.widget_graph(window_id)?;
        let host = before_graph
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should be present");
        assert_eq!(host.surfaces.len(), 2);
        let layer_widget_ids = scene_layer_widget_ids(&before.frame.scene);
        assert!(
            host.surfaces
                .iter()
                .all(|surface| layer_widget_ids.contains(surface))
        );
        let first_surface = host.surfaces[0];

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        let reordered = runtime.render(window_id)?;

        let after = runtime.widget_graph(window_id)?;
        let host = after
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should still be present");
        assert_eq!(host.surfaces.len(), 2);
        assert_eq!(host.surfaces[1], first_surface);
        assert!(
            reordered
                .frame
                .layer_updates
                .iter()
                .any(|update| update.kind == SceneLayerUpdateKind::Ordering)
        );
        Ok(())
    }

    #[test]
    fn floating_workspace_drag_updates_view_bounds() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let mut workspace = FloatingWorkspace::new(state.clone());
        let first_id = workspace.push_view(
            FloatingViewConfig::new("First", Rect::new(16.0, 16.0, 180.0, 140.0))
                .min_size(Size::new(140.0, 110.0)),
            SizedBox::new().width(180.0).height(140.0),
        );
        workspace.push_view(
            FloatingViewConfig::new("Second", Rect::new(240.0, 48.0, 180.0, 140.0))
                .min_size(Size::new(140.0, 110.0)),
            SizedBox::new().width(180.0).height(140.0),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(48.0, 32.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(132.0, 96.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(132.0, 96.0), false),
        )?;

        let moved = state.snapshot(first_id).expect("first view state present");
        assert!(moved.bounds.x() > 72.0);
        assert!(moved.bounds.y() > 56.0);
        Ok(())
    }

    #[test]
    fn floating_workspace_resize_drag_remeasures_content_before_drop() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let seen_constraints = Rc::new(RefCell::new(Vec::new()));
        let mut workspace = FloatingWorkspace::new(state.clone());
        let view_id = workspace.push_view(
            FloatingViewConfig::new("Inspector", Rect::new(24.0, 24.0, 240.0, 180.0)),
            ConstraintProbe::new(
                "Resizable content",
                Size::new(420.0, 300.0),
                Color::rgba(0.22, 0.48, 0.72, 1.0),
                Rc::clone(&seen_constraints),
            ),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );
        let _ = runtime.render(window_id)?;
        let initial_count = seen_constraints.borrow().len();
        let initial_constraints = *seen_constraints
            .borrow()
            .last()
            .expect("content should be measured initially");
        let initial_bounds = state.snapshot(view_id).expect("view state present").bounds;
        let handle =
            super::floating_view_resize_handle_rect(&DefaultTheme::default(), initial_bounds);
        let resize_start = Point::new(
            handle.x() + handle.width() * 0.5,
            handle.y() + handle.height() * 0.5,
        );
        let resize_end = Point::new(resize_start.x + 72.0, resize_start.y + 48.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, resize_start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, resize_end, true),
        )?;
        let _ = runtime.render(window_id)?;

        let seen = seen_constraints.borrow();
        let latest = seen
            .last()
            .copied()
            .expect("content should be measured after resize move");
        assert!(seen.len() > initial_count);
        assert!(latest.max.width > initial_constraints.max.width + 40.0);
        assert!(latest.max.height > initial_constraints.max.height + 20.0);

        Ok(())
    }

    #[test]
    fn floating_workspace_body_click_brings_view_to_front() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let mut workspace = FloatingWorkspace::new(state.clone());
        let first_id = workspace.push_view(
            FloatingViewConfig::new("First", Rect::new(16.0, 16.0, 180.0, 140.0)),
            crate::Button::new("First action"),
        );
        workspace.push_view(
            FloatingViewConfig::new("Second", Rect::new(120.0, 40.0, 180.0, 140.0)),
            crate::Button::new("Second action"),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(420.0)
                .height(260.0)
                .with_child(workspace),
        );

        let _ = runtime.render(window_id)?;
        let first_surface = state
            .snapshot(first_id)
            .and_then(|view| view.surface_widget_id)
            .expect("first view surface should be registered");
        let before = runtime.widget_graph(window_id)?;
        let host = before
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should be present");
        assert_eq!(host.surfaces.len(), 2);
        assert_eq!(host.surfaces[0], first_surface);
        assert_ne!(host.surfaces[1], first_surface);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(44.0, 88.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(44.0, 88.0), false),
        )?;

        let reordered = runtime.render(window_id)?;
        let after = runtime.widget_graph(window_id)?;
        let host = after
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should still be present");
        assert_eq!(host.surfaces.len(), 2);
        assert_eq!(host.surfaces[1], first_surface);
        assert!(
            reordered
                .frame
                .layer_updates
                .iter()
                .any(|update| update.kind == SceneLayerUpdateKind::Ordering)
        );
        Ok(())
    }

    #[test]
    fn floating_workspace_view_content_receives_size_hint_and_scroll_bars() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let seen_constraints = Rc::new(RefCell::new(Vec::new()));
        let mut workspace = FloatingWorkspace::new(state.clone());
        let view_id = workspace.push_view(
            FloatingViewConfig::new("Inspector", Rect::new(24.0, 24.0, 240.0, 180.0)),
            ConstraintProbe::new(
                "Overflowing content",
                Size::new(420.0, 300.0),
                Color::rgba(0.22, 0.48, 0.72, 1.0),
                Rc::clone(&seen_constraints),
            ),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );
        let output = runtime.render(window_id)?;
        let snapshot = state.snapshot(view_id).expect("view state present");
        let content = super::floating_view_content_rect(
            &crate::DefaultTheme::default(),
            snapshot.bounds,
            snapshot.maximized,
        );
        let hinted = seen_constraints
            .borrow()
            .last()
            .copied()
            .expect("content should be measured");

        assert_eq!(hinted.max, content.size);

        let vertical = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some("Vertical scroll bar")
            })
            .expect("vertical scroll bar semantics present");
        let horizontal = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider
                    && node.name.as_deref() == Some("Horizontal scroll bar")
            })
            .expect("horizontal scroll bar semantics present");

        assert!(!vertical.state.disabled);
        assert!(!horizontal.state.disabled);
        assert!(matches!(
            vertical.value,
            Some(SemanticsValue::Range { max, .. }) if max > 0.0
        ));
        assert!(matches!(
            horizontal.value,
            Some(SemanticsValue::Range { max, .. }) if max > 0.0
        ));
        Ok(())
    }

    #[test]
    fn floating_workspace_active_resize_clips_preview_without_host_scroll_bars() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let seen_constraints = Rc::new(RefCell::new(Vec::new()));
        let mut workspace = FloatingWorkspace::new(state.clone());
        let view_id = workspace.push_view(
            FloatingViewConfig::new("Inspector", Rect::new(24.0, 24.0, 240.0, 180.0)),
            ConstraintProbe::new(
                "Overflowing content",
                Size::new(420.0, 300.0),
                Color::rgba(0.22, 0.48, 0.72, 1.0),
                Rc::clone(&seen_constraints),
            ),
        );
        state.set_active_resize_view(Some(view_id));

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );
        let output = runtime.render(window_id)?;

        assert!(output.semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Overflowing content")
        }));
        assert!(!output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Slider
                && matches!(
                    node.name.as_deref(),
                    Some("Vertical scroll bar" | "Horizontal scroll bar")
                )
        }));
        Ok(())
    }

    #[test]
    fn floating_workspace_view_allows_horizontal_scrolling() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let seen_constraints = Rc::new(RefCell::new(Vec::new()));
        let mut workspace = FloatingWorkspace::new(state.clone());
        workspace.push_view(
            FloatingViewConfig::new("Inspector", Rect::new(24.0, 24.0, 240.0, 180.0)),
            ConstraintProbe::new(
                "Wide content",
                Size::new(520.0, 120.0),
                Color::rgba(0.22, 0.48, 0.72, 1.0),
                seen_constraints,
            ),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );
        let before = runtime.render(window_id)?;
        let before_content_id = before
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Wide content"))
            .map(|node| node.id)
            .expect("content semantics present");
        let before_graph = runtime.widget_graph(window_id)?;
        let before_bounds = before_graph
            .nodes
            .iter()
            .find(|node| node.id == before_content_id)
            .map(|node| node.bounds)
            .expect("content graph node present");

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(72.0, 96.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(-80.0, 0.0)));
        runtime.handle_event(window_id, Event::Pointer(scroll))?;

        let after = runtime.render(window_id)?;
        let after_content_id = after
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Wide content"))
            .map(|node| node.id)
            .expect("content semantics present after scroll");
        let after_graph = runtime.widget_graph(window_id)?;
        let after_bounds = after_graph
            .nodes
            .iter()
            .find(|node| node.id == after_content_id)
            .map(|node| node.bounds)
            .expect("content graph node present after scroll");

        assert!(after_bounds.x() < before_bounds.x() - 40.0);
        Ok(())
    }

    #[test]
    fn floating_workspace_title_label_visual_center_matches_title_slot_center() -> Result<()> {
        let default_theme = DefaultTheme::default();
        let mut constrained_theme = default_theme;
        constrained_theme.metrics.floating_view_title_padding = sui_layout::Padding {
            left: 10.0,
            top: 9.0,
            right: 10.0,
            bottom: 9.0,
        };
        let state = FloatingWorkspaceState::new();
        let mut workspace = FloatingWorkspace::new(state.clone()).theme(constrained_theme);
        let view_bounds = Rect::new(24.0, 24.0, 240.0, 180.0);
        workspace.push_view(
            FloatingViewConfig::new("Inspector", view_bounds),
            ColorFill::new(Color::rgba(0.22, 0.48, 0.72, 1.0)),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );
        let output = runtime.render(window_id)?;
        let text = text_run_for(&output, "Inspector");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("floating view title should shape");
        let line = layout
            .lines()
            .first()
            .expect("floating view title should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let title_bar = super::floating_view_title_bar_rect(&constrained_theme, view_bounds);
        let padding = constrained_theme.metrics.floating_view_title_padding;
        let title_slot = Rect::new(
            title_bar.x() + padding.left,
            title_bar.y() + padding.top,
            (title_bar.width() - padding.left - padding.right).max(0.0),
            (title_bar.height() - padding.top - padding.bottom).max(0.0),
        );
        let slot_center = title_slot.y() + (title_slot.height() * 0.5);

        assert!(
            (actual_visual_center - slot_center).abs() <= 0.75,
            "floating title visual center {actual_visual_center} did not match slot center {slot_center}; text rect {:?}, title slot {:?}",
            text.rect,
            title_slot
        );
        assert_eq!(
            text.style.line_height,
            default_theme
                .text_style(default_theme.palette.text)
                .line_height
        );
        Ok(())
    }

    #[test]
    fn floating_workspace_title_label_preserves_tall_measurement() -> Result<()> {
        let mut theme = DefaultTheme::default();
        theme.typography.body_font_size = 28.0;
        theme.typography.body_line_height = 12.0;
        theme.metrics.floating_view_title_bar_height = 68.0;
        theme.metrics.floating_view_title_padding = sui_layout::Padding {
            left: 12.0,
            top: 8.0,
            right: 12.0,
            bottom: 8.0,
        };

        let state = FloatingWorkspaceState::new();
        let mut workspace = FloatingWorkspace::new(state.clone()).theme(theme);
        let view_bounds = Rect::new(24.0, 24.0, 260.0, 190.0);
        workspace.push_view(
            FloatingViewConfig::new("Inspector", view_bounds),
            ColorFill::new(Color::rgba(0.22, 0.48, 0.72, 1.0)),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );
        let output = runtime.render(window_id)?;
        let text = text_run_for(&output, "Inspector");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("floating view title should shape");
        let line = layout
            .lines()
            .first()
            .expect("floating view title should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());
        let title_bar = super::floating_view_title_bar_rect(&theme, view_bounds);
        let padding = theme.metrics.floating_view_title_padding;
        let title_slot = Rect::new(
            title_bar.x() + padding.left,
            title_bar.y() + padding.top,
            (title_bar.width() - padding.left - padding.right).max(0.0),
            (title_bar.height() - padding.top - padding.bottom).max(0.0),
        );
        let slot_center = title_slot.y() + (title_slot.height() * 0.5);

        assert_eq!(text.style.font_size, 28.0);
        assert_eq!(text.style.line_height, 12.0);
        assert!(text.rect.height() >= layout.measurement().height - 0.01);
        assert!(text.rect.height() > text.style.line_height);
        assert!((text.rect.x() - title_slot.x()).abs() < 0.75);
        assert!((actual_visual_center - slot_center).abs() <= 0.75);
        Ok(())
    }

    #[test]
    fn floating_workspace_title_bar_fronting_updates_renderer_output() -> Result<()> {
        let first_color = Color::rgba(0.82, 0.36, 0.18, 1.0);
        let second_color = Color::rgba(0.18, 0.54, 0.82, 1.0);
        let state = FloatingWorkspaceState::new();
        let mut workspace = FloatingWorkspace::new(state.clone());
        let first_id = workspace.push_view(
            FloatingViewConfig::new("First", Rect::new(16.0, 16.0, 180.0, 140.0)),
            ColorFill::new(first_color),
        );
        workspace.push_view(
            FloatingViewConfig::new("Second", Rect::new(120.0, 40.0, 180.0, 140.0)),
            ColorFill::new(second_color),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(420.0)
                .height(260.0)
                .with_child(workspace),
        );
        let mut renderer = WgpuRenderer::default();

        let (_, initial_image) = render_rgba(&mut runtime, &mut renderer, window_id)?;
        let initial_pixel = rgba_pixel(&initial_image, 140, 100);
        assert!(initial_pixel[2] > initial_pixel[0]);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(44.0, 32.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(44.0, 32.0), false),
        )?;

        let (reordered, reordered_image) = render_rgba(&mut runtime, &mut renderer, window_id)?;
        let host = runtime
            .widget_graph(window_id)?
            .stack_hosts
            .into_iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should still be present");
        let first_surface = state
            .snapshot(first_id)
            .and_then(|view| view.surface_widget_id)
            .expect("first view surface should be registered");
        assert_eq!(host.surfaces.last().copied(), Some(first_surface));
        assert!(
            reordered
                .frame
                .layer_updates
                .iter()
                .any(|update| update.kind == SceneLayerUpdateKind::Ordering)
        );
        let reordered_pixel = rgba_pixel(&reordered_image, 140, 100);
        assert!(reordered_pixel[0] > reordered_pixel[2]);
        Ok(())
    }

    #[test]
    fn floating_workspace_title_bar_drag_keeps_fronted_view_visually_on_top() -> Result<()> {
        let first_color = Color::rgba(0.82, 0.36, 0.18, 1.0);
        let second_color = Color::rgba(0.18, 0.54, 0.82, 1.0);
        let state = FloatingWorkspaceState::new();
        let mut workspace = FloatingWorkspace::new(state.clone());
        let first_id = workspace.push_view(
            FloatingViewConfig::new("First", Rect::new(16.0, 16.0, 180.0, 140.0)),
            ColorFill::new(first_color),
        );
        workspace.push_view(
            FloatingViewConfig::new("Second", Rect::new(120.0, 40.0, 180.0, 140.0)),
            ColorFill::new(second_color),
        );

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(420.0)
                .height(260.0)
                .with_child(workspace),
        );
        let mut renderer = WgpuRenderer::default();

        let (_, initial_image) = render_rgba(&mut runtime, &mut renderer, window_id)?;
        let initial_pixel = rgba_pixel(&initial_image, 140, 100);
        assert!(initial_pixel[2] > initial_pixel[0]);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(44.0, 32.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(52.0, 36.0), true),
        )?;

        let (dragging, dragging_image) = render_rgba(&mut runtime, &mut renderer, window_id)?;
        let host = runtime
            .widget_graph(window_id)?
            .stack_hosts
            .into_iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should still be present");
        let first_surface = state
            .snapshot(first_id)
            .and_then(|view| view.surface_widget_id)
            .expect("first view surface should be registered");
        assert_eq!(host.surfaces.last().copied(), Some(first_surface));
        assert!(
            dragging
                .frame
                .layer_updates
                .iter()
                .any(|update| update.kind == SceneLayerUpdateKind::Ordering)
        );
        let dragging_pixel = rgba_pixel(&dragging_image, 140, 100);
        assert!(dragging_pixel[0] > dragging_pixel[2]);
        Ok(())
    }

    #[test]
    fn floating_workspace_popover_resolves_to_nearest_view_host() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new().width(520.0).height(360.0).with_child(
                FloatingWorkspace::new(state)
                    .with_view(
                        FloatingViewConfig::new(
                            "Popover view",
                            Rect::new(24.0, 24.0, 240.0, 180.0),
                        ),
                        crate::Padding::all(
                            16.0,
                            crate::Popover::new(
                                "Options",
                                crate::Button::new("Open"),
                                crate::Label::new("Popover body"),
                            )
                            .open(true),
                        ),
                    )
                    .with_view(
                        FloatingViewConfig::new("Inspector", Rect::new(292.0, 64.0, 180.0, 140.0)),
                        SizedBox::new().width(180.0).height(140.0),
                    ),
            ),
        );

        let output = runtime.render(window_id)?;
        let graph = runtime.widget_graph(window_id)?;
        let popover = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Popover)
            .expect("popover semantics present");
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == popover.id)
            .expect("popover graph node present");

        assert_ne!(node.stack_host, graph.root);
        assert!(
            graph
                .stack_hosts
                .iter()
                .any(|host| host.host == node.stack_host)
        );
        Ok(())
    }

    #[test]
    fn floating_workspace_maximize_limits_workspace_host_surfaces() -> Result<()> {
        let state = FloatingWorkspaceState::new();
        let mut workspace = FloatingWorkspace::new(state.clone());
        workspace.push_view(
            FloatingViewConfig::new("First", Rect::new(16.0, 16.0, 180.0, 140.0)),
            SizedBox::new().width(180.0).height(140.0),
        );
        let second_id = workspace.push_view(
            FloatingViewConfig::new("Second", Rect::new(200.0, 32.0, 180.0, 140.0)),
            SizedBox::new().width(180.0).height(140.0),
        );
        state.set_view_maximized(second_id, true);

        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .width(520.0)
                .height(360.0)
                .with_child(workspace),
        );

        let _ = runtime.render(window_id)?;
        let graph = runtime.widget_graph(window_id)?;
        let host = graph
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("workspace host present");

        assert_eq!(host.surfaces.len(), 1);
        Ok(())
    }
}
