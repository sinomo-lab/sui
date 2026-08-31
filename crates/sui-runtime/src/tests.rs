use std::{
    cell::RefCell,
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::{
    Application, ArrangeCtx, Command, CommandController, CommandCtx, CommandKey, CommandTarget,
    EventCtx, EventPhase, EventRoutePhase, FocusScope, FocusScopeState, FocusState, FrameSchedule,
    LayerOptions, MeasureCtx, OVERLAY_DISMISS_REQUEST, OverlayDismissReason, OverlayFocusBehavior,
    OverlayKind, OverlayOptions, OverlayTraceKind, PaintBoundaryMode, PaintCtx, RenderOutput,
    Runtime, SceneStatisticsDetailMode, SemanticsCtx, SingleChild, StackSurfaceOptions, Widget,
    WidgetChildren, WidgetDiagnosticsCtx, WidgetGraphSnapshot, WidgetNodeSnapshot,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder, WindowIcon, WindowRenderOptions,
    set_window_render_options, set_window_scene_statistics_detail_mode, window_render_options,
};
use sui_core::{
    AsyncWakeToken, Color, CursorGrabMode, CustomEvent, DragEventKind, DragOutcome, DragPayload,
    DragScopeId, DragSessionId, DropEffect, Event, FontHandle, ImageHandle, InvalidationKind,
    KeyState, KeyboardEvent, Modifiers, Point, PointerButton, PointerButtons, PointerEvent,
    PointerEventKind, PointerKind, RawMouseMotionEvent, Rect, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsValue, Size, TimerToken,
    Transform, Vector, WakeEvent, WidgetId, WindowEvent,
};
use sui_layout::Constraints;
use sui_reactive::Signal;
use sui_scene::{
    LayerCompositionMode, LayerProperties, RegisteredExternalImage, RegisteredImage, Scene,
    SceneCommand, SceneLayerUpdateKind,
};
use sui_text::{PersistentTextLayout, RegisteredFont, TextStyle, TextSystem};

#[derive(Default)]
struct Counters {
    paint: usize,
    semantics: usize,
    keyboard: usize,
    focus_changes: usize,
}

struct FocusLeaf {
    counters: Rc<RefCell<Counters>>,
}

struct WindowEventRecorder {
    events: Rc<RefCell<Vec<WindowEvent>>>,
}

impl Widget for WindowEventRecorder {
    fn event(&mut self, _ctx: &mut EventCtx, event: &Event) {
        if let Event::Window(event) = event {
            self.events.borrow_mut().push(event.clone());
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 80.0))
    }
}

const SYNTHETIC_ACTION_ID: WidgetId = WidgetId::new(u64::MAX - 1);

#[derive(Debug, Default)]
struct SemanticActionState {
    actions: Vec<(WidgetId, SemanticsActionRequest)>,
    pointer_activations: usize,
    disabled: bool,
    expanded: bool,
}

struct SyntheticSemanticActionLeaf {
    state: Rc<RefCell<SemanticActionState>>,
}

struct UnfocusableSemanticLeaf;

impl Widget for UnfocusableSemanticLeaf {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some("unfocusable semantic node".to_string());
        node.actions = vec![SemanticsAction::Focus];
        ctx.push(node);
    }
}

impl Widget for SyntheticSemanticActionLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Semantics(event) => {
                self.state
                    .borrow_mut()
                    .actions
                    .push((event.target, event.action.clone()));
                if !matches!(
                    event.action,
                    SemanticsActionRequest::Activate
                        | SemanticsActionRequest::Blur
                        | SemanticsActionRequest::Expand
                        | SemanticsActionRequest::Collapse
                ) {
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Up => {
                let mut state = self.state.borrow_mut();
                state.pointer_activations += 1;
                state.expanded = !state.expanded;
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(160.0, 64.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(Color::rgba(0.16, 0.24, 0.34, 1.0));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut owner = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        owner.name = Some("semantic action owner".to_string());
        ctx.push(owner);

        let mut action = SemanticsNode::new(
            SYNTHETIC_ACTION_ID,
            SemanticsRole::Button,
            Rect::new(ctx.bounds().x() + 8.0, ctx.bounds().y() + 8.0, 80.0, 32.0),
        );
        action.parent = Some(ctx.widget_id());
        action.name = Some("virtual action".to_string());
        let state = self.state.borrow();
        action.state.disabled = state.disabled;
        action.state.expanded = Some(state.expanded);
        action.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Blur,
            SemanticsAction::Activate,
            SemanticsAction::Expand,
            SemanticsAction::Collapse,
            SemanticsAction::SetValue,
            SemanticsAction::InsertText,
        ];
        ctx.push(action);
    }

    fn accepts_focus(&self) -> bool {
        true
    }
}

impl Widget for FocusLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Keyboard(_) = event {
            self.counters.borrow_mut().keyboard += 1;
            ctx.set_handled();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.counters.borrow_mut().paint += 1;
        ctx.fill_bounds(Color::rgba(0.22, 0.31, 0.42, 1.0));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.counters.borrow_mut().semantics += 1;
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some("focus-leaf".to_string());
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Blur];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        self.counters.borrow_mut().focus_changes += 1;
        ctx.request_paint_rect(ctx.bounds());
        ctx.request_semantics();
    }
}

#[test]
fn focus_scope_restores_the_last_focused_descendant() {
    let state = FocusScopeState::new();
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new().title("Focus Scope").root(
                FocusScope::new(FocusLeaf {
                    counters: Rc::new(RefCell::new(Counters::default())),
                })
                .state(state.clone()),
            ),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let target = runtime
        .render(window_id)
        .unwrap()
        .semantics
        .into_iter()
        .find(|node| node.name.as_deref() == Some("focus-leaf"))
        .unwrap()
        .id;

    assert!(
        runtime
            .handle_semantics_action(window_id, target, SemanticsActionRequest::Focus)
            .unwrap()
    );
    let _ = runtime.render(window_id).unwrap();
    assert_eq!(state.last_focused(), Some(target));
    assert!(
        runtime
            .handle_semantics_action(window_id, target, SemanticsActionRequest::Blur)
            .unwrap()
    );
    assert_eq!(runtime.focused_widget(window_id).unwrap(), None);

    state.request_restore();
    let _ = runtime.render(window_id).unwrap();
    let ready = runtime.drain_ready_events();
    assert_eq!(ready.len(), 1);
    for (ready_window, event) in ready {
        runtime.handle_event(ready_window, event).unwrap();
    }

    assert_eq!(runtime.focused_widget(window_id).unwrap(), Some(target));
}

struct TestRoot {
    counters: Rc<RefCell<Counters>>,
    child: SingleChild,
}

