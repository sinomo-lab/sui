use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

use sui_core::{
    Error, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
    PointerButtons, PointerEvent, PointerEventKind, PointerKind, Result, ScrollDelta, Size, Vector,
    WindowEvent, WindowId,
};
use sui_platform::{
    AccessibilitySnapshot, HeadlessPlatform, WindowOutputDiagnostics,
    detect_window_display_capabilities, publish_window_output_diagnostics,
};
use sui_render_wgpu::{
    ColorManagementMode, DebugCaptureArtifact, DebugCaptureRequest, FeatheringOptions,
    RequestedColorManagementMode, RequestedDynamicRangeMode, RequestedOutputColorPrimaries,
    RequestedToneMappingMode, WgpuRenderer,
};
use sui_runtime::{
    CacheMetrics, FocusState, FramePhase, FramePhaseSample, PresentationLatencyDiagnostics,
    RenderOutput, RendererSubmissionDiagnostics, Runtime, SceneStatistics, TextCacheDiagnostics,
    WidgetGraphSnapshot, WindowColorManagementMode, WindowDynamicRangeMode,
    WindowOutputColorPrimaries, WindowPerformanceSnapshot, WindowToneMappingMode,
    clear_window_performance_snapshots, publish_window_performance_snapshot,
    window_performance_snapshot, window_performance_text_caches, window_render_options,
    window_scene_statistics_detail_mode,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    error::{EventLoopError, OsError},
    event::WindowEvent as WinitWindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowAttributes, WindowId as HostWindowId},
};

use crate::{
    screenshot::{ArtifactBundle, Screenshot, semantics_overlay, widget_overlay},
    snapshot::{SceneSummary, WindowSnapshot},
};

const DEFAULT_WINDOW_SIZE: sui_core::Size = sui_core::Size::new(1280.0, 720.0);
const REDRAW_FLUSH_LIMIT: usize = 256;
const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(16);

fn map_window_color_management_for_harness(
    mode: WindowColorManagementMode,
    primaries: WindowOutputColorPrimaries,
    dynamic_range: WindowDynamicRangeMode,
    tone_mapping: WindowToneMappingMode,
    sdr_content_brightness_nits: f32,
) -> ColorManagementMode {
    ColorManagementMode {
        mode: match mode {
            WindowColorManagementMode::Automatic => RequestedColorManagementMode::Automatic,
            WindowColorManagementMode::ForceSdr => RequestedColorManagementMode::ForceSdr,
            WindowColorManagementMode::PreferWideGamut => {
                RequestedColorManagementMode::PreferWideGamut
            }
            WindowColorManagementMode::PreferHdr => RequestedColorManagementMode::PreferHdr,
        },
        output_primaries: match primaries {
            WindowOutputColorPrimaries::Automatic => RequestedOutputColorPrimaries::Automatic,
            WindowOutputColorPrimaries::Srgb => RequestedOutputColorPrimaries::Srgb,
            WindowOutputColorPrimaries::DisplayP3 => RequestedOutputColorPrimaries::DisplayP3,
        },
        dynamic_range: match dynamic_range {
            WindowDynamicRangeMode::Automatic => RequestedDynamicRangeMode::Automatic,
            WindowDynamicRangeMode::StandardDynamicRange => {
                RequestedDynamicRangeMode::StandardDynamicRange
            }
            WindowDynamicRangeMode::HighDynamicRange => RequestedDynamicRangeMode::HighDynamicRange,
        },
        tone_mapping: match tone_mapping {
            WindowToneMappingMode::Automatic => RequestedToneMappingMode::Automatic,
            WindowToneMappingMode::Clamp => RequestedToneMappingMode::Clamp,
            WindowToneMappingMode::Reinhard => RequestedToneMappingMode::Reinhard,
        },
        sdr_content_brightness_nits,
    }
}

pub(crate) struct Harness {
    backend: HarnessBackend,
    default_timeout: f64,
}

enum HarnessBackend {
    Headless(HeadlessHarness),
    Live(LiveHarness),
}

struct HeadlessHarness {
    runtime: Runtime,
    platform: HeadlessPlatform,
}

struct LiveHarness {
    proxy: EventLoopProxy<HarnessCommand>,
    _guard: MutexGuard<'static, ()>,
}

#[derive(Debug, Clone)]
enum HostInputEvent {
    Focused(bool),
    Resized {
        size: Size,
    },
    CursorEntered,
    CursorLeft,
    CursorMoved {
        position: Point,
    },
    MouseInput {
        pressed: bool,
        button: PointerButton,
    },
    MouseWheel {
        delta: ScrollDelta,
    },
    Keyboard {
        key: String,
        code: String,
        text: Option<String>,
        state: KeyState,
        repeat: bool,
        modifiers: Modifiers,
    },
    Ime(ImeEvent),
    RedrawRequested,
}

enum HarnessCommand {
    Launch {
        build_runtime: RuntimeBuilder,
        vsync_enabled: bool,
        visible: bool,
        reply: SyncSender<Result<()>>,
    },
    Flush {
        reply: SyncSender<Result<()>>,
    },
    Dispatch {
        window_id: WindowId,
        event: HostInputEvent,
        reply: SyncSender<Result<()>>,
    },
    ListWindows {
        reply: SyncSender<Result<Vec<(WindowId, String)>>>,
    },
    Snapshot {
        window_id: WindowId,
        reply: SyncSender<Result<WindowSnapshot>>,
    },
    Capture {
        window_id: WindowId,
        reply: SyncSender<Result<Screenshot>>,
    },
    CaptureDebug {
        window_id: WindowId,
        request: DebugCaptureRequest,
        reply: SyncSender<Result<DebugCaptureArtifact>>,
    },
}

