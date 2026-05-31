use std::{collections::HashMap, sync::Arc, time::Duration};

use sui_core::{
    Error, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
    PointerButtons, PointerEvent, PointerEventKind, PointerKind, Result, ScrollDelta,
    SemanticsRole, Size, Vector, WindowEvent, WindowId,
};
use sui_render_wgpu::{FeatheringOptions, WgpuRenderer};
use sui_runtime::{
    PresentationLatencyDiagnostics, Runtime, WindowPerformanceSnapshot, WindowRenderOptions,
    window_performance_snapshot, window_render_options, window_scene_statistics_detail_mode,
};
use web_time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    error::{EventLoopError, OsError},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent as WinitWindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey, PhysicalKey},
    window::{Window, WindowAttributes, WindowId as HostWindowId},
};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::{
    AccessibilityBridge, WindowOutputDiagnostics, detect_window_display_capabilities,
    headless::PlatformWindow, map_window_color_management, map_window_stem_darkening,
    map_window_text_hinting, publish_window_output_diagnostics,
    resolve_sdr_content_brightness_nits,
};

#[derive(Debug, Default)]
pub struct DesktopPlatform {
    renderer: WgpuRenderer,
    automation: Option<DesktopAutomationConfig>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DesktopAutomationAction {
    ScrollPixels { delta: Vector },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopAutomationConfig {
    pub label: String,
    pub target_role: SemanticsRole,
    pub target_name: String,
    pub action: DesktopAutomationAction,
    pub step_interval: Duration,
    pub duration: Duration,
    pub report_interval: Duration,
    pub startup_timeout: Duration,
}

#[derive(Debug, Clone)]
struct DesktopAutomationState {
    config: DesktopAutomationConfig,
    armed_at: Instant,
    started_at: Option<Instant>,
    next_step_at: Instant,
    last_report_at: Option<Instant>,
    last_report_frame_index: u64,
    target_window_id: Option<WindowId>,
    pointer_primed: bool,
    shutdown_requested: bool,
}

impl DesktopAutomationState {
    fn new(config: DesktopAutomationConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            armed_at: now,
            started_at: None,
            next_step_at: now,
            last_report_at: None,
            last_report_frame_index: 0,
            target_window_id: None,
            pointer_primed: false,
            shutdown_requested: false,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        if self.shutdown_requested {
            return None;
        }

        let mut next = self.next_step_at;
        if let Some(started_at) = self.started_at {
            next = next.min(started_at + self.config.duration);
            if let Some(last_report_at) = self.last_report_at {
                next = next.min(last_report_at + self.config.report_interval);
            }
        } else {
            next = next.min(self.armed_at + self.config.startup_timeout);
        }

        Some(next)
    }
}

impl DesktopPlatform {
    const DEFAULT_WINDOW_SIZE: Size = Size::new(1280.0, 720.0);

    pub fn new() -> Self {
        crate::reset_window_performance_store();
        Self::default()
    }

    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.set_feather_width(feather_width);
        self
    }

    pub fn with_feathering_enabled(mut self, enabled: bool) -> Self {
        self.set_feathering_enabled(enabled);
        self
    }

    pub fn with_vsync_enabled(mut self, enabled: bool) -> Self {
        self.set_vsync_enabled(enabled);
        self
    }

    pub fn with_automation(mut self, automation: DesktopAutomationConfig) -> Self {
        self.automation = Some(automation);
        self
    }

    pub fn renderer(&self) -> &WgpuRenderer {
        &self.renderer
    }

    pub fn feather_width(&self) -> f32 {
        self.renderer.feather_width()
    }

    pub fn feathering_enabled(&self) -> bool {
        self.renderer.feathering_enabled()
    }

    pub fn vsync_enabled(&self) -> bool {
        self.renderer.vsync_enabled()
    }

    pub fn set_feather_width(&mut self, feather_width: f32) {
        self.renderer.set_feather_width(feather_width);
    }

    pub fn set_feathering_enabled(&mut self, enabled: bool) {
        self.renderer.set_feathering_enabled(enabled);
    }

    pub fn set_vsync_enabled(&mut self, enabled: bool) {
        self.renderer.set_vsync_enabled(enabled);
    }

    pub fn renderer_mut(&mut self) -> &mut WgpuRenderer {
        &mut self.renderer
    }

    pub fn run(self, runtime: Runtime) -> Result<Vec<PlatformWindow>> {
        let event_loop = EventLoop::new().map_err(map_event_loop_error)?;

        #[cfg(target_arch = "wasm32")]
        {
            let mut app = DesktopApp::new(runtime, self.renderer, self.automation);
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = app.renderer.initialize_async(None).await {
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&error.to_string()));
                    return;
                }
                event_loop.spawn_app(app);
            });
            Ok(Vec::new())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut app = DesktopApp::new(runtime, self.renderer, self.automation);
            event_loop.run_app(&mut app).map_err(map_event_loop_error)?;

            if let Some(error) = app.last_error.take() {
                return Err(error);
            }

            Ok(app.snapshot_windows())
        }
    }
}

struct DesktopApp {
    runtime: Runtime,
    renderer: WgpuRenderer,
    automation: Option<DesktopAutomationState>,
    started_at: Instant,
    frame_clock: f64,
    windows: HashMap<WindowId, WindowState>,
    host_to_runtime: HashMap<HostWindowId, WindowId>,
    last_error: Option<Error>,
}