impl Widget for TestRoot {
    fn diagnostics(&self, ctx: &mut WidgetDiagnosticsCtx) {
        ctx.record("test state", "available");
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event
            && custom.kind == "semantics-only"
        {
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.child
            .measure(ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.counters.borrow_mut().paint += 1;
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.counters.borrow_mut().semantics += 1;
        ctx.push(SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Window,
            ctx.bounds(),
        ));
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

#[derive(Debug, Default)]
struct VirtualizedPaintState {
    painted: Vec<usize>,
}

struct VirtualizedLeaf {
    index: usize,
    state: Rc<RefCell<VirtualizedPaintState>>,
}

impl Widget for VirtualizedLeaf {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(48.0, 24.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.state.borrow_mut().painted.push(self.index);
        ctx.fill_bounds(Color::rgba(0.18, 0.31, 0.42, 1.0));
    }
}

struct VirtualizedLogicalRoot {
    children: WidgetChildren,
    visible_count: usize,
}

impl VirtualizedLogicalRoot {
    fn new(
        logical_count: usize,
        visible_count: usize,
        state: Rc<RefCell<VirtualizedPaintState>>,
    ) -> Self {
        let mut children = WidgetChildren::with_capacity(logical_count);
        for index in 0..logical_count {
            children.push(VirtualizedLeaf {
                index,
                state: Rc::clone(&state),
            });
        }
        Self {
            children,
            visible_count,
        }
    }
}

impl Widget for VirtualizedLogicalRoot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        for index in 0..self.children.len() {
            self.children
                .measure_child(index, ctx, Constraints::tight(Size::new(48.0, 24.0)));
        }
        constraints.clamp(Size::new(320.0, 180.0))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        for index in 0..self.children.len() {
            self.children.arrange_child(
                index,
                ctx,
                Rect::new(bounds.x() + (index as f32 * 52.0), bounds.y(), 48.0, 24.0),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        for child in self.children.as_slice().iter().take(self.visible_count) {
            child.paint(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

#[derive(Debug, Default)]
struct ThreadedSnapshotState {
    async_token: Option<AsyncWakeToken>,
    latest: Option<String>,
    paints: usize,
}

struct ThreadedSnapshotRoot {
    local: Rc<RefCell<ThreadedSnapshotState>>,
    inbox: Arc<Mutex<VecDeque<String>>>,
}

impl Widget for ThreadedSnapshotRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Custom(custom) if custom.kind == "arm-threaded-snapshot" => {
                self.local.borrow_mut().async_token = Some(ctx.register_async_wakeup());
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::Async { token, .. }) => {
                let mut local = self.local.borrow_mut();
                if local.async_token == Some(*token) {
                    while let Some(value) = self.inbox.lock().unwrap().pop_front() {
                        local.latest = Some(value);
                    }
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(160.0, 64.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.local.borrow_mut().paints += 1;
        let active = self.local.borrow().latest.is_some();
        let color = if active {
            Color::rgba(0.20, 0.46, 0.30, 1.0)
        } else {
            Color::rgba(0.20, 0.24, 0.32, 1.0)
        };
        ctx.fill_bounds(color);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CachedLayerPresentation {
    translation: Vector,
    opacity: f32,
}

impl Default for CachedLayerPresentation {
    fn default() -> Self {
        Self {
            translation: Vector::ZERO,
            opacity: 1.0,
        }
    }
}

struct CachedMoveLeaf {
    counters: Rc<RefCell<Counters>>,
    presentation: Rc<RefCell<CachedLayerPresentation>>,
}

impl Widget for CachedMoveLeaf {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(220.0, 72.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.counters.borrow_mut().paint += 1;
        ctx.fill_bounds(Color::rgba(0.14, 0.42, 0.72, 1.0));
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: sui_scene::LayerCompositionMode::Scroll,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        let presentation = *self.presentation.borrow();
        LayerProperties::default()
            .with_translation(presentation.translation)
            .with_opacity(presentation.opacity)
    }
}

struct DirectMoveLeaf {
    counters: Rc<RefCell<Counters>>,
}

impl Widget for DirectMoveLeaf {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(140.0, 48.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.counters.borrow_mut().paint += 1;
        ctx.fill_bounds(Color::rgba(0.78, 0.43, 0.14, 1.0));
    }
}

struct OverpaintLeaf {
    size: Size,
    color: Color,
    bleed: Vector,
}

impl Widget for OverpaintLeaf {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_rect(
            ctx.bounds().inflate(self.bleed.x.abs(), self.bleed.y.abs()),
            self.color,
        );
    }
}

struct PointerCountingLeaf {
    pointer_downs: Rc<RefCell<usize>>,
}

impl Widget for PointerCountingLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(
            event,
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
        ) {
            *self.pointer_downs.borrow_mut() += 1;
            ctx.set_handled();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(Color::rgba(0.15, 0.35, 0.58, 1.0));
    }
}

struct NonHitTestOverlayLeaf;

impl Widget for NonHitTestOverlayLeaf {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(Color::rgba(0.9, 0.2, 0.2, 0.35));
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Overlay,
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        Some(StackSurfaceOptions {
            hit_test: false,
            ..StackSurfaceOptions::default()
        })
    }
}

struct OverlayPassThroughRoot {
    children: WidgetChildren,
}

impl OverlayPassThroughRoot {
    fn new(pointer_downs: Rc<RefCell<usize>>) -> Self {
        let mut children = WidgetChildren::with_capacity(2);
        children.push(PointerCountingLeaf { pointer_downs });
        children.push(NonHitTestOverlayLeaf);
        Self { children }
    }
}

impl Widget for OverlayPassThroughRoot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.children
            .measure_child(0, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        self.children
            .measure_child(1, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let child_bounds = Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0);
        self.children.arrange_child(0, ctx, child_bounds);
        self.children.arrange_child(1, ctx, child_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.children.paint(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

struct HitTestOverlaySurface {
    child: SingleChild,
}

impl HitTestOverlaySurface {
    fn new(pointer_downs: Rc<RefCell<usize>>) -> Self {
        Self {
            child: SingleChild::new(PointerCountingLeaf { pointer_downs }),
        }
    }
}

impl Widget for HitTestOverlaySurface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child
            .measure(ctx, Constraints::tight(Size::new(120.0, 40.0)));
        constraints.clamp(Size::new(180.0, 90.0))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(bounds.x() + 24.0, bounds.y() + 20.0, 120.0, 40.0),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(Color::rgba(0.9, 0.2, 0.2, 0.35));
        self.child.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Effect,
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        Some(StackSurfaceOptions::default())
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct HitTestOverlayRoot {
    child: SingleChild,
}

impl HitTestOverlayRoot {
    fn new(pointer_downs: Rc<RefCell<usize>>) -> Self {
        Self {
            child: SingleChild::new(HitTestOverlaySurface::new(pointer_downs)),
        }
    }
}

impl Widget for HitTestOverlayRoot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child
            .measure(ctx, Constraints::tight(Size::new(180.0, 90.0)));
        constraints.clamp(Size::new(320.0, 180.0))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(bounds.x() + 20.0, bounds.y() + 20.0, 180.0, 90.0),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.child.paint(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct DirectMoveRoot {
    counters: Rc<RefCell<Counters>>,
    child: SingleChild,
    offset_x: f32,
}

impl Widget for DirectMoveRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event
            && custom.kind == "shift-direct"
        {
            self.offset_x += 36.0;
            ctx.request_arrange();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.child
            .measure(ctx, Constraints::tight(Size::new(140.0, 48.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x() + 28.0 + self.offset_x,
                bounds.y() + 32.0,
                140.0,
                48.0,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.counters.borrow_mut().paint += 1;
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.child.paint(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct CachedMoveRoot {
    counters: Rc<RefCell<Counters>>,
    child: SingleChild,
    presentation: Rc<RefCell<CachedLayerPresentation>>,
    offset_x: f32,
    paint_background: bool,
}

impl Widget for CachedMoveRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event
            && custom.kind == "shift-cached"
        {
            self.offset_x += 48.0;
            ctx.request_arrange();
        }
        if let Event::Custom(custom) = event
            && custom.kind == "nudge-cached-layer-property"
        {
            self.presentation.borrow_mut().translation += Vector::new(18.0, 0.0);
            ctx.request_transform();
        }
        if let Event::Custom(custom) = event
            && custom.kind == "fade-cached-layer"
        {
            self.presentation.borrow_mut().opacity = 0.5;
            ctx.request_effect();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.child
            .measure(ctx, Constraints::tight(Size::new(220.0, 72.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x() + 24.0 + self.offset_x,
                bounds.y() + 28.0,
                220.0,
                72.0,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.counters.borrow_mut().paint += 1;
        if self.paint_background {
            ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        }
        self.child.paint(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct FocusTraversalRoot {
    children: WidgetChildren,
}

struct ManagedOverlayTestRoot {
    open: Rc<RefCell<bool>>,
    children: WidgetChildren,
}

impl ManagedOverlayTestRoot {
    fn new(open: Rc<RefCell<bool>>, dismissals: Rc<RefCell<Vec<OverlayDismissReason>>>) -> Self {
        let mut overlay_children = WidgetChildren::with_capacity(2);
        overlay_children.push(FocusLeaf {
            counters: Rc::new(RefCell::new(Counters::default())),
        });
        overlay_children.push(FocusLeaf {
            counters: Rc::new(RefCell::new(Counters::default())),
        });

        let mut children = WidgetChildren::with_capacity(2);
        children.push(FocusLeaf {
            counters: Rc::new(RefCell::new(Counters::default())),
        });
        children.push(ManagedOverlayTestWidget {
            open: Rc::clone(&open),
            dismissals,
            children: overlay_children,
        });
        Self { open, children }
    }
}

impl Widget for ManagedOverlayTestRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event
            && custom.kind == "open-managed-overlay"
        {
            *self.open.borrow_mut() = true;
            ctx.request_measure();
            ctx.request_semantics();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(360.0, 220.0));
        self.children
            .measure_child(0, ctx, Constraints::tight(Size::new(100.0, 40.0)));
        self.children
            .measure_child(1, ctx, Constraints::tight(Size::new(160.0, 120.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children.arrange_child(
            0,
            ctx,
            Rect::new(bounds.x() + 20.0, bounds.y() + 20.0, 100.0, 40.0),
        );
        self.children.arrange_child(
            1,
            ctx,
            Rect::new(bounds.x() + 160.0, bounds.y() + 20.0, 160.0, 120.0),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        ctx.push(SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Window,
            ctx.bounds(),
        ));
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

struct ManagedOverlayTestWidget {
    open: Rc<RefCell<bool>>,
    dismissals: Rc<RefCell<Vec<OverlayDismissReason>>>,
    children: WidgetChildren,
}

impl Widget for ManagedOverlayTestWidget {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        let Some(request) = command.get(OVERLAY_DISMISS_REQUEST) else {
            return;
        };
        self.dismissals.borrow_mut().push(request.reason);
        *self.open.borrow_mut() = false;
        ctx.request_measure();
        ctx.request_semantics();
        ctx.set_handled();
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(160.0, 120.0));
        self.children
            .measure_child(0, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        self.children
            .measure_child(1, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children.arrange_child(
            0,
            ctx,
            Rect::new(bounds.x() + 20.0, bounds.y() + 12.0, 120.0, 40.0),
        );
        self.children.arrange_child(
            1,
            ctx,
            Rect::new(bounds.x() + 20.0, bounds.y() + 64.0, 120.0, 40.0),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if *self.open.borrow() {
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Dialog, ctx.bounds());
            node.name = Some("managed overlay".to_string());
            ctx.push(node);
        }
        self.children.semantics(ctx);
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        (*self.open.borrow()).then_some(
            OverlayOptions::new(OverlayKind::Dialog)
                .modal(true)
                .focus(OverlayFocusBehavior::CONTAINED),
        )
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

impl FocusTraversalRoot {
    fn new(first: Rc<RefCell<Counters>>, second: Rc<RefCell<Counters>>) -> Self {
        let mut children = WidgetChildren::with_capacity(2);
        children.push(FocusLeaf { counters: first });
        children.push(FocusLeaf { counters: second });
        Self { children }
    }
}

impl Widget for FocusTraversalRoot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.children
            .measure_child(0, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        self.children
            .measure_child(1, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children.arrange_child(
            0,
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
        self.children.arrange_child(
            1,
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 80.0, 120.0, 40.0),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        ctx.push(SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Window,
            ctx.bounds(),
        ));
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

struct HoverTransitionRoot {
    children: WidgetChildren,
}

impl HoverTransitionRoot {
    fn new(
        first: Rc<RefCell<HoverTransitionState>>,
        second: Rc<RefCell<HoverTransitionState>>,
    ) -> Self {
        let mut children = WidgetChildren::with_capacity(2);
        children.push(HoverTransitionLeaf {
            name: "hover-first",
            state: first,
        });
        children.push(HoverTransitionLeaf {
            name: "hover-second",
            state: second,
        });
        Self { children }
    }
}

impl Widget for HoverTransitionRoot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.children
            .measure_child(0, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        self.children
            .measure_child(1, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children.arrange_child(
            0,
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
        self.children.arrange_child(
            1,
            ctx,
            Rect::new(bounds.x() + 172.0, bounds.y() + 24.0, 120.0, 40.0),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
        self.children.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        ctx.push(SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Window,
            ctx.bounds(),
        ));
        self.children.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

#[derive(Default)]
struct PointerCaptureState {
    moves: usize,
    ups: usize,
    cancels: usize,
    cancel_followups: usize,
    last_cancel: Option<PointerEvent>,
}

#[derive(Debug, Default)]
struct RuntimeDragState {
    source_moves: usize,
    target_overs: usize,
    drops: usize,
    ends: usize,
    outcome: Option<DragOutcome>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct HoverTransitionState {
    enters: usize,
    leaves: usize,
    hovered: bool,
}

struct PointerCaptureLeaf {
    state: Rc<RefCell<PointerCaptureState>>,
    recapture_on_move: bool,
}

#[derive(Default)]
struct CursorRequestState {
    raw_motion: Vec<Vector>,
}

struct CursorRequestLeaf {
    state: Rc<RefCell<CursorRequestState>>,
}

impl Widget for CursorRequestLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                ctx.request_cursor_grab(CursorGrabMode::Locked);
                ctx.request_cursor_visibility(false);
                ctx.set_handled();
            }
            Event::RawMouseMotion(motion) => {
                self.state.borrow_mut().raw_motion.push(motion.delta);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }
}

struct PointerCaptureTransferRoot {
    child: SingleChild,
}

#[derive(Default)]
struct ReentrantCaptureState {
    intermediate_id: Option<WidgetId>,
    pointer_id: Option<u64>,
    source_cancels: usize,
    intermediate_captures: usize,
    intermediate_cancels: usize,
}

struct ReentrantCaptureSource {
    state: Rc<RefCell<ReentrantCaptureState>>,
}

struct ReentrantCaptureIntermediate {
    state: Rc<RefCell<ReentrantCaptureState>>,
}

struct ReentrantCaptureRoot {
    children: WidgetChildren,
}

struct RuntimeDragSource {
    state: Rc<RefCell<RuntimeDragState>>,
    scope: DragScopeId,
    session: Option<DragSessionId>,
}

struct RuntimeDragTarget {
    state: Rc<RefCell<RuntimeDragState>>,
    scope: DragScopeId,
}

struct RuntimeDragRoot {
    children: WidgetChildren,
}

struct HoverTransitionLeaf {
    name: &'static str,
    state: Rc<RefCell<HoverTransitionState>>,
}

impl Widget for PointerCaptureLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event
            && custom.kind == "pointer-cancel-followup"
        {
            self.state.borrow_mut().cancel_followups += 1;
            return;
        }

        let Event::Pointer(pointer) = event else {
            return;
        };

        match pointer.kind {
            PointerEventKind::Down => {
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            PointerEventKind::Move => {
                self.state.borrow_mut().moves += 1;
                if self.recapture_on_move {
                    ctx.request_pointer_capture(pointer.pointer_id);
                }
                ctx.set_handled();
            }
            PointerEventKind::Up => {
                self.state.borrow_mut().ups += 1;
                ctx.set_handled();
            }
            PointerEventKind::Cancel => {
                let mut state = self.state.borrow_mut();
                state.cancels += 1;
                state.last_cancel = Some(pointer.clone());
                drop(state);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.post_event(
                    ctx.widget_id(),
                    Event::Custom(CustomEvent::new("pointer-cancel-followup")),
                );
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }
}

impl PointerCaptureTransferRoot {
    fn new(child: PointerCaptureLeaf) -> Self {
        Self {
            child: SingleChild::new(child),
        }
    }
}

impl Widget for PointerCaptureTransferRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Move
            && ctx.phase() == EventPhase::Capture
        {
            ctx.request_pointer_capture(pointer.pointer_id);
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.child
            .measure(ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

impl ReentrantCaptureRoot {
    fn new(state: Rc<RefCell<ReentrantCaptureState>>) -> Self {
        let mut children = WidgetChildren::with_capacity(2);
        children.push(ReentrantCaptureSource {
            state: Rc::clone(&state),
        });
        children.push(ReentrantCaptureIntermediate { state });
        Self { children }
    }
}

impl Widget for ReentrantCaptureRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Move
            && ctx.phase() == EventPhase::Capture
        {
            ctx.request_pointer_capture(pointer.pointer_id);
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.children
            .measure_child(0, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        self.children
            .measure_child(1, ctx, Constraints::tight(Size::new(80.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children.arrange_child(
            0,
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
        self.children.arrange_child(
            1,
            ctx,
            Rect::new(bounds.x() + 180.0, bounds.y() + 24.0, 80.0, 40.0),
        );
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

impl Widget for ReentrantCaptureSource {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Pointer(pointer) = event else {
            return;
        };

        match pointer.kind {
            PointerEventKind::Down => {
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            PointerEventKind::Move => {
                ctx.set_handled();
            }
            PointerEventKind::Cancel => {
                let intermediate_id = {
                    let mut state = self.state.borrow_mut();
                    state.source_cancels += 1;
                    state.pointer_id = Some(pointer.pointer_id);
                    state.intermediate_id.expect("intermediate measured")
                };
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.post_event(
                    intermediate_id,
                    Event::Custom(CustomEvent::new("capture-intermediate")),
                );
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }
}

impl Widget for ReentrantCaptureIntermediate {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Custom(custom) if custom.kind == "capture-intermediate" => {
                let pointer_id = self.state.borrow().pointer_id.expect("pointer recorded");
                self.state.borrow_mut().intermediate_captures += 1;
                ctx.request_pointer_capture(pointer_id);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                self.state.borrow_mut().intermediate_cancels += 1;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.state.borrow_mut().intermediate_id = Some(ctx.widget_id());
        constraints.clamp(Size::new(80.0, 40.0))
    }
}

impl Widget for RuntimeDragSource {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) => match pointer.kind {
                PointerEventKind::Down => {
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
                PointerEventKind::Move if pointer.buttons.contains(PointerButton::Primary) => {
                    self.state.borrow_mut().source_moves += 1;
                    if self.session.is_none() {
                        self.session = Some(ctx.begin_drag(
                            self.scope,
                            pointer.pointer_id,
                            pointer.position,
                            DragPayload::text("runtime"),
                            DropEffect::Move,
                            Some("runtime".to_string()),
                        ));
                    }
                    ctx.set_handled();
                }
                PointerEventKind::Up => {
                    ctx.set_handled();
                }
                _ => {}
            },
            Event::Drag(drag)
                if drag.kind == DragEventKind::End && self.session == Some(drag.session_id) =>
            {
                let mut state = self.state.borrow_mut();
                state.ends += 1;
                state.outcome = drag.outcome;
                self.session = None;
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }
}

impl Widget for RuntimeDragTarget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Drag(drag) = event else {
            return;
        };
        if drag.scope_id != self.scope {
            return;
        }
        match drag.kind {
            DragEventKind::Over => {
                self.state.borrow_mut().target_overs += 1;
                ctx.accept_drop(DropEffect::Move);
            }
            DragEventKind::Drop => {
                self.state.borrow_mut().drops += 1;
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }
}

impl RuntimeDragRoot {
    fn new(state: Rc<RefCell<RuntimeDragState>>) -> Self {
        let scope = DragScopeId::new(77);
        let mut children = WidgetChildren::with_capacity(2);
        children.push(RuntimeDragSource {
            state: Rc::clone(&state),
            scope,
            session: None,
        });
        children.push(RuntimeDragTarget { state, scope });
        Self { children }
    }
}

impl Widget for RuntimeDragRoot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.children
            .measure_child(0, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        self.children
            .measure_child(1, ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children.arrange_child(
            0,
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
        self.children.arrange_child(
            1,
            ctx,
            Rect::new(bounds.x() + 172.0, bounds.y() + 24.0, 120.0, 40.0),
        );
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.children.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.children.visit_children_mut(visitor);
    }
}

impl Widget for HoverTransitionLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Pointer(pointer) = event else {
            return;
        };

        let mut state = self.state.borrow_mut();
        match pointer.kind {
            PointerEventKind::Enter => {
                state.enters += 1;
                state.hovered = true;
                ctx.request_paint();
                ctx.request_semantics();
            }
            PointerEventKind::Leave => {
                state.leaves += 1;
                state.hovered = false;
                ctx.request_paint();
                ctx.request_semantics();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let color = if self.state.borrow().hovered {
            Color::rgba(0.28, 0.45, 0.68, 1.0)
        } else {
            Color::rgba(0.20, 0.28, 0.38, 1.0)
        };
        ctx.fill_bounds(color);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(self.name.to_string());
        node.state.hovered = self.state.borrow().hovered;
        ctx.push(node);
    }
}

#[derive(Default)]
struct WakeState {
    timer_token: Option<TimerToken>,
    async_token: Option<AsyncWakeToken>,
    timer_wakes: usize,
    async_wakes: usize,
}

struct WakeLeaf {
    state: Rc<RefCell<WakeState>>,
}

impl Widget for WakeLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                let mut state = self.state.borrow_mut();
                state.timer_token = Some(ctx.schedule_timer_after(3.0));
                state.async_token = Some(ctx.register_async_wakeup());
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::Timer { token, .. }) => {
                let mut state = self.state.borrow_mut();
                if state.timer_token == Some(*token) {
                    state.timer_wakes += 1;
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::Async { token, .. }) => {
                let mut state = self.state.borrow_mut();
                if state.async_token == Some(*token) {
                    state.async_wakes += 1;
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }
}

#[derive(Default)]
struct AnimationWakeState {
    request_twice: bool,
    animation_wakes: usize,
    last_animation_time: Option<f64>,
    last_animation_delta: Option<f64>,
    last_animation_frame_index: Option<u64>,
}

struct AnimationWakeLeaf {
    state: Rc<RefCell<AnimationWakeState>>,
}

impl Widget for AnimationWakeLeaf {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                let request_twice = self.state.borrow().request_twice;
                ctx.request_animation_frame();
                if request_twice {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame {
                time,
                delta,
                frame_index,
            }) => {
                let mut state = self.state.borrow_mut();
                state.animation_wakes += 1;
                state.last_animation_time = Some(*time);
                state.last_animation_delta = Some(*delta);
                state.last_animation_frame_index = Some(*frame_index);
                ctx.request_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 40.0))
    }
}

struct AnimationWakeCapturingRoot {
    child: SingleChild,
    ancestor_wakes: Rc<RefCell<usize>>,
}

impl Widget for AnimationWakeCapturingRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Wake(WakeEvent::AnimationFrame { .. })) {
            *self.ancestor_wakes.borrow_mut() += 1;
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.child
            .measure(ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct RemovableAnimationRoot {
    child: Option<SingleChild>,
}

impl Widget for RemovableAnimationRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event
            && custom.kind == "drop-animation-child"
        {
            self.child = None;
            ctx.request_measure();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        if let Some(child) = &mut self.child {
            child.measure(ctx, Constraints::tight(Size::new(120.0, 40.0)));
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if let Some(child) = &mut self.child {
            child.arrange(
                ctx,
                Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
            );
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(child) = &self.child {
            child.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(child) = &mut self.child {
            child.visit_children_mut(visitor);
        }
    }
}

struct TextImeLeaf {
    layout: RefCell<Option<PersistentTextLayout>>,
}

struct FocusedImeLeaf;

impl Widget for FocusedImeLeaf {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(160.0, 32.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if ctx.is_focused() {
            ctx.set_ime_composition_rect(Rect::new(
                ctx.bounds().x() + 8.0,
                ctx.bounds().y() + 8.0,
                1.0,
                16.0,
            ));
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TextInput, ctx.bounds());
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }
}

impl Widget for TextImeLeaf {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(160.0, 32.0));
        let layout = ctx
            .layout()
            .shape_text_persistent(None, "compose", size, TextStyle::new(Color::WHITE))
            .unwrap();
        *self.layout.borrow_mut() = Some(layout);
        size
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let layout = self.layout.borrow();
        let layout = layout
            .as_ref()
            .expect("measure pass should shape text first");
        let origin = ctx.bounds().origin;
        ctx.draw_persistent_text_layout(origin, layout);
        ctx.set_ime_composition_rect(layout.caret_rect(3).translate(origin.to_vector()));
    }

    fn accepts_focus(&self) -> bool {
        true
    }
}

struct ChildRoot<W> {
    child: SingleChild,
    _marker: std::marker::PhantomData<W>,
}

impl<W> ChildRoot<W> {
    fn new(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            child: SingleChild::new(child),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<W> Widget for ChildRoot<W>
where
    W: Widget + 'static,
{
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.child
            .measure(ctx, Constraints::tight(Size::new(120.0, 40.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 120.0, 40.0),
        );
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

struct PaintedChildRoot<W> {
    child: SingleChild,
    _marker: std::marker::PhantomData<W>,
}

impl<W> PaintedChildRoot<W> {
    fn new(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            child: SingleChild::new(child),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<W> Widget for PaintedChildRoot<W>
where
    W: Widget + 'static,
{
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(320.0, 180.0));
        self.child
            .measure(ctx, Constraints::tight(Size::new(160.0, 32.0)));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::new(bounds.x() + 32.0, bounds.y() + 24.0, 160.0, 32.0),
        );
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

fn build_runtime() -> (
    Runtime,
    sui_core::WindowId,
    Rc<RefCell<Counters>>,
    Rc<RefCell<Counters>>,
) {
    let root_counters = Rc::new(RefCell::new(Counters::default()));
    let leaf_counters = Rc::new(RefCell::new(Counters::default()));

    let runtime = Application::new()
        .window(WindowBuilder::new().title("Test").root(TestRoot {
            counters: Rc::clone(&root_counters),
            child: SingleChild::new(FocusLeaf {
                counters: Rc::clone(&leaf_counters),
            }),
        }))
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, root_counters, leaf_counters)
}

fn build_semantics_action_runtime() -> (
    Runtime,
    sui_core::WindowId,
    Rc<RefCell<SemanticActionState>>,
) {
    let state = Rc::new(RefCell::new(SemanticActionState::default()));
    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Semantic Actions")
                .root(PaintedChildRoot::new(SyntheticSemanticActionLeaf {
                    state: Rc::clone(&state),
                })),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    (runtime, window_id, state)
}

#[test]
fn window_builder_uses_default_sui_icon() {
    let (runtime, window_id, _, _) = build_runtime();

    let icon = runtime.window_icon(window_id).unwrap().unwrap();
    assert!(matches!(icon, WindowIcon::Svg { .. }));
    assert!(
        std::str::from_utf8(icon.as_svg().unwrap())
            .unwrap()
            .contains("SUI logo")
    );
}

#[test]
fn window_builder_can_override_or_disable_icon() {
    let override_icon = WindowIcon::from_svg("<svg viewBox=\"0 0 1 1\"></svg>");
    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Custom icon")
                .icon(override_icon.clone())
                .root(FocusLeaf {
                    counters: Rc::new(RefCell::new(Counters::default())),
                }),
        )
        .window(
            WindowBuilder::new()
                .title("No icon")
                .without_icon()
                .root(FocusLeaf {
                    counters: Rc::new(RefCell::new(Counters::default())),
                }),
        )
        .build()
        .unwrap();

    let window_ids = runtime.window_ids();
    assert_eq!(
        runtime.window_icon(window_ids[0]).unwrap(),
        Some(&override_icon)
    );
    assert_eq!(runtime.window_icon(window_ids[1]).unwrap(), None);
}

#[test]
fn window_builder_preserves_requested_initial_placement() {
    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Placed window")
                .initial_size(Size::new(1_440.0, 900.0))
                .initial_position(Point::new(-1_920.0, 120.0))
                .root(FocusLeaf {
                    counters: Rc::new(RefCell::new(Counters::default())),
                }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    assert_eq!(
        runtime.window_initial_size(window_id).unwrap(),
        Some(Size::new(1_440.0, 900.0))
    );
    assert_eq!(
        runtime.window_initial_position(window_id).unwrap(),
        Some(Point::new(-1_920.0, 120.0))
    );
}

#[test]
fn independent_runtimes_allocate_distinct_window_ids() {
    let first = Application::new()
        .window(WindowBuilder::new().title("First").root(FocusLeaf {
            counters: Rc::new(RefCell::new(Counters::default())),
        }))
        .build()
        .unwrap();
    let second = Application::new()
        .window(WindowBuilder::new().title("Second").root(FocusLeaf {
            counters: Rc::new(RefCell::new(Counters::default())),
        }))
        .build()
        .unwrap();

    assert_ne!(first.window_ids()[0], second.window_ids()[0]);
}

#[test]
fn rgba_window_icon_validates_buffer_shape() {
    assert!(WindowIcon::from_rgba8(1, 1, vec![0, 0, 0, 255]).is_ok());
    assert!(WindowIcon::from_rgba8(0, 1, Vec::new()).is_err());
    assert!(WindowIcon::from_rgba8(1, 1, vec![0, 0, 0]).is_err());
}

fn build_cached_move_runtime() -> (
    Runtime,
    sui_core::WindowId,
    Rc<RefCell<Counters>>,
    Rc<RefCell<Counters>>,
) {
    let root_counters = Rc::new(RefCell::new(Counters::default()));
    let leaf_counters = Rc::new(RefCell::new(Counters::default()));
    let presentation = Rc::new(RefCell::new(CachedLayerPresentation::default()));

    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Cached move")
                .root(CachedMoveRoot {
                    counters: Rc::clone(&root_counters),
                    child: SingleChild::new(CachedMoveLeaf {
                        counters: Rc::clone(&leaf_counters),
                        presentation: Rc::clone(&presentation),
                    }),
                    presentation,
                    offset_x: 0.0,
                    paint_background: true,
                }),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, root_counters, leaf_counters)
}

fn build_direct_move_runtime() -> (
    Runtime,
    sui_core::WindowId,
    Rc<RefCell<Counters>>,
    Rc<RefCell<Counters>>,
) {
    let root_counters = Rc::new(RefCell::new(Counters::default()));
    let leaf_counters = Rc::new(RefCell::new(Counters::default()));

    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Direct move")
                .root(DirectMoveRoot {
                    counters: Rc::clone(&root_counters),
                    child: SingleChild::new(DirectMoveLeaf {
                        counters: Rc::clone(&leaf_counters),
                    }),
                    offset_x: 0.0,
                }),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, root_counters, leaf_counters)
}

fn build_overpaint_runtime() -> (Runtime, sui_core::WindowId) {
    let runtime = Application::new()
        .window(WindowBuilder::new().title("Overpaint").root(TestRoot {
            counters: Rc::new(RefCell::new(Counters::default())),
            child: SingleChild::new(OverpaintLeaf {
                size: Size::new(120.0, 40.0),
                color: Color::rgba(0.92, 0.42, 0.18, 1.0),
                bleed: Vector::new(6.0, 4.0),
            }),
        }))
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id)
}

fn build_focus_traversal_runtime() -> (
    Runtime,
    sui_core::WindowId,
    Rc<RefCell<Counters>>,
    Rc<RefCell<Counters>>,
) {
    let first = Rc::new(RefCell::new(Counters::default()));
    let second = Rc::new(RefCell::new(Counters::default()));

    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Focus Traversal")
                .root(FocusTraversalRoot::new(
                    Rc::clone(&first),
                    Rc::clone(&second),
                )),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, first, second)
}

fn graph_child(graph: &WidgetGraphSnapshot) -> &WidgetNodeSnapshot {
    &graph.nodes[1]
}

fn scene_has_layer_for(scene: &sui_scene::Scene, widget_id: sui_core::WidgetId) -> bool {
    scene.commands().iter().any(|command| match command {
        SceneCommand::Layer(layer) => layer.widget_id() == widget_id,
        _ => false,
    })
}

fn build_pointer_capture_runtime() -> (
    Runtime,
    sui_core::WindowId,
    Rc<RefCell<PointerCaptureState>>,
) {
    let state = Rc::new(RefCell::new(PointerCaptureState::default()));

    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Pointer Capture")
                .root(ChildRoot::new(PointerCaptureLeaf {
                    state: Rc::clone(&state),
                    recapture_on_move: false,
                })),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, state)
}

fn build_hover_transition_runtime() -> (
    Runtime,
    sui_core::WindowId,
    Rc<RefCell<HoverTransitionState>>,
    Rc<RefCell<HoverTransitionState>>,
) {
    let first = Rc::new(RefCell::new(HoverTransitionState::default()));
    let second = Rc::new(RefCell::new(HoverTransitionState::default()));

    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Hover transitions")
                .root(HoverTransitionRoot::new(
                    Rc::clone(&first),
                    Rc::clone(&second),
                )),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, first, second)
}

fn build_wake_runtime() -> (Runtime, sui_core::WindowId, Rc<RefCell<WakeState>>) {
    let state = Rc::new(RefCell::new(WakeState::default()));

    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Wake")
                .root(ChildRoot::new(WakeLeaf {
                    state: Rc::clone(&state),
                })),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, state)
}

fn build_animation_wake_runtime(
    request_twice: bool,
) -> (Runtime, sui_core::WindowId, Rc<RefCell<AnimationWakeState>>) {
    let state = Rc::new(RefCell::new(AnimationWakeState {
        request_twice,
        ..AnimationWakeState::default()
    }));

    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Animation Wake")
                .root(ChildRoot::new(AnimationWakeLeaf {
                    state: Rc::clone(&state),
                })),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id, state)
}

fn build_removable_animation_runtime() -> (Runtime, sui_core::WindowId) {
    let state = Rc::new(RefCell::new(AnimationWakeState::default()));
    let runtime =
        Application::new()
            .window(WindowBuilder::new().title("Removable Animation").root(
                RemovableAnimationRoot {
                    child: Some(SingleChild::new(AnimationWakeLeaf { state })),
                },
            ))
            .build()
            .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id)
}

fn build_text_runtime() -> (Runtime, sui_core::WindowId) {
    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Text")
                .root(PaintedChildRoot::new(TextImeLeaf {
                    layout: RefCell::new(None),
                })),
        )
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    (runtime, window_id)
}

#[test]
fn runtime_exposes_retained_widget_graph() {
    let (mut runtime, window_id, _, _) = build_runtime();

    let output = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();

    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.nodes[0].id, graph.root);
    assert_eq!(graph.nodes[0].stack_host, graph.root);
    assert!(graph.nodes[0].is_stack_host);
    assert_eq!(graph_child(&graph).parent, Some(graph.root));
    assert_eq!(graph_child(&graph).stack_host, graph.root);
    assert!(!graph_child(&graph).is_stack_surface);
    assert_eq!(graph.stack_hosts.len(), 1);
    assert_eq!(graph.stack_hosts[0].host, graph.root);
    assert!(graph.stack_hosts[0].surfaces.is_empty());
    assert!(graph_child(&graph).accepts_focus);
    assert_eq!(
        graph_child(&graph).geometry.layout_bounds,
        graph_child(&graph).bounds
    );
    assert_eq!(
        graph_child(&graph).geometry.input_bounds,
        graph_child(&graph).geometry.layout_bounds
    );
    assert_eq!(
        graph_child(&graph).geometry.paint_bounds,
        graph_child(&graph).geometry.layout_bounds
    );
    assert_eq!(output.frame.viewport, Size::new(320.0, 180.0));
    assert_eq!(output.frame.surface_size, Size::new(320.0, 180.0));
    assert_eq!(output.frame.scale_factor, 1.0);
}

#[test]
fn synthetic_semantic_activate_falls_back_to_pointer_routing() {
    let (mut runtime, window_id, state) = build_semantics_action_runtime();
    let _ = runtime.render(window_id).unwrap();

    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Activate,
            )
            .unwrap()
    );

    let state = state.borrow();
    assert_eq!(
        state.actions,
        vec![(SYNTHETIC_ACTION_ID, SemanticsActionRequest::Activate)]
    );
    assert_eq!(state.pointer_activations, 1);
}

#[test]
fn semantic_expand_and_collapse_use_idempotent_pointer_fallbacks() {
    let (mut runtime, window_id, state) = build_semantics_action_runtime();
    let _ = runtime.render(window_id).unwrap();

    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Expand,
            )
            .unwrap()
    );
    assert!(state.borrow().expanded);
    assert_eq!(state.borrow().pointer_activations, 1);

    let _ = runtime.render(window_id).unwrap();
    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Expand,
            )
            .unwrap()
    );
    assert_eq!(state.borrow().pointer_activations, 1);

    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Collapse,
            )
            .unwrap()
    );
    assert!(!state.borrow().expanded);
    assert_eq!(state.borrow().pointer_activations, 2);
}

#[test]
fn synthetic_semantic_focus_targets_graph_owner_and_retains_semantic_identity() {
    let (mut runtime, window_id, state) = build_semantics_action_runtime();
    let first = runtime.render(window_id).unwrap();
    let owner_id = first
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("semantic action owner"))
        .unwrap()
        .id;

    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Focus,
            )
            .unwrap()
    );
    assert_eq!(runtime.focused_widget(window_id).unwrap(), Some(owner_id));

    let focused = runtime.render(window_id).unwrap();
    assert!(
        focused
            .semantics
            .iter()
            .find(|node| node.id == SYNTHETIC_ACTION_ID)
            .unwrap()
            .state
            .focused
    );
    assert_eq!(
        state.borrow().actions,
        vec![(SYNTHETIC_ACTION_ID, SemanticsActionRequest::Focus)]
    );

    let mut pointer = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 32.0));
    pointer.button = Some(PointerButton::Primary);
    pointer.buttons.insert(PointerButton::Primary);
    runtime
        .handle_event(window_id, Event::Pointer(pointer))
        .unwrap();
    let pointer_focused = runtime.render(window_id).unwrap();
    assert!(
        pointer_focused
            .semantics
            .iter()
            .find(|node| node.id == owner_id)
            .unwrap()
            .state
            .focused
    );
    assert!(
        !pointer_focused
            .semantics
            .iter()
            .find(|node| node.id == SYNTHETIC_ACTION_ID)
            .unwrap()
            .state
            .focused
    );
}

#[test]
fn semantic_blur_only_clears_the_current_semantic_focus_target() {
    let (mut runtime, window_id, _state) = build_semantics_action_runtime();
    let _ = runtime.render(window_id).unwrap();

    assert!(
        !runtime
            .handle_semantics_action(window_id, SYNTHETIC_ACTION_ID, SemanticsActionRequest::Blur,)
            .unwrap()
    );
    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Focus,
            )
            .unwrap()
    );
    assert!(
        runtime
            .handle_semantics_action(window_id, SYNTHETIC_ACTION_ID, SemanticsActionRequest::Blur,)
            .unwrap()
    );
    assert_eq!(runtime.focused_widget(window_id).unwrap(), None);
}

#[test]
fn semantic_focus_rejects_an_unhandled_target_without_a_focusable_owner() {
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Unfocusable Semantic Action")
                .root(ChildRoot::new(UnfocusableSemanticLeaf)),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id).unwrap();
    let target = output
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("unfocusable semantic node"))
        .unwrap()
        .id;

    assert!(
        !runtime
            .handle_semantics_action(window_id, target, SemanticsActionRequest::Focus,)
            .unwrap()
    );
    assert_eq!(runtime.focused_widget(window_id).unwrap(), None);
    assert!(
        !runtime
            .render(window_id)
            .unwrap()
            .semantics
            .iter()
            .find(|node| node.id == target)
            .unwrap()
            .state
            .focused
    );
}

#[test]
fn stale_disabled_and_unadvertised_semantic_actions_are_rejected() {
    let (mut stale_runtime, stale_window_id, stale_state) = build_semantics_action_runtime();
    let _ = stale_runtime.render(stale_window_id).unwrap();

    assert!(
        !stale_runtime
            .handle_semantics_action(
                stale_window_id,
                WidgetId::new(u64::MAX - 2),
                SemanticsActionRequest::Activate,
            )
            .unwrap()
    );
    assert!(
        !stale_runtime
            .handle_semantics_action(
                stale_window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Increment,
            )
            .unwrap()
    );
    assert!(stale_state.borrow().actions.is_empty());

    let (mut disabled_runtime, disabled_window_id, disabled_state) =
        build_semantics_action_runtime();
    disabled_state.borrow_mut().disabled = true;
    let _ = disabled_runtime.render(disabled_window_id).unwrap();
    assert!(
        !disabled_runtime
            .handle_semantics_action(
                disabled_window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Activate,
            )
            .unwrap()
    );
    assert!(disabled_state.borrow().actions.is_empty());
}

#[test]
fn semantic_action_payload_is_typed_and_unsupported_actions_are_rejected() {
    let (mut runtime, window_id, state) = build_semantics_action_runtime();
    let _ = runtime.render(window_id).unwrap();

    let set_value_handled = runtime
        .handle_semantics_action(
            window_id,
            SYNTHETIC_ACTION_ID,
            SemanticsActionRequest::SetValue(SemanticsValue::Text("forty two".into())),
        )
        .unwrap();
    assert!(
        set_value_handled,
        "state after SetValue: {:?}; semantics: {:?}; graph: {:?}",
        state.borrow(),
        runtime.semantics(window_id).unwrap(),
        runtime.widget_graph(window_id).unwrap()
    );
    assert!(
        runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::InsertText("hello".into()),
            )
            .unwrap()
    );
    assert!(
        !runtime
            .handle_semantics_action(
                window_id,
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::Increment,
            )
            .unwrap()
    );

    assert_eq!(
        state.borrow().actions,
        vec![
            (
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::SetValue(SemanticsValue::Text("forty two".into())),
            ),
            (
                SYNTHETIC_ACTION_ID,
                SemanticsActionRequest::InsertText("hello".into()),
            ),
        ]
    );
}

#[test]
fn render_output_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<RenderOutput>();
}

#[test]
fn widget_graph_visits_logical_children_even_when_paint_is_virtualized() {
    let paint_state = Rc::new(RefCell::new(VirtualizedPaintState::default()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Virtualized")
                .root(VirtualizedLogicalRoot::new(4, 2, Rc::clone(&paint_state))),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();

    assert_eq!(root.children.len(), 4);
    assert_eq!(graph.nodes.len(), 5);
    assert_eq!(paint_state.borrow().painted, vec![0, 1]);
}

#[test]
fn worker_owned_snapshot_state_reenters_runtime_without_widget_send_sync() {
    let local = Rc::new(RefCell::new(ThreadedSnapshotState::default()));
    let inbox = Arc::new(Mutex::new(VecDeque::new()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Threaded Snapshot")
                .root(ThreadedSnapshotRoot {
                    local: Rc::clone(&local),
                    inbox: Arc::clone(&inbox),
                }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();
    runtime
        .handle_event(
            window_id,
            Event::Custom(CustomEvent::new("arm-threaded-snapshot")),
        )
        .unwrap();
    let async_token = local.borrow().async_token.unwrap();

    let worker_inbox = Arc::clone(&inbox);
    std::thread::spawn(move || {
        worker_inbox
            .lock()
            .unwrap()
            .push_back("worker-ready".to_string());
    })
    .join()
    .unwrap();

    assert!(runtime.wake_async(window_id, async_token).unwrap());
    for (ready_window, event) in runtime.drain_ready_events() {
        runtime.handle_event(ready_window, event).unwrap();
    }

    assert_eq!(local.borrow().latest.as_deref(), Some("worker-ready"));
    assert!(runtime.needs_render(window_id).unwrap());

    let _ = runtime.render(window_id).unwrap();
    assert_eq!(local.borrow().paints, 2);
}

struct EventPoster {
    log: Rc<RefCell<Vec<String>>>,
}

impl Widget for EventPoster {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Custom(custom) = event {
            self.log.borrow_mut().push(custom.kind.clone());
            if custom.kind == "posted-trigger" {
                ctx.post_event(
                    ctx.widget_id(),
                    Event::Custom(CustomEvent::new("posted-response")),
                );
            }
        }
    }
}

#[test]
fn posted_events_are_delivered_after_the_current_dispatch() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Posted").root(EventPoster {
            log: Rc::clone(&log),
        }))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let _ = runtime.render(window_id).unwrap();

    runtime
        .handle_event(window_id, Event::Custom(CustomEvent::new("posted-trigger")))
        .unwrap();

    assert_eq!(
        log.borrow().as_slice(),
        ["posted-trigger".to_string(), "posted-response".to_string()]
    );
}

#[test]
fn runtime_clipboard_is_shared_with_window_event_contexts() {
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Clipboard").root(FocusLeaf {
            counters: Rc::new(RefCell::new(Counters::default())),
        }))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let _ = runtime.render(window_id).unwrap();

    runtime.clipboard().set_text("shared");
    assert_eq!(runtime.clipboard().text().as_deref(), Some("shared"));

    runtime.set_clipboard_backend(sui_core::LocalClipboardBackend::new());
    assert_eq!(runtime.clipboard().text(), None);
}

#[test]
fn explicit_paint_boundary_direct_child_remains_a_stack_surface() {
    let (mut runtime, window_id, _, _) = build_cached_move_runtime();

    let _ = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();
    let child = graph_child(&graph);

    assert!(child.is_stack_surface);
    assert_eq!(graph.stack_hosts.len(), 1);
    assert_eq!(graph.stack_hosts[0].host, graph.root);
    assert_eq!(graph.stack_hosts[0].surfaces, vec![child.id]);
}

#[test]
fn non_hit_test_stack_surface_allows_underlying_pointer_target() {
    let pointer_downs = Rc::new(RefCell::new(0));
    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Overlay pass-through")
                .root(OverlayPassThroughRoot::new(Rc::clone(&pointer_downs))),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let mut runtime = runtime;

    let output = runtime.render(window_id).unwrap();
    let mut overlay_hit_test = None;
    output.frame.scene.visit_layers(&mut |layer| {
        if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
            overlay_hit_test = Some(layer.descriptor.hit_test);
        }
    });
    assert_eq!(overlay_hit_test, Some(false));

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 36.0));
    down.pointer_id = 1;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    assert_eq!(*pointer_downs.borrow(), 1);
}

#[test]
fn hit_test_stack_surface_targets_deepest_child() {
    let pointer_downs = Rc::new(RefCell::new(0));
    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Overlay child hit")
                .root(HitTestOverlayRoot::new(Rc::clone(&pointer_downs))),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let mut runtime = runtime;

    let output = runtime.render(window_id).unwrap();
    let mut effect_hit_test = None;
    output.frame.scene.visit_layers(&mut |layer| {
        if layer.descriptor.composition_mode == LayerCompositionMode::Effect {
            effect_hit_test = Some(layer.descriptor.hit_test);
        }
    });
    assert_eq!(effect_hit_test, Some(true));

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(80.0, 60.0));
    down.pointer_id = 1;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    assert_eq!(*pointer_downs.borrow(), 1);
}

#[test]
fn runtime_reports_surface_size_and_scale_factor_for_hidpi_windows() {
    let (mut runtime, window_id, _, _) = build_runtime();

    runtime
        .handle_event(
            window_id,
            Event::Window(WindowEvent::ScaleFactorChanged {
                scale_factor: 2.0,
                raw_dpi: Some(192.0),
                suggested_size: Some(Size::new(320.0, 180.0)),
            }),
        )
        .unwrap();

    let output = runtime.render(window_id).unwrap();

    assert_eq!(output.frame.viewport, Size::new(320.0, 180.0));
    assert_eq!(output.frame.surface_size, Size::new(640.0, 360.0));
    assert_eq!(output.frame.scale_factor, 2.0);
}

#[test]
fn runtime_delivers_external_file_events_without_changing_window_geometry() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("External file events")
                .root(WindowEventRecorder {
                    events: Rc::clone(&events),
                }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let before = runtime.render(window_id).unwrap().frame;
    let path = PathBuf::from(r"C:\workspace\agent.json");

    for event in [
        WindowEvent::ExternalFileHovered(path.clone()),
        WindowEvent::ExternalFileDropped(path.clone()),
        WindowEvent::ExternalFileHoverCancelled,
    ] {
        runtime
            .handle_event(window_id, Event::Window(event))
            .unwrap();
    }

    assert_eq!(
        events.borrow().as_slice(),
        &[
            WindowEvent::ExternalFileHovered(path.clone()),
            WindowEvent::ExternalFileDropped(path),
            WindowEvent::ExternalFileHoverCancelled,
        ]
    );
    let after = runtime.render(window_id).unwrap().frame;
    assert_eq!(after.viewport, before.viewport);
    assert_eq!(after.surface_size, before.surface_size);
    assert_eq!(after.scale_factor, before.scale_factor);
}

#[test]
fn runtime_attaches_registered_fonts_to_render_output() {
    let (mut runtime, window_id, _, _) = build_runtime();
    let handle = FontHandle::new(33);

    runtime
        .register_font(handle, RegisteredFont::from_bytes(vec![0, 1, 2, 3]))
        .unwrap();

    let output = runtime.render(window_id).unwrap();

    assert!(output.frame.font_registry.contains(handle));
}

#[test]
fn runtime_attaches_registered_images_to_render_output() {
    let (mut runtime, window_id, _, _) = build_runtime();
    let handle = ImageHandle::new(7);

    runtime
        .register_image(
            handle,
            RegisteredImage::from_rgba8(1, 1, vec![255, 0, 0, 255]).unwrap(),
        )
        .unwrap();

    let output = runtime.render(window_id).unwrap();

    assert!(output.frame.image_registry.contains(handle));
}

#[test]
fn paint_registered_external_images_are_included_in_render_output() {
    struct ExternalImageLeaf {
        handle: ImageHandle,
    }

    impl Widget for ExternalImageLeaf {
        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.register_external_image(
                self.handle,
                RegisteredExternalImage::new(640, 360).unwrap(),
            );
            ctx.draw_image(ctx.bounds(), self.handle);
        }
    }

    let handle = ImageHandle::new(71);
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("External image")
                .root(ExternalImageLeaf { handle }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let output = runtime.render(window_id).unwrap();
    assert_eq!(
        output.frame.image_registry.dimensions(handle),
        Some((640, 360))
    );
    assert!(output.frame.image_registry.get(handle).is_none());
    assert!(output.frame.image_registry.get_external(handle).is_some());
}

#[test]
fn runtime_attaches_registered_svg_images_to_render_output() {
    let (mut runtime, window_id, _, _) = build_runtime();
    let handle = ImageHandle::new(41);
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><rect width="4" height="4" fill="#27B7C8"/></svg>"##;

    runtime
        .register_svg_image_at_size_with_handle(handle, 16, 16, svg)
        .unwrap();

    let output = runtime.render(window_id).unwrap();
    let image = output.frame.image_registry.get(handle).unwrap();

    assert_eq!(image.width(), 16);
    assert_eq!(image.height(), 16);
    assert!(image.bytes().chunks_exact(4).any(|pixel| pixel[3] > 0));
}

#[test]
fn semantics_only_invalidation_skips_repaint() {
    let (mut runtime, window_id, root_counters, leaf_counters) = build_runtime();

    let _ = runtime.render(window_id).unwrap();
    let root_paint_before = root_counters.borrow().paint;
    let leaf_paint_before = leaf_counters.borrow().paint;

    runtime
        .handle_event(window_id, Event::Custom(CustomEvent::new("semantics-only")))
        .unwrap();

    assert_eq!(
        runtime.schedule(window_id).unwrap(),
        FrameSchedule {
            semantics: true,
            ..FrameSchedule::default()
        }
    );

    let _ = runtime.render(window_id).unwrap();

    assert_eq!(root_counters.borrow().paint, root_paint_before);
    assert_eq!(leaf_counters.borrow().paint, leaf_paint_before);
    assert!(root_counters.borrow().semantics >= 2);
}

#[test]
fn ordinary_widgets_flatten_scene_output_into_root() {
    let (mut runtime, window_id, _, _) = build_runtime();

    let output = runtime.render(window_id).unwrap();
    let leaf_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;

    assert!(!scene_has_layer_for(&output.frame.scene, leaf_id));
}

#[test]
fn explicit_paint_boundary_widgets_emit_layers() {
    let (mut runtime, window_id, _, _) = build_cached_move_runtime();

    let output = runtime.render(window_id).unwrap();
    let layer_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;

    assert!(scene_has_layer_for(&output.frame.scene, layer_id));
}

#[test]
fn flat_widgets_preserve_paint_bounds_larger_than_layout_bounds() {
    let (mut runtime, window_id) = build_overpaint_runtime();

    let _ = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();
    let child = graph_child(&graph);

    assert_eq!(child.bounds, Rect::new(32.0, 24.0, 120.0, 40.0));
    assert_eq!(child.geometry.paint_bounds, child.bounds.inflate(6.0, 4.0));
}

#[test]
fn paint_invalidation_repaints_nearest_boundary_root_for_flat_child() {
    let (mut runtime, window_id, root_counters, leaf_counters) = build_runtime();

    let _ = runtime.render(window_id).unwrap();
    let leaf_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;
    let root_paint_before = root_counters.borrow().paint;
    let leaf_paint_before = leaf_counters.borrow().paint;

    let mut pointer = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    pointer.button = Some(PointerButton::Primary);
    pointer.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(pointer))
        .unwrap();

    let output = runtime.render(window_id).unwrap();

    assert_eq!(root_counters.borrow().paint, root_paint_before + 1);
    assert_eq!(leaf_counters.borrow().paint, leaf_paint_before + 1);
    assert!(!scene_has_layer_for(&output.frame.scene, leaf_id));
    assert!(output.frame.layer_updates.is_empty());
}

#[test]
fn cached_layer_translation_updates_scene_without_repaint() {
    let (mut runtime, window_id, root_counters, leaf_counters) = build_cached_move_runtime();

    let first = runtime.render(window_id).unwrap();
    let first_graph = runtime.widget_graph(window_id).unwrap();
    let initial_graph_bounds = graph_child(&first_graph).geometry.paint_bounds;
    let layer_id = graph_child(&first_graph).id;
    let root_paint_before = root_counters.borrow().paint;
    let leaf_paint_before = leaf_counters.borrow().paint;
    let initial_bounds = first
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            SceneCommand::Layer(layer) if layer.widget_id() == layer_id => Some(layer.bounds()),
            _ => None,
        })
        .expect("cached layer present before translation");

    runtime
        .handle_event(window_id, Event::Custom(CustomEvent::new("shift-cached")))
        .unwrap();

    let second = runtime.render(window_id).unwrap();

    assert_eq!(root_counters.borrow().paint, root_paint_before);
    assert_eq!(leaf_counters.borrow().paint, leaf_paint_before);
    assert_eq!(second.frame.layer_updates.len(), 1);
    assert_eq!(second.frame.layer_updates[0].owner, layer_id);
    assert_eq!(
        second.frame.layer_updates[0].kind,
        SceneLayerUpdateKind::Transform
    );

    let translated_bounds = second
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            SceneCommand::Layer(layer) if layer.widget_id() == layer_id => Some(layer.bounds()),
            _ => None,
        })
        .expect("cached layer present after translation");

    assert_eq!(translated_bounds.x(), initial_bounds.x() + 48.0);
    assert_eq!(translated_bounds.y(), initial_bounds.y());
    let translated_graph_bounds = graph_child(&runtime.widget_graph(window_id).unwrap())
        .geometry
        .paint_bounds;
    assert_eq!(
        translated_graph_bounds,
        initial_graph_bounds.translate(Vector::new(48.0, 0.0))
    );
}

#[test]
fn composition_only_translation_keeps_ancestor_paint_bounds_conservative() {
    let root_counters = Rc::new(RefCell::new(Counters::default()));
    let leaf_counters = Rc::new(RefCell::new(Counters::default()));
    let presentation = Rc::new(RefCell::new(CachedLayerPresentation::default()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Bare cached move")
                .root(CachedMoveRoot {
                    counters: Rc::clone(&root_counters),
                    child: SingleChild::new(CachedMoveLeaf {
                        counters: Rc::clone(&leaf_counters),
                        presentation: Rc::clone(&presentation),
                    }),
                    presentation,
                    offset_x: 0.0,
                    paint_background: false,
                }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let _ = runtime.render(window_id).unwrap();
    let initial_graph = runtime.widget_graph(window_id).unwrap();
    let initial_root_paint = initial_graph.nodes[0].geometry.paint_bounds;
    let initial_child_paint = graph_child(&initial_graph).geometry.paint_bounds;
    let root_paint_before = root_counters.borrow().paint;
    let leaf_paint_before = leaf_counters.borrow().paint;

    runtime
        .handle_event(window_id, Event::Custom(CustomEvent::new("shift-cached")))
        .unwrap();
    let _ = runtime.render(window_id).unwrap();
    let shifted_graph = runtime.widget_graph(window_id).unwrap();
    let shifted_child_paint = graph_child(&shifted_graph).geometry.paint_bounds;

    assert_eq!(root_counters.borrow().paint, root_paint_before);
    assert_eq!(leaf_counters.borrow().paint, leaf_paint_before);
    assert_eq!(
        shifted_graph.nodes[0].geometry.paint_bounds,
        initial_root_paint.union(shifted_child_paint)
    );
    assert_eq!(
        shifted_child_paint,
        initial_child_paint.translate(Vector::new(48.0, 0.0))
    );
}

#[test]
fn composition_only_translation_updates_focused_ime_rect() {
    let root_counters = Rc::new(RefCell::new(Counters::default()));
    let presentation = Rc::new(RefCell::new(CachedLayerPresentation::default()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Cached IME move")
                .root(CachedMoveRoot {
                    counters: Rc::clone(&root_counters),
                    child: SingleChild::new_with_paint_boundary(TextImeLeaf {
                        layout: RefCell::new(None),
                    }),
                    presentation,
                    offset_x: 0.0,
                    paint_background: true,
                }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let _ = runtime.render(window_id).unwrap();

    let mut pointer = PointerEvent::new(PointerEventKind::Down, Point::new(40.0, 40.0));
    pointer.button = Some(PointerButton::Primary);
    pointer.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(pointer))
        .unwrap();
    let focused = runtime.render(window_id).unwrap();
    let initial_ime = focused
        .ime_composition_rect
        .expect("focused text layer should publish an IME rectangle");
    let initial_paint_bounds = graph_child(&runtime.widget_graph(window_id).unwrap())
        .geometry
        .paint_bounds;
    let root_paint_before = root_counters.borrow().paint;

    runtime
        .handle_event(window_id, Event::Custom(CustomEvent::new("shift-cached")))
        .unwrap();
    let shifted = runtime.render(window_id).unwrap();

    assert_eq!(root_counters.borrow().paint, root_paint_before);
    assert_eq!(
        shifted.ime_composition_rect,
        Some(initial_ime.translate(Vector::new(48.0, 0.0)))
    );
    assert_eq!(
        graph_child(&runtime.widget_graph(window_id).unwrap())
            .geometry
            .paint_bounds,
        initial_paint_bounds.translate(Vector::new(48.0, 0.0))
    );
    assert!(
        shifted
            .frame
            .layer_updates
            .iter()
            .any(|update| { update.kind == SceneLayerUpdateKind::Transform })
    );
}

#[test]
fn explicit_boundary_layer_property_translation_marks_transform_without_content_rebuild() {
    let (mut runtime, window_id, root_counters, leaf_counters) = build_cached_move_runtime();

    let first = runtime.render(window_id).unwrap();
    let layer_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;
    let root_paint_before = root_counters.borrow().paint;
    let leaf_paint_before = leaf_counters.borrow().paint;
    let initial_layer = first
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            SceneCommand::Layer(layer) if layer.widget_id() == layer_id => Some(layer.clone()),
            _ => None,
        })
        .expect("cached layer present before property translation");

    runtime
        .handle_event(
            window_id,
            Event::Custom(CustomEvent::new("nudge-cached-layer-property")),
        )
        .unwrap();

    let second = runtime.render(window_id).unwrap();
    let updated_layer = second
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            SceneCommand::Layer(layer) if layer.widget_id() == layer_id => Some(layer.clone()),
            _ => None,
        })
        .expect("cached layer present after property translation");

    assert_eq!(root_counters.borrow().paint, root_paint_before);
    assert_eq!(leaf_counters.borrow().paint, leaf_paint_before);
    assert_eq!(second.frame.layer_updates.len(), 1);
    assert_eq!(second.frame.layer_updates[0].owner, layer_id);
    assert_eq!(
        second.frame.layer_updates[0].kind,
        SceneLayerUpdateKind::Transform
    );
    assert_eq!(updated_layer.bounds(), initial_layer.bounds());
    assert_eq!(
        updated_layer.descriptor.properties.translation,
        Vector::new(18.0, 0.0)
    );
    assert_eq!(updated_layer.descriptor.properties.opacity, 1.0);
}

#[test]
fn explicit_boundary_layer_opacity_marks_effect_without_content_rebuild() {
    let (mut runtime, window_id, root_counters, leaf_counters) = build_cached_move_runtime();

    let _ = runtime.render(window_id).unwrap();
    let layer_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;
    let root_paint_before = root_counters.borrow().paint;
    let leaf_paint_before = leaf_counters.borrow().paint;

    runtime
        .handle_event(
            window_id,
            Event::Custom(CustomEvent::new("fade-cached-layer")),
        )
        .unwrap();

    let second = runtime.render(window_id).unwrap();
    let updated_layer = second
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            SceneCommand::Layer(layer) if layer.widget_id() == layer_id => Some(layer.clone()),
            _ => None,
        })
        .expect("cached layer present after opacity update");

    assert_eq!(root_counters.borrow().paint, root_paint_before);
    assert_eq!(leaf_counters.borrow().paint, leaf_paint_before);
    assert_eq!(second.frame.layer_updates.len(), 1);
    assert_eq!(second.frame.layer_updates[0].owner, layer_id);
    assert_eq!(
        second.frame.layer_updates[0].kind,
        SceneLayerUpdateKind::Effect
    );
    assert_eq!(
        updated_layer.descriptor.properties.translation,
        Vector::ZERO
    );
    assert_eq!(updated_layer.descriptor.properties.opacity, 0.5);
}

#[test]
fn direct_flat_child_translation_repaints_nearest_boundary_root() {
    let (mut runtime, window_id, root_counters, leaf_counters) = build_direct_move_runtime();

    let first = runtime.render(window_id).unwrap();
    let layer_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;
    let root_paint_before = root_counters.borrow().paint;
    let leaf_paint_before = leaf_counters.borrow().paint;
    let initial_rect = first
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            SceneCommand::FillRect { rect, .. } if *rect != Rect::new(0.0, 0.0, 320.0, 180.0) => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("flat child fill present before translation");

    runtime
        .handle_event(window_id, Event::Custom(CustomEvent::new("shift-direct")))
        .unwrap();

    let second = runtime.render(window_id).unwrap();

    assert_eq!(root_counters.borrow().paint, root_paint_before + 1);
    assert_eq!(leaf_counters.borrow().paint, leaf_paint_before + 1);
    assert!(!scene_has_layer_for(&second.frame.scene, layer_id));
    assert!(second.frame.layer_updates.is_empty());

    let translated_rect = second
        .frame
        .scene
        .commands()
        .iter()
        .find_map(|command| match command {
            SceneCommand::FillRect { rect, .. } if *rect != Rect::new(0.0, 0.0, 320.0, 180.0) => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("flat child fill present after translation");

    assert_eq!(translated_rect.x(), initial_rect.x() + 36.0);
    assert_eq!(translated_rect.y(), initial_rect.y());
}

#[test]
fn semantics_attach_to_the_nearest_ancestor_node() {
    let root_counters = Rc::new(RefCell::new(Counters::default()));
    let leaf_counters = Rc::new(RefCell::new(Counters::default()));

    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Test").root(TestRoot {
            counters: Rc::clone(&root_counters),
            child: SingleChild::new(ChildRoot::new(FocusLeaf {
                counters: Rc::clone(&leaf_counters),
            })),
        }))
        .build()
        .unwrap();

    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id).unwrap();
    let root_id = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Window)
        .map(|node| node.id)
        .unwrap();
    let leaf = output
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::Button)
        .unwrap();

    assert_eq!(leaf.parent, Some(root_id));
}