type RuntimeBuilder = Box<dyn FnOnce() -> Result<Runtime> + Send>;

#[derive(Debug)]
struct LiveWindowState {
    title: String,
    redraw_requested: bool,
    redraw_requested_at_ms: Option<f64>,
    frame_index: u64,
    pending_event_time_ms: f64,
    last_non_redraw_event_at_ms: Option<f64>,
    pointer: PointerState,
    scale_factor: f64,
    window: Arc<Window>,
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

struct LiveHarnessApp {
    runtime: Runtime,
    renderer: WgpuRenderer,
    vsync_enabled: bool,
    window_visible: bool,
    started_at: Instant,
    frame_clock: f64,
    windows: HashMap<WindowId, LiveWindowState>,
    host_to_runtime: HashMap<HostWindowId, WindowId>,
    last_error: Option<Error>,
}

struct HarnessService {
    proxy: EventLoopProxy<HarnessCommand>,
}

static HARNESS_SERVICE: OnceLock<HarnessService> = OnceLock::new();
static LIVE_TEST_LOCK: Mutex<()> = Mutex::new(());

impl Harness {
    pub(crate) fn new_headless(runtime: Runtime) -> Result<Self> {
        let mut harness = Self {
            backend: HarnessBackend::Headless(HeadlessHarness {
                runtime,
                platform: HeadlessPlatform::new(),
            }),
            default_timeout: 5.0,
        };
        harness.run_until_idle()?;
        Ok(harness)
    }

    pub(crate) fn new_live<F>(build_runtime: F) -> Result<Self>
    where
        F: FnOnce() -> Result<Runtime> + Send + 'static,
    {
        Self::new_live_with_options(build_runtime, true, false)
    }

    pub(crate) fn new_live_with_vsync<F>(build_runtime: F, vsync_enabled: bool) -> Result<Self>
    where
        F: FnOnce() -> Result<Runtime> + Send + 'static,
    {
        Self::new_live_with_options(build_runtime, vsync_enabled, false)
    }

    pub(crate) fn new_live_with_options<F>(
        build_runtime: F,
        vsync_enabled: bool,
        visible: bool,
    ) -> Result<Self>
    where
        F: FnOnce() -> Result<Runtime> + Send + 'static,
    {
        let guard = LIVE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let proxy = harness_service().proxy.clone();
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        proxy
            .send_event(HarnessCommand::Launch {
                build_runtime: Box::new(build_runtime),
                vsync_enabled,
                visible,
                reply: reply_tx,
            })
            .map_err(|_| Error::new("live test harness service is unavailable"))?;
        recv_result(&reply_rx, "live harness launch", Duration::from_secs(5))?;

        Ok(Self {
            backend: HarnessBackend::Live(LiveHarness {
                proxy,
                _guard: guard,
            }),
            default_timeout: 5.0,
        })
    }

    pub(crate) fn default_timeout(&self) -> f64 {
        self.default_timeout
    }

    pub(crate) fn set_default_timeout(&mut self, timeout: f64) {
        self.default_timeout = timeout;
    }

