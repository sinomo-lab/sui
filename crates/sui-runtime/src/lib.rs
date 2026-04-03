#![forbid(unsafe_code)]

mod widget;
mod diagnostics;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Instant,
};

use sui_core::{
    AsyncWakeToken, DirtyRegion, Error, Event, FontHandle, ImageHandle, InvalidationKind,
    InvalidationRequest, InvalidationTarget, KeyState, Point, PointerEventKind, Rect, Result,
    SemanticsNode, Size, TimerToken, WakeEvent, WidgetId, WindowEvent, WindowId,
};
use sui_layout::Constraints;
use sui_scene::{ImageRegistry, RegisteredImage, SceneFrame};
use sui_text::{FontRegistry, RegisteredFont, TextSystem};

pub use sui_core::DpiInfo;
pub use diagnostics::{
    FramePhase, FramePhaseSample, RenderDiagnostics, SceneStatistics,
    WindowPerformanceSnapshot, clear_window_performance_snapshot,
    clear_window_performance_snapshots, publish_window_performance_snapshot,
    window_performance_snapshot,
};
pub use widget::{
    EventCtx, EventPhase, LayoutCtx, PaintCtx, SemanticsCtx, SingleChild, Widget, WidgetChildren,
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
    pub layout: bool,
    pub paint: bool,
    pub semantics: bool,
    pub hit_test: bool,
    pub text: bool,
    pub resources: bool,
}

impl FrameSchedule {
    fn bootstrap() -> Self {
        Self {
            layout: true,
            paint: true,
            semantics: true,
            hit_test: true,
            text: false,
            resources: false,
        }
    }

    pub const fn any(self) -> bool {
        self.layout || self.paint || self.semantics || self.hit_test || self.text || self.resources
    }