#[test]
fn tab_traversal_moves_focus_between_focusable_widgets() {
    let (mut runtime, window_id, first_counters, second_counters) = build_focus_traversal_runtime();

    let _ = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();
    let first_id = graph.nodes[1].id;
    let second_id = graph.nodes[2].id;

    let mut pointer = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    pointer.button = Some(PointerButton::Primary);
    pointer.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(pointer))
        .unwrap();

    assert_eq!(
        runtime.focus_state(window_id).unwrap(),
        FocusState {
            focused_widget: Some(first_id),
            window_focused: true,
        }
    );
    assert!(
        runtime
            .widget_graph(window_id)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.id == first_id)
            .is_some_and(|node| node.focused)
    );

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
        )
        .unwrap();

    assert_eq!(first_counters.borrow().keyboard, 1);
    assert_eq!(
        runtime.focus_state(window_id).unwrap(),
        FocusState {
            focused_widget: Some(second_id),
            window_focused: true,
        }
    );
    assert!(
        runtime
            .widget_graph(window_id)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.id == second_id)
            .is_some_and(|node| node.focused)
    );

    let output = runtime.render(window_id).unwrap();
    assert!(
        output
            .semantics
            .iter()
            .find(|node| node.id == second_id)
            .is_some_and(|node| node.state.focused)
    );

    let mut reverse_tab = KeyboardEvent::new("Tab", KeyState::Pressed);
    reverse_tab.modifiers.shift = true;
    runtime
        .handle_event(window_id, Event::Keyboard(reverse_tab))
        .unwrap();

    assert_eq!(second_counters.borrow().keyboard, 1);
    assert_eq!(
        runtime.focus_state(window_id).unwrap(),
        FocusState {
            focused_widget: Some(first_id),
            window_focused: true,
        }
    );
    assert!(
        runtime
            .widget_graph(window_id)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.id == first_id)
            .is_some_and(|node| node.focused)
    );
}