    pub(crate) fn window_ids(&self) -> Vec<WindowId> {
        match &self.backend {
            HarnessBackend::Headless(harness) => harness.runtime.window_ids(),
            HarnessBackend::Live(harness) => harness
                .list_windows()
                .map(|windows| {
                    windows
                        .into_iter()
                        .map(|(window_id, _)| window_id)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub(crate) fn window_id_by_title(&self, title: &str) -> Option<WindowId> {
        match &self.backend {
            HarnessBackend::Headless(harness) => {
                harness.runtime.window_ids().into_iter().find(|window_id| {
                    harness
                        .runtime
                        .window_title(*window_id)
                        .is_ok_and(|window_title| window_title == title)
                })
            }
            HarnessBackend::Live(harness) => harness.list_windows().ok().and_then(|windows| {
                windows
                    .into_iter()
                    .find(|(_, window_title)| window_title == title)
                    .map(|(window_id, _)| window_id)
            }),
        }
    }

    pub(crate) fn advance_time(&mut self, delta: f64) -> Result<()> {
        if delta.is_sign_negative() {
            return Err(Error::new("time delta must be >= 0"));
        }

        match &mut self.backend {
            HarnessBackend::Headless(harness) => {
                harness.platform.advance_time(delta);
                self.run_until_idle()
            }
            HarnessBackend::Live(_) => {
                if delta > 0.0 {
                    thread::sleep(Duration::from_secs_f64(delta));
                }
                self.run_until_idle()
            }
        }
    }

    pub(crate) fn run_until_idle(&mut self) -> Result<()> {
        match &mut self.backend {
            HarnessBackend::Headless(harness) => {
                while harness.platform.pump(&mut harness.runtime)? {}
                Ok(())
            }
            HarnessBackend::Live(harness) => harness.flush(),
        }
    }

    pub(crate) fn run_until<T, F>(&mut self, timeout: f64, mut predicate: F) -> Result<T>
    where
        F: FnMut(&Self) -> Result<Option<T>>,
    {
        let timeout = timeout.max(0.0);
        let deadline = Instant::now() + Duration::from_secs_f64(timeout);

        loop {
            self.run_until_idle()?;
            if let Some(value) = predicate(self)? {
                return Ok(value);
            }

            let now = Instant::now();
            if now >= deadline {
                break;
            }

            if !self.wait_for_progress(deadline.saturating_duration_since(now))? {
                break;
            }
        }

        self.run_until_idle()?;
        predicate(self)?.ok_or_else(|| Error::new("condition not satisfied before timeout"))
    }

    pub(crate) fn dispatch_event(&mut self, window_id: WindowId, event: Event) -> Result<()> {
        match &mut self.backend {
            HarnessBackend::Headless(harness) => {
                harness
                    .platform
                    .dispatch_event(&harness.runtime, window_id, event)?;
                self.run_until_idle()
            }
            HarnessBackend::Live(harness) => harness.dispatch_event(window_id, event),
        }
    }

    pub(crate) fn snapshot(&self, window_id: WindowId) -> Result<WindowSnapshot> {
        match &self.backend {
            HarnessBackend::Headless(harness) => snapshot_headless(harness, window_id),
            HarnessBackend::Live(harness) => harness.snapshot(window_id),
        }
    }

    pub(crate) fn capture_screenshot(&self, window_id: WindowId) -> Result<Screenshot> {
        match &self.backend {
            HarnessBackend::Headless(harness) => {
                let image = harness.platform.capture_rgba(window_id)?;
                Ok(Screenshot::from_rgba_image(image))
            }
            HarnessBackend::Live(harness) => harness.capture(window_id),
        }
    }

    pub(crate) fn capture_debug_frame(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        match &mut self.backend {
            HarnessBackend::Headless(harness) => {
                harness.platform.capture_debug_frame(window_id, request)
            }
            HarnessBackend::Live(harness) => harness.capture_debug(window_id, request),
        }
    }

    pub(crate) fn capture_artifacts(&self, window_id: WindowId) -> Result<ArtifactBundle> {
        let snapshot = self.snapshot(window_id)?;
        let screenshot = self.capture_screenshot(window_id).ok();
        let semantics_overlay = screenshot
            .as_ref()
            .map(|image| semantics_overlay(image, &snapshot));
        let widget_overlay = screenshot
            .as_ref()
            .map(|image| widget_overlay(image, &snapshot));

        Ok(ArtifactBundle {
            snapshot,
            screenshot,
            semantics_overlay,
            widget_overlay,
        })
    }

    pub(crate) fn performance_snapshot(
        &self,
        window_id: WindowId,
    ) -> Result<WindowPerformanceSnapshot> {
        window_performance_snapshot(window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have a performance snapshot yet",
                window_id.get()
            ))
        })
    }

    pub(crate) fn fallback_snapshot(&self, window_id: WindowId) -> WindowSnapshot {
        match &self.backend {
            HarnessBackend::Headless(harness) => WindowSnapshot {
                window_id,
                title: harness
                    .runtime
                    .window_title(window_id)
                    .unwrap_or("<unknown>")
                    .to_string(),
                accessibility: AccessibilitySnapshot {
                    window_id,
                    root: None,
                    focused_widget: None,
                    nodes: Vec::new(),
                },
                widget_graph: harness.runtime.widget_graph(window_id).unwrap_or(
                    WidgetGraphSnapshot {
                        root: Default::default(),
                        nodes: Vec::new(),
                        stack_hosts: Vec::new(),
                    },
                ),
                focus_state: harness
                    .runtime
                    .focus_state(window_id)
                    .unwrap_or(FocusState::default()),
                scene_summary: None,
            },
            HarnessBackend::Live(harness) => harness.fallback_snapshot(window_id),
        }
    }

    fn wait_for_progress(&mut self, remaining: Duration) -> Result<bool> {
        match &mut self.backend {
            HarnessBackend::Headless(harness) => {
                let Some(next_wakeup) = next_headless_wakeup(harness)? else {
                    return Ok(false);
                };
                let now = harness.platform.current_time();
                let delta = (next_wakeup - now).max(0.0);
                harness
                    .platform
                    .advance_time(delta.min(remaining.as_secs_f64()));
                Ok(true)
            }
            HarnessBackend::Live(_) => {
                thread::sleep(remaining.min(LIVE_POLL_INTERVAL));
                Ok(true)
            }
        }
    }
}

impl LiveHarness {
    fn flush(&self) -> Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.proxy
            .send_event(HarnessCommand::Flush { reply: reply_tx })
            .map_err(|_| Error::new("live harness service is unavailable"))?;
        recv_result(&reply_rx, "live harness flush", Duration::from_secs(5))
    }

    fn dispatch_event(&self, window_id: WindowId, event: Event) -> Result<()> {
        for host_event in map_runtime_event_to_host_inputs(event)? {
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            self.proxy
                .send_event(HarnessCommand::Dispatch {
                    window_id,
                    event: host_event,
                    reply: reply_tx,
                })
                .map_err(|_| Error::new("live harness service is unavailable"))?;
            recv_result(&reply_rx, "live harness dispatch", Duration::from_secs(5))?;
        }
        Ok(())
    }

    fn list_windows(&self) -> Result<Vec<(WindowId, String)>> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.proxy
            .send_event(HarnessCommand::ListWindows { reply: reply_tx })
            .map_err(|_| Error::new("live harness service is unavailable"))?;
        recv_result(
            &reply_rx,
            "live harness window listing",
            Duration::from_secs(5),
        )
    }

