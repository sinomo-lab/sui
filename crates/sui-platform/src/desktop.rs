use std::{collections::HashMap, sync::Arc, time::Duration};

use sui_core::{
    Error, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
    PointerButtons, PointerEvent, PointerEventKind, PointerKind, Result, ScrollDelta,
    SemanticsRole, Size, Vector, WindowEvent, WindowId,
};
use sui_render_wgpu::{FeatheringOptions, WgpuRenderer};
use sui_runtime::{
    PresentationLatencyDiagnostics, Runtime, WindowIcon as RuntimeWindowIcon,
    WindowPerformanceSnapshot, WindowRenderOptions, window_performance_snapshot,
    window_render_options, window_scene_statistics_detail_mode,
};
use web_time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    error::{EventLoopError, OsError},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent as WinitWindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, ModifiersState, NamedKey, PhysicalKey},
    window::{Window, WindowAttributes, WindowId as HostWindowId},
};

#[cfg(not(target_arch = "wasm32"))]
use winit::window::Icon as WinitIcon;

#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_WINDOW_ICON_SIZE: u32 = 256;

#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
const WEB_LEGACY_FAVICON_ID: &str = "sui-window-icon";
#[cfg(target_arch = "wasm32")]
const WEB_SVG_FAVICON_ID: &str = "sui-window-icon-svg";
#[cfg(target_arch = "wasm32")]
const WEB_PNG_FAVICON_ID: &str = "sui-window-icon-png";
#[cfg(target_arch = "wasm32")]
const WEB_APPLE_TOUCH_ICON_ID: &str = "sui-window-apple-touch-icon";

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
        self.run_with(runtime, |_| {})
    }

    /// Like [`run`](Self::run) but hands the caller a [`Waker`] (before the loop starts) that
    /// wakes the UI from any thread — used to drive non-blocking startup work.
    pub fn run_with(
        self,
        runtime: Runtime,
        on_ready: impl FnOnce(Waker),
    ) -> Result<Vec<PlatformWindow>> {
        let event_loop = EventLoop::<WakeSignal>::with_user_event()
            .build()
            .map_err(map_event_loop_error)?;
        on_ready(Waker {
            proxy: event_loop.create_proxy(),
        });

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
    fn web_viewport_logical_size() -> LogicalSize<f64> {
        Self::web_canvas_logical_size(None).unwrap_or_else(Self::web_window_logical_size)
    }

    #[cfg(target_arch = "wasm32")]
    fn web_window_logical_size() -> LogicalSize<f64> {
        let width = web_sys::window()
            .and_then(|window| window.inner_width().ok())
            .and_then(|value| value.as_f64())
            .unwrap_or(DesktopPlatform::DEFAULT_WINDOW_SIZE.width as f64)
            .max(1.0);
        let height = web_sys::window()
            .and_then(|window| window.inner_height().ok())
            .and_then(|value| value.as_f64())
            .unwrap_or(DesktopPlatform::DEFAULT_WINDOW_SIZE.height as f64)
            .max(1.0);

        LogicalSize::new(width, height)
    }

    #[cfg(target_arch = "wasm32")]
    fn web_element_logical_size(element: &web_sys::Element) -> Option<LogicalSize<f64>> {
        let rect = element.get_bounding_client_rect();
        let width = rect.width();
        let height = rect.height();
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            return Some(LogicalSize::new(width, height));
        }

        let width = f64::from(element.client_width());
        let height = f64::from(element.client_height());
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            return Some(LogicalSize::new(width, height));
        }

        None
    }

    #[cfg(target_arch = "wasm32")]
    fn web_element_is_document_sizing_root(element: &web_sys::Element) -> bool {
        matches!(
            element.tag_name().to_ascii_lowercase().as_str(),
            "body" | "html"
        )
    }

    #[cfg(target_arch = "wasm32")]
    fn web_find_resize_container(element: &web_sys::Element) -> Option<web_sys::Element> {
        let mut current = Some(element.clone());
        while let Some(candidate) = current {
            if candidate.has_attribute("data-sui-resize-container") {
                return Some(candidate);
            }
            current = candidate.parent_element();
        }
        None
    }

    #[cfg(target_arch = "wasm32")]
    fn web_canvas_sizing_element(canvas: &web_sys::HtmlCanvasElement) -> Option<web_sys::Element> {
        let canvas_element = canvas.clone().dyn_into::<web_sys::Element>().ok()?;
        if let Some(container) = Self::web_find_resize_container(&canvas_element) {
            return Some(container);
        }

        let root = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("sui-root"));

        if let Some(root) = root {
            if let Some(parent) = root.parent_element()
                && !Self::web_element_is_document_sizing_root(&parent)
                && Self::web_element_logical_size(&parent).is_some()
            {
                return Some(parent);
            }
            return Some(root);
        }

        canvas.parent_element().or(Some(canvas_element))
    }

    #[cfg(target_arch = "wasm32")]
    fn web_canvas_logical_size(
        canvas: Option<&web_sys::HtmlCanvasElement>,
    ) -> Option<LogicalSize<f64>> {
        let canvas = canvas.cloned().or_else(|| {
            web_sys::window()?
                .document()
                .and_then(|document| document.get_element_by_id("sui-main-canvas"))
                .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        })?;
        let canvas_element: &web_sys::Element = canvas.as_ref();

        Self::web_canvas_sizing_element(&canvas)
            .as_ref()
            .and_then(Self::web_element_logical_size)
            .or_else(|| Self::web_element_logical_size(canvas_element))
            .or_else(|| {
                canvas
                    .parent_element()
                    .as_ref()
                    .and_then(Self::web_element_logical_size)
            })
    }

    #[cfg(target_arch = "wasm32")]
    fn web_canvas_for_window() -> Option<web_sys::HtmlCanvasElement> {
        let window = web_sys::window()?;
        let canvas = window
            .document()
            .and_then(|document| document.get_element_by_id("sui-main-canvas"))
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())?;
        let scale_factor = window.device_pixel_ratio().max(1.0);
        let size = Self::web_canvas_logical_size(Some(&canvas))
            .unwrap_or_else(Self::web_window_logical_size);
        let width = size.width;
        let height = size.height;
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
            let window_icon = self.runtime.window_icon(window_id)?.cloned();
            #[cfg(target_arch = "wasm32")]
            let initial_size = Self::web_viewport_logical_size();
            #[cfg(not(target_arch = "wasm32"))]
            let initial_size = LogicalSize::new(
                DesktopPlatform::DEFAULT_WINDOW_SIZE.width,
                DesktopPlatform::DEFAULT_WINDOW_SIZE.height,
            );
            #[allow(unused_mut)]
            let mut attributes = WindowAttributes::default()
                .with_title(title.clone())
                .with_inner_size(initial_size);
            #[cfg(not(target_arch = "wasm32"))]
            {
                attributes = attributes.with_window_icon(
                    window_icon
                        .as_ref()
                        .map(window_icon_to_winit_icon)
                        .transpose()?,
                );
            }
            #[cfg(target_arch = "wasm32")]
            {
                install_web_favicon(window_icon.as_ref())?;
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

        #[cfg(target_arch = "wasm32")]
        self.process_web_interop_commands(event_loop)?;

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

        // Event handlers start widget motion from EventCtx::current_time(). Keep that
        // time current for input after the event loop has been idle, not only for
        // redraw and ready-event delivery.
        self.update_clock();
        self.runtime.tick(self.frame_clock);

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

        // Render only in response to `RedrawRequested`. Every other event (resize, scale
        // change, input) marks the schedule and asks winit for a redraw via
        // `request_redraw_if_needed` above; winit then delivers `RedrawRequested` and we render
        // here. Rendering synchronously inside the originating event instead clears the frame
        // schedule, so the redraw winit subsequently delivers finds `needs_render() == false`
        // and becomes a no-op — and a synchronous present issued inside a `Resized` callback is
        // not reliably composited (notably during the Windows modal resize loop), leaving the
        // window stale until an unrelated event repaints it.
        if is_redraw {
            if let Some(window) = self.windows.get_mut(&window_id) {
                window.redraw_requested = false;
            }

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

        #[cfg(target_arch = "wasm32")]
        if let Some(window) = self.windows.get(&window_id) {
            let performance = window_performance_snapshot(window_id);
            crate::web_interop::publish_snapshot(
                window_id,
                frame_index,
                window.scale_factor,
                output.frame.viewport,
                &output.semantics,
                performance.as_ref(),
            );
        }

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

    #[cfg(target_arch = "wasm32")]
    fn process_web_interop_commands(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        for command in crate::web_interop::drain_commands() {
            match command {
                crate::web_interop::WebInteropCommand::Click { target } => {
                    if let Some((window_id, point)) = self.find_semantics_node_point(&target) {
                        self.inject_pointer_click(event_loop, window_id, point)?;
                    }
                }
                crate::web_interop::WebInteropCommand::Scroll {
                    target,
                    delta_x,
                    delta_y,
                } => {
                    if let Some((window_id, point)) = self.find_semantics_node_point(&target) {
                        self.inject_pointer_scroll(
                            event_loop,
                            window_id,
                            point,
                            Vector::new(delta_x, delta_y),
                        )?;
                    }
                }
                crate::web_interop::WebInteropCommand::Key { target, key } => {
                    if let Some((window_id, point)) = self.find_semantics_node_point(&target) {
                        self.inject_pointer_click(event_loop, window_id, point)?;
                        self.inject_key(event_loop, window_id, key)?;
                    }
                }
                crate::web_interop::WebInteropCommand::Text { target, text } => {
                    if let Some((window_id, point)) = self.find_semantics_node_point(&target) {
                        self.inject_pointer_click(event_loop, window_id, point)?;
                        self.inject_text(event_loop, window_id, text)?;
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn find_semantics_node_point(
        &self,
        target: &crate::web_interop::WebInteropTarget,
    ) -> Option<(WindowId, Point)> {
        self.windows.iter().find_map(|(window_id, window)| {
            let snapshot = window.accessibility.snapshot()?;
            let node = target
                .widget_id
                .and_then(|widget_id| snapshot.nodes.iter().find(|node| node.id == widget_id))
                .or_else(|| {
                    snapshot.nodes.iter().find(|node| {
                        target.role.as_deref().is_none_or(|role| {
                            format!("{:?}", node.role).eq_ignore_ascii_case(role)
                        }) && target
                            .name
                            .as_deref()
                            .is_none_or(|name| node.name.as_deref() == Some(name))
                    })
                })?;
            Some((*window_id, center(node.bounds)))
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn inject_pointer_click(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        point: Point,
    ) -> Result<()> {
        self.inject_pointer_move(event_loop, window_id, point, false)?;
        let modifiers = self
            .windows
            .get(&window_id)
            .map(|window| window.pointer.modifiers)
            .ok_or_else(|| Error::new(format!("missing window {}", window_id.get())))?;

        let down_buttons = {
            let window = self
                .windows
                .get_mut(&window_id)
                .ok_or_else(|| Error::new(format!("missing window {}", window_id.get())))?;
            window.pointer.buttons.insert(PointerButton::Primary);
            window.pointer.buttons
        };
        self.process_event(
            event_loop,
            window_id,
            Event::Pointer(PointerEvent {
                pointer_id: 1,
                kind: PointerEventKind::Down,
                position: point,
                delta: Vector::ZERO,
                scroll_delta: None,
                button: Some(PointerButton::Primary),
                buttons: down_buttons,
                modifiers,
                pointer_kind: PointerKind::Mouse,
                is_primary: true,
            }),
        )?;

        let up_buttons = {
            let window = self
                .windows
                .get_mut(&window_id)
                .ok_or_else(|| Error::new(format!("missing window {}", window_id.get())))?;
            window.pointer.buttons =
                remove_pointer_button(window.pointer.buttons, PointerButton::Primary);
            window.pointer.buttons
        };
        self.process_event(
            event_loop,
            window_id,
            Event::Pointer(PointerEvent {
                pointer_id: 1,
                kind: PointerEventKind::Up,
                position: point,
                delta: Vector::ZERO,
                scroll_delta: None,
                button: Some(PointerButton::Primary),
                buttons: up_buttons,
                modifiers,
                pointer_kind: PointerKind::Mouse,
                is_primary: true,
            }),
        )
    }

    #[cfg(target_arch = "wasm32")]
    fn inject_pointer_scroll(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        point: Point,
        delta: Vector,
    ) -> Result<()> {
        self.inject_pointer_move(event_loop, window_id, point, false)?;
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
                position: point,
                delta: Vector::ZERO,
                scroll_delta: Some(ScrollDelta::Pixels(delta)),
                button: None,
                buttons,
                modifiers,
                pointer_kind: PointerKind::Mouse,
                is_primary: true,
            }),
        )
    }

    #[cfg(target_arch = "wasm32")]
    fn inject_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        key: String,
    ) -> Result<()> {
        self.process_event(
            event_loop,
            window_id,
            Event::Keyboard(KeyboardEvent::new(key.clone(), KeyState::Pressed)),
        )?;
        self.process_event(
            event_loop,
            window_id,
            Event::Keyboard(KeyboardEvent::new(key, KeyState::Released)),
        )
    }

    #[cfg(target_arch = "wasm32")]
    fn inject_text(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        text: String,
    ) -> Result<()> {
        self.process_event(
            event_loop,
            window_id,
            Event::Ime(ImeEvent::CompositionStart),
        )?;
        self.process_event(
            event_loop,
            window_id,
            Event::Ime(ImeEvent::CompositionUpdate {
                text: text.clone(),
                cursor_range: None,
            }),
        )?;
        self.process_event(
            event_loop,
            window_id,
            Event::Ime(ImeEvent::CompositionCommit { text }),
        )?;
        self.process_event(event_loop, window_id, Event::Ime(ImeEvent::CompositionEnd))
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

/// Background-thread signal that wakes the desktop event loop (carried as a winit user event).
#[derive(Debug, Clone, Copy)]
pub struct WakeSignal;

/// A cheap, cloneable, `Send` handle that wakes the running desktop UI from any thread. Each
/// [`wake`](Self::wake) delivers an external-wake event ([`sui_runtime::EXTERNAL_WAKE_KIND`]) to
/// every window's root widget, so a widget can drain cross-thread work (channels, async results)
/// without polling on animation frames.
#[derive(Clone)]
pub struct Waker {
    proxy: EventLoopProxy<WakeSignal>,
}

impl Waker {
    pub fn wake(&self) {
        let _ = self.proxy.send_event(WakeSignal);
    }
}

impl ApplicationHandler<WakeSignal> for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.drive_runtime(event_loop) {
            self.handle_error(event_loop, error);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _signal: WakeSignal) {
        // A background thread asked us to wake: deliver an external-wake event to every window's
        // root (so widgets can drain cross-thread work), then drive the runtime — which
        // re-renders affected windows and updates control flow.
        for window_id in self.runtime.window_ids() {
            if let Err(error) = self.runtime.wake_root(window_id) {
                self.handle_error(event_loop, error);
                return;
            }
        }
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

#[cfg(target_arch = "wasm32")]
fn center(bounds: sui_core::Rect) -> Point {
    Point::new(
        bounds.x() + (bounds.width() * 0.5),
        bounds.y() + (bounds.height() * 0.5),
    )
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

#[cfg(not(target_arch = "wasm32"))]
fn window_icon_to_winit_icon(icon: &RuntimeWindowIcon) -> Result<WinitIcon> {
    match icon {
        RuntimeWindowIcon::Svg { data } => svg_window_icon_to_winit_icon(data),
        RuntimeWindowIcon::Rgba8 {
            width,
            height,
            data,
        } => WinitIcon::from_rgba(data.to_vec(), *width, *height).map_err(map_icon_error),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn svg_window_icon_to_winit_icon(svg: &[u8]) -> Result<WinitIcon> {
    let (width, height, rgba) = rasterize_svg_window_icon_rgba8(svg, DEFAULT_WINDOW_ICON_SIZE)?;
    WinitIcon::from_rgba(rgba, width, height).map_err(map_icon_error)
}

#[cfg(not(target_arch = "wasm32"))]
fn rasterize_svg_window_icon_rgba8(svg: &[u8], size: u32) -> Result<(u32, u32, Vec<u8>)> {
    if size == 0 {
        return Err(Error::new("window icon bitmap size must be non-zero"));
    }
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(svg, &options)
        .map_err(|error| Error::new(format!("failed to parse window icon SVG: {error}")))?;
    let source_size = tree.size();
    let scale = (size as f32 / source_size.width()).min(size as f32 / source_size.height());
    let offset_x = (size as f32 - source_size.width() * scale) * 0.5;
    let offset_y = (size as f32 - source_size.height() * scale) * 0.5;
    let transform =
        resvg::tiny_skia::Transform::from_translate(offset_x, offset_y).pre_scale(scale, scale);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| Error::new("failed to allocate window icon bitmap"))?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for pixel in pixmap.pixels() {
        let color = pixel.demultiply();
        rgba.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
    }

    Ok((size, size, rgba))
}

#[cfg(not(target_arch = "wasm32"))]
fn map_icon_error(error: winit::window::BadIcon) -> Error {
    Error::new(format!("failed to create window icon: {error}"))
}

#[cfg(target_arch = "wasm32")]
fn install_web_favicon(icon: Option<&RuntimeWindowIcon>) -> Result<()> {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return Ok(());
    };
    let Some(head) = document.head() else {
        return Ok(());
    };

    remove_web_icon_link(&document, WEB_LEGACY_FAVICON_ID);
    let Some(RuntimeWindowIcon::Svg { data }) = icon else {
        remove_web_icon_links(&document);
        return Ok(());
    };

    let svg_url = svg_favicon_data_url(data);
    install_web_icon_link(
        &document,
        &head,
        WEB_SVG_FAVICON_ID,
        "icon",
        "image/svg+xml",
        "any",
        &svg_url,
    )?;
    install_web_png_icon(
        &document,
        &head,
        data,
        WEB_PNG_FAVICON_ID,
        "icon",
        "64x64",
        64,
    );
    install_web_png_icon(
        &document,
        &head,
        data,
        WEB_APPLE_TOUCH_ICON_ID,
        "apple-touch-icon",
        "180x180",
        180,
    );
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn install_web_icon_link(
    document: &web_sys::Document,
    head: &web_sys::HtmlHeadElement,
    id: &str,
    rel: &str,
    mime_type: &str,
    sizes: &str,
    href: &str,
) -> Result<()> {
    let link = if let Some(link) = document.get_element_by_id(id) {
        link
    } else {
        let link = document
            .create_element("link")
            .map_err(|_| Error::new("failed to create web icon link element"))?;
        link.set_attribute("id", id)
            .map_err(|_| Error::new("failed to configure web icon id"))?;
        head.append_child(&link)
            .map_err(|_| Error::new("failed to attach web icon link"))?;
        link
    };

    link.set_attribute("rel", rel)
        .map_err(|_| Error::new("failed to configure web icon rel"))?;
    link.set_attribute("type", mime_type)
        .map_err(|_| Error::new("failed to configure web icon type"))?;
    if sizes.is_empty() {
        link.remove_attribute("sizes")
            .map_err(|_| Error::new("failed to clear web icon sizes"))?;
    } else {
        link.set_attribute("sizes", sizes)
            .map_err(|_| Error::new("failed to configure web icon sizes"))?;
    }
    link.set_attribute("href", href)
        .map_err(|_| Error::new("failed to configure web icon href"))
}

#[cfg(target_arch = "wasm32")]
fn remove_web_icon_links(document: &web_sys::Document) {
    remove_web_icon_link(document, WEB_SVG_FAVICON_ID);
    remove_web_icon_link(document, WEB_PNG_FAVICON_ID);
    remove_web_icon_link(document, WEB_APPLE_TOUCH_ICON_ID);
}

#[cfg(target_arch = "wasm32")]
fn remove_web_icon_link(document: &web_sys::Document, id: &str) {
    let Some(link) = document.get_element_by_id(id) else {
        return;
    };
    let Some(parent) = link.parent_node() else {
        return;
    };
    let _ = parent.remove_child(&link);
}

#[cfg(target_arch = "wasm32")]
fn svg_favicon_data_url(svg: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut url = String::from("data:image/svg+xml;charset=utf-8,");
    for byte in svg {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                url.push(*byte as char)
            }
            byte => {
                url.push('%');
                url.push(HEX[(byte >> 4) as usize] as char);
                url.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    url
}

#[cfg(target_arch = "wasm32")]
fn install_web_png_icon(
    document: &web_sys::Document,
    head: &web_sys::HtmlHeadElement,
    svg: &[u8],
    link_id: &str,
    rel: &str,
    sizes: &str,
    size: u32,
) {
    use wasm_bindgen::closure::Closure;

    let Ok(canvas_element) = document.create_element("canvas") else {
        return;
    };
    let Ok(canvas) = canvas_element.dyn_into::<web_sys::HtmlCanvasElement>() else {
        return;
    };
    canvas.set_width(size);
    canvas.set_height(size);

    let Ok(Some(context)) = canvas.get_context("2d") else {
        return;
    };
    let Ok(context) = context.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
        return;
    };
    let Ok(image) = web_sys::HtmlImageElement::new() else {
        return;
    };

    let svg_url = svg_favicon_data_url(svg);
    let document = document.clone();
    let head = head.clone();
    let link_id = link_id.to_string();
    let rel = rel.to_string();
    let sizes = sizes.to_string();
    let image_for_load = image.clone();
    let onload = Closure::<dyn FnMut()>::new(move || {
        let size = size as f64;
        context.clear_rect(0.0, 0.0, size, size);
        if context
            .draw_image_with_html_image_element_and_dw_and_dh(&image_for_load, 0.0, 0.0, size, size)
            .is_ok()
            && let Ok(png_url) = canvas.to_data_url_with_type("image/png")
        {
            let _ = install_web_icon_link(
                &document,
                &head,
                &link_id,
                &rel,
                "image/png",
                &sizes,
                &png_url,
            );
        }
    });

    image.set_onload(Some(onload.as_ref().unchecked_ref()));
    image.set_src(&svg_url);
    onload.forget();
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
        physical_position_to_logical_point, physical_position_to_logical_vector,
        physical_size_to_logical_size, rasterize_svg_window_icon_rgba8, sanitize_ime_cursor_area,
        window_icon_to_winit_icon,
    };
    use sui_core::Rect;
    use sui_runtime::WindowIcon;
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

    #[test]
    fn default_svg_window_icon_rasterizes_to_winit_icon() {
        assert!(window_icon_to_winit_icon(&WindowIcon::sui()).is_ok());
    }

    #[test]
    fn default_svg_window_icon_rasterizes_to_bitmap() {
        let (width, height, rgba) =
            rasterize_svg_window_icon_rgba8(WindowIcon::sui().as_svg().unwrap(), 64).unwrap();

        assert_eq!((width, height), (64, 64));
        assert_eq!(rgba.len(), 64 * 64 * 4);
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
}