#[test]
fn managed_overlay_traps_restores_and_traces_focus_and_dismissal() {
    let open = Rc::new(RefCell::new(false));
    let dismissals = Rc::new(RefCell::new(Vec::new()));
    let mut runtime =
        Application::new()
            .window(WindowBuilder::new().title("Managed overlay").root(
                ManagedOverlayTestRoot::new(Rc::clone(&open), Rc::clone(&dismissals)),
            ))
            .build()
            .unwrap();
    let window_id = runtime.window_ids()[0];
    let initial = runtime.render(window_id).unwrap();
    assert!(
        runtime
            .overlay_snapshot(window_id)
            .unwrap()
            .overlays
            .is_empty()
    );

    let graph = runtime.widget_graph(window_id).unwrap();
    let root_id = graph.nodes[0].id;
    let background_id = graph
        .nodes
        .iter()
        .find(|node| node.parent == Some(root_id) && node.accepts_focus)
        .unwrap()
        .id;
    assert!(
        initial
            .semantics
            .iter()
            .any(|node| node.id == background_id)
    );

    assert!(
        runtime
            .handle_semantics_action(window_id, background_id, SemanticsActionRequest::Focus,)
            .unwrap()
    );
    assert_eq!(
        runtime.focused_widget(window_id).unwrap(),
        Some(background_id)
    );

    runtime
        .handle_event(
            window_id,
            Event::Custom(CustomEvent::new("open-managed-overlay")),
        )
        .unwrap();
    let opened = runtime.render(window_id).unwrap();
    let snapshot = runtime.overlay_snapshot(window_id).unwrap();
    assert_eq!(snapshot.overlays.len(), 1);
    assert_eq!(snapshot.active_modal, Some(snapshot.overlays[0].owner));
    assert_eq!(snapshot.focus_trap, Some(snapshot.overlays[0].owner));
    assert!(!opened.semantics.iter().any(|node| node.id == background_id));
    assert!(
        opened
            .semantics
            .iter()
            .any(|node| { node.id == snapshot.overlays[0].owner && node.state.modal })
    );

    let first_overlay_focus = runtime.focused_widget(window_id).unwrap().unwrap();
    assert_ne!(first_overlay_focus, background_id);
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
        )
        .unwrap();
    let second_overlay_focus = runtime.focused_widget(window_id).unwrap().unwrap();
    assert_ne!(second_overlay_focus, first_overlay_focus);
    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Tab", KeyState::Pressed)),
        )
        .unwrap();
    assert_eq!(
        runtime.focused_widget(window_id).unwrap(),
        Some(first_overlay_focus)
    );

    runtime
        .handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("Escape", KeyState::Pressed)),
        )
        .unwrap();
    assert_eq!(
        dismissals.borrow().as_slice(),
        &[OverlayDismissReason::Escape]
    );
    let _ = runtime.render(window_id).unwrap();
    assert!(
        runtime
            .overlay_snapshot(window_id)
            .unwrap()
            .overlays
            .is_empty()
    );
    assert_eq!(
        runtime.focused_widget(window_id).unwrap(),
        Some(background_id)
    );

    let traces = runtime.take_overlay_traces(window_id).unwrap();
    for expected in [
        OverlayTraceKind::Opened,
        OverlayTraceKind::FocusEntered,
        OverlayTraceKind::DismissRequested,
        OverlayTraceKind::Closed,
        OverlayTraceKind::FocusRestored,
    ] {
        assert!(traces.iter().any(|trace| trace.kind == expected));
    }
}