    fn snapshot(&self, window_id: WindowId) -> Result<WindowSnapshot> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.proxy
            .send_event(HarnessCommand::Snapshot {
                window_id,
                reply: reply_tx,
            })
            .map_err(|_| Error::new("live harness service is unavailable"))?;
        recv_result(&reply_rx, "live harness snapshot", Duration::from_secs(5))
    }

    fn capture(&self, window_id: WindowId) -> Result<Screenshot> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.proxy
            .send_event(HarnessCommand::Capture {
                window_id,
                reply: reply_tx,
            })
            .map_err(|_| Error::new("live harness service is unavailable"))?;
        recv_result(&reply_rx, "live harness capture", Duration::from_secs(5))
    }

    fn capture_debug(
        &self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.proxy
            .send_event(HarnessCommand::CaptureDebug {
                window_id,
                request,
                reply: reply_tx,
            })
            .map_err(|_| Error::new("live harness service is unavailable"))?;
        recv_result(
            &reply_rx,
            "live harness debug capture",
            Duration::from_secs(5),
        )
    }

    fn fallback_snapshot(&self, window_id: WindowId) -> WindowSnapshot {
        let title = self
            .list_windows()
            .ok()
            .and_then(|windows| {
                windows
                    .into_iter()
                    .find(|(candidate_id, _)| *candidate_id == window_id)
                    .map(|(_, title)| title)
            })
            .unwrap_or_else(|| "<unknown>".to_string());

        WindowSnapshot {
            window_id,
            title,
            accessibility: AccessibilitySnapshot {
                window_id,
                root: None,
                focused_widget: None,
                nodes: Vec::new(),
            },
            widget_graph: WidgetGraphSnapshot {
                root: Default::default(),
                nodes: Vec::new(),
                stack_hosts: Vec::new(),
            },
            focus_state: FocusState::default(),
            scene_summary: None,
        }
    }
}

impl LiveHarnessApp {
    fn new() -> Self {
        Self {
            runtime: Runtime::new(),
            renderer: WgpuRenderer::default(),
            vsync_enabled: true,
            window_visible: false,
            started_at: Instant::now(),
            frame_clock: 0.0,
            windows: HashMap::new(),
            host_to_runtime: HashMap::new(),
            last_error: None,
        }
    }

    fn report_error(&mut self, error: Error) {
        self.last_error = Some(error);
        self.reset_runtime_state();
    }

    fn reset_runtime_state(&mut self) {
        for window_id in self.windows.keys().copied().collect::<Vec<_>>() {
            self.renderer.remove_window(window_id);
        }
        self.windows.clear();
        self.host_to_runtime.clear();
        self.runtime = Runtime::new();
        self.renderer = WgpuRenderer::default().with_vsync_enabled(self.vsync_enabled);
        self.started_at = Instant::now();
        self.frame_clock = 0.0;
        clear_window_performance_snapshots();
    }

