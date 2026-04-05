use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use sui_core::{
    AsyncWakeToken, Color, DpiInfo, Event, InvalidationKind, InvalidationRequest,
    InvalidationTarget, Path, Point, Rect, SemanticsNode, Size, TimerToken, Transform, Vector,
    WidgetId, WindowId,
};
use sui_layout::Constraints;
use sui_scene::{Brush, ImageRegistry, ImageSource, Scene, SceneCommand, SceneLayer, StrokeStyle};
use sui_text::{
    FontRegistry, ShapedText, TextLayout, TextMeasurement, TextRun, TextStyle, TextSystem,
};

static NEXT_WIDGET_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_TIMER_TOKEN: AtomicU64 = AtomicU64::new(1);
static NEXT_ASYNC_WAKE_TOKEN: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

pub trait WidgetPodVisitor {
    fn visit(&mut self, child: &WidgetPod);
}

pub trait WidgetPodMutVisitor {
    fn visit(&mut self, child: &mut WidgetPod);
}

pub trait Widget {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.max
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: Rect) {}

    fn paint(&self, _ctx: &mut PaintCtx) {}

    fn semantics(&self, _ctx: &mut SemanticsCtx) {}

    fn accepts_focus(&self) -> bool {
        false
    }

    fn focus_changed(&mut self, _ctx: &mut EventCtx, _focused: bool) {}

    fn visit_children(&self, _visitor: &mut dyn WidgetPodVisitor) {}

    fn visit_children_mut(&mut self, _visitor: &mut dyn WidgetPodMutVisitor) {}
}

pub struct SingleChild {
    child: WidgetPod,
}

impl SingleChild {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::from_pod(WidgetPod::new(child))
    }

    pub fn from_pod(child: WidgetPod) -> Self {
        Self { child }
    }

    pub fn child(&self) -> &WidgetPod {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        &mut self.child
    }

    pub fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    pub fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    pub fn set_bounds(&mut self, bounds: Rect) {
        self.child.set_bounds(bounds);
    }

    pub fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    pub fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    pub fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        visitor.visit(&self.child);
    }

    pub fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        visitor.visit(&mut self.child);
    }
}

#[derive(Default)]
pub struct WidgetChildren {
    children: Vec<WidgetPod>,
}

impl WidgetChildren {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            children: Vec::with_capacity(capacity),
        }
    }

    pub fn push<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        self.push_pod(WidgetPod::new(child));
    }

    pub fn push_pod(&mut self, child: WidgetPod) {
        self.children.push(child);
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    pub fn as_slice(&self) -> &[WidgetPod] {
        &self.children
    }

    pub fn as_mut_slice(&mut self) -> &mut [WidgetPod] {
        &mut self.children
    }

    pub fn measure_child(
        &mut self,
        index: usize,
        ctx: &mut MeasureCtx,
        constraints: Constraints,
    ) -> Size {
        self.children[index].measure(ctx, constraints)
    }

    pub fn arrange_child(&mut self, index: usize, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children[index].arrange(ctx, bounds);
    }

    pub fn paint(&self, ctx: &mut PaintCtx) {
        for child in &self.children {
            child.paint(ctx);
        }
    }

    pub fn semantics(&self, ctx: &mut SemanticsCtx) {
        for child in &self.children {
            child.semantics(ctx);
        }
    }

    pub fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for child in &self.children {
            visitor.visit(child);
        }
    }

    pub fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for child in &mut self.children {
            visitor.visit(child);
        }
    }
}

pub struct WidgetPod {
    id: WidgetId,
    layout_state: LayoutState,
    widget: Box<dyn Widget>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LayoutState {
    measured_size: Size,
    arranged_bounds: Rect,
    last_constraints: Constraints,
    measure_valid: bool,
    arrange_valid: bool,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            measured_size: Size::ZERO,
            arranged_bounds: Rect::ZERO,
            last_constraints: Constraints::default(),
            measure_valid: false,
            arrange_valid: false,
        }
    }
}