impl DesktopApp {
    #[cfg(target_arch = "wasm32")]
    fn web_canvas_for_window() -> Option<web_sys::HtmlCanvasElement> {
        let window = web_sys::window()?;
        let canvas = window
            .document()
            .and_then(|document| document.get_element_by_id("sui-main-canvas"))
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())?;
        let scale_factor = window.device_pixel_ratio().max(1.0);
        let width = window
            .inner_width()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(DesktopPlatform::DEFAULT_WINDOW_SIZE.width as f64);
        let height = window
            .inner_height()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(DesktopPlatform::DEFAULT_WINDOW_SIZE.height as f64);
        canvas.set_width(((width * scale_factor).round().max(1.0)) as u32);
        canvas.set_height(((height * scale_factor).round().max(1.0)) as u32);
        Some(canvas)
    }

    #[cfg(target_arch = "wasm32")]
    fn initial_web_physical_size(window: &Window) -> PhysicalSize<u32> {
        window
            .canvas()
            .map(|canvas| PhysicalSize::new(canvas.width().max(1), canvas.height().max(1)))
            .unwrap_or_else(|| window.inner_size())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn initial_window_physical_size(window: &Window) -> PhysicalSize<u32> {
        window.inner_size()
    }

    #[cfg(target_arch = "wasm32")]
    fn initial_window_physical_size(window: &Window) -> PhysicalSize<u32> {
        Self::initial_web_physical_size(window)
    }

    fn new(
        runtime: Runtime,
        renderer: WgpuRenderer,
        automation: Option<DesktopAutomationConfig>,
    ) -> Self {
        Self {
            runtime,
            renderer,
            automation: automation.map(DesktopAutomationState::new),
            started_at: Instant::now(),
            frame_clock: 0.0,
            windows: HashMap::new(),
            host_to_runtime: HashMap::new(),
            last_error: None,
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn snapshot_windows(&self) -> Vec<PlatformWindow> {
        self.windows.values().map(WindowState::snapshot).collect()
    }

    fn refresh_window_display_capabilities(&mut self, window_id: WindowId) -> Result<()> {
        let capabilities = self
            .windows
            .get(&window_id)
            .map(|window| detect_window_display_capabilities(window.window.as_ref()))
            .ok_or_else(|| Error::new(format!("missing window {}", window_id.get())))?;
        self.renderer
            .set_window_display_capabilities(window_id, capabilities)?;
        Ok(())
    }

    fn sync_windows(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let runtime_window_ids = self.runtime.window_ids();

        let removed_ids: Vec<_> = self
            .windows
            .keys()
            .copied()
            .filter(|window_id| !runtime_window_ids.contains(window_id))
            .collect();

        for window_id in removed_ids {
            if let Some(window) = self.windows.remove(&window_id) {
                self.renderer.remove_window(window_id);
                self.host_to_runtime.remove(&window.window.id());
                crate::clear_window_performance(window_id);
            }
        }

        for window_id in runtime_window_ids {
            if self.windows.contains_key(&window_id) {
                continue;
            }

            let title = self.runtime.window_title(window_id)?.to_string();
            #[allow(unused_mut)]
            let mut attributes = WindowAttributes::default()
                .with_title(title.clone())
                .with_inner_size(LogicalSize::new(
                    DesktopPlatform::DEFAULT_WINDOW_SIZE.width,
                    DesktopPlatform::DEFAULT_WINDOW_SIZE.height,
                ));
            #[cfg(target_arch = "wasm32")]
            {
                attributes = attributes
                    .with_canvas(Self::web_canvas_for_window())
                    .with_append(false);
            }
            let window = Arc::new(event_loop.create_window(attributes).map_err(map_os_error)?);
            window.set_ime_allowed(false);

            let host_id = window.id();
            let scale_factor = window.scale_factor();
            let size = physical_size_to_logical_size(
                Self::initial_window_physical_size(&window),
                scale_factor,
            );
            self.renderer
                .register_window(window_id, Arc::clone(&window))?;

            self.host_to_runtime.insert(host_id, window_id);
            self.windows.insert(
                window_id,
                WindowState {
                    id: window_id,
                    title,
                    display_capabilities_dirty: false,
                    awaiting_performance_bootstrap: true,
                    redraw_requested: false,
                    redraw_requested_at_ms: None,
                    frame_index: 0,
                    pending_event_time_ms: 0.0,
                    last_non_redraw_event_at_ms: None,
                    accessibility: AccessibilityBridge::default(),
                    pointer: PointerState::default(),
                    scale_factor,
                    window,
                },
            );
            self.refresh_window_display_capabilities(window_id)?;

            self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    raw_dpi: None,
                    suggested_size: Some(size),
                }),
            )?;

            self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::Resized(size)),
            )?;
        }

        if self.windows.is_empty() {
            event_loop.exit();
        }

        Ok(())
    }

    fn drive_runtime(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        self.sync_windows(event_loop)?;
        if self.windows.is_empty() {
            return Ok(());
        }

        self.update_clock();
        self.runtime.tick(self.frame_clock);

        loop {
            let ready_events = self.runtime.drain_ready_events();
            if ready_events.is_empty() {
                break;
            }

            for (window_id, event) in ready_events {
                if self.windows.contains_key(&window_id) {
                    self.process_event(event_loop, window_id, event)?;
                }
            }
        }

        self.sync_windows(event_loop)?;

        let window_ids: Vec<_> = self.windows.keys().copied().collect();
        for window_id in window_ids.iter().copied() {
            self.request_redraw_if_needed(window_id)?;
        }

        self.drive_automation(event_loop)?;
        if self
            .automation
            .as_ref()
            .is_some_and(|automation| automation.shutdown_requested)
        {
            let window_ids = self.runtime.window_ids();
            for window_id in window_ids {
                self.runtime.remove_window(window_id)?;
                crate::clear_window_performance(window_id);
            }
            self.sync_windows(event_loop)?;
            return Ok(());
        }

        self.update_control_flow(event_loop)?;
        Ok(())
    }

    fn request_redraw_if_needed(&mut self, window_id: WindowId) -> Result<()> {
        if !self.runtime.needs_render(window_id)? {
            return Ok(());
        }

        let now_ms = self.current_time_ms();

        let Some(window) = self.windows.get_mut(&window_id) else {
            return Ok(());
        };

        if window.redraw_requested {
            return Ok(());
        }

        window.redraw_requested = true;
        window.redraw_requested_at_ms = Some(now_ms);
        window.window.request_redraw();
        Ok(())
    }

    fn update_clock(&mut self) {
        self.frame_clock = self.started_at.elapsed().as_secs_f64();
    }

    fn current_time_ms(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }

    fn update_control_flow(&self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.windows.is_empty() {
            event_loop.exit();
            return Ok(());
        }

        let mut next_deadline: Option<f64> = None;

        for window_id in self.runtime.window_ids() {
            let deadline = self.runtime.next_wakeup_time(window_id)?;
            next_deadline = match (next_deadline, deadline) {
                (Some(current), Some(candidate)) => Some(current.min(candidate)),
                (None, Some(candidate)) => Some(candidate),
                (current, None) => current,
            };
        }

        if let Some(automation_deadline) = self
            .automation
            .as_ref()
            .and_then(DesktopAutomationState::next_deadline)
        {
            let candidate = (automation_deadline - self.started_at).as_secs_f64();
            next_deadline = match next_deadline {
                Some(current) => Some(current.min(candidate)),
                None => Some(candidate),
            };
        }

        match next_deadline {
            Some(deadline) if deadline <= self.frame_clock => {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            Some(deadline) => {
                let when = self.started_at + Duration::from_secs_f64(deadline.max(0.0));
                event_loop.set_control_flow(ControlFlow::WaitUntil(when));
            }
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }

        Ok(())
    }

    fn process_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: Event,
    ) -> Result<()> {
        if !self.windows.contains_key(&window_id) {
            return Ok(());
        }

        let event_arrived_at_ms = self.current_time_ms();

        let is_redraw = matches!(event, Event::Window(WindowEvent::RedrawRequested));
        let is_close = matches!(event, Event::Window(WindowEvent::CloseRequested));
        let render_immediately = event_renders_immediately(&event);

        let event_started = Instant::now();
        self.runtime.handle_event(window_id, event)?;
        let event_time_ms = event_started.elapsed().as_secs_f64() * 1000.0;

        if let Some(window) = self.windows.get_mut(&window_id) {
            if !is_redraw {
                window.pending_event_time_ms += event_time_ms;
            }
            if !is_redraw && !is_close {
                window.last_non_redraw_event_at_ms = Some(event_arrived_at_ms);
            }
        }

        if !is_redraw && !is_close {
            self.request_redraw_if_needed(window_id)?;
        }

        if is_redraw {
            if let Some(window) = self.windows.get_mut(&window_id) {
                window.redraw_requested = false;
            }

            self.render_window_if_needed(window_id, event_time_ms)?;
        } else if render_immediately && !is_close {
            self.render_window_if_needed(window_id, event_time_ms)?;
        }

        if is_close {
            self.runtime.remove_window(window_id)?;
            crate::clear_window_performance(window_id);
            self.sync_windows(event_loop)?;
        }

        if self.windows.is_empty() {
            event_loop.exit();
        }

        Ok(())
    }

    fn render_window_if_needed(&mut self, window_id: WindowId, event_time_ms: f64) -> Result<()> {
        if !self.runtime.needs_render(window_id)? {
            return Ok(());
        }

        self.update_clock();
        self.runtime.tick(self.frame_clock);

        let render_started_at_ms = self.current_time_ms();
        let mut presentation_latency = PresentationLatencyDiagnostics::default();
        if let Some(window) = self.windows.get(&window_id) {
            presentation_latency = PresentationLatencyDiagnostics::new(
                window
                    .last_non_redraw_event_at_ms
                    .map(|timestamp| (render_started_at_ms - timestamp).max(0.0))
                    .unwrap_or(0.0),
                0.0,
                window
                    .redraw_requested_at_ms
                    .map(|timestamp| (render_started_at_ms - timestamp).max(0.0))
                    .unwrap_or(0.0),
            );
        }

        let runtime_started = Instant::now();
        let output = self.runtime.render(window_id)?;
        let runtime_time_ms = runtime_started.elapsed().as_secs_f64() * 1000.0;
        let semantics = output.semantics.clone();
        let renderer_started = Instant::now();
        let diagnostics_enabled = window_scene_statistics_detail_mode(window_id).is_detailed();
        self.renderer
            .set_runtime_diagnostics_enabled(diagnostics_enabled);
        let render_options = window_render_options(window_id);
        if self
            .windows
            .get(&window_id)
            .is_some_and(|window| window.display_capabilities_dirty)
        {
            self.refresh_window_display_capabilities(window_id)?;
            if let Some(window) = self.windows.get_mut(&window_id) {
                window.display_capabilities_dirty = false;
            }
        }
        self.renderer
            .set_runtime_feathering_override(render_options.map(|options| {
                FeatheringOptions::new(options.feathering_enabled, options.feather_width)
            }));
        self.renderer.set_runtime_text_hinting_override(
            render_options.map(|options| map_window_text_hinting(options.text_hinting)),
        );
        self.renderer.set_runtime_stem_darkening_override(
            render_options.map(|options| map_window_stem_darkening(options.stem_darkening)),
        );
        let active_render_options =
            render_options.unwrap_or_else(|| WindowRenderOptions::new(true, 1.0));
        let display_capabilities_for_brightness = self
            .renderer
            .window_display_capabilities(window_id)
            .unwrap_or_default();
        let sdr_content_brightness_nits = resolve_sdr_content_brightness_nits(
            active_render_options.sdr_content_brightness_nits,
            active_render_options.use_system_sdr_content_brightness,
            &display_capabilities_for_brightness,
        );
        self.renderer.set_window_color_management(
            window_id,
            map_window_color_management(
                active_render_options.color_management_mode,
                active_render_options.output_color_primaries,
                active_render_options.dynamic_range_mode,
                active_render_options.tone_mapping_mode,
                sdr_content_brightness_nits,
            ),
        )?;
        self.renderer.render(&output.frame)?;
        if let (Some(display_capabilities), Some(active_output_strategy)) = (
            self.renderer.window_display_capabilities(window_id),
            self.renderer.window_output_strategy(window_id),
        ) {
            let system_sdr_content_brightness_nits = display_capabilities.sdr_white_nits;
            publish_window_output_diagnostics(
                window_id,
                WindowOutputDiagnostics {
                    display_capabilities,
                    requested_color_management_mode: active_render_options.color_management_mode,
                    requested_output_primaries: active_render_options.output_color_primaries,
                    requested_dynamic_range_mode: active_render_options.dynamic_range_mode,
                    requested_tone_mapping_mode: active_render_options.tone_mapping_mode,
                    requested_sdr_content_brightness_nits: sdr_content_brightness_nits,
                    configured_sdr_content_brightness_nits: active_render_options
                        .sdr_content_brightness_nits,
                    system_sdr_content_brightness_nits,
                    use_system_sdr_content_brightness: active_render_options
                        .use_system_sdr_content_brightness,
                    active_output_strategy,
                },
            );
        }
        let renderer_time_ms = renderer_started.elapsed().as_secs_f64() * 1000.0;
        let presented_at_ms = self.current_time_ms();
        if let Some(window) = self.windows.get(&window_id) {
            presentation_latency.event_to_present_ms = window
                .last_non_redraw_event_at_ms
                .map(|timestamp| (presented_at_ms - timestamp).max(0.0))
                .unwrap_or(0.0);
        }

        let mut frame_index = 0;
        let mut pending_event_time_ms = 0.0;

        if let Some(window) = self.windows.get_mut(&window_id) {
            frame_index = window.frame_index + 1;
            pending_event_time_ms = std::mem::take(&mut window.pending_event_time_ms);
            window.frame_index = frame_index;

            if window.title != output.title {
                window.title = output.title.clone();
                window.window.set_title(&output.title);
            }

            window.accessibility.update(window_id, semantics);
            window.last_non_redraw_event_at_ms = None;
            window.redraw_requested_at_ms = None;

            apply_ime_composition_rect(window.window.as_ref(), output.ime_composition_rect);
        }

        crate::publish_frame_performance(
            window_id,
            frame_index,
            pending_event_time_ms,
            event_time_ms,
            runtime_time_ms,
            presentation_latency,
            &output,
            &self.renderer,
            renderer_time_ms,
        );

        let bootstrap_redraw_at_ms = self.current_time_ms();
        if let Some(window) = self.windows.get_mut(&window_id) {
            if window.awaiting_performance_bootstrap {
                window.awaiting_performance_bootstrap = false;
                if !window.redraw_requested {
                    window.redraw_requested = true;
                    window.redraw_requested_at_ms = Some(bootstrap_redraw_at_ms);
                    window.window.request_redraw();
                }
            }
        }

        Ok(())
    }

    fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        host_id: HostWindowId,
        event: WinitWindowEvent,
    ) -> Result<()> {
        let Some(window_id) = self.host_to_runtime.get(&host_id).copied() else {
            return Ok(());
        };

        match event {
            WinitWindowEvent::CloseRequested => self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::CloseRequested),
            ),
            WinitWindowEvent::Resized(size) => self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::Resized(
                    self.windows
                        .get(&window_id)
                        .map(|window| physical_size_to_logical_size(size, window.scale_factor))
                        .unwrap_or_else(|| physical_size_to_logical_size(size, 1.0)),
                )),
            ),
            WinitWindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let suggested_size = self.windows.get_mut(&window_id).map(|window| {
                    window.scale_factor = scale_factor;
                    window.display_capabilities_dirty = true;
                    physical_size_to_logical_size(window.window.inner_size(), scale_factor)
                });
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Window(WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        raw_dpi: None,
                        suggested_size,
                    }),
                )
            }
            WinitWindowEvent::Focused(focused) => self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::Focused(focused)),
            ),
            WinitWindowEvent::Occluded(occluded) => self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::Occluded(occluded)),
            ),
            WinitWindowEvent::RedrawRequested => self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::RedrawRequested),
            ),
            WinitWindowEvent::ModifiersChanged(modifiers) => {
                if let Some(window) = self.windows.get_mut(&window_id) {
                    window.pointer.modifiers = modifiers_state_to_modifiers(modifiers.state());
                }
                Ok(())
            }
            WinitWindowEvent::CursorMoved { position, .. } => {
                let event = if let Some(window) = self.windows.get_mut(&window_id) {
                    let next_position =
                        physical_position_to_logical_point(position, window.scale_factor);
                    let delta = Vector::new(
                        next_position.x - window.pointer.position.x,
                        next_position.y - window.pointer.position.y,
                    );
                    window.pointer.position = next_position;
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Move,
                        position: next_position,
                        delta,
                        scroll_delta: None,
                        button: None,
                        buttons: window.pointer.buttons,
                        modifiers: window.pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    })
                } else {
                    return Ok(());
                };

                self.process_event(event_loop, window_id, event)
            }
            WinitWindowEvent::CursorEntered { .. } => {
                let event = if let Some(window) = self.windows.get(&window_id) {
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Enter,
                        position: window.pointer.position,
                        delta: Vector::ZERO,
                        scroll_delta: None,
                        button: None,
                        buttons: window.pointer.buttons,
                        modifiers: window.pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    })
                } else {
                    return Ok(());
                };

                self.process_event(event_loop, window_id, event)
            }
            WinitWindowEvent::CursorLeft { .. } => {
                let event = if let Some(window) = self.windows.get(&window_id) {
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Leave,
                        position: window.pointer.position,
                        delta: Vector::ZERO,
                        scroll_delta: None,
                        button: None,
                        buttons: window.pointer.buttons,
                        modifiers: window.pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    })
                } else {
                    return Ok(());
                };

                self.process_event(event_loop, window_id, event)
            }
            WinitWindowEvent::MouseInput { state, button, .. } => {
                let event = if let Some(window) = self.windows.get_mut(&window_id) {
                    let Some(pointer_button) = map_mouse_button(button) else {
                        return Ok(());
                    };

                    match state {
                        ElementState::Pressed => window.pointer.buttons.insert(pointer_button),
                        ElementState::Released => {
                            window.pointer.buttons =
                                remove_pointer_button(window.pointer.buttons, pointer_button);
                        }
                    }

                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: match state {
                            ElementState::Pressed => PointerEventKind::Down,
                            ElementState::Released => PointerEventKind::Up,
                        },
                        position: window.pointer.position,
                        delta: Vector::ZERO,
                        scroll_delta: None,
                        button: Some(pointer_button),
                        buttons: window.pointer.buttons,
                        modifiers: window.pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    })
                } else {
                    return Ok(());
                };

                self.process_event(event_loop, window_id, event)
            }
            WinitWindowEvent::MouseWheel { delta, .. } => {
                let event = if let Some(window) = self.windows.get(&window_id) {
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Scroll,
                        position: window.pointer.position,
                        delta: Vector::ZERO,
                        scroll_delta: Some(match delta {
                            MouseScrollDelta::LineDelta(x, y) => {
                                ScrollDelta::Lines(Vector::new(x, y))
                            }
                            MouseScrollDelta::PixelDelta(position) => ScrollDelta::Pixels(
                                physical_position_to_logical_vector(position, window.scale_factor),
                            ),
                        }),
                        button: None,
                        buttons: window.pointer.buttons,
                        modifiers: window.pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    })
                } else {
                    return Ok(());
                };

                self.process_event(event_loop, window_id, event)
            }
            WinitWindowEvent::KeyboardInput { event, .. } => {
                let modifiers = self
                    .windows
                    .get(&window_id)
                    .map(|window| window.pointer.modifiers)
                    .unwrap_or(Modifiers::NONE);
                let keyboard_event = KeyboardEvent {
                    key: logical_key_to_string(&event.logical_key),
                    code: physical_key_to_string(&event.physical_key),
                    text: event.text.as_ref().map(|text| text.to_string()),
                    state: match event.state {
                        ElementState::Pressed => KeyState::Pressed,
                        ElementState::Released => KeyState::Released,
                    },
                    modifiers,
                    repeat: event.repeat,
                    is_composing: false,
                };
                self.process_event(event_loop, window_id, Event::Keyboard(keyboard_event))
            }
            WinitWindowEvent::Ime(ime) => {
                if let Some(ime_event) = map_ime_event(ime) {
                    self.process_event(event_loop, window_id, Event::Ime(ime_event))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    fn handle_error(&mut self, event_loop: &ActiveEventLoop, error: Error) {
        self.last_error = Some(error);
        event_loop.exit();
    }

    fn drive_automation(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let now = Instant::now();
        let mut state = match self.automation.take() {
            Some(state) => state,
            None => return Ok(()),
        };

        if state.shutdown_requested {
            self.automation = Some(state);
            return Ok(());
        }

        if state.started_at.is_none() && now >= state.armed_at + state.config.startup_timeout {
            println!(
                "[desktop automation:{}] timed out waiting for target {:?} named {:?}",
                state.config.label, state.config.target_role, state.config.target_name
            );
            state.shutdown_requested = true;
            self.automation = Some(state);
            return Ok(());
        }

        if let Some((window_id, target_point)) = self.find_automation_target(&state) {
            if state.started_at.is_none() {
                state.started_at = Some(now);
                state.last_report_at = Some(now);
                state.last_report_frame_index = window_performance_snapshot(window_id)
                    .map(|snapshot| snapshot.frame_index)
                    .unwrap_or(0);
                state.target_window_id = Some(window_id);
                println!(
                    "[desktop automation:{}] started on window {}",
                    state.config.label,
                    window_id.get()
                );
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Window(WindowEvent::Focused(true)),
                )?;
            }

            if !state.pointer_primed {
                self.inject_pointer_move(event_loop, window_id, target_point, true)?;
                state.pointer_primed = true;
            }

            self.report_automation_progress(&mut state, false);

            let started_at = state.started_at.expect("automation should have started");
            if now >= started_at + state.config.duration {
                self.report_automation_progress(&mut state, true);
                println!("[desktop automation:{}] completed", state.config.label);
                state.shutdown_requested = true;
                self.automation = Some(state);
                return Ok(());
            }

            if now >= state.next_step_at {
                self.inject_automation_action(
                    event_loop,
                    window_id,
                    target_point,
                    &state.config.action,
                )?;
                state.next_step_at = now + state.config.step_interval;
            }
        }

        self.automation = Some(state);
        Ok(())
    }

    fn find_automation_target(&self, state: &DesktopAutomationState) -> Option<(WindowId, Point)> {
        self.windows.iter().find_map(|(window_id, window)| {
            let snapshot = window.accessibility.snapshot()?;
            let node = snapshot.nodes.iter().find(|node| {
                node.role == state.config.target_role
                    && node.name.as_deref() == Some(state.config.target_name.as_str())
            })?;
            let point = if node.role == SemanticsRole::ScrollView {
                Point::new(
                    node.bounds.x() + node.bounds.width().min(48.0),
                    node.bounds.y() + node.bounds.height() * 0.5,
                )
            } else {
                Point::new(
                    node.bounds.x() + node.bounds.width() * 0.5,
                    node.bounds.y() + node.bounds.height() * 0.5,
                )
            };
            Some((*window_id, point))
        })
    }

    fn report_automation_progress(&self, state: &mut DesktopAutomationState, force: bool) {
        let Some(window_id) = state.target_window_id else {
            return;
        };
        let now = Instant::now();
        let Some(last_report_at) = state.last_report_at else {
            state.last_report_at = Some(now);
            return;
        };
        if !force && now < last_report_at + state.config.report_interval {
            return;
        }
        let Some(snapshot) = window_performance_snapshot(window_id) else {
            state.last_report_at = Some(now);
            return;
        };
        let elapsed = (now - last_report_at).as_secs_f64().max(f64::EPSILON);
        let frame_delta = snapshot
            .frame_index
            .saturating_sub(state.last_report_frame_index);
        let observed_fps = frame_delta as f64 / elapsed;
        Self::print_automation_snapshot(&state.config.label, observed_fps, &snapshot);
        state.last_report_at = Some(now);
        state.last_report_frame_index = snapshot.frame_index;
    }

    fn print_automation_snapshot(
        label: &str,
        observed_fps: f64,
        snapshot: &WindowPerformanceSnapshot,
    ) {
        let slowest_phase = snapshot
            .slowest_phase()
            .map(|sample| format!("{}:{:.3}ms", sample.phase.label(), sample.duration_ms))
            .unwrap_or_else(|| "n/a".to_string());
        let phase_breakdown = if snapshot.phase_timings.is_empty() {
            "n/a".to_string()
        } else {
            snapshot
                .phase_timings
                .iter()
                .map(|sample| format!("{}={:.3}", sample.phase.label(), sample.duration_ms))
                .collect::<Vec<_>>()
                .join(",")
        };
        let renderer_breakdown = format!(
            "comp={:.3},traverse={:.3},batch={:.3},upload={:.3},encode={:.3},submit={:.3},res={:.3},bind={:.3},atlas_miss={}({:.3}ms)",
            snapshot.renderer_submission.composition_time_us as f64 / 1000.0,
            snapshot
                .renderer_submission
                .retained_scene_traversal_time_us as f64
                / 1000.0,
            snapshot.renderer_submission.batch_prepare_time_us as f64 / 1000.0,
            snapshot.renderer_submission.gpu_upload_time_us as f64 / 1000.0,
            snapshot.renderer_submission.pass_encode_time_us as f64 / 1000.0,
            snapshot.renderer_submission.queue_submit_time_us as f64 / 1000.0,
            snapshot.renderer_submission.resource_collection_time_us as f64 / 1000.0,
            snapshot.renderer_submission.bind_group_prepare_time_us as f64 / 1000.0,
            snapshot.renderer_submission.text_atlas_miss_count,
            snapshot.renderer_submission.text_atlas_miss_time_us as f64 / 1000.0,
        );
        let hotspot = snapshot
            .retained_packet_hotspot
            .as_ref()
            .map(|hotspot| {
                format!(
                    "widget={:?},total={:.3}ms,cmds={},text={},paths={},rects={}",
                    hotspot.owner_widget_id,
                    hotspot.total_time_us as f64 / 1000.0,
                    hotspot.command_count,
                    hotspot.text_command_count,
                    hotspot.path_command_count,
                    hotspot.rect_command_count,
                )
            })
            .unwrap_or_else(|| "n/a".to_string());
        println!(
            "[desktop automation:{label}] observed_fps={observed_fps:.1} frame={} total={:.3}ms slowest={} dirty={:.1}% cmds={} acq={:.3}ms pres={:.3}ms build={:.3}ms state={:.3}ms layers={} draws={} phases=[{}] renderer=[{}] hotspot=[{}]",
            snapshot.frame_index,
            snapshot.total_time_ms,
            slowest_phase,
            snapshot.scene.dirty_coverage,
            snapshot.scene.command_count,
            snapshot.renderer_submission.surface_acquire_time_us as f64 / 1000.0,
            snapshot.renderer_submission.surface_present_time_us as f64 / 1000.0,
            snapshot.renderer_submission.retained_packet_build_time_us as f64 / 1000.0,
            snapshot.renderer_submission.retained_state_update_time_us as f64 / 1000.0,
            snapshot.renderer_submission.visible_layer_count,
            snapshot.renderer_submission.draw_count,
            phase_breakdown,
            renderer_breakdown,
            hotspot,
        );
    }

    fn inject_automation_action(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        target_point: Point,
        action: &DesktopAutomationAction,
    ) -> Result<()> {
        match action {
            DesktopAutomationAction::ScrollPixels { delta } => {
                self.inject_pointer_move(event_loop, window_id, target_point, false)?;
                let (buttons, modifiers) = self
                    .windows
                    .get(&window_id)
                    .map(|window| (window.pointer.buttons, window.pointer.modifiers))
                    .ok_or_else(|| Error::new(format!("missing window {}", window_id.get())))?;
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Scroll,
                        position: target_point,
                        delta: Vector::ZERO,
                        scroll_delta: Some(ScrollDelta::Pixels(*delta)),
                        button: None,
                        buttons,
                        modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    }),
                )
            }
        }
    }

    fn inject_pointer_move(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        position: Point,
        emit_enter: bool,
    ) -> Result<()> {
        let (previous, buttons, modifiers) = {
            let window = self
                .windows
                .get(&window_id)
                .ok_or_else(|| Error::new(format!("missing window {}", window_id.get())))?;
            (
                window.pointer.position,
                window.pointer.buttons,
                window.pointer.modifiers,
            )
        };

        if emit_enter {
            self.process_event(
                event_loop,
                window_id,
                Event::Pointer(PointerEvent {
                    pointer_id: 1,
                    kind: PointerEventKind::Enter,
                    position,
                    delta: Vector::ZERO,
                    scroll_delta: None,
                    button: None,
                    buttons,
                    modifiers,
                    pointer_kind: PointerKind::Mouse,
                    is_primary: true,
                }),
            )?;
        }

        if let Some(window) = self.windows.get_mut(&window_id) {
            window.pointer.position = position;
        }

        self.process_event(
            event_loop,
            window_id,
            Event::Pointer(PointerEvent {
                pointer_id: 1,
                kind: PointerEventKind::Move,
                position,
                delta: Vector::new(position.x - previous.x, position.y - previous.y),
                scroll_delta: None,
                button: None,
                buttons,
                modifiers,
                pointer_kind: PointerKind::Mouse,
                is_primary: true,
            }),
        )
    }
}