    fn take_last_error(&mut self) -> Result<()> {
        if let Some(error) = self.last_error.take() {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn launch_runtime(
        &mut self,
        event_loop: &ActiveEventLoop,
        build_runtime: RuntimeBuilder,
        vsync_enabled: bool,
        visible: bool,
    ) -> Result<()> {
        self.vsync_enabled = vsync_enabled;
        self.window_visible = visible;
        self.reset_runtime_state();
        self.last_error = None;
        self.runtime = build_runtime()?;
        self.flush_pending_frames(event_loop)
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
            }
        }

        for window_id in runtime_window_ids {
            if self.windows.contains_key(&window_id) {
                continue;
            }

            let title = self.runtime.window_title(window_id)?.to_string();
            let window = Arc::new(
                event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_visible(self.window_visible)
                            .with_title(title.clone())
                            .with_inner_size(LogicalSize::new(
                                DEFAULT_WINDOW_SIZE.width,
                                DEFAULT_WINDOW_SIZE.height,
                            )),
                    )
                    .map_err(map_os_error)?,
            );
            window.set_ime_allowed(false);

            let host_id = window.id();
            let scale_factor = window.scale_factor();
            let size = physical_size_to_logical_size(window.inner_size(), scale_factor);
            self.renderer
                .register_window(window_id, Arc::clone(&window))?;

            self.host_to_runtime.insert(host_id, window_id);
            self.windows.insert(
                window_id,
                LiveWindowState {
                    title,
                    redraw_requested: false,
                    redraw_requested_at_ms: None,
                    frame_index: 0,
                    pending_event_time_ms: 0.0,
                    last_non_redraw_event_at_ms: None,
                    pointer: PointerState::default(),
                    scale_factor,
                    window,
                },
            );

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
            event_loop.set_control_flow(ControlFlow::Wait);
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
        for window_id in window_ids {
            self.request_redraw_if_needed(window_id)?;
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

    fn flush_pending_frames(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        for _ in 0..REDRAW_FLUSH_LIMIT {
            self.drive_runtime(event_loop)?;

            let pending: Vec<_> = self
                .windows
                .values()
                .filter(|window| window.redraw_requested)
                .map(|window| window.window.id())
                .collect();

            if pending.is_empty() {
                return Ok(());
            }

            for host_id in pending {
                self.handle_window_event(event_loop, host_id, WinitWindowEvent::RedrawRequested)?;
            }
        }

        Err(Error::new(
            "live harness exceeded the redraw flush budget; likely stuck in a redraw loop",
        ))
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

            if self.runtime.needs_render(window_id)? {
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
                let renderer_started = Instant::now();
                let diagnostics_enabled =
                    window_scene_statistics_detail_mode(window_id).is_detailed();
                self.renderer
                    .set_runtime_diagnostics_enabled(diagnostics_enabled);
                let render_options = window_render_options(window_id);
                let display_capabilities = self
                    .windows
                    .get(&window_id)
                    .map(|window| detect_window_display_capabilities(window.window.as_ref()))
                    .unwrap_or_default();
                self.renderer
                    .set_window_display_capabilities(window_id, display_capabilities)?;
                self.renderer
                    .set_runtime_feathering_override(render_options.map(|options| {
                        FeatheringOptions::new(options.feathering_enabled, options.feather_width)
                    }));
                self.renderer.set_window_color_management(
                    window_id,
                    render_options
                        .map(|options| {
                            map_window_color_management_for_harness(
                                options.color_management_mode,
                                options.output_color_primaries,
                                options.dynamic_range_mode,
                                options.tone_mapping_mode,
                                options.sdr_content_brightness_nits,
                            )
                        })
                        .unwrap_or_default(),
                )?;
                self.renderer.render(&output.frame)?;
                if let (Some(mut display_capabilities), Some(active_output_strategy)) = (
                    self.renderer.window_display_capabilities(window_id),
                    self.renderer.window_output_strategy(window_id),
                ) {
                    if let Some(formats) = self.renderer.window_surface_formats(window_id) {
                        display_capabilities
                            .notes
                            .push_str(&format!(" Surface formats: {:?}.", formats));
                    }
                    let options = render_options
                        .unwrap_or_else(|| sui_runtime::WindowRenderOptions::new(true, 1.0));
                    publish_window_output_diagnostics(
                        window_id,
                        WindowOutputDiagnostics {
                            display_capabilities,
                            requested_color_management_mode: options.color_management_mode,
                            requested_output_primaries: options.output_color_primaries,
                            requested_dynamic_range_mode: options.dynamic_range_mode,
                            requested_tone_mapping_mode: options.tone_mapping_mode,
                            requested_sdr_content_brightness_nits: options
                                .sdr_content_brightness_nits,
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
                    window.last_non_redraw_event_at_ms = None;
                    window.redraw_requested_at_ms = None;
                    if window.title != output.title {
                        window.title = output.title.clone();
                        window.window.set_title(&output.title);
                    }
                    apply_ime_composition_rect(window.window.as_ref(), output.ime_composition_rect);
                }

                publish_frame_performance(
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
            }
        }

        if is_close {
            self.runtime.remove_window(window_id)?;
            self.sync_windows(event_loop)?;
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
            _ => Ok(()),
        }
    }

    fn dispatch_host_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: HostInputEvent,
    ) -> Result<()> {
        match event {
            HostInputEvent::Resized { size } => {
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Window(WindowEvent::Resized(size)),
                )?;
            }
            HostInputEvent::Focused(focused) => self.process_event(
                event_loop,
                window_id,
                Event::Window(WindowEvent::Focused(focused)),
            )?,
            HostInputEvent::CursorEntered => {
                let pointer = self
                    .windows
                    .get(&window_id)
                    .map(|window| window.pointer)
                    .ok_or_else(|| {
                        Error::new(format!("window {} is not registered", window_id.get()))
                    })?;
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Enter,
                        position: pointer.position,
                        delta: Vector::ZERO,
                        scroll_delta: None,
                        button: None,
                        buttons: pointer.buttons,
                        modifiers: pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    }),
                )?;
            }
            HostInputEvent::CursorLeft => {
                let pointer = self
                    .windows
                    .get(&window_id)
                    .map(|window| window.pointer)
                    .ok_or_else(|| {
                        Error::new(format!("window {} is not registered", window_id.get()))
                    })?;
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Leave,
                        position: pointer.position,
                        delta: Vector::ZERO,
                        scroll_delta: None,
                        button: None,
                        buttons: pointer.buttons,
                        modifiers: pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    }),
                )?;
            }
            HostInputEvent::CursorMoved { position } => {
                let window = self.windows.get_mut(&window_id).ok_or_else(|| {
                    Error::new(format!("window {} is not registered", window_id.get()))
                })?;
                let delta = Vector::new(
                    position.x - window.pointer.position.x,
                    position.y - window.pointer.position.y,
                );
                window.pointer.position = position;
                let buttons = window.pointer.buttons;
                let modifiers = window.pointer.modifiers;
                let _ = window;
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Move,
                        position,
                        delta,
                        scroll_delta: None,
                        button: None,
                        buttons,
                        modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    }),
                )?;
            }
            HostInputEvent::MouseInput { pressed, button } => {
                let window = self.windows.get_mut(&window_id).ok_or_else(|| {
                    Error::new(format!("window {} is not registered", window_id.get()))
                })?;
                if pressed {
                    window.pointer.buttons.insert(button);
                } else {
                    window.pointer.buttons = remove_pointer_button(window.pointer.buttons, button);
                }
                let position = window.pointer.position;
                let buttons = window.pointer.buttons;
                let modifiers = window.pointer.modifiers;
                let _ = window;
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: if pressed {
                            PointerEventKind::Down
                        } else {
                            PointerEventKind::Up
                        },
                        position,
                        delta: Vector::ZERO,
                        scroll_delta: None,
                        button: Some(button),
                        buttons,
                        modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    }),
                )?;
            }
            HostInputEvent::MouseWheel { delta } => {
                let window = self.windows.get(&window_id).ok_or_else(|| {
                    Error::new(format!("window {} is not registered", window_id.get()))
                })?;
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Pointer(PointerEvent {
                        pointer_id: 1,
                        kind: PointerEventKind::Scroll,
                        position: window.pointer.position,
                        delta: Vector::ZERO,
                        scroll_delta: Some(delta),
                        button: None,
                        buttons: window.pointer.buttons,
                        modifiers: window.pointer.modifiers,
                        pointer_kind: PointerKind::Mouse,
                        is_primary: true,
                    }),
                )?;
            }
            HostInputEvent::Keyboard {
                key,
                code,
                text,
                state,
                repeat,
                modifiers,
            } => {
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Keyboard(KeyboardEvent {
                        key,
                        code,
                        text,
                        state,
                        modifiers,
                        repeat,
                        is_composing: false,
                    }),
                )?;
            }
            HostInputEvent::Ime(event) => {
                self.process_event(event_loop, window_id, Event::Ime(event))?;
            }
            HostInputEvent::RedrawRequested => {
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Window(WindowEvent::RedrawRequested),
                )?;
            }
        }

        self.flush_pending_frames(event_loop)
    }

    fn list_windows(&mut self, event_loop: &ActiveEventLoop) -> Result<Vec<(WindowId, String)>> {
        self.flush_pending_frames(event_loop)?;
        Ok(self
            .runtime
            .window_ids()
            .into_iter()
            .filter_map(|window_id| {
                self.runtime
                    .window_title(window_id)
                    .ok()
                    .map(|title| (window_id, title.to_string()))
            })
            .collect())
    }

    fn snapshot(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
    ) -> Result<WindowSnapshot> {
        self.flush_pending_frames(event_loop)?;

        let semantics = self.runtime.semantics(window_id)?.to_vec();
        let accessibility = AccessibilitySnapshot {
            window_id,
            root: semantics
                .iter()
                .find(|node| node.parent.is_none())
                .map(|node| node.id),
            focused_widget: self.runtime.focused_widget(window_id)?,
            nodes: semantics,
        };

        Ok(WindowSnapshot {
            window_id,
            title: self.runtime.window_title(window_id)?.to_string(),
            accessibility,
            widget_graph: self.runtime.widget_graph(window_id)?,
            focus_state: self.runtime.focus_state(window_id)?,
            scene_summary: self
                .renderer
                .last_frame(window_id)
                .map(SceneSummary::from_frame),
        })
    }

    fn capture(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) -> Result<Screenshot> {
        self.flush_pending_frames(event_loop)?;
        let image = self.renderer.capture_last_frame_rgba(window_id)?;
        Ok(Screenshot::from_rgba_image(image))
    }

    fn capture_debug(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        self.flush_pending_frames(event_loop)?;
        self.renderer.capture_last_frame_debug(window_id, request)
    }

    fn handle_command(&mut self, event_loop: &ActiveEventLoop, command: HarnessCommand) {
        match command {
            HarnessCommand::Launch {
                build_runtime,
                vsync_enabled,
                visible,
                reply,
            } => {
                let _ = reply.send(self.launch_runtime(
                    event_loop,
                    build_runtime,
                    vsync_enabled,
                    visible,
                ));
            }
            HarnessCommand::Flush { reply } => {
                let _ = reply.send(
                    self.take_last_error()
                        .and_then(|()| self.flush_pending_frames(event_loop)),
                );
            }
            HarnessCommand::Dispatch {
                window_id,
                event,
                reply,
            } => {
                let _ = reply.send(
                    self.take_last_error()
                        .and_then(|()| self.dispatch_host_event(event_loop, window_id, event)),
                );
            }
            HarnessCommand::ListWindows { reply } => {
                let _ = reply.send(
                    self.take_last_error()
                        .and_then(|()| self.list_windows(event_loop)),
                );
            }
            HarnessCommand::Snapshot { window_id, reply } => {
                let _ = reply.send(
                    self.take_last_error()
                        .and_then(|()| self.snapshot(event_loop, window_id)),
                );
            }
            HarnessCommand::Capture { window_id, reply } => {
                let _ = reply.send(
                    self.take_last_error()
                        .and_then(|()| self.capture(event_loop, window_id)),
                );
            }
            HarnessCommand::CaptureDebug {
                window_id,
                request,
                reply,
            } => {
                let _ = reply.send(
                    self.take_last_error()
                        .and_then(|()| self.capture_debug(event_loop, window_id, request)),
                );
            }
        }
    }
}