impl WidgetPod {
    pub fn new<W>(widget: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            id: WidgetId::new(NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed)),
            layout_state: LayoutState::default(),
            widget: Box::new(widget),
        }
    }

    pub const fn id(&self) -> WidgetId {
        self.id
    }

    pub const fn bounds(&self) -> Rect {
        self.layout_state.arranged_bounds
    }

    pub const fn measured_size(&self) -> Size {
        self.layout_state.measured_size
    }

    pub fn set_bounds(&mut self, bounds: Rect) {
        let delta = bounds.origin - self.layout_state.arranged_bounds.origin;
        self.layout_state.arranged_bounds = bounds;
        self.layout_state.measured_size = bounds.size;
        self.layout_state.arrange_valid = true;
        self.translate_descendants(delta);
    }

    pub fn measure(&mut self, parent_ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let origin = self.layout_state.arranged_bounds.origin;
        let mut child_ctx = MeasureCtx::new(
            parent_ctx.window_id(),
            self.id,
            self.layout_state.arranged_bounds,
            parent_ctx.dpi(),
            Arc::clone(&parent_ctx.text_system),
            Arc::clone(&parent_ctx.font_registry),
            Arc::clone(&parent_ctx.image_registry),
        );
        let size = self.widget.measure(&mut child_ctx, constraints);
        self.layout_state.measured_size = size;
        self.layout_state.last_constraints = constraints;
        self.layout_state.measure_valid = true;
        self.layout_state.arranged_bounds = Rect::from_origin_size(origin, size);
        parent_ctx.extend_invalidations(child_ctx.take_invalidations());
        size
    }

    pub fn arrange(&mut self, parent_ctx: &mut ArrangeCtx, bounds: Rect) {
        let delta = bounds.origin - self.layout_state.arranged_bounds.origin;
        self.layout_state.arranged_bounds = bounds;
        self.layout_state.arrange_valid = true;
        self.translate_descendants(delta);

        let mut child_ctx = ArrangeCtx::new(parent_ctx.window_id(), self.id, parent_ctx.dpi());
        self.widget.arrange(&mut child_ctx, bounds);
        parent_ctx.extend_invalidations(child_ctx.take_invalidations());
    }

    pub fn paint(&self, parent_ctx: &mut PaintCtx) {
        let mut child_ctx = PaintCtx::new(
            parent_ctx.window_id(),
            self.id,
            self.layout_state.arranged_bounds,
            parent_ctx.focused_widget_id(),
            parent_ctx.dpi(),
        );
        self.widget.paint(&mut child_ctx);

        let (scene, invalidations, ime_composition_rect) = child_ctx.into_parts();
        parent_ctx.push_layer(self.id, self.layout_state.arranged_bounds, scene);
        parent_ctx.extend_invalidations(invalidations);
        parent_ctx.extend_ime_composition_rect(ime_composition_rect);
    }

    pub(crate) fn paint_layer_contents_for(
        &mut self,
        target: WidgetId,
        parent_ctx: &mut PaintCtx,
    ) -> bool {
        self.find_mut(target, &mut |pod| pod.widget.paint(parent_ctx))
            .is_some()
    }

    pub fn semantics(&self, parent_ctx: &mut SemanticsCtx) {
        let mut child_ctx = SemanticsCtx::new(
            parent_ctx.window_id(),
            self.id,
            parent_ctx.root_widget_id(),
            self.layout_state.arranged_bounds,
            parent_ctx.focused_widget_id(),
        );
        self.widget.semantics(&mut child_ctx);
        parent_ctx.extend_nodes(child_ctx.into_nodes());
    }

    pub(crate) fn accepts_focus(&self) -> bool {
        self.widget.accepts_focus()
    }

    pub(crate) fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.widget.visit_children(visitor);
    }

    pub(crate) fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.widget.visit_children_mut(visitor);
    }

    pub(crate) fn dispatch_event_for(
        &mut self,
        target: WidgetId,
        window_id: WindowId,
        current_time: f64,
        phase: EventPhase,
        focused_widget: Option<WidgetId>,
        event: &Event,
    ) -> Option<EventDispatch> {
        self.find_mut(target, &mut |pod| {
            pod.dispatch_event(window_id, current_time, phase, focused_widget, event)
        })
    }

    pub(crate) fn notify_focus_change_for(
        &mut self,
        target: WidgetId,
        window_id: WindowId,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        focused: bool,
    ) -> Option<EventDispatch> {
        self.find_mut(target, &mut |pod| {
            pod.focus_changed(window_id, current_time, focused_widget, focused)
        })
    }

    fn dispatch_event(
        &mut self,
        window_id: WindowId,
        current_time: f64,
        phase: EventPhase,
        focused_widget: Option<WidgetId>,
        event: &Event,
    ) -> EventDispatch {
        let mut ctx = EventCtx::new(
            window_id,
            self.id,
            self.layout_state.arranged_bounds,
            current_time,
            phase,
            focused_widget,
        );
        self.widget.event(&mut ctx, event);
        EventDispatch {
            handled: ctx.is_handled(),
            invalidations: ctx.take_invalidations(),
            focus_request: ctx.take_focus_request(),
            wake_requests: ctx.take_wake_requests(),
            pointer_capture_requests: ctx.take_pointer_capture_requests(),
        }
    }

    fn focus_changed(
        &mut self,
        window_id: WindowId,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        focused: bool,
    ) -> EventDispatch {
        let mut ctx = EventCtx::new(
            window_id,
            self.id,
            self.layout_state.arranged_bounds,
            current_time,
            EventPhase::Target,
            focused_widget,
        );
        self.widget.focus_changed(&mut ctx, focused);
        EventDispatch {
            handled: ctx.is_handled(),
            invalidations: ctx.take_invalidations(),
            focus_request: ctx.take_focus_request(),
            wake_requests: ctx.take_wake_requests(),
            pointer_capture_requests: ctx.take_pointer_capture_requests(),
        }
    }

    fn find_mut<R, F>(&mut self, target: WidgetId, f: &mut F) -> Option<R>
    where
        F: FnMut(&mut WidgetPod) -> R,
    {
        if self.id == target {
            return Some(f(self));
        }

        let mut result = None;
        let mut visitor = FindMutVisitor {
            target,
            callback: f,
            result: &mut result,
        };
        self.visit_children_mut(&mut visitor);
        result
    }

    fn translate_descendants(&mut self, delta: Vector) {
        if delta == Vector::ZERO {
            return;
        }

        let mut visitor = TranslateVisitor { delta };
        self.visit_children_mut(&mut visitor);
    }

    fn translate_subtree(&mut self, delta: Vector) {
        if delta == Vector::ZERO {
            return;
        }

        self.layout_state.arranged_bounds = self.layout_state.arranged_bounds.translate(delta);
        self.translate_descendants(delta);
    }
}