#[test]
fn pointer_capture_routes_drag_events_until_pointer_up() {
    let (mut runtime, window_id, state) = build_pointer_capture_runtime();

    let _ = runtime.render(window_id).unwrap();
    let child_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 7;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    assert_eq!(
        runtime.pointer_capture_target(window_id, 7).unwrap(),
        Some(child_id)
    );

    let mut moved = PointerEvent::new(PointerEventKind::Move, Point::new(260.0, 140.0));
    moved.pointer_id = 7;
    moved.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(moved))
        .unwrap();

    let mut up = PointerEvent::new(PointerEventKind::Up, Point::new(260.0, 140.0));
    up.pointer_id = 7;
    up.button = Some(PointerButton::Primary);
    runtime.handle_event(window_id, Event::Pointer(up)).unwrap();

    assert_eq!(state.borrow().moves, 1);
    assert_eq!(state.borrow().ups, 1);
    assert_eq!(state.borrow().cancels, 0);
    assert_eq!(runtime.pointer_capture_target(window_id, 7).unwrap(), None);
}

#[test]
fn window_focus_loss_cancels_all_pointer_captures_and_applies_cleanup_effects() {
    let (mut runtime, window_id, state) = build_pointer_capture_runtime();

    let _ = runtime.render(window_id).unwrap();
    let child_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;

    for pointer_id in [31, 47] {
        let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
        down.pointer_id = pointer_id;
        down.pointer_kind = PointerKind::Touch;
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        runtime
            .handle_event(window_id, Event::Pointer(down))
            .unwrap();
        assert_eq!(
            runtime
                .pointer_capture_target(window_id, pointer_id)
                .unwrap(),
            Some(child_id)
        );
    }

    runtime
        .handle_event(window_id, Event::Window(WindowEvent::Focused(false)))
        .unwrap();

    assert_eq!(state.borrow().cancels, 2);
    assert_eq!(state.borrow().cancel_followups, 2);
    assert!(runtime.needs_render(window_id).unwrap());
    for pointer_id in [31, 47] {
        assert_eq!(
            runtime
                .pointer_capture_target(window_id, pointer_id)
                .unwrap(),
            None
        );
    }
}

