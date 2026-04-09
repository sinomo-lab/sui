#![forbid(unsafe_code)]

mod diagnostics;
mod widget;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Instant,
};

use sui_core::{
    AsyncWakeToken, DirtyRegion, Error, Event, FontHandle, ImageHandle, InvalidationKind,
    InvalidationRequest, InvalidationTarget, KeyState, Point, PointerEvent, PointerEventKind, Rect,
    Result, SemanticsNode, Size, TimerToken, Vector, WakeEvent, WidgetId, WindowEvent, WindowId,
};
use sui_layout::Constraints;
use sui_scene::{
    ImageRegistry, LayerCompositionMode, RegisteredImage, Scene, SceneCommand, SceneFrame,
    SceneLayer, SceneLayerDescriptor, SceneLayerUpdate, SceneLayerUpdateKind,
};
use sui_text::{FontRegistry, RegisteredFont, TextSystem};

pub use diagnostics::{
    CacheMetrics, CacheMetricsDelta, FramePhase, FramePhaseSample, PresentationLatencyDiagnostics,
    RenderDiagnostics, RendererSubmissionDiagnostics, SceneStatistics, SceneStatisticsDetailMode,
    TextCacheDeltaDiagnostics, TextCacheDiagnostics, WindowPerformanceSnapshot,
    WindowPerformanceSummary, WindowRenderOptions, WindowTextRenderPolicy,
    clear_window_performance_snapshot, clear_window_performance_snapshots,
    clear_window_render_options, publish_window_performance_snapshot, set_window_render_options,
    set_window_scene_statistics_detail_mode, window_performance_snapshot,
    window_performance_summary, window_performance_text_caches, window_render_options,
    window_scene_statistics_detail_mode,
};
pub use sui_core::DpiInfo;
pub use widget::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintCtx, SemanticsCtx,
    SingleChild, StackHostOptions, StackOrderPolicy, StackSurfaceOptions, Widget, WidgetChildren,
    WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use widget::{FocusRequest, PointerCaptureRequest, WakeRequest};

pub struct WindowBuilder {
    title: String,
    root: Option<WidgetPod>,
}

impl WindowBuilder {
    pub fn new() -> Self {
        Self {
            title: "SUI Window".to_string(),
            root: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn root<W>(mut self, root: W) -> Self
    where
        W: Widget + 'static,
    {
        self.root = Some(WidgetPod::new(root));
        self
    }

    fn build(self, window_id: WindowId) -> Result<WindowState> {
        let root = self
            .root
            .ok_or_else(|| Error::new("window root widget must be set before building"))?;

        Ok(WindowState::new(window_id, self.title, root))
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Application {
    windows: Vec<WindowBuilder>,
    next_font_id: u64,
    next_image_id: u64,
    font_registry: Arc<FontRegistry>,
    image_registry: Arc<ImageRegistry>,
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window(mut self, window: WindowBuilder) -> Self {
        self.windows.push(window);
        self
    }

    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        if Arc::make_mut(&mut self.font_registry)
            .insert(handle, font)
            .is_some()
        {
            return Err(Error::new(format!(
                "font handle {} is already registered",
                handle.get()
            )));
        }

        self.next_font_id = self.next_font_id.max(handle.get() + 1);
        Ok(())
    }

    pub fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        let handle = FontHandle::new(self.next_font_id.max(1));
        self.next_font_id = handle.get() + 1;
        self.register_font(handle, RegisteredFont::from_bytes(data))?;
        Ok(handle)
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) -> Result<()> {
        if Arc::make_mut(&mut self.image_registry)
            .insert(handle, image)
            .is_some()
        {
            return Err(Error::new(format!(
                "image handle {} is already registered",
                handle.get()
            )));
        }

        self.next_image_id = self.next_image_id.max(handle.get() + 1);
        Ok(())
    }

    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.next_image_id = handle.get() + 1;
        self.register_image(handle, RegisteredImage::from_rgba8(width, height, data)?)?;
        Ok(handle)
    }

    pub fn build(self) -> Result<Runtime> {
        let mut runtime = Runtime::with_registries(
            self.next_font_id,
            self.font_registry,
            self.next_image_id,
            self.image_registry,
        );

        for window in self.windows {
            runtime.add_window(window)?;
        }

        Ok(runtime)
    }
}

impl Default for Application {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            next_font_id: 1,
            next_image_id: 1,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        }
    }
}

pub struct Runtime {
    next_window_id: u64,
    next_font_id: u64,
    next_image_id: u64,
    font_registry: Arc<FontRegistry>,
    image_registry: Arc<ImageRegistry>,
    text_system: Arc<TextSystem>,
    windows: Vec<WindowState>,
}

impl Runtime {
    pub fn new() -> Self {
        Self::with_registries(
            1,
            Arc::new(FontRegistry::new()),
            1,
            Arc::new(ImageRegistry::new()),
        )
    }

    fn with_registries(
        next_font_id: u64,
        font_registry: Arc<FontRegistry>,
        next_image_id: u64,
        image_registry: Arc<ImageRegistry>,
    ) -> Self {
        Self {
            next_window_id: 1,
            next_font_id: next_font_id.max(1),
            next_image_id: next_image_id.max(1),
            font_registry,
            image_registry,
            text_system: Arc::new(TextSystem::new()),
            windows: Vec::new(),
        }
    }

    pub fn add_window(&mut self, builder: WindowBuilder) -> Result<WindowId> {
        let window_id = self.alloc_window_id();
        let window = builder.build(window_id)?;
        self.windows.push(window);
        Ok(window_id)
    }

    pub fn remove_window(&mut self, window_id: WindowId) -> Result<()> {
        let Some(window_index) = self
            .windows
            .iter()
            .position(|window| window.id == window_id)
        else {
            return Err(Error::new(format!(
                "window {} does not exist",
                window_id.get()
            )));
        };

        self.windows.remove(window_index);
        Ok(())
    }

    pub fn handle_event(&mut self, window_id: WindowId, event: Event) -> Result<()> {
        let text_system = Arc::clone(&self.text_system);
        let font_registry = Arc::clone(&self.font_registry);
        let image_registry = Arc::clone(&self.image_registry);
        let window = self.window_mut(window_id)?;
        window.handle_event(event, text_system, font_registry, image_registry);
        Ok(())
    }

    pub fn tick(&mut self, frame_time: f64) {
        for window in &mut self.windows {
            window.last_tick_time = frame_time;
        }
    }

    pub fn drain_ready_events(&mut self) -> Vec<(WindowId, Event)> {
        let mut ready = Vec::new();

        for window in &mut self.windows {
            let window_id = window.id;
            ready.extend(
                window
                    .drain_ready_events()
                    .into_iter()
                    .map(|event| (window_id, event)),
            );
        }

        ready
    }

    pub fn next_wakeup_time(&self, window_id: WindowId) -> Result<Option<f64>> {
        let window = self.window(window_id)?;
        Ok(window.next_wakeup_time())
    }

    pub fn wake_async(&mut self, window_id: WindowId, token: AsyncWakeToken) -> Result<bool> {
        let window = self.window_mut(window_id)?;
        Ok(window.wake_async(token))
    }

    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        if Arc::make_mut(&mut self.font_registry)
            .insert(handle, font)
            .is_some()
        {
            return Err(Error::new(format!(
                "font handle {} is already registered",
                handle.get()
            )));
        }