impl ApplicationHandler for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.drive_runtime(event_loop) {
            self.handle_error(event_loop, error);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: HostWindowId,
        event: WinitWindowEvent,
    ) {
        if let Err(error) = self.handle_window_event(event_loop, window_id, event) {
            self.handle_error(event_loop, error);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.drive_runtime(event_loop) {
            self.handle_error(event_loop, error);
        }
    }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct WindowState {
    id: WindowId,
    title: String,
    display_capabilities_dirty: bool,
    awaiting_performance_bootstrap: bool,
    redraw_requested: bool,
    redraw_requested_at_ms: Option<f64>,
    frame_index: u64,
    pending_event_time_ms: f64,
    last_non_redraw_event_at_ms: Option<f64>,
    accessibility: AccessibilityBridge,
    pointer: PointerState,
    scale_factor: f64,
    window: Arc<Window>,
}

impl WindowState {
    fn snapshot(&self) -> PlatformWindow {
        PlatformWindow {
            id: self.id,
            title: self.title.clone(),
            accessibility: self.accessibility.snapshot().cloned(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PointerState {
    position: Point,
    buttons: PointerButtons,
    modifiers: Modifiers,
}

impl Default for PointerState {
    fn default() -> Self {
        Self {
            position: Point::ZERO,
            buttons: PointerButtons::NONE,
            modifiers: Modifiers::NONE,
        }
    }
}

fn physical_size_to_logical_size(size: PhysicalSize<u32>, scale_factor: f64) -> Size {
    let logical = size.to_logical::<f32>(scale_factor);
    Size::new(logical.width, logical.height)
}

fn physical_position_to_logical_point(position: PhysicalPosition<f64>, scale_factor: f64) -> Point {
    let logical = position.to_logical::<f32>(scale_factor);
    Point::new(logical.x, logical.y)
}

fn physical_position_to_logical_vector(
    position: PhysicalPosition<f64>,
    scale_factor: f64,
) -> Vector {
    let logical = position.to_logical::<f32>(scale_factor);
    Vector::new(logical.x, logical.y)
}

fn event_renders_immediately(event: &Event) -> bool {
    matches!(
        event,
        Event::Window(WindowEvent::Resized(_))
            | Event::Window(WindowEvent::ScaleFactorChanged { .. })
    )
}

fn apply_ime_composition_rect(window: &Window, rect: Option<sui_core::Rect>) {
    let cursor_area = rect.and_then(|rect| sanitize_ime_cursor_area(rect, window.scale_factor()));
    window.set_ime_allowed(cursor_area.is_some());

    if let Some((position, size)) = cursor_area {
        window.set_ime_cursor_area(position, size);
    }
}

fn sanitize_ime_cursor_area(
    rect: sui_core::Rect,
    scale_factor: f64,
) -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    if !rect.x().is_finite()
        || !rect.y().is_finite()
        || !rect.width().is_finite()
        || !rect.height().is_finite()
    {
        return None;
    }

    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let position = LogicalPosition::new(rect.x() as f64, rect.y() as f64);
    let size = LogicalSize::new(rect.width().max(1.0) as f64, rect.height().max(1.0) as f64);
    let PhysicalPosition { x, y } = position.to_physical::<i32>(scale_factor);
    let PhysicalSize { width, height } = size.to_physical::<i32>(scale_factor);
    let x = x.min(i32::MAX - 1);
    let y = y.min(i32::MAX - 1);
    let max_width = ((i32::MAX as i64) - (x as i64)).clamp(1, i32::MAX as i64) as i32;
    let max_height = ((i32::MAX as i64) - (y as i64)).clamp(1, i32::MAX as i64) as i32;
    let width = width.clamp(1, max_width) as u32;
    let height = height.clamp(1, max_height) as u32;

    Some((
        PhysicalPosition::new(x, y),
        PhysicalSize::new(width, height),
    ))
}

fn modifiers_state_to_modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        shift: state.shift_key(),
        control: state.control_key(),
        alt: state.alt_key(),
        meta: state.super_key(),
    }
}

fn map_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Back => Some(PointerButton::Back),
        MouseButton::Forward => Some(PointerButton::Forward),
        MouseButton::Other(value) => Some(PointerButton::Other(value)),
    }
}