#[test]
fn cursor_requests_route_raw_motion_and_focus_loss_restores_host_defaults() {
    let state = Rc::new(RefCell::new(CursorRequestState::default()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Cursor requests")
                .root(ChildRoot::new(CursorRequestLeaf {
                    state: Rc::clone(&state),
                })),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let _ = runtime.render(window_id).unwrap();

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Down,
                Point::new(48.0, 40.0),
            )),
        )
        .unwrap();
    let requested = runtime.window_cursor_state(window_id).unwrap();
    assert_eq!(requested.grab_mode, CursorGrabMode::Locked);
    assert!(!requested.visible);
    assert_eq!(requested.revision, 2);

    runtime
        .handle_event(
            window_id,
            Event::RawMouseMotion(RawMouseMotionEvent {
                delta: Vector::new(7.0, -3.0),
                modifiers: Modifiers::NONE,
            }),
        )
        .unwrap();
    assert_eq!(state.borrow().raw_motion, [Vector::new(7.0, -3.0)]);

    runtime
        .handle_event(window_id, Event::Window(WindowEvent::Focused(false)))
        .unwrap();
    let released = runtime.window_cursor_state(window_id).unwrap();
    assert_eq!(released.grab_mode, CursorGrabMode::None);
    assert!(released.visible);
    assert_eq!(released.revision, 3);

    runtime
        .handle_event(
            window_id,
            Event::RawMouseMotion(RawMouseMotionEvent {
                delta: Vector::new(1.0, 1.0),
                modifiers: Modifiers::NONE,
            }),
        )
        .unwrap();
    assert_eq!(state.borrow().raw_motion.len(), 1);
}

#[test]
fn pointer_capture_transfer_cancels_displaced_owner_and_preserves_new_owner() {
    let state = Rc::new(RefCell::new(PointerCaptureState::default()));
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Pointer Capture Transfer").root(
            PointerCaptureTransferRoot::new(PointerCaptureLeaf {
                state: Rc::clone(&state),
                recapture_on_move: false,
            }),
        ))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();
    let root_id = graph.root;
    let child_id = graph_child(&graph).id;

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 17;
    down.pointer_kind = PointerKind::Touch;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();
    assert_eq!(
        runtime.pointer_capture_target(window_id, 17).unwrap(),
        Some(child_id)
    );

    let mut moved = PointerEvent::new(PointerEventKind::Move, Point::new(48.0, 18.0));
    moved.pointer_id = 17;
    moved.pointer_kind = PointerKind::Touch;
    moved.delta = Vector::new(0.0, -22.0);
    moved.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(moved))
        .unwrap();

    assert_eq!(
        runtime.pointer_capture_target(window_id, 17).unwrap(),
        Some(root_id),
        "the displaced owner's release request must not clear the new capture"
    );
    assert!(
        runtime.needs_render(window_id).unwrap(),
        "the displaced owner's invalidations must be applied"
    );
    let state = state.borrow();
    assert_eq!(state.cancels, 1);
    assert_eq!(state.cancel_followups, 1);
    let cancel = state.last_cancel.as_ref().expect("cancel event recorded");
    assert_eq!(cancel.pointer_id, 17);
    assert_eq!(cancel.kind, PointerEventKind::Cancel);
    assert_eq!(cancel.position, Point::new(48.0, 18.0));
    assert_eq!(cancel.delta, Vector::ZERO);
    assert_eq!(cancel.pointer_kind, PointerKind::Touch);
    assert_eq!(cancel.button, None);
    assert_eq!(cancel.buttons, PointerButtons::NONE);
    assert_eq!(cancel.scroll_delta, None);
}