        self.next_font_id = self.next_font_id.max(handle.get() + 1);
        Ok(())
    }

    pub fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        let handle = FontHandle::new(self.next_font_id.max(1));
        self.next_font_id = handle.get() + 1;
        self.register_font(handle, RegisteredFont::from_bytes(data))?;
        Ok(handle)
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) -> Result<()> {
        if Arc::make_mut(&mut self.image_registry)
            .insert(handle, image)
            .is_some()
        {
            return Err(Error::new(format!(
                "image handle {} is already registered",
                handle.get()
            )));
        }

        self.next_image_id = self.next_image_id.max(handle.get() + 1);
        Ok(())
    }

    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.next_image_id = handle.get() + 1;
        self.register_image(handle, RegisteredImage::from_rgba8(width, height, data)?)?;
        Ok(handle)
    }

    pub fn font_registry(&self) -> &Arc<FontRegistry> {
        &self.font_registry
    }

    pub fn image_registry(&self) -> &Arc<ImageRegistry> {
        &self.image_registry
    }

    pub fn render(&mut self, window_id: WindowId) -> Result<RenderOutput> {
        let text_system = Arc::clone(&self.text_system);
        let font_registry = Arc::clone(&self.font_registry);
        let image_registry = Arc::clone(&self.image_registry);
        let window = self.window_mut(window_id)?;
        Ok(window.render(text_system, font_registry, image_registry))
    }

    pub fn semantics(&self, window_id: WindowId) -> Result<&[SemanticsNode]> {
        let window = self.window(window_id)?;
        Ok(&window.last_semantics)
    }

    pub fn window_title(&self, window_id: WindowId) -> Result<&str> {
        let window = self.window(window_id)?;
        Ok(&window.title)
    }

    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.iter().map(|window| window.id).collect()
    }

    pub fn needs_render(&self, window_id: WindowId) -> Result<bool> {
        let window = self.window(window_id)?;
        Ok(window.needs_render())
    }

    pub fn schedule(&self, window_id: WindowId) -> Result<FrameSchedule> {
        let window = self.window(window_id)?;
        Ok(window.schedule)
    }

    pub fn focus_state(&self, window_id: WindowId) -> Result<FocusState> {
        let window = self.window(window_id)?;
        Ok(window.focus)
    }

    pub fn focused_widget(&self, window_id: WindowId) -> Result<Option<WidgetId>> {
        Ok(self.focus_state(window_id)?.focused_widget)
    }

    pub fn pointer_capture_target(
        &self,
        window_id: WindowId,
        pointer_id: u64,
    ) -> Result<Option<WidgetId>> {
        let window = self.window(window_id)?;
        Ok(window.pointer_capture.get(&pointer_id).copied())
    }

    pub fn widget_graph(&self, window_id: WindowId) -> Result<WidgetGraphSnapshot> {
        let window = self.window(window_id)?;
        Ok(window.graph.snapshot())
    }

    fn alloc_window_id(&mut self) -> WindowId {
        let id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        id
    }

    fn window(&self, window_id: WindowId) -> Result<&WindowState> {
        self.windows
            .iter()
            .find(|window| window.id == window_id)
            .ok_or_else(|| Error::new(format!("window {} does not exist", window_id.get())))
    }

    fn window_mut(&mut self, window_id: WindowId) -> Result<&mut WindowState> {
        self.windows
            .iter_mut()
            .find(|window| window.id == window_id)
            .ok_or_else(|| Error::new(format!("window {} does not exist", window_id.get())))
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FocusState {
    pub focused_widget: Option<WidgetId>,
    pub window_focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameSchedule {
    pub measure: bool,
    pub arrange: bool,
    pub ordering: bool,
    pub paint: bool,
    pub semantics: bool,
    pub hit_test: bool,
    pub text: bool,
    pub resources: bool,
}

impl FrameSchedule {
    fn bootstrap() -> Self {
        Self {
            measure: true,
            arrange: true,
            ordering: false,
            paint: true,
            semantics: true,
            hit_test: true,
            text: false,
            resources: false,
        }
    }

    pub const fn any(self) -> bool {
        self.measure
            || self.arrange
            || self.ordering
            || self.paint
            || self.semantics
            || self.hit_test
            || self.text
            || self.resources
    }

    pub const fn needs_render(self) -> bool {
        self.measure
            || self.arrange
            || self.ordering
            || self.paint
            || self.semantics
            || self.text
            || self.resources
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn mark(&mut self, kind: InvalidationKind) {
        match kind {
            InvalidationKind::Measure => {
                self.measure = true;
                self.arrange = true;
                self.ordering = true;
                self.paint = true;
                self.hit_test = true;
                self.semantics = true;
            }
            InvalidationKind::Arrange
            | InvalidationKind::Transform
            | InvalidationKind::Clip
            | InvalidationKind::Visibility => {
                self.ordering = true;
                self.arrange = true;
                self.paint = true;
                self.hit_test = true;
                self.semantics = true;
            }
            InvalidationKind::Ordering => {
                self.ordering = true;
                self.hit_test = true;
            }
            InvalidationKind::Effect => {
                self.paint = true;
            }
            InvalidationKind::Paint => {
                self.paint = true;
            }
            InvalidationKind::HitTest => {
                self.hit_test = true;
            }
            InvalidationKind::Text => {
                self.text = true;
                self.measure = true;
                self.arrange = true;
                self.paint = true;
                self.semantics = true;
            }
            InvalidationKind::Semantics => {
                self.semantics = true;
            }
            InvalidationKind::Resources => {
                self.resources = true;
                self.paint = true;
            }
        }
    }

    fn extend(&mut self, invalidations: &[InvalidationRequest]) {
        for request in invalidations {
            self.mark(request.kind);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetGeometrySnapshot {
    pub layout_bounds: Rect,
    pub input_bounds: Rect,
    pub paint_bounds: Rect,
}

impl WidgetGeometrySnapshot {
    pub const fn new(layout_bounds: Rect, input_bounds: Rect, paint_bounds: Rect) -> Self {
        Self {
            layout_bounds,
            input_bounds,
            paint_bounds,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetNodeSnapshot {
    pub id: WidgetId,
    pub parent: Option<WidgetId>,
    pub children: Vec<WidgetId>,
    pub measured_size: Size,
    pub geometry: WidgetGeometrySnapshot,
    // Backward-compatible alias of geometry.layout_bounds.
    pub bounds: Rect,
    pub stack_host: WidgetId,
    pub stack_surface: WidgetId,
    pub stack_surface_order: usize,
    pub transient_owner_surface: Option<WidgetId>,
    pub is_stack_host: bool,
    pub is_stack_surface: bool,
    pub stack_order_policy: StackOrderPolicy,
    pub accepts_focus: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackHostSnapshot {
    pub host: WidgetId,
    pub order_policy: StackOrderPolicy,
    pub surfaces: Vec<WidgetId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetGraphSnapshot {
    pub root: WidgetId,
    pub nodes: Vec<WidgetNodeSnapshot>,
    pub stack_hosts: Vec<StackHostSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScheduledTimer {
    token: TimerToken,
    deadline: f64,
    target: WidgetId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LayerTranslation {
    widget_id: WidgetId,
    delta: Vector,
}

#[derive(Debug, Default)]
struct GraphChangeSet {
    repaint_widgets: Vec<WidgetId>,
    transform_widgets: Vec<LayerTranslation>,
}

struct WindowState {
    id: WindowId,
    title: String,
    root: WidgetPod,
    graph: WidgetGraph,
    focus: FocusState,
    schedule: FrameSchedule,
    scale_factor: f32,
    raw_dpi: Option<f32>,
    viewport_hint: Option<Size>,
    viewport: Option<Size>,
    last_frame: Option<SceneFrame>,
    last_semantics: Vec<SemanticsNode>,
    pending_invalidations: Vec<InvalidationRequest>,
    pointer_capture: HashMap<u64, WidgetId>,
    pointer_hover_paths: HashMap<u64, Vec<WidgetId>>,
    scheduled_timers: Vec<ScheduledTimer>,
    delivering_timers: HashMap<TimerToken, WidgetId>,
    async_wake_targets: HashMap<AsyncWakeToken, WidgetId>,
    pending_async_wakeups: VecDeque<AsyncWakeToken>,
    ime_composition_rect: Option<Rect>,
    last_tick_time: f64,
}

impl WindowState {
    fn new(id: WindowId, title: String, root: WidgetPod) -> Self {
        let focus = FocusState {
            focused_widget: None,
            window_focused: true,
        };

        Self {
            id,
            title,
            graph: WidgetGraph::empty(root.id()),
            root,
            focus,
            schedule: FrameSchedule::bootstrap(),
            scale_factor: 1.0,
            raw_dpi: None,
            viewport_hint: None,
            viewport: None,
            last_frame: None,
            last_semantics: Vec::new(),
            pending_invalidations: Vec::new(),
            pointer_capture: HashMap::new(),
            pointer_hover_paths: HashMap::new(),
            scheduled_timers: Vec::new(),
            delivering_timers: HashMap::new(),
            async_wake_targets: HashMap::new(),
            pending_async_wakeups: VecDeque::new(),
            ime_composition_rect: None,
            last_tick_time: 0.0,
        }
    }

    fn needs_render(&self) -> bool {
        self.last_frame.is_none() || self.schedule.needs_render()
    }

    fn handle_event(
        &mut self,
        event: Event,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) {
        self.preprocess_window_event(&event);
        self.ensure_graph_for_event(&event, text_system, font_registry, image_registry);

        let hit_target = match &event {
            Event::Pointer(pointer) => {
                let floating_hit = self.floating_layer_hit_test(pointer.position);
                let focused_floating_hit = if floating_hit.is_none() {
                    self.focused_floating_layer_hit_test(pointer.position)
                } else {
                    None
                };
                floating_hit
                    .or(focused_floating_hit)
                    .or_else(|| self.graph.hit_test(pointer.position))
            }
            _ => None,
        };

        let mut invalidations = Vec::new();
        let mut skip_primary_route = false;
        if let Event::Pointer(pointer) = &event {
            let hover_route = self.update_pointer_hover_path(pointer, hit_target);
            invalidations.extend(hover_route.effects.invalidations);

            if let Some(request) = hover_route.focus_request {
                let focus_effects = self.apply_focus_request(request);
                invalidations.extend(focus_effects.invalidations);
                self.apply_wake_requests(focus_effects.wake_requests);
                self.apply_pointer_capture_requests(focus_effects.pointer_capture_requests);
            }

            self.apply_wake_requests(hover_route.effects.wake_requests);
            self.apply_pointer_capture_requests(hover_route.effects.pointer_capture_requests);
            skip_primary_route = hover_route.skip_primary_route;
        }

        if skip_primary_route {
            self.finish_event(&event);
            self.schedule.extend(&invalidations);
            self.pending_invalidations.extend(invalidations);
            return;
        }

        let target = self.resolve_event_target(&event, hit_target);

        let route = self.route_event(target, &event);
        invalidations.extend(route.effects.invalidations);

        let focus_request = route
            .focus_request
            .or_else(|| self.default_focus_request(&event, hit_target, &route.path));

        if let Some(request) = focus_request {
            let focus_effects = self.apply_focus_request(request);
            invalidations.extend(focus_effects.invalidations);
            self.apply_wake_requests(focus_effects.wake_requests);
            self.apply_pointer_capture_requests(focus_effects.pointer_capture_requests);
        }

        self.apply_wake_requests(route.effects.wake_requests);
        self.apply_pointer_capture_requests(route.effects.pointer_capture_requests);
        self.finish_event(&event);
        self.schedule.extend(&invalidations);
        self.pending_invalidations.extend(invalidations);
    }

    fn update_pointer_hover_path(
        &mut self,
        pointer: &PointerEvent,
        hit_target: Option<WidgetId>,
    ) -> HoverTransitionResult {
        let skip_primary_route = matches!(
            pointer.kind,
            PointerEventKind::Enter | PointerEventKind::Leave
        );

        let next_path = match pointer.kind {
            PointerEventKind::Move | PointerEventKind::Enter
                if pointer.buttons.is_empty()
                    && !self.pointer_capture.contains_key(&pointer.pointer_id) =>
            {
                self.hover_transition_path(hit_target)
            }
            PointerEventKind::Leave | PointerEventKind::Cancel => Vec::new(),
            _ => {
                return HoverTransitionResult {
                    skip_primary_route,
                    ..HoverTransitionResult::default()
                };
            }
        };

        let previous_path = self
            .pointer_hover_paths
            .remove(&pointer.pointer_id)
            .unwrap_or_default();
        if previous_path == next_path {
            if !next_path.is_empty() {
                self.pointer_hover_paths
                    .insert(pointer.pointer_id, next_path);
            }
            return HoverTransitionResult {
                skip_primary_route,
                ..HoverTransitionResult::default()
            };
        }

        let shared_prefix_len = previous_path
            .iter()
            .zip(next_path.iter())
            .take_while(|(left, right)| left == right)
            .count();

        let mut result = HoverTransitionResult {
            skip_primary_route,
            ..HoverTransitionResult::default()
        };

        let mut leave_pointer = pointer.clone();
        leave_pointer.kind = PointerEventKind::Leave;
        let leave_event = Event::Pointer(leave_pointer);
        for widget_id in previous_path[shared_prefix_len..].iter().rev().copied() {
            let dispatch = self.dispatch_direct_event(widget_id, &leave_event);
            let dispatch_focus_request = dispatch.focus_request;
            result.effects.extend(dispatch);
            if dispatch_focus_request.is_some() {
                result.focus_request = dispatch_focus_request;
            }
        }

        let mut enter_pointer = pointer.clone();
        enter_pointer.kind = PointerEventKind::Enter;
        let enter_event = Event::Pointer(enter_pointer);
        for widget_id in next_path[shared_prefix_len..].iter().copied() {
            let dispatch = self.dispatch_direct_event(widget_id, &enter_event);
            let dispatch_focus_request = dispatch.focus_request;
            result.effects.extend(dispatch);
            if dispatch_focus_request.is_some() {
                result.focus_request = dispatch_focus_request;
            }
        }

        if !next_path.is_empty() {
            self.pointer_hover_paths
                .insert(pointer.pointer_id, next_path);
        }

        result
    }

    fn hover_transition_path(&self, hit_target: Option<WidgetId>) -> Vec<WidgetId> {
        hit_target
            .and_then(|target| self.graph.path_to(target))
            .map(|path| path.into_iter().skip(1).collect())
            .unwrap_or_default()
    }

    fn dispatch_direct_event(&mut self, target: WidgetId, event: &Event) -> widget::EventDispatch {
        self.root
            .dispatch_event_for(
                target,
                self.id,
                self.last_tick_time,
                EventPhase::Target,
                self.focus.focused_widget,
                event,
            )
            .unwrap_or_else(empty_dispatch)
    }

    fn floating_layer_hit_test(&mut self, point: Point) -> Option<WidgetId> {
        let effect_hit = {
            let scene = self.last_frame.as_ref().map(|frame| &frame.scene)?;
            scene_hit_test_for_phase(
                scene,
                point,
                HitTestCompositionPhase::Effect,
                HitTestCompositionPhase::Normal,
            )
        };
        if let Some(widget_id) = effect_hit {
            if self
                .current_widget_matches_hit_test_phase(widget_id, HitTestCompositionPhase::Effect)
            {
                return Some(widget_id);
            }
        }

        let overlay_hit = {
            let scene = self.last_frame.as_ref().map(|frame| &frame.scene)?;
            scene_hit_test_for_phase(
                scene,
                point,
                HitTestCompositionPhase::Overlay,
                HitTestCompositionPhase::Normal,
            )
        };
        overlay_hit.filter(|widget_id| {
            self.current_widget_matches_hit_test_phase(*widget_id, HitTestCompositionPhase::Overlay)
        })
    }

    fn focused_floating_layer_hit_test(&mut self, point: Point) -> Option<WidgetId> {
        let focused = self.focus.focused_widget?;
        if !self.current_widget_matches_hit_test_phase(focused, HitTestCompositionPhase::Overlay)
            && !self.current_widget_matches_hit_test_phase(focused, HitTestCompositionPhase::Effect)
        {
            return None;
        }

        let scene = self.last_frame.as_ref().map(|frame| &frame.scene)?;
        let descriptor = collect_scene_layers(scene).get(&focused)?.clone();
        descriptor.paint_bounds.contains(point).then_some(focused)
    }

    fn current_widget_matches_hit_test_phase(
        &mut self,
        widget_id: WidgetId,
        target_phase: HitTestCompositionPhase,
    ) -> bool {
        let Some(composition_mode) = self.current_widget_composition_mode(widget_id) else {
            return false;
        };

        next_hit_test_phase(HitTestCompositionPhase::Normal, composition_mode) == target_phase
    }

    fn current_widget_composition_mode(
        &mut self,
        widget_id: WidgetId,
    ) -> Option<LayerCompositionMode> {
        self.root.layer_composition_mode_for(widget_id)
    }

    fn next_wakeup_time(&self) -> Option<f64> {
        if !self.pending_async_wakeups.is_empty() {
            return Some(self.last_tick_time);
        }

        self.scheduled_timers
            .iter()
            .map(|timer| timer.deadline)
            .min_by(|left, right| left.total_cmp(right))
    }

    fn drain_ready_events(&mut self) -> Vec<Event> {
        let now = self.last_tick_time;
        let mut ready = Vec::new();
        let mut pending_timers = Vec::with_capacity(self.scheduled_timers.len());

        for timer in self.scheduled_timers.drain(..) {
            if timer.deadline <= now {
                self.delivering_timers.insert(timer.token, timer.target);
                ready.push(Event::Wake(WakeEvent::Timer {
                    token: timer.token,
                    time: now,
                    deadline: timer.deadline,
                }));
            } else {
                pending_timers.push(timer);
            }
        }
        self.scheduled_timers = pending_timers;

        while let Some(token) = self.pending_async_wakeups.pop_front() {
            if self.async_wake_targets.contains_key(&token) {
                ready.push(Event::Wake(WakeEvent::Async { token, time: now }));
            }
        }

        ready
    }

    fn wake_async(&mut self, token: AsyncWakeToken) -> bool {
        if !self.async_wake_targets.contains_key(&token) {
            return false;
        }

        self.pending_async_wakeups.push_back(token);
        true
    }

    fn preprocess_window_event(&mut self, event: &Event) {
        let Event::Window(window_event) = event else {
            return;
        };

        match window_event {
            WindowEvent::Resized(size) => {
                self.viewport_hint = Some(*size);
                self.schedule.mark(InvalidationKind::Measure);
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                raw_dpi,
                suggested_size,
            } => {
                self.scale_factor = *scale_factor as f32;
                self.raw_dpi = *raw_dpi;
                if let Some(size) = suggested_size {
                    self.viewport_hint = Some(*size);
                }
                self.schedule.mark(InvalidationKind::Measure);
            }
            WindowEvent::Focused(focused) => {
                self.focus.window_focused = *focused;
                if !focused {
                    self.pointer_capture.clear();
                }
            }
            WindowEvent::RedrawRequested => {
                self.schedule.mark(InvalidationKind::Paint);
            }
            WindowEvent::CloseRequested | WindowEvent::Occluded(_) => {}
        }
    }

    fn ensure_graph_for_event(
        &mut self,
        event: &Event,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) {
        if self.schedule.measure || self.schedule.arrange || self.viewport.is_none() {
            let invalidations =
                self.run_measure_arrange_pass(text_system, font_registry, image_registry);
            self.schedule.extend(&invalidations);
            self.pending_invalidations.extend(invalidations);
            return;
        }

        if self.schedule.hit_test || self.graph.is_empty() {
            self.refresh_graph();
        }

        if matches!(event, Event::Keyboard(_) | Event::Ime(_))
            && self.focus.focused_widget.is_some()
            && !self
                .graph
                .contains(self.focus.focused_widget.unwrap_or_default())
        {
            self.focus.focused_widget = None;
        }
    }

    fn resolve_event_target(&self, event: &Event, hit_target: Option<WidgetId>) -> WidgetId {
        match event {
            Event::Pointer(pointer) => self
                .pointer_capture
                .get(&pointer.pointer_id)
                .copied()
                .or(hit_target)
                .unwrap_or(self.root.id()),
            Event::Keyboard(_) | Event::Ime(_) => {
                self.focus.focused_widget.unwrap_or(self.root.id())
            }
            Event::Wake(wake_event) => self.wake_target(*wake_event).unwrap_or(self.root.id()),
            _ => self.root.id(),
        }
    }

    fn wake_target(&self, wake_event: WakeEvent) -> Option<WidgetId> {
        match wake_event {
            WakeEvent::Timer { token, .. } => self.delivering_timers.get(&token).copied(),
            WakeEvent::Async { token, .. } => self.async_wake_targets.get(&token).copied(),
        }
    }

    fn apply_wake_requests(&mut self, requests: Vec<WakeRequest>) {
        for request in requests {
            match request {
                WakeRequest::ScheduleTimer {
                    token,
                    deadline,
                    target,
                } => {
                    self.scheduled_timers.retain(|timer| timer.token != token);
                    self.scheduled_timers.push(ScheduledTimer {
                        token,
                        deadline,
                        target,
                    });
                }
                WakeRequest::CancelTimer { token } => {
                    self.scheduled_timers.retain(|timer| timer.token != token);
                    self.delivering_timers.remove(&token);
                }
                WakeRequest::RegisterAsync { token, target } => {
                    self.async_wake_targets.insert(token, target);
                }
                WakeRequest::UnregisterAsync { token } => {
                    self.async_wake_targets.remove(&token);
                    self.pending_async_wakeups.retain(|queued| *queued != token);
                }
            }
        }
    }

    fn apply_pointer_capture_requests(&mut self, requests: Vec<PointerCaptureRequest>) {
        for request in requests {
            match request {
                PointerCaptureRequest::Capture { pointer_id, target } => {
                    self.pointer_capture.insert(pointer_id, target);
                }
                PointerCaptureRequest::Release { pointer_id } => {
                    self.pointer_capture.remove(&pointer_id);
                }
            }
        }
    }

    fn finish_event(&mut self, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Up | PointerEventKind::Cancel
                ) =>
            {
                self.pointer_capture.remove(&pointer.pointer_id);
            }
            Event::Wake(WakeEvent::Timer { token, .. }) => {
                self.delivering_timers.remove(token);
            }
            _ => {}
        }
    }

    fn route_event(&mut self, target: WidgetId, event: &Event) -> EventRouteResult {
        let path = self
            .graph
            .path_to(target)
            .unwrap_or_else(|| vec![self.root.id()]);
        let mut effects = EventEffects::default();
        let mut handled = false;
        let mut focus_request = None;

        if path.len() > 1 {
            for &widget_id in &path[..path.len() - 1] {
                let dispatch = self
                    .root
                    .dispatch_event_for(
                        widget_id,
                        self.id,
                        self.last_tick_time,
                        EventPhase::Capture,
                        self.focus.focused_widget,
                        event,
                    )
                    .unwrap_or_else(|| empty_dispatch());
                let dispatch_handled = dispatch.handled;
                let dispatch_focus_request = dispatch.focus_request;
                effects.extend(dispatch);
                if dispatch_focus_request.is_some() {
                    focus_request = dispatch_focus_request;
                }
                if dispatch_handled {
                    handled = true;
                    break;
                }
            }
        }

        if !handled {
            let target_id = *path.last().unwrap_or(&self.root.id());
            let dispatch = self
                .root
                .dispatch_event_for(
                    target_id,
                    self.id,
                    self.last_tick_time,
                    EventPhase::Target,
                    self.focus.focused_widget,
                    event,
                )
                .unwrap_or_else(|| empty_dispatch());
            let dispatch_handled = dispatch.handled;
            let dispatch_focus_request = dispatch.focus_request;
            effects.extend(dispatch);
            if dispatch_focus_request.is_some() {
                focus_request = dispatch_focus_request;
            }
            handled = dispatch_handled;
        }

        if !handled && path.len() > 1 {
            for &widget_id in path[..path.len() - 1].iter().rev() {
                let dispatch = self
                    .root
                    .dispatch_event_for(
                        widget_id,
                        self.id,
                        self.last_tick_time,
                        EventPhase::Bubble,
                        self.focus.focused_widget,
                        event,
                    )
                    .unwrap_or_else(|| empty_dispatch());
                let dispatch_handled = dispatch.handled;
                let dispatch_focus_request = dispatch.focus_request;
                effects.extend(dispatch);
                if dispatch_focus_request.is_some() {
                    focus_request = dispatch_focus_request;
                }
                if dispatch_handled {
                    break;
                }
            }
        }

        EventRouteResult {
            path,
            effects,
            focus_request,
        }
    }

    fn default_focus_request(
        &self,
        event: &Event,
        hit_target: Option<WidgetId>,
        path: &[WidgetId],
    ) -> Option<FocusRequest> {
        match event {
            Event::Keyboard(keyboard)
                if keyboard.state == KeyState::Pressed
                    && keyboard.key == "Tab"
                    && !keyboard.repeat
                    && !keyboard.is_composing
                    && !keyboard.modifiers.control
                    && !keyboard.modifiers.alt
                    && !keyboard.modifiers.meta =>
            {
                self.graph
                    .next_focusable(self.focus.focused_widget, keyboard.modifiers.shift)
                    .map(FocusRequest::Focus)
                    .or(Some(FocusRequest::Clear))
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                let Some(hit_target) = hit_target else {
                    return Some(FocusRequest::Clear);
                };

                path.iter()
                    .rev()
                    .copied()
                    .find(|widget_id| {
                        self.graph
                            .node(*widget_id)
                            .is_some_and(|node| node.accepts_focus)
                    })
                    .map(FocusRequest::Focus)
                    .or_else(|| Some(FocusRequest::Clear))
                    .filter(|request| {
                        matches!(request, FocusRequest::Focus(id) if *id == hit_target)
                            || matches!(request, FocusRequest::Clear)
                    })
            }
            Event::Window(WindowEvent::Focused(false)) => Some(FocusRequest::Clear),
            _ => None,
        }
    }

    fn apply_focus_request(&mut self, request: FocusRequest) -> EventEffects {
        let next_focus = match request {
            FocusRequest::Focus(widget_id) => Some(widget_id),
            FocusRequest::Clear => None,
        };

        if self.focus.focused_widget == next_focus {
            return EventEffects::default();
        }

        let previous_focus = self.focus.focused_widget;
        self.focus.focused_widget = next_focus;

        let mut effects = EventEffects::default();

        if let Some(widget_id) = previous_focus {
            effects
                .invalidations
                .extend(self.focus_transition_invalidations(widget_id));
            if let Some(extra) = self.root.notify_focus_change_for(
                widget_id,
                self.id,
                self.last_tick_time,
                self.focus.focused_widget,
                false,
            ) {
                effects.extend(extra);
            }
        }

        if let Some(widget_id) = next_focus {
            effects
                .invalidations
                .extend(self.focus_transition_invalidations(widget_id));
            if let Some(extra) = self.root.notify_focus_change_for(
                widget_id,
                self.id,
                self.last_tick_time,
                self.focus.focused_widget,
                true,
            ) {
                effects.extend(extra);
            }
        }

        self.schedule.mark(InvalidationKind::Paint);
        self.schedule.mark(InvalidationKind::Semantics);
        self.schedule.mark(InvalidationKind::HitTest);
        self.refresh_graph();

        effects
    }

    fn focus_transition_invalidations(&self, widget_id: WidgetId) -> Vec<InvalidationRequest> {
        let mut invalidations = vec![InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            InvalidationKind::Semantics,
        )];

        if let Some(node) = self.graph.node(widget_id) {
            invalidations.push(
                InvalidationRequest::new(
                    InvalidationTarget::Widget(widget_id),
                    InvalidationKind::Paint,
                )
                .with_region(node.geometry.paint_bounds),
            );
        }

        invalidations
    }

    fn render(
        &mut self,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> RenderOutput {
        let diagnostics_enabled = window_scene_statistics_detail_mode(self.id).is_detailed();
        let mut diagnostics = RenderDiagnostics::default();
        let mut invalidations = std::mem::take(&mut self.pending_invalidations);
        let mut repainted = false;
        let mut repaint_layers = Vec::new();
        let mut dirty_layers = Vec::new();
        let mut layer_updates = Vec::new();
        let previous_graph =
            if (self.schedule.measure || self.schedule.arrange) && !self.graph.is_empty() {
                Some(self.graph.snapshot())
            } else {
                None
            };
        let mut graph_changes = GraphChangeSet::default();

        if self.last_frame.is_none() {
            self.schedule = FrameSchedule::bootstrap();
        }

        if self.schedule.measure || self.schedule.arrange || self.viewport.is_none() {
            let started = Instant::now();
            let pass_invalidations = self.run_measure_arrange_pass(
                Arc::clone(&text_system),
                Arc::clone(&font_registry),
                Arc::clone(&image_registry),
            );
            self.schedule.extend(&pass_invalidations);
            invalidations.extend(pass_invalidations);
            graph_changes = self.collect_graph_changes(previous_graph.as_ref());
            if diagnostics_enabled {
                diagnostics.push(FramePhase::MeasureArrange, started.elapsed());
            }
        } else if self.schedule.hit_test || self.graph.is_empty() {
            let started = Instant::now();
            self.refresh_graph();
            if diagnostics_enabled {
                diagnostics.push(FramePhase::HitTest, started.elapsed());
            }
        }

        let viewport = self.viewport.unwrap_or(Size::ZERO);
        let dpi_info = self.dpi_info_for_viewport(viewport);
        let mut composition_only_transforms = self.select_composition_only_transforms(
            self.last_frame.as_ref().map(|frame| &frame.scene),
            &graph_changes.transform_widgets,
        );
        for translation in self.select_explicit_transform_translations(
            self.last_frame.as_ref().map(|frame| &frame.scene),
            &invalidations,
        ) {
            if composition_only_transforms
                .iter()
                .all(|existing| existing.widget_id != translation.widget_id)
            {
                composition_only_transforms.push(translation);
            }
        }
        let composition_only_transform_ids = composition_only_transforms
            .iter()
            .map(|translation| translation.widget_id)
            .collect::<HashSet<_>>();
        let mut repaint_graph_widgets = graph_changes.repaint_widgets.clone();
        for translation in &graph_changes.transform_widgets {
            if !composition_only_transform_ids.contains(&translation.widget_id) {
                repaint_graph_widgets.push(translation.widget_id);
            }
        }

        if self.schedule.paint || self.last_frame.is_none() {
            repaint_layers = self.collect_dirty_layers(&invalidations);
            dirty_layers = repaint_layers.clone();
            for widget_id in repaint_graph_widgets.iter().copied() {
                if self.graph.contains(widget_id) && !dirty_layers.contains(&widget_id) {
                    dirty_layers.push(widget_id);
                }
            }
            dirty_layers.sort_by_key(|widget_id| {
                (
                    self.graph
                        .path_to(*widget_id)
                        .map_or(usize::MAX, |path| path.len()),
                    widget_id.get(),
                )
            });

            composition_only_transforms.retain(|translation| {
                !dirty_layers.iter().any(|dirty_widget| {
                    *dirty_widget == translation.widget_id
                        || self.widget_is_ancestor_of(*dirty_widget, translation.widget_id)
                })
            });
        }

        let mut scene_changed = false;
        let has_ordering_updates = invalidations
            .iter()
            .any(|request| request.kind == InvalidationKind::Ordering);
        if self.last_frame.is_none()
            || !repaint_layers.is_empty()
            || !composition_only_transforms.is_empty()
            || has_ordering_updates
        {
            let started = Instant::now();
            repainted = self.last_frame.is_none() || !repaint_layers.is_empty();
            scene_changed = true;

            let focused_path = self
                .focus
                .focused_widget
                .and_then(|widget_id| self.graph.path_to(widget_id));
            let preserve_ime = focused_path.as_ref().is_none_or(|path| {
                !repaint_layers
                    .iter()
                    .any(|widget_id| path.iter().any(|candidate| candidate == widget_id))
            });
            let baseline_ime_composition_rect = if preserve_ime {
                self.ime_composition_rect
            } else {
                None
            };

            let (scene, paint_invalidations, ime_composition_rect) = if self.last_frame.is_none()
                || repaint_layers.contains(&self.root.id())
            {
                self.paint_full_scene(dpi_info)
            } else if repaint_layers.is_empty() {
                (
                    self.last_frame
                        .as_ref()
                        .map(|frame| frame.scene.clone())
                        .unwrap_or_default(),
                    Vec::new(),
                    baseline_ime_composition_rect,
                )
            } else {
                self.repaint_dirty_layers(dpi_info, &repaint_layers, baseline_ime_composition_rect)
            };
            let mut scene = scene;
            for translation in &composition_only_transforms {
                let _ = scene.translate_layer(translation.widget_id, translation.delta);
            }
            self.sync_scene_stack_metadata(&mut scene);
            self.graph.update_paint_bounds_from_scene(&scene);
            invalidations.extend(paint_invalidations);
            self.ime_composition_rect = ime_composition_rect;
            let previous_scene = self.last_frame.as_ref().map(|frame| &frame.scene);
            layer_updates = self.collect_layer_updates(
                previous_scene,
                &scene,
                &invalidations,
                &dirty_layers,
                &composition_only_transforms
                    .iter()
                    .map(|translation| translation.widget_id)
                    .collect::<Vec<_>>(),
            );
            self.last_frame = Some(SceneFrame {
                window_id: self.id,
                viewport,
                surface_size: dpi_info.surface_size,
                scale_factor: dpi_info.scale_factor,
                dirty_regions: Vec::new(),
                layer_updates: layer_updates.clone(),
                scene,
                font_registry: Arc::clone(&font_registry),
                image_registry: Arc::clone(&image_registry),
            });
            if diagnostics_enabled && repainted {
                diagnostics.push(FramePhase::Paint, started.elapsed());
            }
        }

        if self.schedule.semantics || self.last_semantics.is_empty() {
            let started = Instant::now();
            let mut semantics_ctx = SemanticsCtx::new(
                self.id,
                self.root.id(),
                self.root.id(),
                self.root.bounds(),
                self.focus.focused_widget,
            );
            self.root.semantics(&mut semantics_ctx);
            self.last_semantics = self.assemble_semantics_tree(semantics_ctx.into_nodes());
            if diagnostics_enabled {
                diagnostics.push(FramePhase::Semantics, started.elapsed());
            }
        }

        let viewport_rect = Rect::from_origin_size(Point::ZERO, viewport);
        let dirty_regions = collect_dirty_regions(viewport, &invalidations, repainted, |request| {
            self.default_dirty_region_for_request(viewport_rect, request)
        });
        let mut frame = self
            .last_frame
            .clone()
            .unwrap_or_else(|| SceneFrame::new(self.id, viewport));
        frame.viewport = viewport;
        frame.surface_size = dpi_info.surface_size;
        frame.scale_factor = dpi_info.scale_factor;
        frame.dirty_regions = dirty_regions;
        frame.layer_updates = if scene_changed {
            layer_updates
        } else {
            self.collect_layer_updates(
                self.last_frame.as_ref().map(|stored| &stored.scene),
                &frame.scene,
                &invalidations,
                &[],
                &composition_only_transforms
                    .iter()
                    .map(|translation| translation.widget_id)
                    .collect::<Vec<_>>(),
            )
        };
        frame.font_registry = font_registry;
        frame.image_registry = image_registry;

        if diagnostics_enabled {
            let layout_cache = text_system.layout_cache_snapshot();
            diagnostics.text_caches.runtime_layout =
                CacheMetrics::new(layout_cache.entries, layout_cache.hits, layout_cache.misses);
        }

        self.schedule.clear();

        RenderOutput {
            title: self.title.clone(),
            frame,
            semantics: self.last_semantics.clone(),
            ime_composition_rect: self.ime_composition_rect,
            diagnostics,
        }
    }

    fn paint_full_scene(
        &mut self,
        dpi_info: DpiInfo,
    ) -> (Scene, Vec<InvalidationRequest>, Option<Rect>) {
        let mut paint_ctx = PaintCtx::new(
            self.id,
            self.root.id(),
            self.root.bounds(),
            self.focus.focused_widget,
            dpi_info,
        );
        let _ = self
            .root
            .paint_layer_contents_for(self.root.id(), &mut paint_ctx);
        paint_ctx.into_parts()
    }

    fn repaint_dirty_layers(
        &mut self,
        dpi_info: DpiInfo,
        dirty_layers: &[WidgetId],
        baseline_ime_composition_rect: Option<Rect>,
    ) -> (Scene, Vec<InvalidationRequest>, Option<Rect>) {
        let mut scene = self
            .last_frame
            .as_ref()
            .map(|frame| frame.scene.clone())
            .unwrap_or_default();
        let mut invalidations = Vec::new();
        let mut ime_composition_rect = baseline_ime_composition_rect;

        for &widget_id in dirty_layers {
            let Some(bounds) = self
                .graph
                .node(widget_id)
                .map(|node| node.geometry.layout_bounds)
            else {
                return self.paint_full_scene(dpi_info);
            };

            let mut paint_ctx = PaintCtx::new(
                self.id,
                widget_id,
                bounds,
                self.focus.focused_widget,
                dpi_info,
            );
            if !self
                .root
                .paint_layer_contents_for(widget_id, &mut paint_ctx)
            {
                return self.paint_full_scene(dpi_info);
            }

            let (layer_scene, layer_invalidations, layer_ime_composition_rect) =
                paint_ctx.into_parts();
            let Some(descriptor) = self.root.layer_descriptor_for(widget_id, &layer_scene) else {
                return self.paint_full_scene(dpi_info);
            };
            if widget_id == self.root.id()
                || !scene.replace_layer(
                    widget_id,
                    SceneLayer::from_descriptor(descriptor, layer_scene),
                )
            {
                return self.paint_full_scene(dpi_info);
            }

            invalidations.extend(layer_invalidations);
            if layer_ime_composition_rect.is_some() {
                ime_composition_rect = layer_ime_composition_rect;
            }
        }

        (scene, invalidations, ime_composition_rect)
    }

    fn collect_dirty_layers(&self, invalidations: &[InvalidationRequest]) -> Vec<WidgetId> {
        let candidates: HashSet<WidgetId> = invalidations
            .iter()
            .filter(|request| {
                matches!(
                    request.kind,
                    InvalidationKind::Measure
                        | InvalidationKind::Clip
                        | InvalidationKind::Effect
                        | InvalidationKind::Visibility
                        | InvalidationKind::Paint
                        | InvalidationKind::Text
                        | InvalidationKind::Resources
                )
            })
            .map(|request| match request.target {
                InvalidationTarget::Widget(widget_id) if self.graph.contains(widget_id) => {
                    widget_id
                }
                InvalidationTarget::Widget(_) => self.root.id(),
                InvalidationTarget::Window(_) | InvalidationTarget::Surface(_) => self.root.id(),
            })
            .collect();

        if self.last_frame.is_none() || candidates.contains(&self.root.id()) {
            return self
                .graph
                .snapshot()
                .nodes
                .into_iter()
                .map(|node| node.id)
                .collect();
        }

        let mut candidates: Vec<_> = candidates
            .into_iter()
            .filter(|widget_id| self.graph.contains(*widget_id))
            .collect();
        candidates.sort_by_key(|widget_id| {
            self.graph
                .path_to(*widget_id)
                .map_or(usize::MAX, |path| path.len())
        });

        let mut minimized = Vec::new();
        for widget_id in candidates {
            if minimized
                .iter()
                .any(|ancestor| self.widget_is_ancestor_of(*ancestor, widget_id))
            {
                continue;
            }
            minimized.push(widget_id);
        }

        minimized
    }

    fn collect_graph_changes(&self, previous: Option<&WidgetGraphSnapshot>) -> GraphChangeSet {
        let Some(previous) = previous else {
            return GraphChangeSet {
                repaint_widgets: vec![self.graph.root],
                transform_widgets: Vec::new(),
            };
        };

        let current_snapshot = self.graph.snapshot();
        let previous_nodes: HashMap<_, _> =
            previous.nodes.iter().map(|node| (node.id, node)).collect();
        let current_nodes: HashMap<_, _> = current_snapshot
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect();
        let widget_ids = previous_nodes
            .keys()
            .chain(current_nodes.keys())
            .copied()
            .collect::<HashSet<_>>();
        let mut repaint_candidates = HashSet::new();
        let mut transform_candidates = Vec::new();

        for widget_id in widget_ids {
            match (
                previous_nodes.get(&widget_id),
                current_nodes.get(&widget_id),
            ) {
                (Some(previous_node), Some(current_node)) => {
                    if previous_node.measured_size != current_node.measured_size
                        || previous_node.geometry.layout_bounds.size
                            != current_node.geometry.layout_bounds.size
                        || previous_node.parent != current_node.parent
                        || previous_node.children != current_node.children
                    {
                        repaint_candidates.insert(current_node.id);
                        continue;
                    }

                    let delta = current_node.geometry.layout_bounds.origin
                        - previous_node.geometry.layout_bounds.origin;
                    if delta != Vector::ZERO {
                        transform_candidates.push(LayerTranslation {
                            widget_id: current_node.id,
                            delta,
                        });
                    }
                }
                (None, Some(current_node)) => {
                    repaint_candidates.insert(current_node.parent.unwrap_or(current_snapshot.root));
                }
                (Some(previous_node), None) => {
                    repaint_candidates.insert(previous_node.parent.unwrap_or(previous.root));
                }
                (None, None) => {}
            }
        }

        let mut repaint_widgets = repaint_candidates.into_iter().collect::<Vec<_>>();
        repaint_widgets.sort_by_key(|widget_id| self.widget_depth(*widget_id));
        let mut minimized_repaint = Vec::new();
        for widget_id in repaint_widgets {
            if minimized_repaint
                .iter()
                .any(|ancestor| self.widget_is_ancestor_of(*ancestor, widget_id))
            {
                continue;
            }
            minimized_repaint.push(widget_id);
        }

        transform_candidates.sort_by_key(|translation| self.widget_depth(translation.widget_id));
        let mut minimized_transforms = Vec::new();
        for candidate in transform_candidates {
            if minimized_repaint
                .iter()
                .any(|ancestor| self.widget_is_ancestor_of(*ancestor, candidate.widget_id))
            {
                continue;
            }

            let inherited_delta = minimized_transforms
                .iter()
                .filter(|translation: &&LayerTranslation| {
                    self.widget_is_ancestor_of(translation.widget_id, candidate.widget_id)
                })
                .fold(Vector::ZERO, |current, translation| {
                    current + translation.delta
                });
            let residual = candidate.delta - inherited_delta;
            if residual == Vector::ZERO {
                continue;
            }

            minimized_transforms.push(LayerTranslation {
                widget_id: candidate.widget_id,
                delta: residual,
            });
        }

        GraphChangeSet {
            repaint_widgets: minimized_repaint,
            transform_widgets: minimized_transforms,
        }
    }

    fn select_composition_only_transforms(
        &self,
        previous_scene: Option<&Scene>,
        transform_widgets: &[LayerTranslation],
    ) -> Vec<LayerTranslation> {
        let previous_layers = previous_scene.map(collect_scene_layers).unwrap_or_default();
        transform_widgets
            .iter()
            .copied()
            .filter(|translation| previous_layers.contains_key(&translation.widget_id))
            .collect()
    }

    fn select_explicit_transform_translations(
        &self,
        previous_scene: Option<&Scene>,
        invalidations: &[InvalidationRequest],
    ) -> Vec<LayerTranslation> {
        let previous_layers = previous_scene.map(collect_scene_layers).unwrap_or_default();
        let mut translations = Vec::new();

        for request in invalidations {
            if request.kind != InvalidationKind::Transform {
                continue;
            }

            let InvalidationTarget::Widget(widget_id) = request.target else {
                continue;
            };
            let Some(previous) = previous_layers.get(&widget_id) else {
                continue;
            };
            let Some(current) = self.graph.node(widget_id) else {
                continue;
            };

            let delta = current.geometry.layout_bounds.origin - previous.bounds.origin;
            if delta == Vector::ZERO {
                continue;
            }

            if translations
                .iter()
                .all(|translation: &LayerTranslation| translation.widget_id != widget_id)
            {
                translations.push(LayerTranslation { widget_id, delta });
            }
        }

        translations
    }

    fn widget_depth(&self, widget_id: WidgetId) -> usize {
        self.graph
            .path_to(widget_id)
            .map_or(usize::MAX, |path| path.len())
    }

    fn default_dirty_region_for_request(
        &self,
        viewport_rect: Rect,
        request: &InvalidationRequest,
    ) -> Rect {
        match request.target {
            InvalidationTarget::Widget(widget_id) => self
                .graph
                .node(widget_id)
                .map(|node| node.geometry.paint_bounds)
                .unwrap_or(viewport_rect),
            InvalidationTarget::Window(_) | InvalidationTarget::Surface(_) => viewport_rect,
        }
    }

    fn collect_layer_updates(
        &self,
        previous_scene: Option<&Scene>,
        scene: &Scene,
        invalidations: &[InvalidationRequest],
        dirty_layers: &[WidgetId],
        graph_dirty_widgets: &[WidgetId],
    ) -> Vec<SceneLayerUpdate> {
        let current_layers = collect_scene_layers(scene);
        if current_layers.is_empty() {
            return Vec::new();
        }

        let previous_layers = previous_scene.map(collect_scene_layers).unwrap_or_default();
        let mut updates = HashMap::<WidgetId, SceneLayerUpdateKind>::new();
        let mut damage_regions = HashMap::<WidgetId, Rect>::new();
        let explicit_transform_widgets = invalidations
            .iter()
            .filter_map(|request| {
                if request.kind != InvalidationKind::Transform {
                    return None;
                }

                match request.target {
                    InvalidationTarget::Widget(widget_id) if self.graph.contains(widget_id) => {
                        Some(widget_id)
                    }
                    _ => None,
                }
            })
            .collect::<HashSet<_>>();

        for request in invalidations {
            let Some(kind) = invalidation_to_layer_update_kind(request.kind) else {
                continue;
            };
            let widget_id = match request.target {
                InvalidationTarget::Widget(widget_id) if self.graph.contains(widget_id) => {
                    widget_id
                }
                InvalidationTarget::Widget(_) => self.root.id(),
                InvalidationTarget::Window(_) | InvalidationTarget::Surface(_) => self.root.id(),
            };
            if request.kind == InvalidationKind::Arrange
                && explicit_transform_widgets
                    .iter()
                    .any(|candidate| self.widget_is_ancestor_of(widget_id, *candidate))
            {
                continue;
            }
            merge_layer_update_kind(&mut updates, widget_id, kind);
            if let Some(region) = request.region {
                damage_regions
                    .entry(widget_id)
                    .and_modify(|current| *current = current.union(region))
                    .or_insert(region);
            }
        }

        for (widget_id, descriptor) in &current_layers {
            let Some(previous) = previous_layers.get(widget_id) else {
                continue;
            };
            if descriptor.stack_host != previous.stack_host
                || descriptor.stack_order != previous.stack_order
                || descriptor.transient_owner_surface != previous.transient_owner_surface
                || descriptor.is_stack_surface != previous.is_stack_surface
            {
                merge_layer_update_kind(&mut updates, *widget_id, SceneLayerUpdateKind::Ordering);
            }
        }

        for widget_id in graph_dirty_widgets.iter().copied() {
            merge_layer_update_kind(&mut updates, widget_id, SceneLayerUpdateKind::Transform);
        }

        for widget_id in dirty_layers.iter().copied() {
            merge_layer_update_kind(&mut updates, widget_id, SceneLayerUpdateKind::Content);
        }

        let mut resolved_updates = Vec::new();
        for (widget_id, kind) in updates {
            let Some(descriptor) = current_layers.get(&widget_id).cloned() else {
                continue;
            };
            let fallback_damage = previous_layers
                .get(&widget_id)
                .map(|previous| previous.paint_bounds.union(descriptor.paint_bounds))
                .unwrap_or(descriptor.paint_bounds);
            let damage = damage_regions
                .get(&widget_id)
                .copied()
                .unwrap_or(fallback_damage);
            resolved_updates
                .push(SceneLayerUpdate::from_descriptor(kind, descriptor).with_damage(damage));
        }

        resolved_updates.sort_by_key(|update| update.owner.get());
        resolved_updates
    }

    fn widget_is_ancestor_of(&self, ancestor: WidgetId, widget_id: WidgetId) -> bool {
        self.graph.path_to(widget_id).is_some_and(|path| {
            path.iter()
                .take(path.len().saturating_sub(1))
                .any(|candidate| *candidate == ancestor)
        })
    }

    fn run_measure_arrange_pass(
        &mut self,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> Vec<InvalidationRequest> {
        let constraints = self.measure_constraints();
        let mut measure_ctx = MeasureCtx::new(
            self.id,
            self.root.id(),
            self.root.bounds(),
            self.current_dpi_info(),
            text_system,
            font_registry,
            image_registry,
        );
        let measured_root = if self.schedule.measure || self.viewport.is_none() {
            self.root.measure(&mut measure_ctx, constraints)
        } else {
            self.root.measured_size()
        };
        let viewport = constraints.clamp(measured_root);

        let mut arrange_ctx = ArrangeCtx::new(self.id, self.root.id(), self.current_dpi_info());
        self.root.arrange(
            &mut arrange_ctx,
            Rect::from_origin_size(Point::ZERO, viewport),
        );
        self.viewport = Some(viewport);
        self.schedule.measure = false;
        self.schedule.arrange = false;
        self.schedule.hit_test = false;
        self.refresh_graph();

        let mut invalidations = measure_ctx.take_invalidations();
        invalidations.extend(arrange_ctx.take_invalidations());
        invalidations
    }

    fn refresh_graph(&mut self) {
        let paint_bounds_by_widget = self
            .last_frame
            .as_ref()
            .map(|frame| {
                collect_scene_layers(&frame.scene)
                    .into_iter()
                    .map(|(widget_id, descriptor)| (widget_id, descriptor.paint_bounds))
                    .collect::<HashMap<WidgetId, Rect>>()
            })
            .unwrap_or_default();
        self.graph = WidgetGraph::rebuild(
            &self.root,
            self.focus.focused_widget,
            &paint_bounds_by_widget,
        );
        self.prune_runtime_state();
        self.schedule.hit_test = false;
    }

    fn sync_scene_stack_metadata(&self, scene: &mut Scene) {
        scene.visit_layers_mut(&mut |layer| {
            if let Some(node) = self.graph.node(layer.widget_id()) {
                layer.descriptor.stack_host = node.stack_host;
                layer.descriptor.stack_order = node.stack_surface_order;
                layer.descriptor.transient_owner_surface = node.transient_owner_surface;
                layer.descriptor.is_stack_surface = node.is_stack_surface;
            }
        });
        scene.reorder_stack_surfaces();
    }

    fn prune_runtime_state(&mut self) {
        self.pointer_capture
            .retain(|_, widget_id| self.graph.contains(*widget_id));
        self.scheduled_timers
            .retain(|timer| self.graph.contains(timer.target));
        self.delivering_timers
            .retain(|_, widget_id| self.graph.contains(*widget_id));
        self.async_wake_targets
            .retain(|_, widget_id| self.graph.contains(*widget_id));
        self.pending_async_wakeups
            .retain(|token| self.async_wake_targets.contains_key(token));

        if self
            .focus
            .focused_widget
            .is_some_and(|widget_id| !self.graph.contains(widget_id))
        {
            self.focus.focused_widget = None;
        }
    }

    fn measure_constraints(&self) -> Constraints {
        self.viewport_hint
            .map(Constraints::tight)
            .unwrap_or(Constraints::UNBOUNDED)
    }

    fn dpi_info_for_viewport(&self, viewport: Size) -> DpiInfo {
        DpiInfo::new(
            self.scale_factor,
            self.raw_dpi,
            viewport,
            scale_viewport_to_surface_size(viewport, self.scale_factor),
        )
    }

    fn current_dpi_info(&self) -> DpiInfo {
        let viewport = self.viewport.or(self.viewport_hint).unwrap_or(Size::ZERO);
        self.dpi_info_for_viewport(viewport)
    }
}

fn normalize_scale_factor(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn scale_viewport_to_surface_size(viewport: Size, scale_factor: f32) -> Size {
    if viewport.is_empty() {
        return Size::ZERO;
    }

    let scale_factor = normalize_scale_factor(scale_factor);
    Size::new(
        (viewport.width * scale_factor).round().max(1.0),
        (viewport.height * scale_factor).round().max(1.0),
    )
}

impl WindowState {
    fn assemble_semantics_tree(&self, nodes: Vec<SemanticsNode>) -> Vec<SemanticsNode> {
        let semantic_ids: HashSet<_> = nodes.iter().map(|node| node.id).collect();

        nodes
            .into_iter()
            .map(|mut node| {
                node.parent = self.resolve_semantics_parent(&semantic_ids, node.id, node.parent);
                node.state.focused = Some(node.id) == self.focus.focused_widget;
                node
            })
            .collect()
    }

    fn resolve_semantics_parent(
        &self,
        semantic_ids: &HashSet<WidgetId>,
        widget_id: WidgetId,
        explicit_parent: Option<WidgetId>,
    ) -> Option<WidgetId> {
        if let Some(parent) =
            explicit_parent.filter(|parent| *parent != widget_id && semantic_ids.contains(parent))
        {
            return Some(parent);
        }

        self.graph.path_to(widget_id).and_then(|path| {
            path.into_iter()
                .rev()
                .skip(1)
                .find(|candidate| semantic_ids.contains(candidate))
        })
    }
}

#[derive(Default)]
struct WidgetGraph {
    root: WidgetId,
    nodes: HashMap<WidgetId, WidgetNodeSnapshot>,
    order: Vec<WidgetId>,
    host_surface_order: HashMap<WidgetId, Vec<WidgetId>>,
    host_order_policy: HashMap<WidgetId, StackOrderPolicy>,
}

impl WidgetGraph {
    fn empty(root: WidgetId) -> Self {
        Self {
            root,
            nodes: HashMap::new(),
            order: Vec::new(),
            host_surface_order: HashMap::new(),
            host_order_policy: HashMap::new(),
        }
    }

    fn rebuild(
        root: &WidgetPod,
        focused_widget: Option<WidgetId>,
        paint_bounds_by_widget: &HashMap<WidgetId, Rect>,
    ) -> Self {
        let mut graph = Self::empty(root.id());
        graph.collect(
            root,
            None,
            focused_widget,
            paint_bounds_by_widget,
            root.id(),
            root.id(),
        );
        graph.recompute_stack_surface_order();
        graph
    }

    fn recompute_stack_surface_order(&mut self) {
        let mut position_by_surface = HashMap::<(WidgetId, WidgetId), usize>::new();
        for (host, surfaces) in &self.host_surface_order {
            for (index, surface) in surfaces.iter().copied().enumerate() {
                position_by_surface.insert((*host, surface), index);
            }
        }

        for node in self.nodes.values_mut() {
            if let Some(order) = position_by_surface.get(&(node.stack_host, node.stack_surface)) {
                node.stack_surface_order = *order;
            }
        }
    }

    fn update_paint_bounds_from_scene(&mut self, scene: &Scene) {
        let paint_bounds_by_widget = collect_scene_layers(scene)
            .into_iter()
            .map(|(widget_id, descriptor)| (widget_id, descriptor.paint_bounds))
            .collect::<HashMap<WidgetId, Rect>>();

        for node in self.nodes.values_mut() {
            node.geometry.paint_bounds = paint_bounds_by_widget
                .get(&node.id)
                .copied()
                .unwrap_or(node.geometry.layout_bounds);
        }
    }

    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn contains(&self, widget_id: WidgetId) -> bool {
        self.nodes.contains_key(&widget_id)
    }

    fn node(&self, widget_id: WidgetId) -> Option<&WidgetNodeSnapshot> {
        self.nodes.get(&widget_id)
    }

    fn snapshot(&self) -> WidgetGraphSnapshot {
        let mut stack_hosts = self
            .host_order_policy
            .iter()
            .map(|(host, order_policy)| StackHostSnapshot {
                host: *host,
                order_policy: *order_policy,
                surfaces: self
                    .host_surface_order
                    .get(host)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        stack_hosts.sort_by_key(|host| host.host.get());

        WidgetGraphSnapshot {
            root: self.root,
            nodes: self
                .order
                .iter()
                .filter_map(|widget_id| self.nodes.get(widget_id).cloned())
                .collect(),
            stack_hosts,
        }
    }

    fn hit_test(&self, point: Point) -> Option<WidgetId> {
        self.hit_test_node(self.root, point)
    }

    fn path_to(&self, target: WidgetId) -> Option<Vec<WidgetId>> {
        let mut path = Vec::new();
        let mut current = Some(target);

        while let Some(widget_id) = current {
            let node = self.node(widget_id)?;
            path.push(widget_id);
            current = node.parent;
        }

        path.reverse();
        Some(path)
    }

    fn next_focusable(&self, current: Option<WidgetId>, backwards: bool) -> Option<WidgetId> {
        let focusable: Vec<_> = self
            .order
            .iter()
            .copied()
            .filter(|widget_id| self.node(*widget_id).is_some_and(|node| node.accepts_focus))
            .collect();

        if focusable.is_empty() {
            return None;
        }

        let fallback = if backwards {
            focusable.last().copied()
        } else {
            focusable.first().copied()
        };

        let Some(current) = current else {
            return fallback;
        };

        let Some(index) = focusable.iter().position(|widget_id| *widget_id == current) else {
            return fallback;
        };

        if backwards {
            Some(focusable[(index + focusable.len() - 1) % focusable.len()])
        } else {
            Some(focusable[(index + 1) % focusable.len()])
        }
    }

    fn collect(
        &mut self,
        pod: &WidgetPod,
        parent: Option<WidgetId>,
        focused_widget: Option<WidgetId>,
        paint_bounds_by_widget: &HashMap<WidgetId, Rect>,
        inherited_host: WidgetId,
        inherited_surface: WidgetId,
    ) {
        let id = pod.id();
        self.order.push(id);

        let host_options = if id == self.root {
            Some(
                pod.current_stack_host_options()
                    .unwrap_or_else(StackHostOptions::default),
            )
        } else {
            pod.current_stack_host_options()
        };
        let is_stack_host = host_options.is_some();
        let resolved_host = if is_stack_host { id } else { inherited_host };
        let host_policy = host_options
            .map(|options| options.order_policy)
            .unwrap_or(StackOrderPolicy::Stable);
        if is_stack_host {
            self.host_order_policy.insert(resolved_host, host_policy);
        } else {
            self.host_order_policy
                .entry(resolved_host)
                .or_insert(StackOrderPolicy::Stable);
        }
        let resolved_policy = *self
            .host_order_policy
            .get(&resolved_host)
            .unwrap_or(&StackOrderPolicy::Stable);

        let surface_options = pod.current_stack_surface_options();
        let is_direct_child_of_host = parent == Some(resolved_host);
        let is_stack_surface = surface_options.is_some() || is_direct_child_of_host;
        let resolved_surface = if is_stack_surface {
            id
        } else {
            inherited_surface
        };
        let transient_owner_surface = surface_options
            .filter(|options| options.transient && inherited_surface != id)
            .map(|_| inherited_surface);
        if is_stack_surface {
            let surfaces = self.host_surface_order.entry(resolved_host).or_default();
            if !surfaces.contains(&id) {
                surfaces.push(id);
            }
        }
        let stack_surface_order = self
            .host_surface_order
            .get(&resolved_host)
            .and_then(|surfaces| {
                surfaces
                    .iter()
                    .position(|surface| *surface == resolved_surface)
            })
            .unwrap_or(0);

        let children = {
            let mut visitor = CollectChildrenVisitor {
                graph: self,
                parent: id,
                focused_widget,
                paint_bounds_by_widget,
                inherited_host: resolved_host,
                inherited_surface: resolved_surface,
                children: Vec::new(),
            };
            pod.visit_children(&mut visitor);
            visitor.children
        };

        let layout_bounds = pod.bounds();
        let input_bounds = layout_bounds;
        let paint_bounds = paint_bounds_by_widget
            .get(&id)
            .copied()
            .unwrap_or(layout_bounds);

        self.nodes.insert(
            id,
            WidgetNodeSnapshot {
                id,
                parent,
                children,
                measured_size: pod.measured_size(),
                geometry: WidgetGeometrySnapshot::new(layout_bounds, input_bounds, paint_bounds),
                bounds: layout_bounds,
                stack_host: resolved_host,
                stack_surface: resolved_surface,
                stack_surface_order,
                transient_owner_surface,
                is_stack_host,
                is_stack_surface,
                stack_order_policy: resolved_policy,
                accepts_focus: pod.accepts_focus(),
                focused: Some(id) == focused_widget,
            },
        );
    }

    fn hit_test_node(&self, widget_id: WidgetId, point: Point) -> Option<WidgetId> {
        let node = self.node(widget_id)?;
        if !node.geometry.input_bounds.contains(point) {
            return None;
        }

        if node.is_stack_host {
            let mut tested_surfaces = HashSet::new();
            if let Some(ordered_surfaces) = self.host_surface_order.get(&node.id) {
                for surface_id in ordered_surfaces.iter().rev().copied() {
                    tested_surfaces.insert(surface_id);
                    if let Some(hit) = self.hit_test_node(surface_id, point) {
                        return Some(hit);
                    }
                }
            }

            for child_id in node.children.iter().rev() {
                if self.node(*child_id).is_some_and(|child| {
                    child.is_stack_surface || tested_surfaces.contains(child_id)
                }) {
                    continue;
                }
                if let Some(hit) = self.hit_test_node(*child_id, point) {
                    return Some(hit);
                }
            }

            return Some(widget_id);
        }

        for child_id in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_node(*child_id, point) {
                return Some(hit);
            }
        }

        Some(widget_id)
    }
}

struct CollectChildrenVisitor<'a> {
    graph: &'a mut WidgetGraph,
    parent: WidgetId,
    focused_widget: Option<WidgetId>,
    paint_bounds_by_widget: &'a HashMap<WidgetId, Rect>,
    inherited_host: WidgetId,
    inherited_surface: WidgetId,
    children: Vec<WidgetId>,
}

impl WidgetPodVisitor for CollectChildrenVisitor<'_> {
    fn visit(&mut self, child: &WidgetPod) {
        self.children.push(child.id());
        self.graph.collect(
            child,
            Some(self.parent),
            self.focused_widget,
            self.paint_bounds_by_widget,
            self.inherited_host,
            self.inherited_surface,
        );
    }
}

struct EventRouteResult {
    path: Vec<WidgetId>,
    effects: EventEffects,
    focus_request: Option<FocusRequest>,
}

#[derive(Default)]
struct HoverTransitionResult {
    effects: EventEffects,
    focus_request: Option<FocusRequest>,
    skip_primary_route: bool,
}

#[derive(Default)]
struct EventEffects {
    invalidations: Vec<InvalidationRequest>,
    wake_requests: Vec<WakeRequest>,
    pointer_capture_requests: Vec<PointerCaptureRequest>,
}

impl EventEffects {
    fn extend(&mut self, dispatch: widget::EventDispatch) {
        self.invalidations.extend(dispatch.invalidations);
        self.wake_requests.extend(dispatch.wake_requests);
        self.pointer_capture_requests
            .extend(dispatch.pointer_capture_requests);
    }
}

fn empty_dispatch() -> widget::EventDispatch {
    widget::EventDispatch {
        handled: false,
        invalidations: Vec::new(),
        focus_request: None,
        wake_requests: Vec::new(),
        pointer_capture_requests: Vec::new(),
    }
}

fn collect_dirty_regions<F>(
    viewport: Size,
    invalidations: &[InvalidationRequest],
    repainted: bool,
    default_region_for: F,
) -> Vec<DirtyRegion>
where
    F: FnMut(&InvalidationRequest) -> Rect,
{
    let viewport_rect = Rect::from_origin_size(Point::ZERO, viewport);
    let mut default_region_for = default_region_for;

    if invalidations.is_empty() {
        return if repainted {
            vec![DirtyRegion::new(viewport_rect, InvalidationKind::Paint)]
        } else {
            Vec::new()
        };
    }
    let mut dirty_regions: Vec<_> = invalidations
        .iter()
        .map(|request| {
            DirtyRegion::new(
                request
                    .region
                    .unwrap_or_else(|| default_region_for(request)),
                request.kind,
            )
        })
        .collect();

    if repainted
        && dirty_regions
            .iter()
            .all(|region| region.kind != InvalidationKind::Paint)
    {
        dirty_regions.push(DirtyRegion::new(viewport_rect, InvalidationKind::Paint));
    }

    dirty_regions
}

fn collect_scene_layers(scene: &Scene) -> HashMap<WidgetId, SceneLayerDescriptor> {
    let mut layers = HashMap::new();
    scene.visit_layers(&mut |layer| {
        layers.insert(layer.widget_id(), layer.descriptor.clone());
    });
    layers
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitTestCompositionPhase {
    Normal,
    Overlay,
    Effect,
}

fn scene_hit_test_for_phase(
    scene: &Scene,
    point: Point,
    target_phase: HitTestCompositionPhase,
    inherited_phase: HitTestCompositionPhase,
) -> Option<WidgetId> {
    for command in scene.commands().iter().rev() {
        let SceneCommand::Layer(layer) = command else {
            continue;
        };

        let layer_phase = next_hit_test_phase(inherited_phase, layer.descriptor.composition_mode);
        if !layer.descriptor.paint_bounds.contains(point) {
            continue;
        }

        if let Some(hit) = scene_hit_test_for_phase(&layer.scene, point, target_phase, layer_phase)
        {
            return Some(hit);
        }

        if layer_phase == target_phase {
            return Some(layer.widget_id());
        }
    }

    None
}

fn next_hit_test_phase(
    inherited_phase: HitTestCompositionPhase,
    composition_mode: LayerCompositionMode,
) -> HitTestCompositionPhase {
    match (inherited_phase, composition_mode) {
        (HitTestCompositionPhase::Effect, _) | (_, LayerCompositionMode::Effect) => {
            HitTestCompositionPhase::Effect
        }
        (HitTestCompositionPhase::Overlay, _) | (_, LayerCompositionMode::Overlay) => {
            HitTestCompositionPhase::Overlay
        }
        _ => HitTestCompositionPhase::Normal,
    }
}

fn invalidation_to_layer_update_kind(kind: InvalidationKind) -> Option<SceneLayerUpdateKind> {
    match kind {
        InvalidationKind::Measure | InvalidationKind::Arrange | InvalidationKind::Transform => {
            Some(SceneLayerUpdateKind::Transform)
        }
        InvalidationKind::Ordering => Some(SceneLayerUpdateKind::Ordering),
        InvalidationKind::Clip => Some(SceneLayerUpdateKind::Clip),
        InvalidationKind::Effect => Some(SceneLayerUpdateKind::Effect),
        InvalidationKind::Visibility => Some(SceneLayerUpdateKind::Visibility),
        InvalidationKind::Paint | InvalidationKind::Text => Some(SceneLayerUpdateKind::Content),
        InvalidationKind::Resources => Some(SceneLayerUpdateKind::Resources),
        InvalidationKind::HitTest | InvalidationKind::Semantics => None,
    }
}

fn merge_layer_update_kind(
    updates: &mut HashMap<WidgetId, SceneLayerUpdateKind>,
    widget_id: WidgetId,
    kind: SceneLayerUpdateKind,
) {
    updates
        .entry(widget_id)
        .and_modify(|current| {
            if layer_update_priority(kind) > layer_update_priority(*current) {
                *current = kind;
            }
        })
        .or_insert(kind);
}

fn layer_update_priority(kind: SceneLayerUpdateKind) -> u8 {
    match kind {
        SceneLayerUpdateKind::Visibility => 0,
        SceneLayerUpdateKind::Ordering => 1,
        SceneLayerUpdateKind::Transform => 2,
        SceneLayerUpdateKind::Clip => 3,
        SceneLayerUpdateKind::Content => 4,
        SceneLayerUpdateKind::Effect => 5,
        SceneLayerUpdateKind::Resources => 6,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderOutput {
    pub title: String,
    pub frame: SceneFrame,
    pub semantics: Vec<SemanticsNode>,
    pub ime_composition_rect: Option<Rect>,
    pub diagnostics: RenderDiagnostics,
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{
        Application, ArrangeCtx, EventCtx, FocusState, FrameSchedule, LayerOptions, MeasureCtx,
        PaintCtx, Runtime, SceneStatisticsDetailMode, SemanticsCtx, SingleChild, Widget,
        WidgetChildren, WidgetGraphSnapshot, WidgetNodeSnapshot, WidgetPodMutVisitor,
        WidgetPodVisitor, WindowBuilder, set_window_scene_statistics_detail_mode,
    };
    use sui_core::{
        AsyncWakeToken, Color, CustomEvent, Event, FontHandle, ImageHandle, KeyState,
        KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect,
        SemanticsNode, SemanticsRole, Size, TimerToken, WakeEvent, WindowEvent,
    };
    use sui_layout::Constraints;
    use sui_scene::{RegisteredImage, SceneCommand, SceneLayerUpdateKind};
    use sui_text::{RegisteredFont, TextLayout, TextStyle};

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

    struct TestRoot {
        counters: Rc<RefCell<Counters>>,
        child: SingleChild,
    }

    impl Widget for TestRoot {
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

    struct CachedMoveLeaf {
        counters: Rc<RefCell<Counters>>,
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
                cache_policy: sui_scene::LayerCachePolicy::Cached,
                composition_mode: sui_scene::LayerCompositionMode::Scroll,
            }
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
        offset_x: f32,
    }

    impl Widget for CachedMoveRoot {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Custom(custom) = event
                && custom.kind == "shift-cached"
            {
                self.offset_x += 48.0;
                ctx.request_arrange();
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

    struct FocusTraversalRoot {
        children: WidgetChildren,
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
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    struct HoverTransitionState {
        enters: usize,
        leaves: usize,
        hovered: bool,
    }

    struct PointerCaptureLeaf {
        state: Rc<RefCell<PointerCaptureState>>,
    }

    struct HoverTransitionLeaf {
        name: &'static str,
        state: Rc<RefCell<HoverTransitionState>>,
    }

    impl Widget for PointerCaptureLeaf {
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
                    self.state.borrow_mut().moves += 1;
                    ctx.set_handled();
                }
                PointerEventKind::Up => {
                    self.state.borrow_mut().ups += 1;
                    ctx.set_handled();
                }
                _ => {}
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 40.0))
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

    struct TextImeLeaf {
        layout: RefCell<Option<TextLayout>>,
    }

    impl Widget for TextImeLeaf {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(160.0, 32.0));
            let layout = ctx
                .shape_text("compose", size, TextStyle::new(Color::WHITE))
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
            ctx.draw_text_layout(origin, layout);
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

    fn build_cached_move_runtime() -> (
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
                    .title("Cached move")
                    .root(CachedMoveRoot {
                        counters: Rc::clone(&root_counters),
                        child: SingleChild::new(CachedMoveLeaf {
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

    fn build_focus_traversal_runtime() -> (
        Runtime,
        sui_core::WindowId,
        Rc<RefCell<Counters>>,
        Rc<RefCell<Counters>>,
    ) {
        let first = Rc::new(RefCell::new(Counters::default()));
        let second = Rc::new(RefCell::new(Counters::default()));

        let runtime =
            Application::new()
                .window(WindowBuilder::new().title("Focus Traversal").root(
                    FocusTraversalRoot::new(Rc::clone(&first), Rc::clone(&second)),
                ))
                .build()
                .unwrap();

        let window_id = runtime.window_ids()[0];
        (runtime, window_id, first, second)
    }

    fn graph_child(graph: &WidgetGraphSnapshot) -> &WidgetNodeSnapshot {
        &graph.nodes[1]
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

        let runtime =
            Application::new()
                .window(WindowBuilder::new().title("Hover transitions").root(
                    HoverTransitionRoot::new(Rc::clone(&first), Rc::clone(&second)),
                ))
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
        assert!(graph_child(&graph).is_stack_surface);
        assert_eq!(graph.stack_hosts.len(), 1);
        assert_eq!(graph.stack_hosts[0].host, graph.root);
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
    fn paint_invalidation_repaints_only_dirty_widget_layer() {
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

        assert_eq!(root_counters.borrow().paint, root_paint_before);
        assert_eq!(leaf_counters.borrow().paint, leaf_paint_before + 1);
        assert_eq!(output.frame.layer_updates.len(), 1);
        assert_eq!(output.frame.layer_updates[0].owner, leaf_id);
    }

    #[test]
    fn cached_layer_translation_updates_scene_without_repaint() {
        let (mut runtime, window_id, root_counters, leaf_counters) = build_cached_move_runtime();

        let first = runtime.render(window_id).unwrap();
        let layer_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;
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
    }

    #[test]
    fn direct_layer_translation_updates_scene_without_repaint() {
        let (mut runtime, window_id, root_counters, leaf_counters) = build_direct_move_runtime();

        let first = runtime.render(window_id).unwrap();
        let layer_id = graph_child(&runtime.widget_graph(window_id).unwrap()).id;
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
            .expect("direct layer present before translation");

        runtime
            .handle_event(window_id, Event::Custom(CustomEvent::new("shift-direct")))
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
            .expect("direct layer present after translation");

        assert_eq!(translated_bounds.x(), initial_bounds.x() + 36.0);
        assert_eq!(translated_bounds.y(), initial_bounds.y());
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
        let (mut runtime, window_id, first_counters, second_counters) =
            build_focus_traversal_runtime();

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
        assert_eq!(runtime.pointer_capture_target(window_id, 7).unwrap(), None);
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
    fn initial_runtime_needs_render() {
        let (runtime, window_id, _, _) = build_runtime();

        assert!(runtime.needs_render(window_id).unwrap());
    }

    #[test]
    fn runtime_render_reports_ime_composition_rect_for_shaped_text_widgets() {
        let (mut runtime, window_id) = build_text_runtime();
        set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);

        let output = runtime.render(window_id).unwrap();

        assert!(output.ime_composition_rect.is_some());
        assert!(output.diagnostics.text_caches.runtime_layout.misses > 0);
        assert!(matches!(
            output.frame.scene.commands()[0],
            sui_scene::SceneCommand::Layer(_)
        ));
        let mut saw_shaped_text = false;
        output.frame.scene.visit_commands(&mut |command| {
            if matches!(command, sui_scene::SceneCommand::DrawShapedText(_)) {
                saw_shaped_text = true;
            }
        });
        assert!(saw_shaped_text);
    }

    #[test]
    fn removing_a_window_tears_down_runtime_state() {
        let (mut runtime, window_id, _, _) = build_runtime();

        runtime.remove_window(window_id).unwrap();

        assert!(runtime.window_ids().is_empty());
        assert!(runtime.needs_render(window_id).is_err());
        assert!(runtime.focus_state(window_id).is_err());
        assert!(runtime.render(window_id).is_err());
    }
}