fn remove_pointer_button(buttons: PointerButtons, removed: PointerButton) -> PointerButtons {
    let mut next = PointerButtons::NONE;

    for button in [
        PointerButton::Primary,
        PointerButton::Secondary,
        PointerButton::Middle,
        PointerButton::Back,
        PointerButton::Forward,
    ] {
        if button != removed && buttons.contains(button) {
            next.insert(button);
        }
    }

    next
}

fn logical_key_to_string(key: &Key) -> String {
    match key {
        Key::Character(text) => text.to_string(),
        Key::Named(named) => named_key_to_string(*named),
        _ => format!("{key:?}"),
    }
}

fn named_key_to_string(key: NamedKey) -> String {
    match key {
        NamedKey::Enter => "Enter".to_string(),
        NamedKey::Space => " ".to_string(),
        NamedKey::Tab => "Tab".to_string(),
        NamedKey::Escape => "Escape".to_string(),
        NamedKey::ArrowDown => "ArrowDown".to_string(),
        NamedKey::ArrowLeft => "ArrowLeft".to_string(),
        NamedKey::ArrowRight => "ArrowRight".to_string(),
        NamedKey::ArrowUp => "ArrowUp".to_string(),
        _ => format!("{key:?}"),
    }
}

fn physical_key_to_string(key: &PhysicalKey) -> String {
    match key {
        PhysicalKey::Code(code) => format!("{code:?}"),
        PhysicalKey::Unidentified(native_key) => format!("{native_key:?}"),
    }
}