    pub const fn needs_render(self) -> bool {
        self.layout || self.paint || self.semantics || self.text || self.resources
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn mark(&mut self, kind: InvalidationKind) {
        match kind {
            InvalidationKind::Layout => {
                self.layout = true;
                self.paint = true;
                self.hit_test = true;
                self.semantics = true;
            }
            InvalidationKind::Paint => {
                self.paint = true;
            }
            InvalidationKind::HitTest => {
                self.hit_test = true;
            }
            InvalidationKind::Text => {
                self.text = true;
                self.layout = true;
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
pub struct WidgetNodeSnapshot {
    pub id: WidgetId,
    pub parent: Option<WidgetId>,
    pub children: Vec<WidgetId>,
    pub bounds: Rect,
    pub accepts_focus: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetGraphSnapshot {
    pub root: WidgetId,
    pub nodes: Vec<WidgetNodeSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScheduledTimer {
    token: TimerToken,
    deadline: f64,
    target: WidgetId,
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
            Event::Pointer(pointer) => self.graph.hit_test(pointer.position),
            _ => None,
        };

        let target = self.resolve_event_target(&event, hit_target);

        let route = self.route_event(target, &event);
        let mut invalidations = route.effects.invalidations;

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
                self.schedule.mark(InvalidationKind::Layout);
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
                self.schedule.mark(InvalidationKind::Layout);
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
        if self.schedule.layout || self.viewport.is_none() {
            let _ = self.run_layout_pass(text_system, font_registry, image_registry);
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
                .with_region(node.bounds),
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
        let mut diagnostics = RenderDiagnostics::default();
        let mut invalidations = std::mem::take(&mut self.pending_invalidations);
        let mut repainted = false;

        if self.last_frame.is_none() {
            self.schedule = FrameSchedule::bootstrap();
        }

        if self.schedule.layout || self.viewport.is_none() {
            let started = Instant::now();
            invalidations.extend(self.run_layout_pass(
                text_system,
                Arc::clone(&font_registry),
                Arc::clone(&image_registry),
            ));
            diagnostics.push(FramePhase::Layout, started.elapsed());
        } else if self.schedule.hit_test || self.graph.is_empty() {
            let started = Instant::now();
            self.refresh_graph();
            diagnostics.push(FramePhase::HitTest, started.elapsed());
        }

        let viewport = self.viewport.unwrap_or(Size::ZERO);
        let dpi_info = self.dpi_info_for_viewport(viewport);

        if self.schedule.paint || self.last_frame.is_none() {
            let started = Instant::now();
            repainted = true;

            let mut paint_ctx = PaintCtx::new(
                self.id,
                self.root.id(),
                self.root.bounds(),
                self.focus.focused_widget,
                dpi_info,
            );
            self.root.paint(&mut paint_ctx);
            let (scene, paint_invalidations, ime_composition_rect) = paint_ctx.into_parts();
            invalidations.extend(paint_invalidations);
            self.ime_composition_rect = ime_composition_rect;
            self.last_frame = Some(SceneFrame {
                window_id: self.id,
                viewport,
                surface_size: dpi_info.surface_size,
                scale_factor: dpi_info.scale_factor,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::clone(&font_registry),
                image_registry: Arc::clone(&image_registry),
            });
            diagnostics.push(FramePhase::Paint, started.elapsed());
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
            diagnostics.push(FramePhase::Semantics, started.elapsed());
        }

        let dirty_regions = collect_dirty_regions(viewport, &invalidations, repainted);
        let mut frame = self
            .last_frame
            .clone()
            .unwrap_or_else(|| SceneFrame::new(self.id, viewport));
        frame.viewport = viewport;
        frame.surface_size = dpi_info.surface_size;
        frame.scale_factor = dpi_info.scale_factor;
        frame.dirty_regions = dirty_regions;
        frame.font_registry = font_registry;
        frame.image_registry = image_registry;

        self.schedule.clear();

        RenderOutput {
            title: self.title.clone(),
            frame,
            semantics: self.last_semantics.clone(),
            ime_composition_rect: self.ime_composition_rect,
            diagnostics,
        }
    }

    fn run_layout_pass(
        &mut self,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> Vec<InvalidationRequest> {
        let mut layout_ctx = LayoutCtx::new(
            self.id,
            self.root.id(),
            self.current_dpi_info(),
            text_system,
            font_registry,
            image_registry,
        );
        let viewport = self.root.layout(&mut layout_ctx, self.layout_constraints());
        self.root
            .set_bounds(Rect::from_origin_size(Point::ZERO, viewport));
        self.viewport = Some(viewport);
        self.schedule.layout = false;
        self.schedule.hit_test = false;
        self.refresh_graph();
        layout_ctx.take_invalidations()
    }

    fn refresh_graph(&mut self) {
        self.graph = WidgetGraph::rebuild(&self.root, self.focus.focused_widget);
        self.prune_runtime_state();
        self.schedule.hit_test = false;
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

    fn layout_constraints(&self) -> Constraints {
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
}

impl WidgetGraph {
    fn empty(root: WidgetId) -> Self {
        Self {
            root,
            nodes: HashMap::new(),
            order: Vec::new(),
        }
    }

    fn rebuild(root: &WidgetPod, focused_widget: Option<WidgetId>) -> Self {
        let mut graph = Self::empty(root.id());
        graph.collect(root, None, focused_widget);
        graph
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
        WidgetGraphSnapshot {
            root: self.root,
            nodes: self
                .order
                .iter()
                .filter_map(|widget_id| self.nodes.get(widget_id).cloned())
                .collect(),
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
    ) {
        let id = pod.id();
        self.order.push(id);

        let children = {
            let mut visitor = CollectChildrenVisitor {
                graph: self,
                parent: id,
                focused_widget,
                children: Vec::new(),
            };
            pod.visit_children(&mut visitor);
            visitor.children
        };

        self.nodes.insert(
            id,
            WidgetNodeSnapshot {
                id,
                parent,
                children,
                bounds: pod.bounds(),
                accepts_focus: pod.accepts_focus(),
                focused: Some(id) == focused_widget,
            },
        );
    }

    fn hit_test_node(&self, widget_id: WidgetId, point: Point) -> Option<WidgetId> {
        let node = self.node(widget_id)?;
        if !node.bounds.contains(point) {
            return None;
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
    children: Vec<WidgetId>,
}

impl WidgetPodVisitor for CollectChildrenVisitor<'_> {
    fn visit(&mut self, child: &WidgetPod) {
        self.children.push(child.id());
        self.graph
            .collect(child, Some(self.parent), self.focused_widget);
    }
}

struct EventRouteResult {
    path: Vec<WidgetId>,
    effects: EventEffects,
    focus_request: Option<FocusRequest>,
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

fn collect_dirty_regions(
    viewport: Size,
    invalidations: &[InvalidationRequest],
    repainted: bool,
) -> Vec<DirtyRegion> {
    let viewport_rect = Rect::from_origin_size(Point::ZERO, viewport);

    if invalidations.is_empty() {
        return if repainted {
            vec![DirtyRegion::new(viewport_rect, InvalidationKind::Paint)]
        } else {
            Vec::new()
        };
    }

    let mut dirty_regions: Vec<_> = invalidations
        .iter()
        .map(|request| DirtyRegion::new(request.region.unwrap_or(viewport_rect), request.kind))
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
        Application, EventCtx, FocusState, FrameSchedule, LayoutCtx, PaintCtx, Runtime,
        SemanticsCtx, SingleChild, Widget, WidgetChildren, WidgetGraphSnapshot, WidgetNodeSnapshot,
        WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
    };
    use sui_core::{
        AsyncWakeToken, Color, CustomEvent, Event, FontHandle, ImageHandle, KeyState,
        KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
        SemanticsNode, SemanticsRole, Size, TimerToken, WakeEvent, WindowEvent,
    };
    use sui_layout::Constraints;
    use sui_scene::RegisteredImage;
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

        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
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

        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(320.0, 180.0));
            self.child.layout_at(
                ctx,
                Constraints::tight(Size::new(120.0, 40.0)),
                Point::new(32.0, 24.0),
            );
            size
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
        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(320.0, 180.0));
            let children = self.children.as_mut_slice();
            children[0].layout_at(
                ctx,
                Constraints::tight(Size::new(120.0, 40.0)),
                Point::new(32.0, 24.0),
            );
            children[1].layout_at(
                ctx,
                Constraints::tight(Size::new(120.0, 40.0)),
                Point::new(32.0, 80.0),
            );
            size
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

    struct PointerCaptureLeaf {
        state: Rc<RefCell<PointerCaptureState>>,
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

        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 40.0))
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

        fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 40.0))
        }
    }

    struct TextImeLeaf {
        layout: RefCell<Option<TextLayout>>,
    }

    impl Widget for TextImeLeaf {
        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
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
                .expect("layout pass should shape text first");
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
        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(320.0, 180.0));
            self.child.layout_at(
                ctx,
                Constraints::tight(Size::new(120.0, 40.0)),
                Point::new(32.0, 24.0),
            );
            size
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
        fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
            let size = constraints.clamp(Size::new(320.0, 180.0));
            self.child.layout_at(
                ctx,
                Constraints::tight(Size::new(160.0, 32.0)),
                Point::new(32.0, 24.0),
            );
            size
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
        assert_eq!(graph_child(&graph).parent, Some(graph.root));
        assert!(graph_child(&graph).accepts_focus);
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

        let output = runtime.render(window_id).unwrap();

        assert!(output.ime_composition_rect.is_some());
        assert!(matches!(
            output.frame.scene.commands()[0],
            sui_scene::SceneCommand::DrawShapedText(_)
        ));
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