impl ApplicationHandler<HarnessCommand> for LiveHarnessApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.flush_pending_frames(event_loop) {
            self.report_error(error);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HarnessCommand) {
        self.handle_command(event_loop, event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: HostWindowId,
        event: WinitWindowEvent,
    ) {
        if let Err(error) = self.handle_window_event(event_loop, window_id, event) {
            self.report_error(error);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.drive_runtime(event_loop) {
            self.report_error(error);
        }
    }
}

fn snapshot_headless(harness: &HeadlessHarness, window_id: WindowId) -> Result<WindowSnapshot> {
    let accessibility = harness
        .platform
        .accessibility_snapshot(window_id)
        .cloned()
        .ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an accessibility snapshot yet",
                window_id.get()
            ))
        })?;
    let title = harness.runtime.window_title(window_id)?.to_string();
    let focus_state = harness.runtime.focus_state(window_id)?;
    let widget_graph = harness.runtime.widget_graph(window_id)?;
    let scene_summary = harness
        .platform
        .renderer()
        .last_frame(window_id)
        .map(SceneSummary::from_frame);

    Ok(WindowSnapshot {
        window_id,
        title,
        accessibility,
        widget_graph,
        focus_state,
        scene_summary,
    })
}

fn next_headless_wakeup(harness: &HeadlessHarness) -> Result<Option<f64>> {
    let mut next: Option<f64> = None;
    for window_id in harness.runtime.window_ids() {
        let candidate = harness.runtime.next_wakeup_time(window_id)?;
        next = match (next, candidate) {
            (Some(current), Some(candidate)) => Some(current.min(candidate)),
            (None, Some(candidate)) => Some(candidate),
            (current, None) => current,
        };
    }
    Ok(next)
}