#[test]
fn pointer_capture_transfer_cancels_reentrant_intermediate_owner() {
    let state = Rc::new(RefCell::new(ReentrantCaptureState::default()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Reentrant Pointer Capture Transfer")
                .root(ReentrantCaptureRoot::new(Rc::clone(&state))),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();
    let root_id = graph.root;
    let source_id = graph.nodes[1].id;

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 59;
    down.pointer_kind = PointerKind::Touch;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();
    assert_eq!(
        runtime.pointer_capture_target(window_id, 59).unwrap(),
        Some(source_id)
    );

    let mut moved = PointerEvent::new(PointerEventKind::Move, Point::new(48.0, 18.0));
    moved.pointer_id = 59;
    moved.pointer_kind = PointerKind::Touch;
    moved.delta = Vector::new(0.0, -22.0);
    moved.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(moved))
        .unwrap();

    let state = state.borrow();
    assert_eq!(state.source_cancels, 1);
    assert_eq!(state.intermediate_captures, 1);
    assert_eq!(state.intermediate_cancels, 1);
    assert_eq!(
        runtime.pointer_capture_target(window_id, 59).unwrap(),
        Some(root_id),
        "the original transfer target must win after intermediate cleanup"
    );
}

#[test]
fn recapturing_pointer_for_same_owner_does_not_emit_cancel() {
    let state = Rc::new(RefCell::new(PointerCaptureState::default()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Pointer Recapture")
                .root(ChildRoot::new(PointerCaptureLeaf {
                    state: Rc::clone(&state),
                    recapture_on_move: true,
                })),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();
    let child_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 23;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let mut moved = PointerEvent::new(PointerEventKind::Move, Point::new(80.0, 40.0));
    moved.pointer_id = 23;
    moved.delta = Vector::new(32.0, 0.0);
    moved.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(moved))
        .unwrap();

    assert_eq!(state.borrow().cancels, 0);
    assert_eq!(
        runtime.pointer_capture_target(window_id, 23).unwrap(),
        Some(child_id)
    );
}

#[test]
fn active_drag_hit_tests_drop_target_while_source_keeps_pointer_capture() {
    let state = Rc::new(RefCell::new(RuntimeDragState::default()));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Runtime Drag")
                .root(RuntimeDragRoot::new(Rc::clone(&state))),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 7;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let mut moved = PointerEvent::new(PointerEventKind::Move, Point::new(188.0, 40.0));
    moved.pointer_id = 7;
    moved.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(moved))
        .unwrap();

    let mut up = PointerEvent::new(PointerEventKind::Up, Point::new(188.0, 40.0));
    up.pointer_id = 7;
    up.button = Some(PointerButton::Primary);
    runtime.handle_event(window_id, Event::Pointer(up)).unwrap();

    let state = state.borrow();
    assert_eq!(state.source_moves, 1);
    assert!(state.target_overs >= 1);
    assert_eq!(state.drops, 1);
    assert_eq!(state.ends, 1);
    assert!(matches!(
        state.outcome,
        Some(DragOutcome::Dropped {
            effect: DropEffect::Move,
            ..
        })
    ));
}

#[test]
fn pointer_move_synthesizes_widget_leave_and_enter_transitions() {
    let (mut runtime, window_id, first, second) = build_hover_transition_runtime();

    let _ = runtime.render(window_id).unwrap();

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                Point::new(48.0, 40.0),
            )),
        )
        .unwrap();

    assert_eq!(
        *first.borrow(),
        HoverTransitionState {
            enters: 1,
            leaves: 0,
            hovered: true,
        }
    );
    assert_eq!(*second.borrow(), HoverTransitionState::default());

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                Point::new(280.0, 140.0),
            )),
        )
        .unwrap();

    assert_eq!(
        *first.borrow(),
        HoverTransitionState {
            enters: 1,
            leaves: 1,
            hovered: false,
        }
    );

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Move,
                Point::new(188.0, 40.0),
            )),
        )
        .unwrap();

    assert_eq!(
        *second.borrow(),
        HoverTransitionState {
            enters: 1,
            leaves: 0,
            hovered: true,
        }
    );
}

#[test]
fn timers_and_async_wakeups_reenter_runtime_with_registered_target() {
    let (mut runtime, window_id, state) = build_wake_runtime();

    let _ = runtime.render(window_id).unwrap();

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 11;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let async_token = state.borrow().async_token.unwrap();
    let timer_token = state.borrow().timer_token.unwrap();

    let scheduler = runtime.inspector_snapshot(window_id).unwrap().scheduler;
    assert!(
        scheduler.timers.iter().any(|timer| {
            timer.token == timer_token && timer.deadline == 3.0 && !timer.delivering
        })
    );
    assert!(
        scheduler
            .async_tasks
            .iter()
            .any(|task| { task.token == async_token && !task.pending_wake })
    );

    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), Some(3.0));
    assert!(runtime.wake_async(window_id, async_token).unwrap());

    let ready = runtime.drain_ready_events();
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        ready[0],
        (ready_window, Event::Wake(WakeEvent::Async { token, time }))
            if ready_window == window_id && token == async_token && time == 0.0
    ));

    for (ready_window, event) in ready {
        runtime.handle_event(ready_window, event).unwrap();
    }

    assert_eq!(state.borrow().async_wakes, 1);

    runtime.tick(3.0);
    let ready = runtime.drain_ready_events();
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        ready[0],
        (ready_window, Event::Wake(WakeEvent::Timer { token, time, deadline }))
            if ready_window == window_id && token == timer_token && time == 3.0 && deadline == 3.0
    ));

    for (ready_window, event) in ready {
        runtime.handle_event(ready_window, event).unwrap();
    }

    assert_eq!(state.borrow().timer_wakes, 1);
    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), None);
}

#[test]
fn animation_frame_wakeups_reenter_runtime_with_registered_target() {
    let (mut runtime, window_id, state) = build_animation_wake_runtime(false);

    let _ = runtime.render(window_id).unwrap();

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 12;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), Some(0.0));

    let ready = runtime.drain_ready_events();
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        ready[0],
        (
            ready_window,
            Event::Wake(WakeEvent::AnimationFrame {
                time,
                delta,
                frame_index,
            })
        ) if ready_window == window_id && time == 0.0 && delta == 0.0 && frame_index == 0
    ));

    for (ready_window, event) in ready {
        runtime.handle_event(ready_window, event).unwrap();
    }

    let state = state.borrow();
    assert_eq!(state.animation_wakes, 1);
    assert_eq!(state.last_animation_time, Some(0.0));
    assert_eq!(state.last_animation_delta, Some(0.0));
    assert_eq!(state.last_animation_frame_index, Some(0));
    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), None);
}

#[test]
fn animation_frame_wakeups_are_target_only_not_captured_by_ancestors() {
    let state = Rc::new(RefCell::new(AnimationWakeState::default()));
    let ancestor_wakes = Rc::new(RefCell::new(0usize));
    let mut runtime =
        Application::new()
            .window(WindowBuilder::new().title("Animation Capture").root(
                AnimationWakeCapturingRoot {
                    child: SingleChild::new(AnimationWakeLeaf {
                        state: Rc::clone(&state),
                    }),
                    ancestor_wakes: Rc::clone(&ancestor_wakes),
                },
            ))
            .build()
            .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 15;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let ready = runtime.drain_ready_events();
    assert_eq!(ready.len(), 1);
    for (ready_window, event) in ready {
        runtime.handle_event(ready_window, event).unwrap();
    }

    assert_eq!(state.borrow().animation_wakes, 1);
    assert_eq!(*ancestor_wakes.borrow(), 0);
    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), None);
}

#[test]
fn repeated_animation_frame_requests_are_idempotent_for_the_same_widget() {
    let (mut runtime, window_id, state) = build_animation_wake_runtime(true);

    let _ = runtime.render(window_id).unwrap();

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 13;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    let ready = runtime.drain_ready_events();
    assert_eq!(ready.len(), 1);

    for (ready_window, event) in ready {
        runtime.handle_event(ready_window, event).unwrap();
    }

    assert_eq!(state.borrow().animation_wakes, 1);
    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), None);
}

#[test]
fn animated_widget_registration_is_cleaned_up_when_widget_disappears() {
    let (mut runtime, window_id) = build_removable_animation_runtime();

    let _ = runtime.render(window_id).unwrap();

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(48.0, 40.0));
    down.pointer_id = 14;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime
        .handle_event(window_id, Event::Pointer(down))
        .unwrap();

    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), Some(0.0));

    runtime
        .handle_event(
            window_id,
            Event::Custom(CustomEvent::new("drop-animation-child")),
        )
        .unwrap();
    let _ = runtime.render(window_id).unwrap();

    assert_eq!(runtime.next_wakeup_time(window_id).unwrap(), None);
    assert!(runtime.drain_ready_events().is_empty());
}

#[test]
fn initial_runtime_needs_render() {
    let (runtime, window_id, _, _) = build_runtime();

    assert!(runtime.needs_render(window_id).unwrap());
}

#[test]
fn runtime_render_populates_widget_count_diagnostics() {
    let (mut runtime, window_id, _, _) = build_runtime();

    let output = runtime.render(window_id).unwrap();
    let graph = runtime.widget_graph(window_id).unwrap();

    assert_eq!(output.diagnostics.widget_count, graph.nodes.len());
}

#[test]
fn inspector_snapshot_unifies_structure_routes_and_widget_diagnostics() {
    let (mut runtime, window_id, _, _) = build_runtime();
    runtime.set_inspector_tracing(window_id, true).unwrap();
    runtime.render(window_id).unwrap();

    runtime
        .handle_event(
            window_id,
            Event::Pointer(PointerEvent::new(
                PointerEventKind::Down,
                Point::new(48.0, 40.0),
            )),
        )
        .unwrap();

    let snapshot = runtime.inspector_snapshot(window_id).unwrap();
    assert_eq!(snapshot.title, "Test");
    assert!(!snapshot.semantics.is_empty());
    assert!(
        snapshot
            .widget_graph
            .nodes
            .iter()
            .any(|node| node.widget_name.ends_with("TestRoot"))
    );
    assert!(snapshot.widget_diagnostics.iter().any(|diagnostics| {
        diagnostics.widget_name.ends_with("TestRoot")
            && diagnostics
                .entries
                .iter()
                .any(|entry| entry.name == "test state" && entry.value == "available")
    }));
    let route = snapshot.history.event_routes.last().unwrap();
    assert_eq!(route.event_kind, "pointer");
    assert!(route.steps.iter().any(|step| {
        step.phase == EventRoutePhase::Capture || step.phase == EventRoutePhase::Target
    }));
    assert!(snapshot.scene.is_some());
}

struct ReactiveTextLeaf {
    text: Signal<String>,
    measures: Arc<AtomicUsize>,
}

impl Widget for ReactiveTextLeaf {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text = ctx.observe_with(&self.text, InvalidationKind::Text);
        self.measures.fetch_add(1, Ordering::Relaxed);
        constraints.clamp(Size::new(text.len() as f32 * 8.0, 24.0))
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.text.get());
        ctx.push(node);
    }
}

#[test]
fn observable_change_targets_widget_and_reports_diagnostics() {
    let text = Signal::named("status_text", "Ready".to_string());
    let measures = Arc::new(AtomicUsize::new(0));
    let mut runtime = Application::new()
        .window(WindowBuilder::new().root(ReactiveTextLeaf {
            text: text.clone(),
            measures: Arc::clone(&measures),
        }))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let wakes = Arc::new(AtomicUsize::new(0));
    let wake_count = Arc::clone(&wakes);
    runtime.set_external_waker(move || {
        wake_count.fetch_add(1, Ordering::Relaxed);
    });

    let first = runtime.render(window_id).unwrap();
    assert_eq!(measures.load(Ordering::Relaxed), 1);
    assert_eq!(first.semantics[0].name.as_deref(), Some("Ready"));

    assert!(text.set("Connecting".to_string()));
    assert!(text.set("Connected".to_string()));
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert!(runtime.needs_render(window_id).unwrap());

    let second = runtime.render(window_id).unwrap();
    assert_eq!(measures.load(Ordering::Relaxed), 2);
    assert_eq!(second.semantics[0].name.as_deref(), Some("Connected"));
    let samples = second
        .diagnostics
        .reactive_invalidations
        .iter()
        .filter(|sample| {
            sample.source_name == "status_text"
                && sample.kind == InvalidationKind::Text
                && sample.delivered
        })
        .collect::<Vec<_>>();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].version, 2);
}

static TEST_COMMAND: CommandKey<u32> = CommandKey::new("runtime.test.command");

struct CommandRoot {
    commands: Arc<AtomicUsize>,
    custom_events: Arc<AtomicUsize>,
}

impl Widget for CommandRoot {
    fn event(&mut self, _ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Custom(_)) {
            self.custom_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(TEST_COMMAND).is_some() {
            self.commands.fetch_add(1, Ordering::Relaxed);
            ctx.request_paint();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 80.0))
    }
}

struct WakeController(Arc<AtomicUsize>);

impl CommandController for WakeController {
    fn wake(&mut self, _ctx: &mut CommandCtx) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

struct EventCommandRoot;

impl Widget for EventCommandRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Custom(_)) {
            ctx.command_sender().send_application(TEST_COMMAND, 41);
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 80.0))
    }
}