fn map_ime_event(event: Ime) -> Option<ImeEvent> {
    match event {
        Ime::Enabled => Some(ImeEvent::CompositionStart),
        Ime::Preedit(text, cursor_range) => Some(ImeEvent::CompositionUpdate {
            text,
            cursor_range: cursor_range.map(|(start, end)| start..end),
        }),
        Ime::Commit(text) => Some(ImeEvent::CompositionCommit { text }),
        Ime::Disabled => Some(ImeEvent::CompositionEnd),
    }
}

fn map_event_loop_error(error: EventLoopError) -> Error {
    Error::new(format!("winit event loop error: {error}"))
}

fn map_os_error(error: OsError) -> Error {
    Error::new(format!("failed to create desktop window: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        event_renders_immediately, physical_position_to_logical_point,
        physical_position_to_logical_vector, physical_size_to_logical_size,
        sanitize_ime_cursor_area,
    };
    use sui_core::{Event, Rect, Size, WindowEvent};
    use winit::dpi::{PhysicalPosition, PhysicalSize};

    #[test]
    fn converts_physical_size_to_logical_size() {
        let size = physical_size_to_logical_size(PhysicalSize::new(640, 360), 2.0);

        assert_eq!(size.width, 320.0);
        assert_eq!(size.height, 180.0);
    }

    #[test]
    fn converts_physical_pointer_position_to_logical_point() {
        let point = physical_position_to_logical_point(PhysicalPosition::new(240.0, 120.0), 1.5);

        assert_eq!(point.x, 160.0);
        assert_eq!(point.y, 80.0);
    }

    #[test]
    fn converts_physical_scroll_delta_to_logical_vector() {
        let delta = physical_position_to_logical_vector(PhysicalPosition::new(90.0, 45.0), 1.5);

        assert_eq!(delta.x, 60.0);
        assert_eq!(delta.y, 30.0);
    }

    #[test]
    fn window_geometry_events_render_immediately() {
        assert!(event_renders_immediately(&Event::Window(
            WindowEvent::Resized(Size::new(640.0, 360.0))
        )));
        assert!(event_renders_immediately(&Event::Window(
            WindowEvent::ScaleFactorChanged {
                scale_factor: 2.0,
                raw_dpi: Some(192.0),
                suggested_size: Some(Size::new(320.0, 180.0)),
            }
        )));
        assert!(!event_renders_immediately(&Event::Window(
            WindowEvent::RedrawRequested
        )));
    }

    #[test]
    fn sanitize_ime_cursor_area_preserves_valid_geometry() {
        let (position, size) =
            sanitize_ime_cursor_area(Rect::new(10.0, 20.0, 4.0, 6.0), 1.5).unwrap();

        assert_eq!(position, PhysicalPosition::new(15, 30));
        assert_eq!(size, PhysicalSize::new(6, 9));
    }

    #[test]
    fn sanitize_ime_cursor_area_clamps_overflowing_geometry() {
        let (position, size) =
            sanitize_ime_cursor_area(Rect::new(f32::MAX, f32::MAX, f32::MAX, f32::MAX), 1.0)
                .unwrap();

        assert_eq!(position, PhysicalPosition::new(i32::MAX - 1, i32::MAX - 1));
        assert_eq!(size, PhysicalSize::new(1, 1));
    }

    #[test]
    fn sanitize_ime_cursor_area_rejects_non_finite_geometry() {
        assert!(sanitize_ime_cursor_area(Rect::new(f32::NAN, 0.0, 10.0, 10.0), 1.0).is_none());
        assert!(sanitize_ime_cursor_area(Rect::new(0.0, 0.0, f32::INFINITY, 10.0), 1.0).is_none());
    }
}