fn harness_service() -> &'static HarnessService {
    HARNESS_SERVICE.get_or_init(|| {
        let (setup_tx, setup_rx) = mpsc::sync_channel(1);

        thread::spawn(move || {
            let mut event_loop_builder = EventLoop::<HarnessCommand>::with_user_event();
            #[cfg(target_os = "windows")]
            {
                use winit::platform::windows::EventLoopBuilderExtWindows;

                event_loop_builder.with_any_thread(true);
            }

            let event_loop = match event_loop_builder.build() {
                Ok(event_loop) => event_loop,
                Err(error) => {
                    let _ = setup_tx.send(Err(map_event_loop_error(error)));
                    return;
                }
            };

            if setup_tx.send(Ok(event_loop.create_proxy())).is_err() {
                return;
            }

            let mut app = LiveHarnessApp::new();
            if let Err(error) = event_loop.run_app(&mut app) {
                app.report_error(map_event_loop_error(error));
            }
        });

        let proxy = recv_result(
            &setup_rx,
            "live harness service setup",
            Duration::from_secs(5),
        )
        .expect("live harness service should start exactly once");

        HarnessService { proxy }
    })
}

fn recv_result<T>(receiver: &Receiver<Result<T>>, label: &str, timeout: Duration) -> Result<T> {
    receiver
        .recv_timeout(timeout)
        .map_err(|error| Error::new(format!("timed out waiting for {label}: {error}")))?
}