#[test]
fn scheduler_wake_invokes_controllers_without_synthesizing_a_root_event() {
    let controller_wakes = Arc::new(AtomicUsize::new(0));
    let custom_events = Arc::new(AtomicUsize::new(0));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .controller(WakeController(Arc::clone(&controller_wakes)))
                .root(CommandRoot {
                    commands: Arc::new(AtomicUsize::new(0)),
                    custom_events: Arc::clone(&custom_events),
                }),
        )
        .build()
        .unwrap();

    runtime.command_sender().wake();
    runtime.process_commands();

    assert_eq!(controller_wakes.load(Ordering::Relaxed), 1);
    assert_eq!(custom_events.load(Ordering::Relaxed), 0);
}

#[test]
fn widget_event_context_can_enqueue_an_application_command() {
    let received = Arc::new(AtomicUsize::new(0));
    let received_by_handler = Arc::clone(&received);
    let mut runtime = Application::new()
        .on_command(TEST_COMMAND, move |ctx, value| {
            received_by_handler.store(*value as usize, Ordering::Relaxed);
            ctx.set_handled();
        })
        .window(WindowBuilder::new().root(EventCommandRoot))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    runtime
        .handle_event(
            window_id,
            Event::Custom(CustomEvent::new("runtime.test.enqueue-command")),
        )
        .unwrap();
    assert_eq!(received.load(Ordering::Relaxed), 0);

    runtime.process_commands();
    assert_eq!(received.load(Ordering::Relaxed), 41);
}

#[test]
fn directed_commands_stop_when_handled_while_broadcast_reaches_all_subscribers() {
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    let first_handler = Arc::clone(&first);
    let second_handler = Arc::clone(&second);
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .on_command(TEST_COMMAND, move |ctx, _| {
                    first_handler.fetch_add(1, Ordering::Relaxed);
                    ctx.set_handled();
                })
                .on_command(TEST_COMMAND, move |_, _| {
                    second_handler.fetch_add(1, Ordering::Relaxed);
                })
                .root(CommandRoot {
                    commands: Arc::new(AtomicUsize::new(0)),
                    custom_events: Arc::new(AtomicUsize::new(0)),
                }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let sender = runtime.command_sender();

    sender.send_window(window_id, TEST_COMMAND, 1);
    runtime.process_commands();
    assert_eq!(first.load(Ordering::Relaxed), 1);
    assert_eq!(second.load(Ordering::Relaxed), 0);

    sender.broadcast_window(window_id, TEST_COMMAND, 2);
    runtime.process_commands();
    assert_eq!(first.load(Ordering::Relaxed), 2);
    assert_eq!(second.load(Ordering::Relaxed), 1);
}

#[test]
fn application_broadcast_is_thread_safe_multicast_and_drops_removed_window_subscriptions() {
    let application_count = Arc::new(AtomicUsize::new(0));
    let first_window_count = Arc::new(AtomicUsize::new(0));
    let second_window_count = Arc::new(AtomicUsize::new(0));
    let application_handler = Arc::clone(&application_count);
    let first_handler = Arc::clone(&first_window_count);
    let second_handler = Arc::clone(&second_window_count);
    let root = || CommandRoot {
        commands: Arc::new(AtomicUsize::new(0)),
        custom_events: Arc::new(AtomicUsize::new(0)),
    };
    let mut runtime = Application::new()
        .on_command(TEST_COMMAND, move |_, _| {
            application_handler.fetch_add(1, Ordering::Relaxed);
        })
        .window(
            WindowBuilder::new()
                .on_command(TEST_COMMAND, move |_, _| {
                    first_handler.fetch_add(1, Ordering::Relaxed);
                })
                .root(root()),
        )
        .window(
            WindowBuilder::new()
                .on_command(TEST_COMMAND, move |_, _| {
                    second_handler.fetch_add(1, Ordering::Relaxed);
                })
                .root(root()),
        )
        .build()
        .unwrap();
    let first_window = runtime.window_ids()[0];
    let sender = runtime.command_sender();
    let producer = std::thread::spawn(move || {
        sender.broadcast_application(TEST_COMMAND, 1);
        sender
    });
    let sender = producer.join().unwrap();

    runtime.process_commands();
    assert_eq!(application_count.load(Ordering::Relaxed), 1);
    assert_eq!(first_window_count.load(Ordering::Relaxed), 1);
    assert_eq!(second_window_count.load(Ordering::Relaxed), 1);

    runtime.remove_window(first_window).unwrap();
    sender.broadcast_application(TEST_COMMAND, 2);
    runtime.process_commands();
    assert_eq!(application_count.load(Ordering::Relaxed), 2);
    assert_eq!(first_window_count.load(Ordering::Relaxed), 1);
    assert_eq!(second_window_count.load(Ordering::Relaxed), 2);
}

#[test]
fn widget_commands_and_controller_invalidations_are_traced() {
    let commands = Arc::new(AtomicUsize::new(0));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .on_command(TEST_COMMAND, |ctx, _| {
                    ctx.request_window_with_reason(
                        InvalidationKind::Paint,
                        "test command changed presentation",
                    );
                })
                .root(CommandRoot {
                    commands: Arc::clone(&commands),
                    custom_events: Arc::new(AtomicUsize::new(0)),
                }),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    runtime.render(window_id).unwrap();
    let root_id = runtime.widget_graph(window_id).unwrap().root;
    let sender = runtime.command_sender();

    sender.send_widget(window_id, root_id, TEST_COMMAND, 1);
    sender.send_widget(window_id, WidgetId::new(u64::MAX), TEST_COMMAND, 2);
    sender.send_window(window_id, TEST_COMMAND, 3);
    runtime.process_commands();
    let output = runtime.render(window_id).unwrap();

    assert_eq!(commands.load(Ordering::Relaxed), 1);
    assert!(output.diagnostics.command_dispatches.iter().any(|sample| {
        sample.name == TEST_COMMAND.name()
            && sample.payload_type == std::any::type_name::<u32>()
            && sample.delivered
    }));
    assert!(output.diagnostics.command_dispatches.iter().any(|sample| {
            sample.name == TEST_COMMAND.name()
                && matches!(sample.target, CommandTarget::Widget { widget_id, .. } if widget_id == WidgetId::new(u64::MAX))
                && !sample.delivered
        }));
    assert!(output.diagnostics.invalidations.iter().any(|sample| {
        sample.source == format!("command:{}", TEST_COMMAND.name())
            && sample.reason.as_deref() == Some("test command changed presentation")
    }));
}

#[test]
fn focused_command_without_focus_is_dropped_instead_of_targeting_the_root() {
    let commands = Arc::new(AtomicUsize::new(0));
    let mut runtime = Application::new()
        .window(WindowBuilder::new().root(CommandRoot {
            commands: Arc::clone(&commands),
            custom_events: Arc::new(AtomicUsize::new(0)),
        }))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    runtime.render(window_id).unwrap();

    runtime
        .command_sender()
        .send_focused(window_id, TEST_COMMAND, 1);
    runtime.process_commands();
    let output = runtime.render(window_id).unwrap();

    assert_eq!(commands.load(Ordering::Relaxed), 0);
    assert!(output.diagnostics.command_dispatches.iter().any(|sample| {
        sample.name == TEST_COMMAND.name()
            && sample.target == CommandTarget::FocusedWidget(window_id)
            && !sample.delivered
    }));
}

#[test]
fn runtime_render_reports_ime_composition_rect_for_shaped_text_widgets() {
    let (mut runtime, window_id) = build_text_runtime();
    set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);

    let output = runtime.render(window_id).unwrap();

    assert!(output.ime_composition_rect.is_some());
    assert!(output.diagnostics.text_caches.runtime_layout.misses > 0);
    assert!(!output.frame.scene.commands().is_empty());
    let mut saw_shaped_text = false;
    output.frame.scene.visit_commands(&mut |command| {
        if matches!(command, sui_scene::SceneCommand::DrawShapedText(_)) {
            saw_shaped_text = true;
        }
    });
    assert!(saw_shaped_text);
}

#[test]
fn browser_back_clears_ime_focus_before_becoming_unhandled() {
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Browser Back IME")
                .root(PaintedChildRoot::new(FocusedImeLeaf)),
        )
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    let initial = runtime.render(window_id).unwrap();
    let text_input = initial
        .semantics
        .iter()
        .find(|node| node.role == SemanticsRole::TextInput)
        .expect("focused IME test input")
        .id;

    assert!(
        runtime
            .handle_semantics_action(window_id, text_input, SemanticsActionRequest::Focus)
            .unwrap()
    );
    assert!(
        runtime
            .render(window_id)
            .unwrap()
            .ime_composition_rect
            .is_some()
    );

    assert!(
        runtime
            .dispatch_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("BrowserBack", KeyState::Pressed)),
            )
            .unwrap()
    );
    assert_eq!(runtime.focused_widget(window_id).unwrap(), None);
    assert!(
        runtime
            .render(window_id)
            .unwrap()
            .ime_composition_rect
            .is_none()
    );

    assert!(
        !runtime
            .dispatch_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("BrowserBack", KeyState::Pressed)),
            )
            .unwrap()
    );
}

#[test]
fn dispatch_event_reports_widget_handled_browser_back() {
    let (mut runtime, window_id, _, leaf_counters) = build_runtime();
    let initial = runtime.render(window_id).unwrap();
    let focus_leaf = initial
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("focus-leaf"))
        .expect("focus leaf")
        .id;
    assert!(
        runtime
            .handle_semantics_action(window_id, focus_leaf, SemanticsActionRequest::Focus)
            .unwrap()
    );

    assert!(
        runtime
            .dispatch_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("BrowserBack", KeyState::Pressed)),
            )
            .unwrap()
    );
    assert_eq!(leaf_counters.borrow().keyboard, 1);
}

#[test]
fn text_layout_retention_follows_retained_scene_handles() {
    let system = TextSystem::new();
    let first = system
        .shape_text_persistent(
            None,
            "first",
            Size::new(120.0, 24.0),
            TextStyle::new(Color::WHITE),
            &sui_text::FontRegistry::new(),
        )
        .unwrap();
    let second = system
        .shape_text_persistent(
            None,
            "second",
            Size::new(120.0, 24.0),
            TextStyle::new(Color::WHITE),
            &sui_text::FontRegistry::new(),
        )
        .unwrap();

    let mut scene = Scene::new();
    scene.push(SceneCommand::DrawShapedText(sui_text::ShapedText::new(
        Point::ZERO,
        &first,
    )));
    super::retain_text_layouts_for_scene(&system, &scene);

    let registry = system.text_layout_registry();
    assert!(registry.contains(first.handle()));
    assert!(!registry.contains(second.handle()));
}

struct TransformedProbe {
    pointer_positions: Rc<RefCell<Vec<Point>>>,
}

impl Widget for TransformedProbe {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Down
        {
            self.pointer_positions.borrow_mut().push(pointer.position);
            ctx.set_handled();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(100.0, 60.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(Color::rgba(0.85, 0.2, 0.1, 1.0));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some("transformed probe".to_string());
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }
}

struct TransformedHost {
    child: SingleChild,
    transform: Transform,
}

impl Widget for TransformedHost {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child
            .measure(ctx, Constraints::tight(Size::new(100.0, 60.0)));
        constraints.clamp(Size::new(480.0, 320.0))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, _bounds: Rect) {
        self.child
            .arrange_transformed(ctx, Rect::new(10.0, 20.0, 100.0, 60.0), self.transform);
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

#[test]
fn transformed_widget_subtree_aligns_paint_input_and_semantics() {
    let pointer_positions = Rc::new(RefCell::new(Vec::new()));
    let transform = Transform::scale(2.0, 2.0).then(Transform::translation(100.0, 50.0));
    let app = Application::new()
        .window(
            WindowBuilder::new()
                .title("transformed")
                .root(TransformedHost {
                    child: SingleChild::new(TransformedProbe {
                        pointer_positions: Rc::clone(&pointer_positions),
                    }),
                    transform,
                }),
        )
        .build()
        .unwrap();
    let mut runtime = app;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id).unwrap();
    let semantics = output
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("transformed probe"))
        .expect("transformed semantics");
    assert_eq!(semantics.bounds, Rect::new(120.0, 90.0, 200.0, 120.0));

    let graph = runtime.widget_graph(window_id).unwrap();
    let node = graph
        .nodes
        .iter()
        .find(|node| node.id == semantics.id)
        .expect("transformed widget graph node");
    assert_eq!(node.local_bounds, Rect::new(10.0, 20.0, 100.0, 60.0));
    assert_eq!(node.geometry.input_bounds, semantics.bounds);
    assert_eq!(node.presentation_transform, transform);

    let mut down = PointerEvent::new(PointerEventKind::Down, Point::new(140.0, 110.0));
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    assert!(
        runtime
            .dispatch_event(window_id, Event::Pointer(down))
            .unwrap()
    );
    assert_eq!(&*pointer_positions.borrow(), &[Point::new(20.0, 30.0)]);

    let has_transform = output.frame.scene.commands().iter().any(|command| {
        matches!(command, SceneCommand::PushTransform { transform: actual } if *actual == transform)
    });
    assert!(has_transform);
}

#[test]
fn removing_a_window_tears_down_runtime_state() {
    let (mut runtime, window_id, _, _) = build_runtime();
    set_window_render_options(window_id, WindowRenderOptions::new(false, 1.0));
    assert!(window_render_options(window_id).is_some());

    runtime.remove_window(window_id).unwrap();

    assert!(runtime.window_ids().is_empty());
    assert!(window_render_options(window_id).is_none());
    assert!(runtime.needs_render(window_id).is_err());
    assert!(runtime.focus_state(window_id).is_err());
    assert!(runtime.render(window_id).is_err());
}