struct TranslateVisitor {
    delta: Vector,
}

impl WidgetPodMutVisitor for TranslateVisitor {
    fn visit(&mut self, child: &mut WidgetPod) {
        child.translate_subtree(self.delta);
    }
}

struct FindMutVisitor<'a, F, R>
where
    F: FnMut(&mut WidgetPod) -> R,
{
    target: WidgetId,
    callback: &'a mut F,
    result: &'a mut Option<R>,
}

impl<F, R> WidgetPodMutVisitor for FindMutVisitor<'_, F, R>
where
    F: FnMut(&mut WidgetPod) -> R,
{
    fn visit(&mut self, child: &mut WidgetPod) {
        if self.result.is_none() {
            *self.result = child.find_mut(self.target, self.callback);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusRequest {
    Focus(WidgetId),
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WakeRequest {
    ScheduleTimer {
        token: TimerToken,
        deadline: f64,
        target: WidgetId,
    },
    CancelTimer {
        token: TimerToken,
    },
    RegisterAsync {
        token: AsyncWakeToken,
        target: WidgetId,
    },
    UnregisterAsync {
        token: AsyncWakeToken,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerCaptureRequest {
    Capture { pointer_id: u64, target: WidgetId },
    Release { pointer_id: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct EventDispatch {
    pub handled: bool,
    pub invalidations: Vec<InvalidationRequest>,
    pub focus_request: Option<FocusRequest>,
    pub wake_requests: Vec<WakeRequest>,
    pub pointer_capture_requests: Vec<PointerCaptureRequest>,
}

#[derive(Debug, Clone)]
pub struct EventCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    bounds: Rect,
    current_time: f64,
    phase: EventPhase,
    focused_widget_id: Option<WidgetId>,
    handled: bool,
    invalidations: Vec<InvalidationRequest>,
    focus_request: Option<FocusRequest>,
    wake_requests: Vec<WakeRequest>,
    pointer_capture_requests: Vec<PointerCaptureRequest>,
}

impl EventCtx {
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        current_time: f64,
        phase: EventPhase,
        focused_widget_id: Option<WidgetId>,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            bounds,
            current_time,
            phase,
            focused_widget_id,
            handled: false,
            invalidations: Vec::new(),
            focus_request: None,
            wake_requests: Vec::new(),
            pointer_capture_requests: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub const fn current_time(&self) -> f64 {
        self.current_time
    }

    pub const fn phase(&self) -> EventPhase {
        self.phase
    }

    pub const fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget_id
    }

    pub fn is_focused(&self) -> bool {
        self.focused_widget_id == Some(self.widget_id)
    }

    pub const fn is_handled(&self) -> bool {
        self.handled
    }

    pub fn set_handled(&mut self) {
        self.handled = true;
    }

    pub fn request_focus(&mut self) {
        self.focus_request = Some(FocusRequest::Focus(self.widget_id));
    }

    pub fn clear_focus(&mut self) {
        self.focus_request = Some(FocusRequest::Clear);
    }

    pub fn schedule_timer_at(&mut self, deadline: f64) -> TimerToken {
        let token = TimerToken::new(NEXT_TIMER_TOKEN.fetch_add(1, Ordering::Relaxed));
        self.wake_requests.push(WakeRequest::ScheduleTimer {
            token,
            deadline,
            target: self.widget_id,
        });
        token
    }

    pub fn schedule_timer_after(&mut self, delay: f64) -> TimerToken {
        self.schedule_timer_at(self.current_time + delay)
    }

    pub fn cancel_timer(&mut self, token: TimerToken) {
        self.wake_requests.push(WakeRequest::CancelTimer { token });
    }

    pub fn register_async_wakeup(&mut self) -> AsyncWakeToken {
        let token = AsyncWakeToken::new(NEXT_ASYNC_WAKE_TOKEN.fetch_add(1, Ordering::Relaxed));
        self.wake_requests.push(WakeRequest::RegisterAsync {
            token,
            target: self.widget_id,
        });
        token
    }

    pub fn unregister_async_wakeup(&mut self, token: AsyncWakeToken) {
        self.wake_requests
            .push(WakeRequest::UnregisterAsync { token });
    }

    pub fn request_pointer_capture(&mut self, pointer_id: u64) {
        self.pointer_capture_requests
            .push(PointerCaptureRequest::Capture {
                pointer_id,
                target: self.widget_id,
            });
    }

    pub fn release_pointer_capture(&mut self, pointer_id: u64) {
        self.pointer_capture_requests
            .push(PointerCaptureRequest::Release { pointer_id });
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_measure(&mut self) {
        self.request_widget(InvalidationKind::Measure);
    }

    pub fn request_arrange(&mut self) {
        self.request_widget(InvalidationKind::Arrange);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request(
            InvalidationRequest::new(
                InvalidationTarget::Widget(self.widget_id),
                InvalidationKind::Paint,
            )
            .with_region(rect),
        );
    }

    pub fn request_hit_test(&mut self) {
        self.request_widget(InvalidationKind::HitTest);
    }

    pub fn request_text(&mut self) {
        self.request_widget(InvalidationKind::Text);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn request_resources(&mut self) {
        self.request_widget(InvalidationKind::Resources);
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    pub(crate) fn take_focus_request(&mut self) -> Option<FocusRequest> {
        self.focus_request.take()
    }

    pub(crate) fn take_wake_requests(&mut self) -> Vec<WakeRequest> {
        std::mem::take(&mut self.wake_requests)
    }

    pub(crate) fn take_pointer_capture_requests(&mut self) -> Vec<PointerCaptureRequest> {
        std::mem::take(&mut self.pointer_capture_requests)
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct MeasureCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    bounds: Rect,
    dpi_info: DpiInfo,
    text_system: Arc<TextSystem>,
    font_registry: Arc<FontRegistry>,
    image_registry: Arc<ImageRegistry>,
    invalidations: Vec<InvalidationRequest>,
}

impl MeasureCtx {
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        dpi_info: DpiInfo,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            bounds,
            dpi_info,
            text_system,
            font_registry,
            image_registry,
            invalidations: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_measure(&mut self) {
        self.request_widget(InvalidationKind::Measure);
    }

    pub fn request_arrange(&mut self) {
        self.request_widget(InvalidationKind::Arrange);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn measure_text(
        &self,
        text: impl Into<String>,
        style: TextStyle,
    ) -> sui_core::Result<TextMeasurement> {
        self.text_system
            .measure_text(text, style, self.font_registry.as_ref())
    }

    pub fn shape_text(
        &self,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
    ) -> sui_core::Result<TextLayout> {
        self.text_system
            .shape_text(text, box_size, style, self.font_registry.as_ref())
    }

    pub fn image_size(&self, image: sui_core::ImageHandle) -> Option<Size> {
        self.image_registry
            .get(image)
            .map(|image| Size::new(image.width() as f32, image.height() as f32))
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    pub(crate) fn extend_invalidations(&mut self, invalidations: Vec<InvalidationRequest>) {
        self.invalidations.extend(invalidations);
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct ArrangeCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    dpi_info: DpiInfo,
    invalidations: Vec<InvalidationRequest>,
}

impl ArrangeCtx {
    pub(crate) fn new(window_id: WindowId, widget_id: WidgetId, dpi_info: DpiInfo) -> Self {
        Self {
            window_id,
            widget_id,
            dpi_info,
            invalidations: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_arrange(&mut self) {
        self.request_widget(InvalidationKind::Arrange);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    pub(crate) fn extend_invalidations(&mut self, invalidations: Vec<InvalidationRequest>) {
        self.invalidations.extend(invalidations);
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct PaintCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    focused_widget_id: Option<WidgetId>,
    bounds: Rect,
    dpi_info: DpiInfo,
    scene: Scene,
    invalidations: Vec<InvalidationRequest>,
    ime_composition_rect: Option<Rect>,
}

impl PaintCtx {
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        focused_widget_id: Option<WidgetId>,
        dpi_info: DpiInfo,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            focused_widget_id,
            bounds,
            dpi_info,
            scene: Scene::new(),
            invalidations: Vec::new(),
            ime_composition_rect: None,
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub const fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget_id
    }

    pub fn is_focused(&self) -> bool {
        self.focused_widget_id == Some(self.widget_id)
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub fn clear(&mut self, color: Color) {
        self.scene.push(SceneCommand::Clear(color));
    }

    pub fn fill(&mut self, path: impl Into<Path>, brush: impl Into<Brush>) {
        self.scene.push(SceneCommand::FillPath {
            path: path.into(),
            brush: brush.into(),
        });
    }

    pub fn fill_rect(&mut self, rect: Rect, brush: impl Into<Brush>) {
        self.scene.push(SceneCommand::FillRect {
            rect,
            brush: brush.into(),
        });
    }

    pub fn fill_bounds(&mut self, brush: impl Into<Brush>) {
        self.fill_rect(self.bounds, brush);
    }

    pub fn stroke(&mut self, path: impl Into<Path>, brush: impl Into<Brush>, stroke: StrokeStyle) {
        self.scene.push(SceneCommand::StrokePath {
            path: path.into(),
            brush: brush.into(),
            stroke,
        });
    }

    pub fn stroke_rect(&mut self, rect: Rect, brush: impl Into<Brush>, stroke: StrokeStyle) {
        self.scene.push(SceneCommand::StrokeRect {
            rect,
            brush: brush.into(),
            stroke,
        });
    }

    pub fn stroke_bounds(&mut self, brush: impl Into<Brush>, stroke: StrokeStyle) {
        self.stroke_rect(self.bounds, brush, stroke);
    }

    pub fn draw_text(&mut self, rect: Rect, text: impl Into<String>, style: TextStyle) {
        self.scene.push(SceneCommand::DrawText(TextRun {
            rect,
            text: text.into(),
            style,
        }));
    }

    pub fn draw_text_layout(&mut self, origin: Point, layout: &TextLayout) {
        self.scene.push(SceneCommand::DrawShapedText(ShapedText {
            origin,
            layout: layout.clone(),
        }));
    }

    pub fn label(&mut self, rect: Rect, text: impl Into<String>, color: Color) {
        self.draw_text(rect, text, TextStyle::new(color));
    }

    pub fn draw_image(&mut self, rect: Rect, image: sui_core::ImageHandle) {
        self.scene.push(SceneCommand::DrawImage {
            rect,
            source: ImageSource::new(image),
        });
    }

    pub fn draw_image_source(&mut self, rect: Rect, source: ImageSource) {
        self.scene.push(SceneCommand::DrawImage { rect, source });
    }

    pub fn push_clip(&mut self, path: impl Into<Path>) {
        self.scene
            .push(SceneCommand::PushClipPath { path: path.into() });
    }

    pub fn push_clip_rect(&mut self, rect: Rect) {
        self.scene.push(SceneCommand::PushClip { rect });
    }

    pub fn pop_clip(&mut self) {
        self.scene.push(SceneCommand::PopClip);
    }

    pub fn push_transform(&mut self, transform: Transform) {
        self.scene.push(SceneCommand::PushTransform { transform });
    }

    pub fn translate(&mut self, delta: Vector) {
        self.push_transform(Transform::translation_vector(delta));
    }

    pub fn pop_transform(&mut self) {
        self.scene.push(SceneCommand::PopTransform);
    }

    pub fn push(&mut self, command: SceneCommand) {
        self.scene.push(command);
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request(
            InvalidationRequest::new(
                InvalidationTarget::Widget(self.widget_id),
                InvalidationKind::Paint,
            )
            .with_region(rect),
        );
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub fn set_ime_composition_rect(&mut self, rect: Rect) {
        self.ime_composition_rect = Some(rect);
    }

    pub fn clear_ime_composition_rect(&mut self) {
        self.ime_composition_rect = None;
    }

    pub const fn ime_composition_rect(&self) -> Option<Rect> {
        self.ime_composition_rect
    }

    pub(crate) fn push_layer(&mut self, widget_id: WidgetId, bounds: Rect, scene: Scene) {
        self.scene
            .push(SceneCommand::Layer(SceneLayer::new(widget_id, bounds, scene)));
    }

    pub(crate) fn extend_invalidations(&mut self, invalidations: Vec<InvalidationRequest>) {
        self.invalidations.extend(invalidations);
    }

    pub(crate) fn extend_ime_composition_rect(&mut self, ime_composition_rect: Option<Rect>) {
        if ime_composition_rect.is_some() {
            self.ime_composition_rect = ime_composition_rect;
        }
    }

    pub(crate) fn into_parts(self) -> (Scene, Vec<InvalidationRequest>, Option<Rect>) {
        (self.scene, self.invalidations, self.ime_composition_rect)
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct SemanticsCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    root_widget_id: WidgetId,
    focused_widget_id: Option<WidgetId>,
    bounds: Rect,
    nodes: Vec<SemanticsNode>,
}

impl SemanticsCtx {
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        root_widget_id: WidgetId,
        bounds: Rect,
        focused_widget_id: Option<WidgetId>,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            root_widget_id,
            focused_widget_id,
            bounds,
            nodes: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub const fn root_widget_id(&self) -> WidgetId {
        self.root_widget_id
    }

    pub const fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget_id
    }

    pub fn is_focused(&self) -> bool {
        self.focused_widget_id == Some(self.widget_id)
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn push(&mut self, node: SemanticsNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[SemanticsNode] {
        &self.nodes
    }

    pub(crate) fn extend_nodes(&mut self, nodes: Vec<SemanticsNode>) {
        self.nodes.extend(nodes);
    }

    pub(crate) fn into_nodes(self) -> Vec<SemanticsNode> {
        self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArrangeCtx, EventCtx, EventPhase, MeasureCtx, PaintCtx, SemanticsCtx, SingleChild,
        Widget, WidgetChildren, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
    };
    use sui_core::{
        Color, DpiInfo, InvalidationKind, Point, Rect, SemanticsNode, SemanticsRole, Vector,
        WidgetId, WindowId,
    };
    use sui_layout::Constraints;
    use sui_scene::{ImageRegistry, SceneCommand, StrokeStyle};
    use sui_text::{FontRegistry, TextStyle, TextSystem};

    fn measure_ctx(window_id: WindowId, widget_id: WidgetId) -> MeasureCtx {
        MeasureCtx::new(
            window_id,
            widget_id,
            Rect::ZERO,
            DpiInfo::default(),
            std::sync::Arc::new(TextSystem::new()),
            std::sync::Arc::new(FontRegistry::new()),
            std::sync::Arc::new(ImageRegistry::new()),
        )
    }

    #[test]
    fn measure_and_paint_ctx_expose_dpi_info() {
        let dpi = DpiInfo::new(
            2.0,
            Some(192.0),
            sui_core::Size::new(320.0, 180.0),
            sui_core::Size::new(640.0, 360.0),
        );

        let measure = MeasureCtx::new(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::ZERO,
            dpi,
            std::sync::Arc::new(TextSystem::new()),
            std::sync::Arc::new(FontRegistry::new()),
            std::sync::Arc::new(ImageRegistry::new()),
        );
        let paint = PaintCtx::new(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            dpi,
        );

        assert_eq!(measure.dpi(), dpi);
        assert_eq!(paint.dpi(), dpi);
    }

    struct LabelWidget;

    impl Widget for LabelWidget {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> sui_core::Size {
            constraints.clamp(sui_core::Size::new(48.0, 20.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::rgba(0.2, 0.3, 0.4, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            ctx.push(SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::Text,
                ctx.bounds(),
            ));
        }
    }

    #[test]
    fn event_ctx_tracks_widget_scoped_invalidations_and_focus() {
        let mut ctx = EventCtx::new(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::new(8.0, 12.0, 24.0, 36.0),
            0.0,
            EventPhase::Target,
            None,
        );

        ctx.request_measure();
        ctx.request_paint_rect(Rect::new(8.0, 12.0, 24.0, 36.0));
        ctx.request_focus();
        ctx.set_handled();

        assert!(ctx.is_handled());
        assert_eq!(ctx.bounds(), Rect::new(8.0, 12.0, 24.0, 36.0));
        assert_eq!(ctx.invalidations().len(), 2);
        assert_eq!(ctx.invalidations()[0].kind, InvalidationKind::Measure);
        assert_eq!(
            ctx.invalidations()[1].region,
            Some(Rect::new(8.0, 12.0, 24.0, 36.0))
        );
    }

    #[test]
    fn widget_pod_merges_child_measure_arrange_paint_and_semantics() {
        let mut pod = WidgetPod::new(LabelWidget);
        pod.set_bounds(Rect::new(4.0, 6.0, 0.0, 0.0));

        let mut measure = measure_ctx(WindowId::new(3), WidgetId::new(4));
        let size = pod.measure(&mut measure, Constraints::tight(sui_core::Size::new(64.0, 32.0)));
        let mut arrange = ArrangeCtx::new(WindowId::new(3), WidgetId::new(4), DpiInfo::default());
        pod.arrange(&mut arrange, Rect::new(4.0, 6.0, 64.0, 32.0));

        let mut paint = PaintCtx::new(
            WindowId::new(3),
            WidgetId::new(4),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
        );
        pod.paint(&mut paint);

        let mut semantics = SemanticsCtx::new(
            WindowId::new(3),
            WidgetId::new(4),
            WidgetId::new(4),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
        );
        pod.semantics(&mut semantics);

        assert_eq!(size, sui_core::Size::new(64.0, 32.0));
        assert_eq!(pod.bounds(), Rect::new(4.0, 6.0, 64.0, 32.0));
        assert_eq!(paint.scene().commands().len(), 1);
        assert_eq!(semantics.nodes().len(), 1);
    }

    #[test]
    fn single_child_wraps_measure_arrange_and_visitation() {
        struct CaptureVisitor {
            ids: Vec<WidgetId>,
        }

        impl WidgetPodVisitor for CaptureVisitor {
            fn visit(&mut self, child: &WidgetPod) {
                self.ids.push(child.id());
            }
        }

        impl WidgetPodMutVisitor for CaptureVisitor {
            fn visit(&mut self, child: &mut WidgetPod) {
                self.ids.push(child.id());
            }
        }

        let mut child = SingleChild::new(LabelWidget);
        let mut measure = measure_ctx(WindowId::new(7), WidgetId::new(8));
        let size = child.measure(&mut measure, Constraints::tight(sui_core::Size::new(80.0, 24.0)));
        let mut arrange = ArrangeCtx::new(WindowId::new(7), WidgetId::new(8), DpiInfo::default());
        child.arrange(&mut arrange, Rect::new(12.0, 18.0, 80.0, 24.0));

        let mut visitor = CaptureVisitor { ids: Vec::new() };
        child.visit_children(&mut visitor);
        child.visit_children_mut(&mut visitor);

        assert_eq!(size, sui_core::Size::new(80.0, 24.0));
        assert_eq!(child.child().bounds(), Rect::new(12.0, 18.0, 80.0, 24.0));
        assert_eq!(visitor.ids, vec![child.child().id(), child.child().id()]);
    }

    #[test]
    fn widget_children_bulk_paint_and_semantics_delegate_to_all_children() {
        let mut children = WidgetChildren::with_capacity(2);
        children.push(LabelWidget);
        children.push(LabelWidget);

        let mut measure = measure_ctx(WindowId::new(9), WidgetId::new(10));
        children.measure_child(0, &mut measure, Constraints::tight(sui_core::Size::new(40.0, 18.0)));
        children.measure_child(1, &mut measure, Constraints::tight(sui_core::Size::new(60.0, 18.0)));
        let mut arrange = ArrangeCtx::new(WindowId::new(9), WidgetId::new(10), DpiInfo::default());
        children.arrange_child(0, &mut arrange, Rect::new(0.0, 0.0, 40.0, 18.0));
        children.arrange_child(1, &mut arrange, Rect::new(44.0, 0.0, 60.0, 18.0));

        let mut paint = PaintCtx::new(
            WindowId::new(9),
            WidgetId::new(10),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            DpiInfo::default(),
        );
        children.paint(&mut paint);

        let mut semantics = SemanticsCtx::new(
            WindowId::new(9),
            WidgetId::new(10),
            WidgetId::new(10),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
        );
        children.semantics(&mut semantics);

        assert_eq!(children.len(), 2);
        assert_eq!(paint.scene().commands().len(), 2);
        assert_eq!(semantics.nodes().len(), 2);
    }

    #[test]
    fn paint_ctx_emits_extended_scene_commands() {
        let mut paint = PaintCtx::new(
            WindowId::new(11),
            WidgetId::new(12),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
        );

        let mut path = sui_core::Path::builder();
        path.move_to(Point::new(4.0, 5.0))
            .line_to(Point::new(24.0, 5.0))
            .line_to(Point::new(14.0, 15.0))
            .close();
        paint.stroke(path.build(), Color::WHITE, StrokeStyle::new(2.0));
        paint.draw_text(
            Rect::new(8.0, 10.0, 80.0, 20.0),
            "hello",
            TextStyle::new(Color::BLACK),
        );
        paint.draw_image(
            Rect::new(0.0, 0.0, 16.0, 16.0),
            sui_core::ImageHandle::new(3),
        );
        paint.push_clip(sui_core::Path::circle(Point::new(12.0, 12.0), 8.0));
        paint.push_clip_rect(Rect::new(0.0, 0.0, 50.0, 50.0));
        paint.translate(Vector::new(3.0, 4.0));
        paint.pop_transform();
        paint.pop_clip();
        paint.pop_clip();

        assert!(matches!(
            paint.scene().commands()[0],
            SceneCommand::StrokePath { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[1],
            SceneCommand::DrawText(_)
        ));
        assert!(matches!(
            paint.scene().commands()[2],
            SceneCommand::DrawImage { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[3],
            SceneCommand::PushClipPath { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[4],
            SceneCommand::PushClip { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[5],
            SceneCommand::PushTransform { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[6],
            SceneCommand::PopTransform
        ));
        assert!(matches!(paint.scene().commands()[7], SceneCommand::PopClip));
        assert!(matches!(paint.scene().commands()[8], SceneCommand::PopClip));
    }

    #[test]
    fn text_layout_shapes_in_measure_and_paints_as_shaped_scene_output() {
        let layout = measure_ctx(WindowId::new(13), WidgetId::new(14))
            .shape_text(
                "hello",
                sui_core::Size::new(80.0, 20.0),
                TextStyle::new(Color::BLACK),
            )
            .unwrap();

        let mut paint = PaintCtx::new(
            WindowId::new(13),
            WidgetId::new(14),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
        );
        let origin = Point::new(8.0, 10.0);
        paint.draw_text_layout(origin, &layout);
        paint.set_ime_composition_rect(layout.caret_rect(3).translate(origin.to_vector()));

        assert!(matches!(
            paint.scene().commands()[0],
            SceneCommand::DrawShapedText(_)
        ));
        assert!(paint.ime_composition_rect().is_some());
        assert!(!layout.selection_rects(1..4).is_empty());
    }
}