fn physical_size_to_logical_size(size: PhysicalSize<u32>, scale_factor: f64) -> sui_core::Size {
    let logical = size.to_logical::<f32>(scale_factor);
    sui_core::Size::new(logical.width, logical.height)
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

fn map_runtime_event_to_host_inputs(event: Event) -> Result<Vec<HostInputEvent>> {
    match event {
        Event::Pointer(pointer) => {
            let mut events = Vec::new();
            match pointer.kind {
                PointerEventKind::Enter => events.push(HostInputEvent::CursorEntered),
                PointerEventKind::Leave => events.push(HostInputEvent::CursorLeft),
                PointerEventKind::Move => {
                    events.push(HostInputEvent::CursorEntered);
                    events.push(HostInputEvent::CursorMoved {
                        position: pointer.position,
                    });
                }
                PointerEventKind::Down | PointerEventKind::Up => {
                    events.push(HostInputEvent::CursorEntered);
                    events.push(HostInputEvent::CursorMoved {
                        position: pointer.position,
                    });
                    let button = pointer.button.ok_or_else(|| {
                        Error::new("pointer button event is missing button information")
                    })?;
                    events.push(HostInputEvent::MouseInput {
                        pressed: pointer.kind == PointerEventKind::Down,
                        button,
                    });
                }
                PointerEventKind::Scroll => {
                    events.push(HostInputEvent::CursorEntered);
                    events.push(HostInputEvent::CursorMoved {
                        position: pointer.position,
                    });
                    events.push(HostInputEvent::MouseWheel {
                        delta: pointer
                            .scroll_delta
                            .unwrap_or(ScrollDelta::Pixels(pointer.delta)),
                    });
                }
                PointerEventKind::Cancel => events.push(HostInputEvent::CursorLeft),
            }
            Ok(events)
        }
        Event::Keyboard(event) => Ok(vec![HostInputEvent::Keyboard {
            key: event.key,
            code: event.code,
            text: event.text,
            state: event.state,
            repeat: event.repeat,
            modifiers: event.modifiers,
        }]),
        Event::Ime(event) => Ok(vec![HostInputEvent::Ime(event)]),
        Event::Window(WindowEvent::Focused(focused)) => Ok(vec![HostInputEvent::Focused(focused)]),
        Event::Window(WindowEvent::Resized(size)) => Ok(vec![HostInputEvent::Resized { size }]),
        Event::Window(WindowEvent::RedrawRequested) => Ok(vec![HostInputEvent::RedrawRequested]),
        other => Err(Error::new(format!(
            "live harness does not support dispatching {other:?}"
        ))),
    }
}

fn publish_frame_performance(
    window_id: WindowId,
    frame_index: u64,
    event_time_ms: f64,
    redraw_time_ms: f64,
    runtime_time_ms: f64,
    presentation_latency: PresentationLatencyDiagnostics,
    output: &RenderOutput,
    renderer: &WgpuRenderer,
    renderer_time_ms: f64,
) {
    let detail_mode = window_scene_statistics_detail_mode(window_id);
    let total_time_ms = event_time_ms + redraw_time_ms + runtime_time_ms + renderer_time_ms;

    if !detail_mode.is_detailed() {
        publish_window_performance_snapshot(
            WindowPerformanceSnapshot::with_total_time_ms(
                window_id,
                frame_index,
                total_time_ms,
                Vec::new(),
                RendererSubmissionDiagnostics::default(),
                TextCacheDiagnostics::default(),
                Default::default(),
                SceneStatistics::minimal(&output.frame, detail_mode),
            )
            .with_presentation_latency(presentation_latency)
            .with_runtime_text_timing(output.diagnostics.runtime_text_timing)
            .with_widget_timings(output.diagnostics.widget_timings.clone()),
        );
        return;
    }

    let diagnostics_started = Instant::now();
    let mut phase_timings = Vec::with_capacity(output.diagnostics.phase_timings.len() + 3);
    let renderer_text_cache = renderer.text_cache_snapshot(window_id);
    let text_caches = TextCacheDiagnostics {
        runtime_layout: output.diagnostics.text_caches.runtime_layout,
        renderer_layout: CacheMetrics::new(
            renderer_text_cache.layout.entries,
            renderer_text_cache.layout.hits,
            renderer_text_cache.layout.misses,
        ),
        renderer_glyph: CacheMetrics::new(
            renderer_text_cache.glyph.entries,
            renderer_text_cache.glyph.hits,
            renderer_text_cache.glyph.misses,
        ),
        renderer_path: CacheMetrics::new(
            renderer_text_cache.path.entries,
            renderer_text_cache.path.hits,
            renderer_text_cache.path.misses,
        ),
    };
    let text_cache_deltas = window_performance_text_caches(window_id)
        .map(|previous| text_caches.delta_from(&previous))
        .unwrap_or_else(|| text_caches.delta_from(&TextCacheDiagnostics::default()));
    let renderer_stats = renderer.last_frame_stats(window_id).unwrap_or_default();

    if event_time_ms > 0.0 {
        phase_timings.push(FramePhaseSample::new(FramePhase::Event, event_time_ms));
    }

    if redraw_time_ms > 0.0 {
        phase_timings.push(FramePhaseSample::new(FramePhase::Redraw, redraw_time_ms));
    }

    phase_timings.extend(output.diagnostics.phase_timings.iter().copied());
    phase_timings.push(FramePhaseSample::new(
        FramePhase::Renderer,
        renderer_time_ms,
    ));
    phase_timings.push(FramePhaseSample::new(
        FramePhase::Diagnostics,
        diagnostics_started.elapsed().as_secs_f64() * 1000.0,
    ));

    let total_time_ms = event_time_ms
        + redraw_time_ms
        + runtime_time_ms
        + renderer_time_ms
        + diagnostics_started.elapsed().as_secs_f64() * 1000.0;

    publish_window_performance_snapshot(
        WindowPerformanceSnapshot::with_total_time_ms(
            window_id,
            frame_index,
            total_time_ms,
            phase_timings,
            RendererSubmissionDiagnostics::new(
                renderer_stats.pass_count,
                renderer_stats.draw_count,
                renderer_stats.uploaded_vertex_bytes,
                renderer_stats.text_glyph_instance_count,
                renderer_stats.text_vertex_bytes,
                renderer_stats.visible_layer_count,
                renderer_stats.direct_packet_count,
                renderer_stats.retained_state_update_time_us,
                renderer_stats.composition_time_us,
                renderer_stats.retained_scene_traversal_time_us,
                renderer_stats.retained_packet_build_time_us,
                renderer_stats.retained_packet_build_count,
                renderer_stats.retained_packet_rebuild_new_count,
                renderer_stats.retained_packet_rebuild_coordinate_space_count,
                renderer_stats.retained_packet_rebuild_signature_count,
                renderer_stats.retained_packet_rebuild_scene_count,
                renderer_stats.retained_packet_rebuild_state_count,
                renderer_stats.text_atlas_miss_count,
                renderer_stats.text_atlas_miss_time_us,
                renderer_stats.surface_acquire_time_us,
                renderer_stats.resource_collection_time_us,
                renderer_stats.bind_group_prepare_time_us,
                renderer_stats.image_bind_group_time_us,
                renderer_stats.analytic_path_bind_group_time_us,
                renderer_stats.analytic_path_bind_group_miss_count,
                renderer_stats.analytic_path_bind_group_upload_bytes,
                renderer_stats.text_atlas_bind_group_time_us,
                renderer_stats.text_atlas_upload_copy_time_us,
                renderer_stats.text_atlas_upload_write_time_us,
                renderer_stats.text_atlas_upload_bytes,
                renderer_stats.batch_prepare_time_us,
                renderer_stats.gpu_upload_time_us,
                renderer_stats.pass_encode_time_us,
                renderer_stats.queue_submit_time_us,
                renderer_stats.surface_present_time_us,
            )
            .with_retained_packet_breakdown(
                renderer_stats.retained_packet_normalize_time_us,
                renderer_stats.retained_packet_signature_time_us,
                renderer_stats.retained_packet_raster_state_init_time_us,
                renderer_stats.retained_packet_scene_build_time_us,
                renderer_stats.retained_packet_command_count,
                renderer_stats.retained_packet_text_command_count,
                renderer_stats.retained_packet_path_command_count,
                renderer_stats.retained_packet_clip_path_command_count,
                renderer_stats.retained_packet_image_command_count,
                renderer_stats.retained_packet_rect_command_count,
                renderer_stats.retained_packet_text_command_time_us,
                renderer_stats.retained_packet_path_command_time_us,
                renderer_stats.retained_packet_clip_path_command_time_us,
                renderer_stats.retained_packet_image_command_time_us,
                renderer_stats.retained_packet_rect_command_time_us,
            ),
            text_caches,
            text_cache_deltas,
            SceneStatistics::from_frame_with_mode(&output.frame, detail_mode),
        )
        .with_presentation_latency(presentation_latency)
        .with_runtime_text_timing(output.diagnostics.runtime_text_timing)
        .with_retained_packet_hotspot(renderer_stats.retained_packet_hotspot.clone().map(
            |hotspot| sui_runtime::RetainedPacketHotspotDiagnostics {
                container_layer_id: hotspot.container_layer_id,
                owner_widget_id: hotspot.owner_widget_id,
                segment_index: hotspot.segment_index,
                total_time_us: hotspot.total_time_us,
                scene_build_time_us: hotspot.scene_build_time_us,
                command_count: hotspot.command_count,
                text_command_count: hotspot.text_command_count,
                path_command_count: hotspot.path_command_count,
                rect_command_count: hotspot.rect_command_count,
                text_command_time_us: hotspot.text_command_time_us,
                path_command_time_us: hotspot.path_command_time_us,
                rect_command_time_us: hotspot.rect_command_time_us,
                text_sample: hotspot.text_sample,
            },
        ))
        .with_widget_timings(output.diagnostics.widget_timings.clone()),
    );
}

fn map_event_loop_error(error: EventLoopError) -> Error {
    Error::new(format!("winit event loop error: {error}"))
}

fn map_os_error(error: OsError) -> Error {
    Error::new(format!("failed to create live test window: {error}"))
}
