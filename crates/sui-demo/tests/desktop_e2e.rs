#![forbid(unsafe_code)]

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

use sui::{
    Alignment, App, Application, Background, Color, Error, Event, ImeEvent, Insets, Label,
    Modifiers, NumberInput, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
    PointerKind, RadioButton, RadioGroup, Rect, Result, SceneCommand, ScrollDelta, ScrollView,
    Select, SemanticsNode, SemanticsRole, SemanticsValue, Size, SizedBox, Slider, SplitView, Stack,
    Switch, Table, TableColumn, TableRow, TextArea, Vector, VirtualScrollView, WgpuRenderer,
    Window as SuiWindow, WindowBuilder, WindowEvent, WindowId, window_performance_snapshot,
};
use sui_demo_app::widget_book::{
    RETAINED_TEXT_BENCHMARK_SCROLL_NAME, RETAINED_TEXT_BENCHMARK_TITLE,
    TEXT_EDITING_BENCHMARK_EDITOR_NAME, TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME,
    TEXT_EDITING_BENCHMARK_TITLE, WidgetBookState, build_retained_text_benchmark_application,
    build_text_editing_benchmark_application, build_widget_book_application,
    build_widget_book_gallery, default_widget_book_state, register_widget_book_images,
};
use sui_platform::publish_frame_performance;
use sui_runtime::{
    PresentationLatencyDiagnostics, RenderOutput, RetainedPacketRebuildDiagnostics,
    SceneStatisticsDetailMode, WidgetTimingPhase, WindowPerformanceSnapshot,
    clear_window_performance_snapshots, set_window_scene_statistics_detail_mode,
    window_scene_statistics_detail_mode,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    error::{EventLoopError, OsError},
    event::{
        DeviceId, ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase,
        WindowEvent as WinitWindowEvent,
    },
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowAttributes, WindowId as HostWindowId},
};

const DEFAULT_WINDOW_SIZE: Size = Size::new(1280.0, 720.0);
const REDRAW_FLUSH_LIMIT: usize = 256;

static DESKTOP_TEST_LOCK: Mutex<()> = Mutex::new(());

fn desktop_display_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var_os("WAYLAND_SOCKET").is_some()
            || std::env::var_os("DISPLAY").is_some()
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn skip_without_desktop_display(test_name: &str) -> bool {
    if desktop_display_available() {
        return false;
    }

    eprintln!("skipping {test_name}: no desktop display server is available");
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone)]
struct DesktopWindowSnapshot {
    title: String,
    semantics: Vec<SemanticsNode>,
    performance: Option<WindowPerformanceSnapshot>,
}

#[derive(Debug, Clone, Copy)]
enum ScrollKind {
    Pixels(Vector),
}

#[derive(Debug, Clone)]
enum HostInputEvent {
    Focused(bool),
    CursorEntered,
    CursorMoved {
        position: Point,
    },
    MouseInput {
        state: ElementState,
        button: MouseButton,
    },
    MouseWheel {
        delta: ScrollKind,
    },
    ImeStart,
    ImePreedit {
        text: String,
        cursor_range: Option<(usize, usize)>,
    },
    ImeCommit {
        text: String,
    },
    ImeEnd,
}

enum HarnessCommand {
    Launch {
        build_runtime: RuntimeBuilder,
        vsync_enabled: bool,
        reply: SyncSender<Result<WindowId>>,
    },
    Dispatch {
        window_id: WindowId,
        event: HostInputEvent,
        reply: SyncSender<Result<()>>,
    },
    Snapshot {
        window_id: WindowId,
        reply: SyncSender<Result<DesktopWindowSnapshot>>,
    },
    Capture {
        window_id: WindowId,
        reply: SyncSender<Result<CapturedFrame>>,
    },
    Reset {
        reply: SyncSender<()>,
    },
}

type RuntimeBuilder = Box<dyn FnOnce() -> Result<sui::Runtime> + Send>;

struct DesktopHarnessService {
    proxy: EventLoopProxy<HarnessCommand>,
}

static DESKTOP_HARNESS_SERVICE: OnceLock<DesktopHarnessService> = OnceLock::new();

fn desktop_harness_service() -> &'static DesktopHarnessService {
    DESKTOP_HARNESS_SERVICE.get_or_init(|| {
        let (setup_tx, setup_rx) = mpsc::sync_channel(1);

        thread::spawn(move || {
            let mut event_loop_builder = EventLoop::<HarnessCommand>::with_user_event();
            #[cfg(target_os = "windows")]
            {
                use winit::platform::windows::EventLoopBuilderExtWindows;

                event_loop_builder.with_any_thread(true);
            }
            #[cfg(target_os = "linux")]
            {
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                use winit::platform::x11::EventLoopBuilderExtX11;

                EventLoopBuilderExtWayland::with_any_thread(&mut event_loop_builder, true);
                EventLoopBuilderExtX11::with_any_thread(&mut event_loop_builder, true);
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

            let mut app = DesktopHarnessApp::new();
            if let Err(error) = event_loop.run_app(&mut app) {
                app.report_error(map_event_loop_error(error));
            }
        });

        let proxy = recv_result(
            &setup_rx,
            "desktop harness service setup",
            Duration::from_secs(3),
        )
        .expect("desktop harness service should start exactly once");

        DesktopHarnessService { proxy }
    })
}

struct DesktopHarness {
    proxy: EventLoopProxy<HarnessCommand>,
    main_window_id: WindowId,
}

impl DesktopHarness {
    fn launch<F>(build_runtime: F) -> Result<Self>
    where
        F: FnOnce() -> Result<sui::Runtime> + Send + 'static,
    {
        Self::launch_with_vsync(build_runtime, true)
    }

    fn launch_with_vsync<F>(build_runtime: F, vsync_enabled: bool) -> Result<Self>
    where
        F: FnOnce() -> Result<sui::Runtime> + Send + 'static,
    {
        let proxy = desktop_harness_service().proxy.clone();
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        proxy
            .send_event(HarnessCommand::Launch {
                build_runtime: Box::new(build_runtime),
                vsync_enabled,
                reply: reply_tx,
            })
            .map_err(|_| Error::new("desktop harness service is unavailable"))?;
        let main_window_id =
            recv_result(&reply_rx, "desktop harness launch", Duration::from_secs(3))?;

        Ok(Self {
            proxy,
            main_window_id,
        })
    }

    fn main_window_id(&self) -> WindowId {
        self.main_window_id
    }

    fn dispatch(&self, window_id: WindowId, event: HostInputEvent) -> Result<()> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_command(HarnessCommand::Dispatch {
            window_id,
            event,
            reply: reply_tx,
        })?;
        recv_result(&reply_rx, "desktop event dispatch", Duration::from_secs(3))
    }

    fn snapshot(&self, window_id: WindowId) -> Result<DesktopWindowSnapshot> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_command(HarnessCommand::Snapshot {
            window_id,
            reply: reply_tx,
        })?;
        recv_result(&reply_rx, "desktop snapshot", Duration::from_secs(3))
    }

    fn capture(&self, window_id: WindowId) -> Result<CapturedFrame> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.send_command(HarnessCommand::Capture {
            window_id,
            reply: reply_tx,
        })?;
        recv_result(&reply_rx, "desktop capture", Duration::from_secs(3))
    }

    fn send_command(&self, command: HarnessCommand) -> Result<()> {
        self.proxy.send_event(command).map_err(|_| {
            Error::new("desktop harness event loop is closed before the command could be delivered")
        })
    }
}

impl Drop for DesktopHarness {
    fn drop(&mut self) {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        let _ = self
            .proxy
            .send_event(HarnessCommand::Reset { reply: reply_tx });
        let _ = reply_rx.recv_timeout(Duration::from_secs(1));
    }
}

struct DesktopHarnessApp {
    runtime: sui::Runtime,
    renderer: WgpuRenderer,
    vsync_enabled: bool,
    started_at: Instant,
    frame_clock: f64,
    windows: HashMap<WindowId, WindowState>,
    host_to_runtime: HashMap<HostWindowId, WindowId>,
    last_error: Option<Error>,
}

impl DesktopHarnessApp {
    fn new() -> Self {
        Self {
            runtime: sui::Runtime::new(),
            renderer: WgpuRenderer::default(),
            vsync_enabled: true,
            started_at: Instant::now(),
            frame_clock: 0.0,
            windows: HashMap::new(),
            host_to_runtime: HashMap::new(),
            last_error: None,
        }
    }

    fn reset_runtime_state(&mut self) {
        for window_id in self.windows.keys().copied().collect::<Vec<_>>() {
            self.renderer.remove_window(window_id);
        }
        self.windows.clear();
        self.host_to_runtime.clear();
        self.runtime = sui::Runtime::new();
        self.renderer = WgpuRenderer::default().with_vsync_enabled(self.vsync_enabled);
        self.started_at = Instant::now();
        self.frame_clock = 0.0;
        clear_window_performance_snapshots();
    }

    fn report_error(&mut self, error: Error) {
        self.last_error = Some(error);
        self.reset_runtime_state();
    }

    fn launch_runtime(
        &mut self,
        event_loop: &ActiveEventLoop,
        build_runtime: RuntimeBuilder,
        vsync_enabled: bool,
    ) -> Result<WindowId> {
        self.vsync_enabled = vsync_enabled;
        self.reset_runtime_state();
        self.last_error = None;
        self.runtime = build_runtime()?;
        self.flush_pending_frames(event_loop)?;
        self.runtime
            .window_ids()
            .first()
            .copied()
            .ok_or_else(|| Error::new("desktop runtime did not create any windows"))
    }

    fn take_last_error(&mut self) -> Result<()> {
        if let Some(error) = self.last_error.take() {
            Err(error)
        } else {
            Ok(())
        }
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
                            .with_visible(false)
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
                WindowState {
                    title,
                    redraw_requested: false,
                    redraw_requested_at_ms: None,
                    frame_index: 0,
                    pending_event_time_ms: 0.0,
                    last_non_redraw_event_at_ms: None,
                    semantics: Vec::new(),
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
            "desktop harness exceeded the redraw flush budget; likely stuck in a redraw loop",
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
                self.renderer.render(&output.frame)?;
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
                    window.semantics = output.semantics.clone();

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

    fn dispatch_host_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: HostInputEvent,
    ) -> Result<()> {
        let host_id = self.host_id(window_id)?;
        let winit_event = self.map_host_event(window_id, event)?;
        self.handle_window_event(event_loop, host_id, winit_event)?;
        self.flush_pending_frames(event_loop)
    }

    fn map_host_event(
        &self,
        window_id: WindowId,
        event: HostInputEvent,
    ) -> Result<WinitWindowEvent> {
        let window = self.windows.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} is not registered in the desktop harness",
                window_id.get()
            ))
        })?;
        let scale_factor = window.scale_factor;
        let device_id = DeviceId::dummy();

        let event = match event {
            HostInputEvent::Focused(focused) => WinitWindowEvent::Focused(focused),
            HostInputEvent::CursorEntered => WinitWindowEvent::CursorEntered { device_id },
            HostInputEvent::CursorMoved { position } => WinitWindowEvent::CursorMoved {
                device_id,
                position: logical_point_to_physical_position(position, scale_factor),
            },
            HostInputEvent::MouseInput { state, button } => WinitWindowEvent::MouseInput {
                device_id,
                state,
                button,
            },
            HostInputEvent::MouseWheel { delta } => WinitWindowEvent::MouseWheel {
                device_id,
                delta: match delta {
                    ScrollKind::Pixels(delta) => MouseScrollDelta::PixelDelta(
                        logical_vector_to_physical_position(delta, scale_factor),
                    ),
                },
                phase: TouchPhase::Moved,
            },
            HostInputEvent::ImeStart => WinitWindowEvent::Ime(Ime::Enabled),
            HostInputEvent::ImePreedit { text, cursor_range } => {
                WinitWindowEvent::Ime(Ime::Preedit(text, cursor_range))
            }
            HostInputEvent::ImeCommit { text } => WinitWindowEvent::Ime(Ime::Commit(text)),
            HostInputEvent::ImeEnd => WinitWindowEvent::Ime(Ime::Disabled),
        };

        Ok(event)
    }

    fn host_id(&self, window_id: WindowId) -> Result<HostWindowId> {
        self.windows
            .get(&window_id)
            .map(|window| window.window.id())
            .ok_or_else(|| {
                Error::new(format!(
                    "window {} is not registered in the desktop harness",
                    window_id.get()
                ))
            })
    }

    fn snapshot(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
    ) -> Result<DesktopWindowSnapshot> {
        self.flush_pending_frames(event_loop)?;

        let window = self.windows.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} is not registered in the desktop harness",
                window_id.get()
            ))
        })?;

        Ok(DesktopWindowSnapshot {
            title: window.title.clone(),
            semantics: window.semantics.clone(),
            performance: window_performance_snapshot(window_id),
        })
    }

    fn capture(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
    ) -> Result<CapturedFrame> {
        self.flush_pending_frames(event_loop)?;

        let image = self.renderer.capture_last_frame_rgba(window_id)?;
        Ok(CapturedFrame {
            width: image.width(),
            height: image.height(),
            pixels: image.into_pixels(),
        })
    }

    fn handle_command(&mut self, event_loop: &ActiveEventLoop, command: HarnessCommand) {
        match command {
            HarnessCommand::Launch {
                build_runtime,
                vsync_enabled,
                reply,
            } => {
                let _ = reply.send(self.launch_runtime(event_loop, build_runtime, vsync_enabled));
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
            HarnessCommand::Reset { reply } => {
                self.reset_runtime_state();
                let _ = reply.send(());
            }
        }
    }
}

impl ApplicationHandler<HarnessCommand> for DesktopHarnessApp {
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

#[derive(Debug)]
struct WindowState {
    title: String,
    redraw_requested: bool,
    redraw_requested_at_ms: Option<f64>,
    frame_index: u64,
    pending_event_time_ms: f64,
    last_non_redraw_event_at_ms: Option<f64>,
    semantics: Vec<SemanticsNode>,
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

fn recv_result<T>(receiver: &Receiver<Result<T>>, label: &str, timeout: Duration) -> Result<T> {
    receiver
        .recv_timeout(timeout)
        .map_err(|error| Error::new(format!("timed out waiting for {label}: {error}")))?
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

fn logical_point_to_physical_position(position: Point, scale_factor: f64) -> PhysicalPosition<f64> {
    let logical = LogicalPosition::new(position.x as f64, position.y as f64);
    logical.to_physical(scale_factor)
}

fn logical_vector_to_physical_position(delta: Vector, scale_factor: f64) -> PhysicalPosition<f64> {
    let logical = LogicalPosition::new(delta.x as f64, delta.y as f64);
    logical.to_physical(scale_factor)
}

fn apply_ime_composition_rect(window: &Window, rect: Option<Rect>) {
    let cursor_area = rect.and_then(|rect| sanitize_ime_cursor_area(rect, window.scale_factor()));
    window.set_ime_allowed(cursor_area.is_some());

    if let Some((position, size)) = cursor_area {
        window.set_ime_cursor_area(position, size);
    }
}

fn sanitize_ime_cursor_area(
    rect: Rect,
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

const FRAME_CHANNEL_TOLERANCE: u8 = 1;

fn frame_pixels_match(before: &[u8], after: &[u8]) -> bool {
    before
        .iter()
        .zip(after.iter())
        .all(|(before, after)| before.abs_diff(*after) <= FRAME_CHANNEL_TOLERANCE)
}

fn frame_pixel_diff_count(before: &CapturedFrame, after: &CapturedFrame) -> usize {
    assert_eq!(
        (before.width, before.height),
        (after.width, after.height),
        "desktop framebuffer size changed unexpectedly during the test"
    );

    before
        .pixels
        .chunks_exact(4)
        .zip(after.pixels.chunks_exact(4))
        .filter(|(left, right)| !frame_pixels_match(left, right))
        .count()
}

fn frame_diff_bounds(before: &CapturedFrame, after: &CapturedFrame) -> Option<Rect> {
    assert_eq!((before.width, before.height), (after.width, after.height));

    let mut min_x = before.width;
    let mut min_y = before.height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut has_diff = false;

    for y in 0..before.height {
        for x in 0..before.width {
            let index = ((y * before.width + x) * 4) as usize;
            if !frame_pixels_match(
                &before.pixels[index..index + 4],
                &after.pixels[index..index + 4],
            ) {
                has_diff = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    has_diff.then(|| {
        Rect::new(
            min_x as f32,
            min_y as f32,
            (max_x - min_x + 1) as f32,
            (max_y - min_y + 1) as f32,
        )
    })
}

#[test]
fn frame_diff_helpers_tolerate_one_channel_value_per_channel() {
    let before = CapturedFrame {
        width: 2,
        height: 1,
        pixels: vec![10, 20, 30, 40, 100, 110, 120, 130],
    };
    let tolerated = CapturedFrame {
        width: 2,
        height: 1,
        pixels: vec![11, 19, 31, 39, 99, 111, 119, 131],
    };
    let different = CapturedFrame {
        width: 2,
        height: 1,
        pixels: vec![12, 20, 30, 40, 99, 111, 119, 131],
    };

    assert_eq!(frame_pixel_diff_count(&before, &tolerated), 0);
    assert_eq!(frame_diff_bounds(&before, &tolerated), None);
    assert_eq!(frame_pixel_diff_count(&before, &different), 1);
    assert_eq!(
        frame_diff_bounds(&before, &different),
        Some(Rect::new(0.0, 0.0, 1.0, 1.0))
    );
}

fn normalized_semantics_snapshot(nodes: &[SemanticsNode]) -> Vec<String> {
    let mut snapshot = nodes
        .iter()
        .map(|node| {
            format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}",
                node.role, node.name, node.value, node.bounds, node.state
            )
        })
        .collect::<Vec<_>>();
    snapshot.sort();
    snapshot
}

fn normalized_scene_snapshot(scene: &sui::Scene) -> Vec<String> {
    let mut snapshot = Vec::new();
    scene.visit_commands(&mut |command| match command {
        SceneCommand::Layer(layer) => snapshot.push(format!(
            "Layer|{:?}|{:?}|{:?}|{:?}",
            layer.descriptor.bounds,
            layer.descriptor.content_bounds,
            layer.descriptor.paint_bounds,
            layer.descriptor.composition_mode,
        )),
        SceneCommand::DrawShapedText(text) => snapshot.push(format!(
            "DrawShapedText|{:?}|{:?}",
            text.origin, text.bounds,
        )),
        SceneCommand::DrawShapedTextWindow(text) => snapshot.push(format!(
            "DrawShapedTextWindow|{:?}|{:?}|{}..{}",
            text.origin, text.bounds, text.line_range.start, text.line_range.end,
        )),
        other => snapshot.push(format!("{:?}", other)),
    });
    snapshot
}

fn normalized_layer_updates_snapshot(output: &RenderOutput) -> Vec<String> {
    let mut snapshot = output
        .frame
        .layer_updates
        .iter()
        .map(|update| {
            format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}",
                update.kind,
                update.bounds,
                update.content_bounds,
                update.paint_bounds,
                update.damage
            )
        })
        .collect::<Vec<_>>();
    snapshot.sort();
    snapshot
}

fn node_center(bounds: Rect) -> Point {
    Point::new(
        bounds.x() + (bounds.width() * 0.5),
        bounds.y() + (bounds.height() * 0.5),
    )
}

fn interpolate_point(start: Point, end: Point, t: f32) -> Point {
    Point::new(
        start.x + ((end.x - start.x) * t),
        start.y + ((end.y - start.y) * t),
    )
}

fn move_cursor(harness: &DesktopHarness, window_id: WindowId, position: Point) -> Result<()> {
    harness.dispatch(window_id, HostInputEvent::CursorEntered)?;
    harness.dispatch(window_id, HostInputEvent::CursorMoved { position })
}

fn click_at(harness: &DesktopHarness, window_id: WindowId, position: Point) -> Result<()> {
    move_cursor(harness, window_id, position)?;
    harness.dispatch(
        window_id,
        HostInputEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
        },
    )?;
    harness.dispatch(
        window_id,
        HostInputEvent::MouseInput {
            state: ElementState::Released,
            button: MouseButton::Left,
        },
    )
}

fn click_primary(harness: &DesktopHarness, window_id: WindowId) -> Result<()> {
    harness.dispatch(
        window_id,
        HostInputEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
        },
    )?;
    harness.dispatch(
        window_id,
        HostInputEvent::MouseInput {
            state: ElementState::Released,
            button: MouseButton::Left,
        },
    )
}

fn find_node(snapshot: &DesktopWindowSnapshot, role: SemanticsRole, name: &str) -> SemanticsNode {
    snapshot
        .semantics
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("missing semantics node {role:?} named {name}"))
}

fn find_node_optional(
    snapshot: &DesktopWindowSnapshot,
    role: SemanticsRole,
    name: &str,
) -> Option<SemanticsNode> {
    snapshot
        .semantics
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .cloned()
}

fn text_input_value(snapshot: &DesktopWindowSnapshot, name: &str) -> String {
    let input = find_node(snapshot, SemanticsRole::TextInput, name);
    match input.value {
        Some(SemanticsValue::Text(value)) => value,
        other => panic!("unexpected text input value for {name}: {other:?}"),
    }
}

#[derive(Debug, Clone)]
struct ScrollBenchmarkFrameSample {
    frame_index: u64,
    total_time_ms: f64,
    draw_count: usize,
    pass_count: usize,
    visible_layer_count: usize,
    direct_packet_count: usize,
    uploaded_vertex_bytes: u64,
    text_vertex_bytes: u64,
    text_glyph_instance_count: usize,
    retained_state_update_time_us: u64,
    composition_time_us: u64,
    retained_scene_traversal_time_us: u64,
    retained_packet_build_time_us: u64,
    retained_packet_build_count: usize,
    retained_packet_rebuilds: RetainedPacketRebuildDiagnostics,
    retained_packet_normalize_time_us: u64,
    retained_packet_signature_time_us: u64,
    retained_packet_raster_state_init_time_us: u64,
    retained_packet_scene_build_time_us: u64,
    retained_packet_command_count: usize,
    retained_packet_text_command_count: usize,
    retained_packet_path_command_count: usize,
    retained_packet_clip_path_command_count: usize,
    retained_packet_image_command_count: usize,
    retained_packet_rect_command_count: usize,
    retained_packet_text_command_time_us: u64,
    retained_packet_path_command_time_us: u64,
    retained_packet_clip_path_command_time_us: u64,
    retained_packet_image_command_time_us: u64,
    retained_packet_rect_command_time_us: u64,
    text_atlas_miss_count: usize,
    text_atlas_miss_time_us: u64,
    surface_acquire_time_us: u64,
    resource_collection_time_us: u64,
    bind_group_prepare_time_us: u64,
    image_bind_group_time_us: u64,
    analytic_path_bind_group_time_us: u64,
    analytic_path_bind_group_miss_count: usize,
    analytic_path_bind_group_upload_bytes: u64,
    text_atlas_bind_group_time_us: u64,
    text_atlas_upload_copy_time_us: u64,
    text_atlas_upload_write_time_us: u64,
    text_atlas_upload_bytes: u64,
    batch_prepare_time_us: u64,
    gpu_upload_time_us: u64,
    pass_encode_time_us: u64,
    queue_submit_time_us: u64,
    surface_present_time_us: u64,
    dirty_region_count: usize,
    dirty_coverage: f32,
    runtime_text_request_count: usize,
    runtime_text_hit_count: usize,
    runtime_text_miss_count: usize,
    runtime_text_total_time_us: u64,
    runtime_text_prelookup_time_us: u64,
    runtime_text_cache_lookup_time_us: u64,
    runtime_text_miss_layout_time_us: u64,
    runtime_layout_entries_delta: isize,
    runtime_layout_hits: usize,
    runtime_layout_misses: usize,
    glyph_cache_entries_delta: isize,
    glyph_cache_hits: usize,
    glyph_cache_misses: usize,
    path_cache_entries_delta: isize,
    path_cache_hits: usize,
    path_cache_misses: usize,
    packet_hotspot_layer_id: Option<u64>,
    packet_hotspot_owner_widget_id: Option<u64>,
    packet_hotspot_segment_index: u32,
    packet_hotspot_total_time_us: u64,
    packet_hotspot_scene_build_time_us: u64,
    packet_hotspot_command_count: usize,
    packet_hotspot_text_command_count: usize,
    packet_hotspot_path_command_count: usize,
    packet_hotspot_rect_command_count: usize,
    packet_hotspot_text_command_time_us: u64,
    packet_hotspot_path_command_time_us: u64,
    packet_hotspot_rect_command_time_us: u64,
    packet_hotspot_text_sample: Option<String>,
}

impl ScrollBenchmarkFrameSample {
    fn from_snapshot(snapshot: &WindowPerformanceSnapshot) -> Self {
        Self {
            frame_index: snapshot.frame_index,
            total_time_ms: snapshot.total_time_ms,
            draw_count: snapshot.renderer_submission.draw_count,
            pass_count: snapshot.renderer_submission.pass_count,
            visible_layer_count: snapshot.renderer_submission.visible_layer_count,
            direct_packet_count: snapshot.renderer_submission.direct_packet_count,
            uploaded_vertex_bytes: snapshot.renderer_submission.uploaded_vertex_bytes,
            text_vertex_bytes: snapshot.renderer_submission.text_vertex_bytes,
            text_glyph_instance_count: snapshot.renderer_submission.text_glyph_instance_count,
            retained_state_update_time_us: snapshot
                .renderer_submission
                .retained_state_update_time_us,
            composition_time_us: snapshot.renderer_submission.composition_time_us,
            retained_scene_traversal_time_us: snapshot
                .renderer_submission
                .retained_scene_traversal_time_us,
            retained_packet_build_time_us: snapshot
                .renderer_submission
                .retained_packet_build_time_us,
            retained_packet_build_count: snapshot.renderer_submission.retained_packet_build_count,
            retained_packet_rebuilds: snapshot.renderer_submission.retained_packet_rebuilds,
            retained_packet_normalize_time_us: snapshot
                .renderer_submission
                .retained_packet_normalize_time_us,
            retained_packet_signature_time_us: snapshot
                .renderer_submission
                .retained_packet_signature_time_us,
            retained_packet_raster_state_init_time_us: snapshot
                .renderer_submission
                .retained_packet_raster_state_init_time_us,
            retained_packet_scene_build_time_us: snapshot
                .renderer_submission
                .retained_packet_scene_build_time_us,
            retained_packet_command_count: snapshot
                .renderer_submission
                .retained_packet_command_count,
            retained_packet_text_command_count: snapshot
                .renderer_submission
                .retained_packet_text_command_count,
            retained_packet_path_command_count: snapshot
                .renderer_submission
                .retained_packet_path_command_count,
            retained_packet_clip_path_command_count: snapshot
                .renderer_submission
                .retained_packet_clip_path_command_count,
            retained_packet_image_command_count: snapshot
                .renderer_submission
                .retained_packet_image_command_count,
            retained_packet_rect_command_count: snapshot
                .renderer_submission
                .retained_packet_rect_command_count,
            retained_packet_text_command_time_us: snapshot
                .renderer_submission
                .retained_packet_text_command_time_us,
            retained_packet_path_command_time_us: snapshot
                .renderer_submission
                .retained_packet_path_command_time_us,
            retained_packet_clip_path_command_time_us: snapshot
                .renderer_submission
                .retained_packet_clip_path_command_time_us,
            retained_packet_image_command_time_us: snapshot
                .renderer_submission
                .retained_packet_image_command_time_us,
            retained_packet_rect_command_time_us: snapshot
                .renderer_submission
                .retained_packet_rect_command_time_us,
            text_atlas_miss_count: snapshot.renderer_submission.text_atlas_miss_count,
            text_atlas_miss_time_us: snapshot.renderer_submission.text_atlas_miss_time_us,
            surface_acquire_time_us: snapshot.renderer_submission.surface_acquire_time_us,
            resource_collection_time_us: snapshot.renderer_submission.resource_collection_time_us,
            bind_group_prepare_time_us: snapshot.renderer_submission.bind_group_prepare_time_us,
            image_bind_group_time_us: snapshot.renderer_submission.image_bind_group_time_us,
            analytic_path_bind_group_time_us: snapshot
                .renderer_submission
                .analytic_path_bind_group_time_us,
            analytic_path_bind_group_miss_count: snapshot
                .renderer_submission
                .analytic_path_bind_group_miss_count,
            analytic_path_bind_group_upload_bytes: snapshot
                .renderer_submission
                .analytic_path_bind_group_upload_bytes,
            text_atlas_bind_group_time_us: snapshot
                .renderer_submission
                .text_atlas_bind_group_time_us,
            text_atlas_upload_copy_time_us: snapshot
                .renderer_submission
                .text_atlas_upload_copy_time_us,
            text_atlas_upload_write_time_us: snapshot
                .renderer_submission
                .text_atlas_upload_write_time_us,
            text_atlas_upload_bytes: snapshot.renderer_submission.text_atlas_upload_bytes,
            batch_prepare_time_us: snapshot.renderer_submission.batch_prepare_time_us,
            gpu_upload_time_us: snapshot.renderer_submission.gpu_upload_time_us,
            pass_encode_time_us: snapshot.renderer_submission.pass_encode_time_us,
            queue_submit_time_us: snapshot.renderer_submission.queue_submit_time_us,
            surface_present_time_us: snapshot.renderer_submission.surface_present_time_us,
            dirty_region_count: snapshot.scene.dirty_region_count,
            dirty_coverage: snapshot.scene.dirty_coverage,
            runtime_text_request_count: snapshot.runtime_text_timing.request_count,
            runtime_text_hit_count: snapshot.runtime_text_timing.cache_hit_count,
            runtime_text_miss_count: snapshot.runtime_text_timing.cache_miss_count,
            runtime_text_total_time_us: snapshot.runtime_text_timing.total_time_us,
            runtime_text_prelookup_time_us: snapshot.runtime_text_timing.prelookup_time_us,
            runtime_text_cache_lookup_time_us: snapshot.runtime_text_timing.cache_lookup_time_us,
            runtime_text_miss_layout_time_us: snapshot.runtime_text_timing.miss_layout_time_us,
            runtime_layout_entries_delta: snapshot.text_cache_deltas.runtime_layout.entries_delta,
            runtime_layout_hits: snapshot.text_cache_deltas.runtime_layout.hits,
            runtime_layout_misses: snapshot.text_cache_deltas.runtime_layout.misses,
            glyph_cache_entries_delta: snapshot.text_cache_deltas.renderer_glyph.entries_delta,
            glyph_cache_hits: snapshot.text_cache_deltas.renderer_glyph.hits,
            glyph_cache_misses: snapshot.text_cache_deltas.renderer_glyph.misses,
            path_cache_entries_delta: snapshot.text_cache_deltas.renderer_path.entries_delta,
            path_cache_hits: snapshot.text_cache_deltas.renderer_path.hits,
            path_cache_misses: snapshot.text_cache_deltas.renderer_path.misses,
            packet_hotspot_layer_id: snapshot
                .retained_packet_hotspot
                .as_ref()
                .and_then(|hotspot| hotspot.container_layer_id),
            packet_hotspot_owner_widget_id: snapshot
                .retained_packet_hotspot
                .as_ref()
                .and_then(|hotspot| hotspot.owner_widget_id),
            packet_hotspot_segment_index: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.segment_index)
                .unwrap_or(0),
            packet_hotspot_total_time_us: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.total_time_us)
                .unwrap_or(0),
            packet_hotspot_scene_build_time_us: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.scene_build_time_us)
                .unwrap_or(0),
            packet_hotspot_command_count: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.command_count)
                .unwrap_or(0),
            packet_hotspot_text_command_count: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.text_command_count)
                .unwrap_or(0),
            packet_hotspot_path_command_count: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.path_command_count)
                .unwrap_or(0),
            packet_hotspot_rect_command_count: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.rect_command_count)
                .unwrap_or(0),
            packet_hotspot_text_command_time_us: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.text_command_time_us)
                .unwrap_or(0),
            packet_hotspot_path_command_time_us: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.path_command_time_us)
                .unwrap_or(0),
            packet_hotspot_rect_command_time_us: snapshot
                .retained_packet_hotspot
                .as_ref()
                .map(|hotspot| hotspot.rect_command_time_us)
                .unwrap_or(0),
            packet_hotspot_text_sample: snapshot
                .retained_packet_hotspot
                .as_ref()
                .and_then(|hotspot| hotspot.text_sample.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogRepaintTransition {
    Open,
    Close,
}

impl DialogRepaintTransition {
    const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Close => "close",
        }
    }

    const fn dialog_should_be_visible(self) -> bool {
        matches!(self, Self::Open)
    }
}

#[derive(Debug, Clone)]
struct DialogRepaintFrameSample {
    transition: DialogRepaintTransition,
    sample: ScrollBenchmarkFrameSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WidgetTimingAggregateKey {
    widget_id: u64,
    widget_name: &'static str,
    phase: WidgetTimingPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct WidgetTimingAggregate {
    total_ms: f64,
    total_calls: usize,
}

fn short_widget_name(widget_name: &'static str) -> &'static str {
    widget_name.rsplit("::").next().unwrap_or(widget_name)
}

fn visible_node_center(node: &SemanticsNode, viewport: Rect) -> Point {
    node_center(node.bounds.intersection(viewport).unwrap_or(node.bounds))
}

fn scroll_gallery_until_visible(
    harness: &DesktopHarness,
    window_id: WindowId,
    role: SemanticsRole,
    name: &str,
) -> Result<(DesktopWindowSnapshot, SemanticsNode, Rect)> {
    const SCROLL_STEP_PX: f32 = -160.0;
    const MAX_SCROLL_STEPS: usize = 96;

    let mut snapshot = harness.snapshot(window_id)?;
    let role_label = format!("{role:?}");
    let gallery = find_node(
        &snapshot,
        SemanticsRole::ScrollView,
        sui_demo_app::widget_book::GALLERY_SCROLL_NAME,
    );
    let gallery_bounds = gallery.bounds;
    let scroll_point = gallery_scroll_point(gallery_bounds);

    move_cursor(harness, window_id, scroll_point)?;

    for _ in 0..MAX_SCROLL_STEPS {
        if let Some(node) = find_node_optional(&snapshot, role.clone(), name)
            .filter(|node| node.bounds.intersection(gallery_bounds).is_some())
        {
            return Ok((snapshot, node, gallery_bounds));
        }

        harness.dispatch(
            window_id,
            HostInputEvent::MouseWheel {
                delta: ScrollKind::Pixels(Vector::new(0.0, SCROLL_STEP_PX)),
            },
        )?;
        snapshot = harness.snapshot(window_id)?;
    }

    Err(Error::new(format!(
        "failed to bring semantics node {role_label} named {name} into the widget-book gallery viewport"
    )))
}

fn run_widget_book_dialog_repaint_benchmark(
    build_runtime: impl FnOnce() -> Application + Send + 'static,
) -> Result<()> {
    const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;
    const WARMUP_CYCLES: usize = 2;
    const MEASURED_CYCLES: usize = 48;

    let harness = DesktopHarness::launch_with_vsync(|| build_runtime().build(), false)?;
    let window_id = harness.main_window_id();

    set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);
    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let (initial_snapshot, dialog_trigger, gallery_bounds) = scroll_gallery_until_visible(
        &harness,
        window_id,
        SemanticsRole::Button,
        sui_demo_app::widget_book::DIALOG_TRIGGER_LABEL,
    )?;
    let mut previous_frame_index = initial_snapshot
        .performance
        .as_ref()
        .expect("widget-book repaint benchmark should publish an initial performance snapshot")
        .frame_index;

    assert!(
        find_node_optional(
            &initial_snapshot,
            SemanticsRole::Dialog,
            sui_demo_app::widget_book::DIALOG_TITLE,
        )
        .is_none(),
        "project settings dialog should start closed",
    );

    move_cursor(
        &harness,
        window_id,
        visible_node_center(&dialog_trigger, gallery_bounds),
    )?;

    let benchmark_start = Instant::now();
    let mut phase_totals: HashMap<&str, f64> = HashMap::new();
    let mut widget_timing_totals: HashMap<WidgetTimingAggregateKey, WidgetTimingAggregate> =
        HashMap::new();
    let mut measured_samples = Vec::with_capacity(MEASURED_CYCLES * 2);
    let mut repaint_diff_checks = 0usize;

    for cycle in 0..(WARMUP_CYCLES + MEASURED_CYCLES) {
        for transition in [
            DialogRepaintTransition::Open,
            DialogRepaintTransition::Close,
        ] {
            let before_frame = (cycle >= WARMUP_CYCLES && repaint_diff_checks < 2)
                .then(|| harness.capture(window_id))
                .transpose()?;

            click_primary(&harness, window_id)?;

            let after_snapshot = harness.snapshot(window_id)?;
            let performance = after_snapshot
                .performance
                .as_ref()
                .expect("widget-book repaint benchmark should publish performance snapshots");

            assert!(
                performance.frame_index > previous_frame_index,
                "dialog repaint benchmark did not render a new frame for cycle {} transition {}",
                cycle,
                transition.label(),
            );
            previous_frame_index = performance.frame_index;

            let dialog_visible = find_node_optional(
                &after_snapshot,
                SemanticsRole::Dialog,
                sui_demo_app::widget_book::DIALOG_TITLE,
            )
            .is_some();
            assert_eq!(
                dialog_visible,
                transition.dialog_should_be_visible(),
                "dialog visibility mismatch after {} transition in cycle {}",
                transition.label(),
                cycle,
            );

            if let Some(before_frame) = before_frame {
                let after_frame = harness.capture(window_id)?;
                assert!(
                    frame_pixel_diff_count(&before_frame, &after_frame) > 0,
                    "dialog repaint benchmark {} transition did not change any rendered pixels",
                    transition.label(),
                );
                repaint_diff_checks += 1;
            }

            if cycle >= WARMUP_CYCLES {
                measured_samples.push(DialogRepaintFrameSample {
                    transition,
                    sample: ScrollBenchmarkFrameSample::from_snapshot(performance),
                });
                for timing in &performance.widget_timings {
                    let entry = widget_timing_totals
                        .entry(WidgetTimingAggregateKey {
                            widget_id: timing.widget_id.get(),
                            widget_name: timing.widget_name,
                            phase: timing.phase,
                        })
                        .or_default();
                    entry.total_ms += timing.duration_ms;
                    entry.total_calls += timing.calls;
                }
                for phase in &performance.phase_timings {
                    *phase_totals.entry(phase.phase.label()).or_insert(0.0) += phase.duration_ms;
                }
            }
        }
    }

    let benchmark_elapsed_ms = benchmark_start.elapsed().as_secs_f64() * 1000.0;
    let valid_count = measured_samples.len();
    assert!(valid_count > 0, "expected measured repaint frames");

    let frame_times_ms: Vec<_> = measured_samples
        .iter()
        .map(|sample| sample.sample.total_time_ms)
        .collect();
    let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
    let avg_ms = total_frame_time_ms / valid_count as f64;
    let min_ms = frame_times_ms
        .iter()
        .copied()
        .min_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);
    let max_ms = frame_times_ms
        .iter()
        .copied()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);
    let mut sorted_times = frame_times_ms.clone();
    sorted_times.sort_by(|a, b| a.total_cmp(b));
    let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
    let p95_ms = sorted_times[p95_index];

    let average_of = |project: fn(&ScrollBenchmarkFrameSample) -> f64| {
        measured_samples
            .iter()
            .map(|sample| project(&sample.sample))
            .sum::<f64>()
            / valid_count as f64
    };

    let avg_draws = average_of(|sample| sample.draw_count as f64);
    let avg_passes = average_of(|sample| sample.pass_count as f64);
    let avg_visible_layers = average_of(|sample| sample.visible_layer_count as f64);
    let avg_packet_rebuilds =
        average_of(|sample| sample.retained_packet_rebuilds.total_count() as f64);
    let avg_uploaded_vertex_bytes = average_of(|sample| sample.uploaded_vertex_bytes as f64);
    let avg_text_vertex_bytes = average_of(|sample| sample.text_vertex_bytes as f64);
    let avg_dirty_regions = average_of(|sample| sample.dirty_region_count as f64);
    let avg_dirty_coverage = average_of(|sample| sample.dirty_coverage as f64);
    let avg_state_update_ms =
        average_of(|sample| sample.retained_state_update_time_us as f64 / 1000.0);
    let avg_composition_ms = average_of(|sample| sample.composition_time_us as f64 / 1000.0);
    let avg_retained_scene_traversal_ms =
        average_of(|sample| sample.retained_scene_traversal_time_us as f64 / 1000.0);
    let avg_retained_packet_build_ms =
        average_of(|sample| sample.retained_packet_build_time_us as f64 / 1000.0);
    let avg_retained_packet_build_count =
        average_of(|sample| sample.retained_packet_build_count as f64);
    let avg_packet_rebuild_new =
        average_of(|sample| sample.retained_packet_rebuilds.new_count as f64);
    let avg_packet_rebuild_coordinate_space =
        average_of(|sample| sample.retained_packet_rebuilds.coordinate_space_count as f64);
    let avg_packet_rebuild_signature =
        average_of(|sample| sample.retained_packet_rebuilds.signature_count as f64);
    let avg_packet_rebuild_scene =
        average_of(|sample| sample.retained_packet_rebuilds.scene_count as f64);
    let avg_packet_rebuild_state =
        average_of(|sample| sample.retained_packet_rebuilds.state_count as f64);
    let avg_text_atlas_miss_count = average_of(|sample| sample.text_atlas_miss_count as f64);
    let avg_text_atlas_miss_ms =
        average_of(|sample| sample.text_atlas_miss_time_us as f64 / 1000.0);
    let avg_glyph_cache_entries_delta =
        average_of(|sample| sample.glyph_cache_entries_delta as f64);
    let avg_glyph_cache_hits = average_of(|sample| sample.glyph_cache_hits as f64);
    let avg_glyph_cache_misses = average_of(|sample| sample.glyph_cache_misses as f64);
    let avg_runtime_layout_entries_delta =
        average_of(|sample| sample.runtime_layout_entries_delta as f64);
    let avg_runtime_layout_hits = average_of(|sample| sample.runtime_layout_hits as f64);
    let avg_runtime_layout_misses = average_of(|sample| sample.runtime_layout_misses as f64);
    let avg_runtime_text_requests = average_of(|sample| sample.runtime_text_request_count as f64);
    let avg_runtime_text_hits = average_of(|sample| sample.runtime_text_hit_count as f64);
    let avg_runtime_text_misses = average_of(|sample| sample.runtime_text_miss_count as f64);
    let avg_runtime_text_total_ms =
        average_of(|sample| sample.runtime_text_total_time_us as f64 / 1000.0);
    let avg_runtime_text_prelookup_ms =
        average_of(|sample| sample.runtime_text_prelookup_time_us as f64 / 1000.0);
    let avg_runtime_text_lookup_ms =
        average_of(|sample| sample.runtime_text_cache_lookup_time_us as f64 / 1000.0);
    let avg_runtime_text_miss_layout_ms =
        average_of(|sample| sample.runtime_text_miss_layout_time_us as f64 / 1000.0);
    let avg_path_cache_entries_delta = average_of(|sample| sample.path_cache_entries_delta as f64);
    let avg_path_cache_hits = average_of(|sample| sample.path_cache_hits as f64);
    let avg_path_cache_misses = average_of(|sample| sample.path_cache_misses as f64);
    let avg_surface_acquire_ms =
        average_of(|sample| sample.surface_acquire_time_us as f64 / 1000.0);
    let avg_resource_collection_ms =
        average_of(|sample| sample.resource_collection_time_us as f64 / 1000.0);
    let avg_bind_group_prepare_ms =
        average_of(|sample| sample.bind_group_prepare_time_us as f64 / 1000.0);
    let avg_batch_prepare_ms = average_of(|sample| sample.batch_prepare_time_us as f64 / 1000.0);
    let avg_gpu_upload_ms = average_of(|sample| sample.gpu_upload_time_us as f64 / 1000.0);
    let avg_pass_encode_ms = average_of(|sample| sample.pass_encode_time_us as f64 / 1000.0);
    let avg_queue_submit_ms = average_of(|sample| sample.queue_submit_time_us as f64 / 1000.0);
    let avg_surface_present_ms =
        average_of(|sample| sample.surface_present_time_us as f64 / 1000.0);

    println!("\n=== Widget Book Dialog Repaint Benchmark ===");
    println!("scenario:         project settings preview open/close");
    println!("frames measured:  {valid_count}");
    println!("cycles measured:  {MEASURED_CYCLES}");
    println!("wall-clock time:  {benchmark_elapsed_ms:.1} ms");
    println!(
        "avg frame time:   {avg_ms:.3} ms ({:.0} fps)",
        1000.0 / avg_ms
    );
    println!("min frame time:   {min_ms:.3} ms");
    println!("max frame time:   {max_ms:.3} ms");
    println!(
        "p95 frame time:   {p95_ms:.3} ms ({:.0} fps)",
        1000.0 / p95_ms
    );
    println!("avg gpu passes:   {avg_passes:.2}");
    println!("avg gpu draws:    {avg_draws:.2}");
    println!("avg visible layers:{avg_visible_layers:.2}");
    println!("avg packet rebuilds:{avg_packet_rebuilds:.2}");
    println!("avg vertex bytes: {:.0}", avg_uploaded_vertex_bytes);
    println!("avg text bytes:   {:.0}", avg_text_vertex_bytes);
    println!("avg dirty regions:{avg_dirty_regions:.2}");
    println!("avg dirty cover:  {avg_dirty_coverage:.1}%");
    println!("avg state update: {avg_state_update_ms:.3} ms");
    println!("avg compose:      {avg_composition_ms:.3} ms");
    println!("avg traverse:     {avg_retained_scene_traversal_ms:.3} ms");
    println!(
        "avg packet build: {avg_retained_packet_build_ms:.3} ms ({avg_retained_packet_build_count:.2} packets)"
    );
    println!(
        "avg packet why:   new {avg_packet_rebuild_new:.2} | coord {avg_packet_rebuild_coordinate_space:.2} | sig {avg_packet_rebuild_signature:.2} | scene {avg_packet_rebuild_scene:.2} | state {avg_packet_rebuild_state:.2}"
    );
    println!(
        "avg runtime text Δ:{avg_runtime_layout_entries_delta:.2} entries / {avg_runtime_layout_hits:.2} hits / {avg_runtime_layout_misses:.2} misses"
    );
    println!(
        "avg runtime text:  {avg_runtime_text_requests:.2} req  hits {avg_runtime_text_hits:.2}  misses {avg_runtime_text_misses:.2}  total {avg_runtime_text_total_ms:.3} ms  pre {avg_runtime_text_prelookup_ms:.3} ms  lookup {avg_runtime_text_lookup_ms:.3} ms  miss-layout {avg_runtime_text_miss_layout_ms:.3} ms"
    );
    println!(
        "avg glyph cache Δ:{avg_glyph_cache_entries_delta:.2} entries / {avg_glyph_cache_hits:.2} hits / {avg_glyph_cache_misses:.2} misses"
    );
    println!(
        "avg path cache Δ: {avg_path_cache_entries_delta:.2} entries / {avg_path_cache_hits:.2} hits / {avg_path_cache_misses:.2} misses"
    );
    println!("avg atlas misses: {avg_text_atlas_miss_count:.2} / {avg_text_atlas_miss_ms:.3} ms");
    println!(
        "avg surface:      acq {avg_surface_acquire_ms:.3} ms  pres {avg_surface_present_ms:.3} ms"
    );
    println!(
        "avg prep:         scan {avg_resource_collection_ms:.3} ms  bind {avg_bind_group_prepare_ms:.3} ms  batch {avg_batch_prepare_ms:.3} ms"
    );
    println!(
        "avg submit path:  upload {avg_gpu_upload_ms:.3} ms  encode {avg_pass_encode_ms:.3} ms  submit {avg_queue_submit_ms:.3} ms"
    );

    println!("\n--- By transition ---");
    for transition in [
        DialogRepaintTransition::Open,
        DialogRepaintTransition::Close,
    ] {
        let transition_samples = measured_samples
            .iter()
            .filter(|sample| sample.transition == transition)
            .collect::<Vec<_>>();
        let transition_count = transition_samples.len();
        let transition_avg = transition_samples
            .iter()
            .map(|sample| sample.sample.total_time_ms)
            .sum::<f64>()
            / transition_count as f64;
        let transition_p95_index =
            ((transition_count as f64 * 0.95).ceil() as usize).min(transition_count - 1);
        let mut transition_times = transition_samples
            .iter()
            .map(|sample| sample.sample.total_time_ms)
            .collect::<Vec<_>>();
        transition_times.sort_by(|a, b| a.total_cmp(b));
        let transition_p95 = transition_times[transition_p95_index];
        let transition_avg_rebuilds = transition_samples
            .iter()
            .map(|sample| sample.sample.retained_packet_rebuilds.total_count() as f64)
            .sum::<f64>()
            / transition_count as f64;
        let transition_avg_packet_build = transition_samples
            .iter()
            .map(|sample| sample.sample.retained_packet_build_time_us as f64 / 1000.0)
            .sum::<f64>()
            / transition_count as f64;
        let transition_avg_state_update = transition_samples
            .iter()
            .map(|sample| sample.sample.retained_state_update_time_us as f64 / 1000.0)
            .sum::<f64>()
            / transition_count as f64;
        let transition_avg_glyph_misses = transition_samples
            .iter()
            .map(|sample| sample.sample.glyph_cache_misses as f64)
            .sum::<f64>()
            / transition_count as f64;
        println!(
            "  {:<5} avg {:>7.3} ms  p95 {:>7.3} ms  rebuild {:>5.2}  state {:>7.3} ms  packet {:>7.3} ms  glyph misses {:>5.2}",
            transition.label(),
            transition_avg,
            transition_p95,
            transition_avg_rebuilds,
            transition_avg_state_update,
            transition_avg_packet_build,
            transition_avg_glyph_misses,
        );
    }

    println!("\n--- Phase breakdown (avg per frame) ---");
    let mut phase_entries: Vec<_> = phase_totals.iter().collect();
    phase_entries.sort_by(|a, b| b.1.total_cmp(a.1));
    for (phase, total_ms) in &phase_entries {
        let avg_phase_ms = *total_ms / valid_count as f64;
        let pct = (*total_ms / total_frame_time_ms) * 100.0;
        println!("  {phase:<22} {avg_phase_ms:>8.3} ms  ({pct:>5.1}%)");
    }

    println!("\n--- Widget hotspots (avg per frame) ---");
    for phase in [
        WidgetTimingPhase::Measure,
        WidgetTimingPhase::Arrange,
        WidgetTimingPhase::Paint,
        WidgetTimingPhase::Semantics,
    ] {
        let mut entries = widget_timing_totals
            .iter()
            .filter(|(key, _)| key.phase == phase)
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| right.1.total_ms.total_cmp(&left.1.total_ms));
        if entries.is_empty() {
            continue;
        }

        println!("  {}:", phase.label());
        for (key, aggregate) in entries.into_iter().take(8) {
            println!(
                "    {:>7.3} ms/frame  {:>5.2} calls/frame  {}#{}",
                aggregate.total_ms / valid_count as f64,
                aggregate.total_calls as f64 / valid_count as f64,
                short_widget_name(key.widget_name),
                key.widget_id,
            );
        }
    }

    println!("\n--- Repeated measure calls ---");
    let mut repeated_measure_entries = widget_timing_totals
        .iter()
        .filter(|(key, aggregate)| {
            key.phase == WidgetTimingPhase::Measure && aggregate.total_calls > valid_count
        })
        .collect::<Vec<_>>();
    repeated_measure_entries.sort_by(|left, right| {
        let left_calls = left.1.total_calls as f64 / valid_count as f64;
        let right_calls = right.1.total_calls as f64 / valid_count as f64;
        right_calls
            .total_cmp(&left_calls)
            .then_with(|| right.1.total_ms.total_cmp(&left.1.total_ms))
    });
    if repeated_measure_entries.is_empty() {
        println!("  none above 1.00 calls/frame in the measured full-redraw frames");
    } else {
        for (key, aggregate) in repeated_measure_entries.into_iter().take(8) {
            println!(
                "  {:>5.2} calls/frame  {:>7.3} ms/frame  {}#{}",
                aggregate.total_calls as f64 / valid_count as f64,
                aggregate.total_ms / valid_count as f64,
                short_widget_name(key.widget_name),
                key.widget_id,
            );
        }
    }

    println!("\n--- Slowest frames ---");
    let mut slowest_samples = measured_samples.clone();
    slowest_samples.sort_by(|a, b| b.sample.total_time_ms.total_cmp(&a.sample.total_time_ms));
    for sample in slowest_samples.iter().take(6) {
        println!(
            "  frame {:>4}  {:<5} total {:>7.3} ms  rebuild {:>2}  upload {:>8}  text {:>8}  dirty {:>5.1}%",
            sample.sample.frame_index,
            sample.transition.label(),
            sample.sample.total_time_ms,
            sample.sample.retained_packet_rebuilds.total_count(),
            sample.sample.uploaded_vertex_bytes,
            sample.sample.text_vertex_bytes,
            sample.sample.dirty_coverage,
        );
        println!(
            "             state {:>7.3} ms  compose {:>7.3} ms  traverse {:>7.3} ms  packet {:>3} / {:>7.3} ms",
            sample.sample.retained_state_update_time_us as f64 / 1000.0,
            sample.sample.composition_time_us as f64 / 1000.0,
            sample.sample.retained_scene_traversal_time_us as f64 / 1000.0,
            sample.sample.retained_packet_build_count,
            sample.sample.retained_packet_build_time_us as f64 / 1000.0,
        );
        println!(
            "             packet why new {:>2} coord {:>2} sig {:>2} scene {:>2} state {:>2}",
            sample.sample.retained_packet_rebuilds.new_count,
            sample
                .sample
                .retained_packet_rebuilds
                .coordinate_space_count,
            sample.sample.retained_packet_rebuilds.signature_count,
            sample.sample.retained_packet_rebuilds.scene_count,
            sample.sample.retained_packet_rebuilds.state_count,
        );
        println!(
            "             text Δ {:+} / {:>3} hits / {:>3} misses  glyph Δ {:+} / {:>3} hits / {:>3} misses",
            sample.sample.runtime_layout_entries_delta,
            sample.sample.runtime_layout_hits,
            sample.sample.runtime_layout_misses,
            sample.sample.glyph_cache_entries_delta,
            sample.sample.glyph_cache_hits,
            sample.sample.glyph_cache_misses,
        );
        println!(
            "             path Δ {:+} / {:>3} hits / {:>3} misses  atlas {:>3} / {:>7.3} ms",
            sample.sample.path_cache_entries_delta,
            sample.sample.path_cache_hits,
            sample.sample.path_cache_misses,
            sample.sample.text_atlas_miss_count,
            sample.sample.text_atlas_miss_time_us as f64 / 1000.0,
        );
    }
    println!("========================================\n");

    assert!(
        measured_samples
            .iter()
            .all(|sample| sample.sample.glyph_cache_misses == 0),
        "expected glyph cache to stay warm during measured repaint frames",
    );
    assert!(
        measured_samples
            .iter()
            .all(|sample| sample.sample.text_atlas_miss_count == 0),
        "expected text atlas to stay warm during measured repaint frames",
    );
    assert!(
        avg_ms < FRAME_BUDGET_MS,
        "average repaint frame time {avg_ms:.3} ms exceeds the 16.67 ms budget for 60 fps",
    );

    Ok(())
}

fn gallery_scroll_point(bounds: Rect) -> Point {
    Point::new(bounds.max_x() - 40.0, bounds.y() + bounds.height() * 0.5)
}

fn build_scroll_history_repro_scroll(name: &str) -> impl sui::Widget {
    const RADIO_OPTIONS: [&str; 3] = ["Balanced", "High", "Fast"];
    const BLEND_MODES: [&str; 4] = ["Normal", "Multiply", "Screen", "Overlay"];

    let choices_panel = SizedBox::new()
        .width(420.0)
        .height(240.0)
        .with_child(Background::new(
            Color::rgba(0.97, 0.97, 0.98, 1.0),
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new("Choices and ranges")
                        .font_size(26.0)
                        .line_height(30.0),
                )
                .with_child(Switch::new("Enable snapping").on(true))
                .with_child(RadioButton::new("Standalone radio sample").selected(false))
                .with_child(
                    SizedBox::new().width(280.0).with_child(
                        RadioGroup::new("Quality")
                            .options(RADIO_OPTIONS)
                            .selected(0),
                    ),
                )
                .with_child(
                    SizedBox::new().width(320.0).with_child(
                        Slider::new("Blend strength")
                            .range(0.0, 100.0)
                            .step(1.0)
                            .value(72.0),
                    ),
                )
                .with_child(
                    SizedBox::new().width(220.0).with_child(
                        NumberInput::new("Sample count")
                            .range(1.0, 256.0)
                            .step(1.0)
                            .precision(0)
                            .value(12.0),
                    ),
                )
                .with_child(
                    SizedBox::new()
                        .width(260.0)
                        .with_child(Select::new("Blend mode").options(BLEND_MODES).selected(0)),
                ),
        ));
    let notes_panel = SizedBox::new().width(420.0).height(280.0).with_child(
        Background::new(
            Color::rgba(0.99, 0.99, 1.0, 1.0),
            Stack::vertical()
                .spacing(14.0)
                .alignment(Alignment::Stretch)
                .with_child(Label::new("Multiline and scroll").font_size(26.0).line_height(30.0))
                .with_child(
                    SizedBox::new().width(420.0).with_child(
                        TextArea::new("Notes")
                            .min_height(160.0)
                            .value(
                                "Pinned notes for inspector workflows.\nSupports multiline editing.\nUsed to reproduce scroll history artifacts.",
                            ),
                    ),
                ),
        ),
    );
    let summary_panel = SizedBox::new().width(420.0).height(220.0).with_child(
        Background::new(
            Color::rgba(0.96, 0.97, 0.99, 1.0),
            Stack::vertical()
                .spacing(12.0)
                .alignment(Alignment::Stretch)
                .with_child(Label::new("Summary").font_size(26.0).line_height(30.0))
                .with_child(Label::new(
                    "Equivalent final offsets should render the same frame, regardless of how many wheel ticks produced them.",
                )),
        ),
    );

    VirtualScrollView::new()
        .name(name)
        .padding(Insets::all(24.0))
        .spacing(18.0)
        .with_child(choices_panel)
        .with_child(notes_panel)
        .with_child(summary_panel)
        .with_child(SizedBox::new().width(420.0).height(220.0).with_child(
            Background::new(
                Color::rgba(0.95, 0.96, 0.98, 1.0),
                Stack::vertical()
                    .spacing(12.0)
                    .alignment(Alignment::Stretch)
                    .with_child(Label::new("Lower content").font_size(26.0).line_height(30.0))
                    .with_child(Label::new(
                        "Extra height keeps the visible range stable during the small scroll sequence.",
                    )),
            ),
        ))
}

fn build_scroll_history_repro_application() -> Application {
    Application::new().window(
        WindowBuilder::new().title("Scroll history repro").root(
            SizedBox::new()
                .size(Size::new(540.0, 360.0))
                .with_child(build_scroll_history_repro_scroll("History repro scroll")),
        ),
    )
}

fn scroll_benchmark_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
    Rc::new(RefCell::new(WidgetBookState {
        name: "Ada".to_string(),
        subscribed: true,
        theme_preview_comparison: false,
        button_presses: 0,
        icon_button_presses: 0,
        switch_on: true,
        standalone_radio_selected: false,
        radio_choice: "Balanced".to_string(),
        slider_value: 72.0,
        number_value: 12.0,
        notes: "Pinned notes for inspector workflows.\nSupports multiline editing.".to_string(),
        mode: "Normal".to_string(),
        tab_bar_choice: "Canvas".to_string(),
        tabs_choice: "Layout".to_string(),
        last_menu_action: String::new(),
        last_context_action: String::new(),
        dialog_apply_count: 0,
    }))
}

fn build_widget_book_gallery_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })
        .expect("widget-book image resources should be valid")
        .window(
            SuiWindow::new(sui_demo_app::widget_book::WINDOW_TITLE)
                .root(build_widget_book_gallery(state)),
        )
        .into_application()
}

fn build_widget_book_application_with_overlay(state: Rc<RefCell<WidgetBookState>>) -> Application {
    sui_demo_app::widget_book::set_widget_book_hdr_theme_mode(sui::HdrThemeMode::Disabled);

    App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })
        .expect("widget-book image resources should be valid")
        .window(
            SuiWindow::new(sui_demo_app::widget_book::WINDOW_TITLE).root(
                sui_demo_app::widget_book::LivePerformanceRoot::new(
                    sui_demo_app::widget_book::WINDOW_TITLE,
                    sui_demo_app::widget_book::WINDOW_DESCRIPTION,
                    build_widget_book_gallery(Rc::clone(&state)),
                )
                .show_performance_overlay()
                .watch_widget_book_state(state),
            ),
        )
        .into_application()
}

fn run_widget_book_scroll_benchmark(
    build_runtime: impl FnOnce() -> Application + Send + 'static,
) -> Result<()> {
    const SCROLL_STEP_PX: f32 = -40.0;
    const MAX_SCROLL_FRAMES: usize = 4096;
    const MIN_VISIBLE_AREA_RATIO: f32 = 0.85;

    let harness = DesktopHarness::launch_with_vsync(|| build_runtime().build(), false)?;
    let window_id = harness.main_window_id();

    set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);

    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let initial_snapshot = harness.snapshot(window_id)?;
    let gallery = find_node(
        &initial_snapshot,
        SemanticsRole::ScrollView,
        sui_demo_app::widget_book::GALLERY_SCROLL_NAME,
    );
    let scroll_point = gallery_scroll_point(gallery.bounds);
    let mut previous_frame_index = initial_snapshot
        .performance
        .as_ref()
        .expect("initial widget-book render should publish a performance snapshot")
        .frame_index;
    let mut reached_bottom = node_is_mostly_visible(
        &initial_snapshot,
        gallery.bounds,
        SemanticsRole::Image,
        sui_demo_app::widget_book::DEMO_IMAGE_LABEL,
        MIN_VISIBLE_AREA_RATIO,
    );

    move_cursor(&harness, window_id, scroll_point)?;

    let mut frame_samples = Vec::new();
    let mut phase_totals: HashMap<&str, f64> = HashMap::new();
    let mut stalled_at_end = false;

    let benchmark_start = Instant::now();

    for _ in 0..MAX_SCROLL_FRAMES {
        harness.dispatch(
            window_id,
            HostInputEvent::MouseWheel {
                delta: ScrollKind::Pixels(Vector::new(0.0, SCROLL_STEP_PX)),
            },
        )?;

        let snapshot = harness.snapshot(window_id)?;
        let performance = snapshot
            .performance
            .as_ref()
            .expect("widget-book scroll benchmark should publish performance snapshots");

        reached_bottom |= node_is_mostly_visible(
            &snapshot,
            gallery.bounds,
            SemanticsRole::Image,
            sui_demo_app::widget_book::DEMO_IMAGE_LABEL,
            MIN_VISIBLE_AREA_RATIO,
        );

        if performance.frame_index == previous_frame_index {
            if reached_bottom {
                stalled_at_end = true;
                break;
            }

            return Err(Error::new(format!(
                "widget-book scroll benchmark stalled before reaching the end after {} scroll frames",
                frame_samples.len()
            )));
        }

        previous_frame_index = performance.frame_index;
        frame_samples.push(ScrollBenchmarkFrameSample::from_snapshot(performance));

        for sample in &performance.phase_timings {
            *phase_totals.entry(sample.phase.label()).or_insert(0.0) += sample.duration_ms;
        }
    }

    let benchmark_elapsed_ms = benchmark_start.elapsed().as_secs_f64() * 1000.0;

    assert!(
        stalled_at_end,
        "widget-book scroll benchmark hit the {}-frame safety limit before the gallery stopped scrolling",
        MAX_SCROLL_FRAMES,
    );
    assert!(
        reached_bottom,
        "widget-book scroll benchmark reached a stall without bringing the final image story into view",
    );

    let valid_count = frame_samples.len();
    assert!(
        valid_count > 0,
        "expected at least one measured scroll frame",
    );

    let frame_times_ms: Vec<_> = frame_samples
        .iter()
        .map(|sample| sample.total_time_ms)
        .collect();
    let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
    let avg_ms: f64 = total_frame_time_ms / valid_count as f64;
    let max_ms: f64 = frame_times_ms
        .iter()
        .copied()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);
    let min_ms: f64 = frame_times_ms
        .iter()
        .copied()
        .min_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);
    let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
    let mut sorted_times = frame_times_ms.clone();
    sorted_times.sort_by(|a, b| a.total_cmp(b));
    let p95_ms = sorted_times[p95_index];
    let avg_draws = frame_samples
        .iter()
        .map(|sample| sample.draw_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_passes = frame_samples
        .iter()
        .map(|sample| sample.pass_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_visible_layers = frame_samples
        .iter()
        .map(|sample| sample.visible_layer_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_packet_rebuilds = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_rebuilds.total_count() as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_uploaded_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.uploaded_vertex_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.text_vertex_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_scene_traversal_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_scene_traversal_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_build_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_build_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_build_count = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_build_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_normalize_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_normalize_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_signature_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_signature_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_raster_state_init_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_raster_state_init_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_scene_build_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_scene_build_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_command_count = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_command_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_text_command_count = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_text_command_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_path_command_count = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_path_command_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_clip_path_command_count = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_clip_path_command_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_image_command_count = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_image_command_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_rect_command_count = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_rect_command_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_text_command_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_text_command_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_path_command_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_path_command_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_clip_path_command_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_clip_path_command_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_image_command_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_image_command_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_rect_command_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_rect_command_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_miss_ms = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_miss_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_miss_count = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_miss_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_surface_acquire_ms = frame_samples
        .iter()
        .map(|sample| sample.surface_acquire_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_resource_collection_ms = frame_samples
        .iter()
        .map(|sample| sample.resource_collection_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_bind_group_prepare_ms = frame_samples
        .iter()
        .map(|sample| sample.bind_group_prepare_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_image_bind_group_ms = frame_samples
        .iter()
        .map(|sample| sample.image_bind_group_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_analytic_path_bind_group_ms = frame_samples
        .iter()
        .map(|sample| sample.analytic_path_bind_group_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_analytic_path_bind_group_misses = frame_samples
        .iter()
        .map(|sample| sample.analytic_path_bind_group_miss_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_analytic_path_bind_group_bytes = frame_samples
        .iter()
        .map(|sample| sample.analytic_path_bind_group_upload_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_bind_group_ms = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_bind_group_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_upload_copy_ms = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_upload_copy_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_upload_write_ms = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_upload_write_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_upload_bytes = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_upload_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_batch_prepare_ms = frame_samples
        .iter()
        .map(|sample| sample.batch_prepare_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_gpu_upload_ms = frame_samples
        .iter()
        .map(|sample| sample.gpu_upload_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_pass_encode_ms = frame_samples
        .iter()
        .map(|sample| sample.pass_encode_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_queue_submit_ms = frame_samples
        .iter()
        .map(|sample| sample.queue_submit_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_surface_present_ms = frame_samples
        .iter()
        .map(|sample| sample.surface_present_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_dirty_regions = frame_samples
        .iter()
        .map(|sample| sample.dirty_region_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_dirty_coverage = frame_samples
        .iter()
        .map(|sample| sample.dirty_coverage as f64)
        .sum::<f64>()
        / valid_count as f64;
    let max_uploaded_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.uploaded_vertex_bytes)
        .max()
        .unwrap_or(0);
    let last_sample = frame_samples
        .last()
        .cloned()
        .expect("scroll benchmark should record at least one sample");

    println!("\n=== Widget Book Scroll FPS Benchmark ===");
    println!("scroll direction: downward");
    println!("scroll step:      {:.0} px/frame", SCROLL_STEP_PX.abs());
    println!("frames measured:  {valid_count}");
    println!(
        "scroll distance:  {:.0} px",
        valid_count as f32 * SCROLL_STEP_PX.abs()
    );
    println!("wall-clock time:  {benchmark_elapsed_ms:.1} ms");
    println!(
        "avg frame time:   {avg_ms:.3} ms ({:.0} fps)",
        1000.0 / avg_ms
    );
    println!("min frame time:   {min_ms:.3} ms");
    println!("max frame time:   {max_ms:.3} ms");
    println!(
        "p95 frame time:   {p95_ms:.3} ms ({:.0} fps)",
        1000.0 / p95_ms
    );
    println!("avg gpu passes:   {avg_passes:.2}");
    println!("avg gpu draws:    {avg_draws:.2}");
    println!("avg visible layers:{avg_visible_layers:.2}");
    println!("avg packet rebuilds:{avg_packet_rebuilds:.2}");
    println!("avg vertex bytes: {:.0}", avg_uploaded_vertex_bytes);
    println!("avg text bytes:   {:.0}", avg_text_vertex_bytes);
    println!("avg traverse:     {avg_retained_scene_traversal_ms:.3} ms");
    println!(
        "avg packet build: {avg_retained_packet_build_ms:.3} ms ({avg_retained_packet_build_count:.2} packets)"
    );
    println!(
        "avg packet stage: norm {avg_retained_packet_normalize_ms:.3} ms  sig {avg_retained_packet_signature_ms:.3} ms  state {avg_retained_packet_raster_state_init_ms:.3} ms  scene {avg_retained_packet_scene_build_ms:.3} ms"
    );
    println!(
        "avg packet cmds:  total {avg_retained_packet_command_count:.2}  text {avg_retained_packet_text_command_count:.2}  path {avg_retained_packet_path_command_count:.2}  clip-path {avg_retained_packet_clip_path_command_count:.2}  image {avg_retained_packet_image_command_count:.2}  rect {avg_retained_packet_rect_command_count:.2}"
    );
    println!(
        "avg packet time:  text {avg_retained_packet_text_command_ms:.3} ms  path {avg_retained_packet_path_command_ms:.3} ms  clip-path {avg_retained_packet_clip_path_command_ms:.3} ms  image {avg_retained_packet_image_command_ms:.3} ms  rect {avg_retained_packet_rect_command_ms:.3} ms"
    );
    println!(
        "avg atlas miss:   {avg_text_atlas_miss_ms:.3} ms ({avg_text_atlas_miss_count:.2} misses)"
    );
    println!(
        "avg surface:      acq {avg_surface_acquire_ms:.3} ms  pres {avg_surface_present_ms:.3} ms"
    );
    println!(
        "avg prep:         scan {avg_resource_collection_ms:.3} ms  bind {avg_bind_group_prepare_ms:.3} ms  batch {avg_batch_prepare_ms:.3} ms"
    );
    println!(
        "avg atlas upload: bind {avg_text_atlas_bind_group_ms:.3} ms  copy {avg_text_atlas_upload_copy_ms:.3} ms  write {avg_text_atlas_upload_write_ms:.3} ms  bytes {:.0}",
        avg_text_atlas_upload_bytes,
    );
    println!("avg image bind:   {avg_image_bind_group_ms:.3} ms");
    println!(
        "avg analytic bg:  {avg_analytic_path_bind_group_ms:.3} ms  misses {avg_analytic_path_bind_group_misses:.2}  bytes {:.0}",
        avg_analytic_path_bind_group_bytes,
    );
    println!(
        "avg submit path:  upload {avg_gpu_upload_ms:.3} ms  encode {avg_pass_encode_ms:.3} ms  submit {avg_queue_submit_ms:.3} ms"
    );
    println!("avg dirty regions:{avg_dirty_regions:.2}");
    println!("avg dirty cover:  {avg_dirty_coverage:.1}%");
    println!("max vertex bytes: {max_uploaded_vertex_bytes}");
    println!("last frame index: {}", last_sample.frame_index);

    println!("\n--- Phase breakdown (avg per frame) ---");
    let mut phase_entries: Vec<_> = phase_totals.iter().collect();
    phase_entries.sort_by(|a, b| b.1.total_cmp(a.1));
    for (phase, total_ms) in &phase_entries {
        let avg_phase_ms = *total_ms / valid_count as f64;
        let pct = (*total_ms / total_frame_time_ms) * 100.0;
        println!("  {phase:<22} {avg_phase_ms:>8.3} ms  ({pct:>5.1}%)");
    }

    println!("\n--- Slowest frames ---");
    let mut slowest_samples = frame_samples.clone();
    slowest_samples.sort_by(|a, b| b.total_time_ms.total_cmp(&a.total_time_ms));
    for sample in slowest_samples.iter().take(5) {
        println!(
            "  frame {:>4}  total {:>7.3} ms  rebuild {:>2}  upload {:>8}  text {:>8}  state {:>7.3} ms  compose {:>7.3} ms",
            sample.frame_index,
            sample.total_time_ms,
            sample.retained_packet_rebuilds.total_count(),
            sample.uploaded_vertex_bytes,
            sample.text_vertex_bytes,
            sample.retained_state_update_time_us as f64 / 1000.0,
            sample.composition_time_us as f64 / 1000.0,
        );
        println!(
            "             traverse {:>7.3} ms  packets {:>3} / {:>7.3} ms  atlas {:>3} / {:>7.3} ms",
            sample.retained_scene_traversal_time_us as f64 / 1000.0,
            sample.retained_packet_build_count,
            sample.retained_packet_build_time_us as f64 / 1000.0,
            sample.text_atlas_miss_count,
            sample.text_atlas_miss_time_us as f64 / 1000.0,
        );
        println!(
            "             packet stage norm {:>7.3} ms  sig {:>7.3} ms  state {:>7.3} ms  scene {:>7.3} ms",
            sample.retained_packet_normalize_time_us as f64 / 1000.0,
            sample.retained_packet_signature_time_us as f64 / 1000.0,
            sample.retained_packet_raster_state_init_time_us as f64 / 1000.0,
            sample.retained_packet_scene_build_time_us as f64 / 1000.0,
        );
        println!(
            "             packet cmds total {:>4}  text {:>4}  path {:>4}  clip {:>4}  image {:>4}  rect {:>4}",
            sample.retained_packet_command_count,
            sample.retained_packet_text_command_count,
            sample.retained_packet_path_command_count,
            sample.retained_packet_clip_path_command_count,
            sample.retained_packet_image_command_count,
            sample.retained_packet_rect_command_count,
        );
        println!(
            "             packet time text {:>7.3} ms  path {:>7.3} ms  clip {:>7.3} ms  image {:>7.3} ms  rect {:>7.3} ms",
            sample.retained_packet_text_command_time_us as f64 / 1000.0,
            sample.retained_packet_path_command_time_us as f64 / 1000.0,
            sample.retained_packet_clip_path_command_time_us as f64 / 1000.0,
            sample.retained_packet_image_command_time_us as f64 / 1000.0,
            sample.retained_packet_rect_command_time_us as f64 / 1000.0,
        );
        if sample.packet_hotspot_total_time_us > 0 {
            let packet_container = sample
                .packet_hotspot_layer_id
                .map(|layer_id| format!("layer {layer_id}"))
                .unwrap_or_else(|| "root".to_string());
            let packet_owner = sample
                .packet_hotspot_owner_widget_id
                .map(|owner_id| owner_id.to_string())
                .unwrap_or_else(|| "-".to_string());
            println!(
                "             hotspot packet {packet_container} seg {:>3}  owner {:>6}  total {:>7.3} ms  scene {:>7.3} ms",
                sample.packet_hotspot_segment_index,
                packet_owner,
                sample.packet_hotspot_total_time_us as f64 / 1000.0,
                sample.packet_hotspot_scene_build_time_us as f64 / 1000.0,
            );
            println!(
                "             hotspot cmds   total {:>4}  text {:>4}  path {:>4}  rect {:>4}  text-time {:>7.3} ms  path-time {:>7.3} ms  rect-time {:>7.3} ms",
                sample.packet_hotspot_command_count,
                sample.packet_hotspot_text_command_count,
                sample.packet_hotspot_path_command_count,
                sample.packet_hotspot_rect_command_count,
                sample.packet_hotspot_text_command_time_us as f64 / 1000.0,
                sample.packet_hotspot_path_command_time_us as f64 / 1000.0,
                sample.packet_hotspot_rect_command_time_us as f64 / 1000.0,
            );
            if let Some(text_sample) = &sample.packet_hotspot_text_sample {
                println!("             hotspot text   {text_sample}");
            }
        }
        println!(
            "             surface {:>7.3} / {:>7.3} ms  prep {:>7.3} / {:>7.3} / {:>7.3} ms  submit {:>7.3} / {:>7.3} / {:>7.3} ms",
            sample.surface_acquire_time_us as f64 / 1000.0,
            sample.surface_present_time_us as f64 / 1000.0,
            sample.resource_collection_time_us as f64 / 1000.0,
            sample.bind_group_prepare_time_us as f64 / 1000.0,
            sample.batch_prepare_time_us as f64 / 1000.0,
            sample.gpu_upload_time_us as f64 / 1000.0,
            sample.pass_encode_time_us as f64 / 1000.0,
            sample.queue_submit_time_us as f64 / 1000.0,
        );
        println!(
            "             atlas-bind {:>7.3} ms  copy {:>7.3} ms  write {:>7.3} ms  bytes {:>8}  image-bind {:>7.3} ms",
            sample.text_atlas_bind_group_time_us as f64 / 1000.0,
            sample.text_atlas_upload_copy_time_us as f64 / 1000.0,
            sample.text_atlas_upload_write_time_us as f64 / 1000.0,
            sample.text_atlas_upload_bytes,
            sample.image_bind_group_time_us as f64 / 1000.0,
        );
        println!(
            "             analytic {:>7.3} ms  misses {:>3}  bytes {:>8}",
            sample.analytic_path_bind_group_time_us as f64 / 1000.0,
            sample.analytic_path_bind_group_miss_count,
            sample.analytic_path_bind_group_upload_bytes,
        );
    }

    if let Some(snapshot) = harness.snapshot(window_id)?.performance {
        println!("\n--- Last frame scene stats ---");
        println!("  commands:        {}", snapshot.scene.command_count);
        println!("  text commands:   {}", snapshot.scene.text_command_count);
        println!("  dirty regions:   {}", snapshot.scene.dirty_region_count);
        println!("  dirty coverage:  {:.1}%", snapshot.scene.dirty_coverage);
        println!(
            "  gpu draws:       {}",
            snapshot.renderer_submission.draw_count
        );
        println!(
            "  gpu passes:      {}",
            snapshot.renderer_submission.pass_count
        );
        println!(
            "  layers visible:  {}",
            snapshot.renderer_submission.visible_layer_count
        );
        println!(
            "  direct packets:  {}",
            snapshot.renderer_submission.direct_packet_count
        );
        println!(
            "  state update us: {}",
            snapshot.renderer_submission.retained_state_update_time_us
        );
        println!(
            "  glyph instances: {}",
            snapshot.renderer_submission.text_glyph_instance_count
        );
        println!(
            "  vertex bytes:    {}",
            snapshot.renderer_submission.uploaded_vertex_bytes
        );
        println!(
            "  text bytes:      {}",
            snapshot.renderer_submission.text_vertex_bytes
        );
        println!(
            "  traverse:        {:.3} ms",
            snapshot
                .renderer_submission
                .retained_scene_traversal_time_us as f64
                / 1000.0,
        );
        println!(
            "  packet build:    {} packets / {:.3} ms",
            snapshot.renderer_submission.retained_packet_build_count,
            snapshot.renderer_submission.retained_packet_build_time_us as f64 / 1000.0,
        );
        println!(
            "  atlas miss:      {} misses / {:.3} ms",
            snapshot.renderer_submission.text_atlas_miss_count,
            snapshot.renderer_submission.text_atlas_miss_time_us as f64 / 1000.0,
        );
        println!(
            "  surface:         {:.3} ms acquire / {:.3} ms present",
            snapshot.renderer_submission.surface_acquire_time_us as f64 / 1000.0,
            snapshot.renderer_submission.surface_present_time_us as f64 / 1000.0,
        );
        println!(
            "  prep:            {:.3} ms scan / {:.3} ms bind / {:.3} ms batch",
            snapshot.renderer_submission.resource_collection_time_us as f64 / 1000.0,
            snapshot.renderer_submission.bind_group_prepare_time_us as f64 / 1000.0,
            snapshot.renderer_submission.batch_prepare_time_us as f64 / 1000.0,
        );
        println!(
            "  atlas upload:    {:.3} ms bind / {:.3} ms copy / {:.3} ms write / {} bytes",
            snapshot.renderer_submission.text_atlas_bind_group_time_us as f64 / 1000.0,
            snapshot.renderer_submission.text_atlas_upload_copy_time_us as f64 / 1000.0,
            snapshot.renderer_submission.text_atlas_upload_write_time_us as f64 / 1000.0,
            snapshot.renderer_submission.text_atlas_upload_bytes,
        );
        println!(
            "  image bind:      {:.3} ms",
            snapshot.renderer_submission.image_bind_group_time_us as f64 / 1000.0,
        );
        println!(
            "  analytic bind:   {:.3} ms / {} misses / {} bytes",
            snapshot
                .renderer_submission
                .analytic_path_bind_group_time_us as f64
                / 1000.0,
            snapshot
                .renderer_submission
                .analytic_path_bind_group_miss_count,
            snapshot
                .renderer_submission
                .analytic_path_bind_group_upload_bytes,
        );
        println!(
            "  submit path:     {:.3} ms upload / {:.3} ms encode / {:.3} ms submit",
            snapshot.renderer_submission.gpu_upload_time_us as f64 / 1000.0,
            snapshot.renderer_submission.pass_encode_time_us as f64 / 1000.0,
            snapshot.renderer_submission.queue_submit_time_us as f64 / 1000.0,
        );
        println!(
            "  path cache:      {} entries, {} hits, {} misses",
            snapshot.text_caches.renderer_path.entries,
            snapshot.text_caches.renderer_path.hits,
            snapshot.text_caches.renderer_path.misses,
        );
        println!(
            "  path cache Δ:    {:+} entries, {} hits, {} misses",
            snapshot.text_cache_deltas.renderer_path.entries_delta,
            snapshot.text_cache_deltas.renderer_path.hits,
            snapshot.text_cache_deltas.renderer_path.misses,
        );
    }
    println!("========================================\n");

    assert!(
        avg_ms < 16.67,
        "average frame time {avg_ms:.3} ms exceeds the 16.67 ms budget for 60 fps",
    );

    Ok(())
}

fn run_retained_text_scroll_benchmark() -> Result<()> {
    const SCROLL_STEP_PX: f32 = -36.0;
    const WARMUP_FRAMES: usize = 24;
    const MEASURED_FRAMES: usize = 160;

    let harness = DesktopHarness::launch_with_vsync(
        || build_retained_text_benchmark_application().build(),
        false,
    )?;
    let window_id = harness.main_window_id();

    set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);
    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let initial_snapshot = harness.snapshot(window_id)?;
    let scroll_view = find_node(
        &initial_snapshot,
        SemanticsRole::ScrollView,
        RETAINED_TEXT_BENCHMARK_SCROLL_NAME,
    );
    let scroll_point = node_center(scroll_view.bounds);
    let mut previous_frame_index = initial_snapshot
        .performance
        .as_ref()
        .expect("retained text benchmark should publish an initial performance snapshot")
        .frame_index;

    move_cursor(&harness, window_id, scroll_point)?;

    let mut frame_samples = Vec::with_capacity(MEASURED_FRAMES);
    let benchmark_start = Instant::now();

    for frame in 0..(WARMUP_FRAMES + MEASURED_FRAMES) {
        harness.dispatch(
            window_id,
            HostInputEvent::MouseWheel {
                delta: ScrollKind::Pixels(Vector::new(0.0, SCROLL_STEP_PX)),
            },
        )?;

        let snapshot = harness.snapshot(window_id)?;
        let performance = snapshot
            .performance
            .as_ref()
            .expect("retained text benchmark should publish performance snapshots");

        if performance.frame_index == previous_frame_index {
            return Err(Error::new(format!(
                "retained text benchmark did not render a new frame for scroll step {}",
                frame + 1,
            )));
        }

        previous_frame_index = performance.frame_index;

        if frame >= WARMUP_FRAMES {
            frame_samples.push(ScrollBenchmarkFrameSample::from_snapshot(performance));
        }
    }

    let benchmark_elapsed_ms = benchmark_start.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(initial_snapshot.title, RETAINED_TEXT_BENCHMARK_TITLE);
    assert_eq!(frame_samples.len(), MEASURED_FRAMES);

    let valid_count = frame_samples.len();
    let frame_times_ms: Vec<_> = frame_samples
        .iter()
        .map(|sample| sample.total_time_ms)
        .collect();
    let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
    let avg_ms = total_frame_time_ms / valid_count as f64;
    let max_ms = frame_times_ms
        .iter()
        .copied()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);
    let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
    let mut sorted_times = frame_times_ms.clone();
    sorted_times.sort_by(|a, b| a.total_cmp(b));
    let p95_ms = sorted_times[p95_index];
    let avg_visible_layers = frame_samples
        .iter()
        .map(|sample| sample.visible_layer_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_packet_rebuilds = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_rebuilds.total_count() as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_direct_packets = frame_samples
        .iter()
        .map(|sample| sample.direct_packet_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_uploaded_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.uploaded_vertex_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.text_vertex_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_glyph_instances = frame_samples
        .iter()
        .map(|sample| sample.text_glyph_instance_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_state_update_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_state_update_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_retained_packet_build_ms = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_build_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_miss_count = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_miss_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_upload_bytes = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_upload_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_surface_acquire_ms = frame_samples
        .iter()
        .map(|sample| sample.surface_acquire_time_us as f64 / 1000.0)
        .sum::<f64>()
        / valid_count as f64;
    let max_uploaded_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.uploaded_vertex_bytes)
        .max()
        .unwrap_or(0);
    let max_text_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.text_vertex_bytes)
        .max()
        .unwrap_or(0);
    let avg_text_bytes_per_glyph = if avg_text_glyph_instances > 0.0 {
        avg_text_vertex_bytes / avg_text_glyph_instances
    } else {
        0.0
    };

    println!("\n=== Retained Text Scroll Benchmark ===");
    println!("warmup frames:    {WARMUP_FRAMES}");
    println!("frames measured:  {valid_count}");
    println!("scroll step:      {:.0} px/frame", SCROLL_STEP_PX.abs());
    println!("wall-clock time:  {benchmark_elapsed_ms:.1} ms");
    println!(
        "avg frame time:   {avg_ms:.3} ms ({:.0} fps)",
        1000.0 / avg_ms
    );
    println!("max frame time:   {max_ms:.3} ms");
    println!(
        "p95 frame time:   {p95_ms:.3} ms ({:.0} fps)",
        1000.0 / p95_ms
    );
    println!("avg layers:       {avg_visible_layers:.2}");
    println!("avg packets:      {avg_direct_packets:.2}");
    println!("avg packet rebuilds:{avg_packet_rebuilds:.2}");
    println!("avg upload bytes: {:.0}", avg_uploaded_vertex_bytes);
    println!("avg text bytes:   {:.0}", avg_text_vertex_bytes);
    println!("avg glyphs:       {avg_text_glyph_instances:.2}");
    println!("avg bytes/glyph:  {avg_text_bytes_per_glyph:.2}");
    println!("max upload bytes: {max_uploaded_vertex_bytes}");
    println!("max text bytes:   {max_text_vertex_bytes}");
    println!("avg atlas misses: {avg_text_atlas_miss_count:.2}");
    println!("avg atlas upload: {:.0}", avg_text_atlas_upload_bytes);
    println!("avg state update: {avg_state_update_ms:.3} ms");
    println!("avg packet build: {avg_retained_packet_build_ms:.3} ms");
    println!("avg surface acq:  {avg_surface_acquire_ms:.3} ms");
    println!("======================================\n");

    assert!(
        avg_text_glyph_instances > 0.0,
        "retained text benchmark should render glyph instances on every measured frame",
    );
    assert!(
        avg_text_vertex_bytes > 0.0,
        "retained text benchmark should upload text payloads while scrolling",
    );
    assert!(
        max_uploaded_vertex_bytes > 0,
        "retained text benchmark should upload geometry while scrolling",
    );

    Ok(())
}

fn run_text_editing_benchmark() -> Result<()> {
    const EDIT_COMMITS: [&str; 10] = [
        " // typed atlas reuse",
        "\nlet pending_frame = cache_hits + 1;",
        "\n// bidi check: abc אבג 123 مرحبا",
        "\nlet emoji = \"🙂✅🎨\";",
        "\nlet ime_probe = \"候補\";",
        "\nlet syntax_band = highlight_rows.len();",
        "\n// fallback sample: Ж 中 नमस्ते",
        "\nrecord_selection_delta(cursor, viewport);",
        "\nlet scroll_budget_ms = 16.67;",
        "\ncommit_overlay_sample(frame_index);",
    ];
    const IME_PREEDIT_UPDATES: [(&str, Option<(usize, usize)>); 3] = [
        ("候", Some((0, 1))),
        ("候補", Some((1, 2))),
        ("候補を", Some((2, 3))),
    ];
    const SELECTION_STEPS: usize = 8;
    const EDITOR_SCROLL_FRAMES: usize = 18;
    const SYNTAX_SCROLL_FRAMES: usize = 28;
    const SCROLL_STEP_PX: f32 = -34.0;

    let harness = DesktopHarness::launch_with_vsync(
        || build_text_editing_benchmark_application().build(),
        false,
    )?;
    let window_id = harness.main_window_id();

    set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);
    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let initial_snapshot = harness.snapshot(window_id)?;
    let editor = find_node(
        &initial_snapshot,
        SemanticsRole::TextInput,
        TEXT_EDITING_BENCHMARK_EDITOR_NAME,
    );
    let syntax_scroll = find_node(
        &initial_snapshot,
        SemanticsRole::ScrollView,
        TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME,
    );
    let editor_point = node_center(editor.bounds);
    let syntax_point = node_center(syntax_scroll.bounds);
    let mut previous_frame_index = initial_snapshot
        .performance
        .as_ref()
        .expect("text editing benchmark should publish an initial performance snapshot")
        .frame_index;
    let mut frame_samples = Vec::new();
    let benchmark_started = Instant::now();

    let mut record_frame = |stage: &str, step: usize| -> Result<()> {
        let snapshot = harness.snapshot(window_id)?;
        let performance = snapshot
            .performance
            .as_ref()
            .expect("text editing benchmark should publish performance snapshots");

        if performance.frame_index == previous_frame_index {
            return Err(Error::new(format!(
                "text editing benchmark did not render a new frame during {stage} step {}",
                step + 1,
            )));
        }

        previous_frame_index = performance.frame_index;
        frame_samples.push(ScrollBenchmarkFrameSample::from_snapshot(performance));
        Ok(())
    };

    click_at(&harness, window_id, editor_point)?;

    harness.dispatch(window_id, HostInputEvent::ImeStart)?;
    for (step, (text, cursor_range)) in IME_PREEDIT_UPDATES.iter().enumerate() {
        harness.dispatch(
            window_id,
            HostInputEvent::ImePreedit {
                text: (*text).to_string(),
                cursor_range: *cursor_range,
            },
        )?;
        record_frame("composition preedit", step)?;
    }
    harness.dispatch(
        window_id,
        HostInputEvent::ImeCommit {
            text: "候補を".to_string(),
        },
    )?;
    record_frame("composition commit", IME_PREEDIT_UPDATES.len())?;
    harness.dispatch(window_id, HostInputEvent::ImeEnd)?;

    for (step, text) in EDIT_COMMITS.iter().enumerate() {
        harness.dispatch(
            window_id,
            HostInputEvent::ImeCommit {
                text: (*text).to_string(),
            },
        )?;
        record_frame("typing", step)?;
    }

    let selection_start = Point::new(editor.bounds.x() + 92.0, editor.bounds.y() + 64.0);
    let selection_end = Point::new(
        editor.bounds.x() + editor.bounds.width() - 84.0,
        editor.bounds.y() + 64.0,
    );
    move_cursor(&harness, window_id, selection_start)?;
    harness.dispatch(
        window_id,
        HostInputEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
        },
    )?;
    for step in 0..SELECTION_STEPS {
        let t = (step + 1) as f32 / SELECTION_STEPS as f32;
        harness.dispatch(
            window_id,
            HostInputEvent::CursorMoved {
                position: interpolate_point(selection_start, selection_end, t),
            },
        )?;
        record_frame("selection drag", step)?;
    }
    harness.dispatch(
        window_id,
        HostInputEvent::MouseInput {
            state: ElementState::Released,
            button: MouseButton::Left,
        },
    )?;

    move_cursor(&harness, window_id, editor_point)?;
    for step in 0..EDITOR_SCROLL_FRAMES {
        harness.dispatch(
            window_id,
            HostInputEvent::MouseWheel {
                delta: ScrollKind::Pixels(Vector::new(0.0, SCROLL_STEP_PX)),
            },
        )?;
        record_frame("editor scroll", step)?;
    }

    move_cursor(&harness, window_id, syntax_point)?;
    for step in 0..SYNTAX_SCROLL_FRAMES {
        harness.dispatch(
            window_id,
            HostInputEvent::MouseWheel {
                delta: ScrollKind::Pixels(Vector::new(0.0, SCROLL_STEP_PX)),
            },
        )?;
        record_frame("syntax scroll", step)?;
    }

    let benchmark_elapsed_ms = benchmark_started.elapsed().as_secs_f64() * 1000.0;
    let valid_count = frame_samples.len();
    let frame_times_ms: Vec<_> = frame_samples
        .iter()
        .map(|sample| sample.total_time_ms)
        .collect();
    let total_frame_time_ms: f64 = frame_times_ms.iter().sum();
    let avg_ms = total_frame_time_ms / valid_count as f64;
    let max_ms = frame_times_ms
        .iter()
        .copied()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);
    let mut sorted_times = frame_times_ms.clone();
    sorted_times.sort_by(|a, b| a.total_cmp(b));
    let p95_index = ((valid_count as f64 * 0.95).ceil() as usize).min(valid_count - 1);
    let p95_ms = sorted_times[p95_index];
    let avg_uploaded_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.uploaded_vertex_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.text_vertex_bytes as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_glyph_instances = frame_samples
        .iter()
        .map(|sample| sample.text_glyph_instance_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_visible_layers = frame_samples
        .iter()
        .map(|sample| sample.visible_layer_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_packet_rebuilds = frame_samples
        .iter()
        .map(|sample| sample.retained_packet_rebuilds.total_count() as f64)
        .sum::<f64>()
        / valid_count as f64;
    let avg_text_atlas_miss_count = frame_samples
        .iter()
        .map(|sample| sample.text_atlas_miss_count as f64)
        .sum::<f64>()
        / valid_count as f64;
    let max_uploaded_vertex_bytes = frame_samples
        .iter()
        .map(|sample| sample.uploaded_vertex_bytes)
        .max()
        .unwrap_or(0);

    assert_eq!(initial_snapshot.title, TEXT_EDITING_BENCHMARK_TITLE);
    assert_eq!(
        valid_count,
        IME_PREEDIT_UPDATES.len()
            + 1
            + EDIT_COMMITS.len()
            + SELECTION_STEPS
            + EDITOR_SCROLL_FRAMES
            + SYNTAX_SCROLL_FRAMES,
    );

    println!("\n=== Text Editing Benchmark ===");
    println!("frames measured:  {valid_count}");
    println!("wall-clock time:  {benchmark_elapsed_ms:.1} ms");
    println!(
        "avg frame time:   {avg_ms:.3} ms ({:.0} fps)",
        1000.0 / avg_ms
    );
    println!("max frame time:   {max_ms:.3} ms");
    println!(
        "p95 frame time:   {p95_ms:.3} ms ({:.0} fps)",
        1000.0 / p95_ms
    );
    println!("avg upload bytes: {:.0}", avg_uploaded_vertex_bytes);
    println!("avg text bytes:   {:.0}", avg_text_vertex_bytes);
    println!("avg glyphs:       {avg_text_glyph_instances:.2}");
    println!("avg visible layers:{avg_visible_layers:.2}");
    println!("avg packet rebuilds:{avg_packet_rebuilds:.2}");
    println!("avg atlas misses: {avg_text_atlas_miss_count:.2}");
    println!("max upload bytes: {max_uploaded_vertex_bytes}");
    println!("==============================\n");

    assert!(
        avg_text_glyph_instances > 0.0,
        "text editing benchmark should render glyph instances on every measured frame",
    );
    assert!(
        avg_text_vertex_bytes > 0.0,
        "text editing benchmark should submit text payloads while typing and scrolling",
    );
    assert!(
        max_uploaded_vertex_bytes > 0,
        "text editing benchmark should upload geometry during interaction",
    );

    Ok(())
}

fn node_is_mostly_visible(
    snapshot: &DesktopWindowSnapshot,
    viewport: Rect,
    role: SemanticsRole,
    name: &str,
    min_visible_area_ratio: f32,
) -> bool {
    snapshot.semantics.iter().any(|node| {
        if node.role != role || node.name.as_deref() != Some(name) {
            return false;
        }

        let Some(visible) = node.bounds.intersection(viewport) else {
            return false;
        };

        let node_area = node.bounds.width() * node.bounds.height();
        let visible_area = visible.width() * visible.height();
        node_area > 0.0 && (visible_area / node_area) >= min_visible_area_ratio
    })
}

#[test]
fn desktop_widget_book_repaints_and_updates_metrics_from_platform_events() -> Result<()> {
    if skip_without_desktop_display(
        "desktop_widget_book_repaints_and_updates_metrics_from_platform_events",
    ) {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let harness = DesktopHarness::launch(|| {
        build_widget_book_application(Rc::new(RefCell::new(WidgetBookState {
            name: String::new(),
            subscribed: false,
            theme_preview_comparison: true,
            button_presses: 0,
            icon_button_presses: 0,
            switch_on: false,
            standalone_radio_selected: false,
            radio_choice: "Balanced".to_string(),
            slider_value: 50.0,
            number_value: 8.0,
            notes: String::new(),
            mode: String::new(),
            tab_bar_choice: "Canvas".to_string(),
            tabs_choice: "Layout".to_string(),
            last_menu_action: String::new(),
            last_context_action: String::new(),
            dialog_apply_count: 0,
        })))
        .build()
    })?;
    let window_id = harness.main_window_id();

    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let before_frame = harness.capture(window_id)?;
    let before_snapshot = harness.snapshot(window_id)?;
    assert_eq!(
        before_snapshot.title,
        sui_demo_app::widget_book::WINDOW_TITLE
    );
    assert_eq!(
        text_input_value(
            &before_snapshot,
            sui_demo_app::widget_book::NAME_INPUT_LABEL
        ),
        ""
    );

    let input = find_node(
        &before_snapshot,
        SemanticsRole::TextInput,
        sui_demo_app::widget_book::NAME_INPUT_LABEL,
    );

    click_at(&harness, window_id, node_center(input.bounds))?;
    harness.dispatch(
        window_id,
        HostInputEvent::ImeCommit {
            text: "Ada".to_string(),
        },
    )?;

    let after_frame = harness.capture(window_id)?;
    let after_snapshot = harness.snapshot(window_id)?;

    assert_eq!(
        text_input_value(&after_snapshot, sui_demo_app::widget_book::NAME_INPUT_LABEL),
        "Ada"
    );
    assert!(
        frame_pixel_diff_count(&before_frame, &after_frame) > 0,
        "desktop IME commit should repaint the real renderer output"
    );
    assert!(
        after_snapshot
            .performance
            .as_ref()
            .is_some_and(|snapshot| snapshot.frame_index > 0),
        "desktop renderer should publish a performance snapshot after IME text entry"
    );

    let before_scroll_frame = after_frame;
    let before_scroll_snapshot = after_snapshot;
    let before_metrics = before_scroll_snapshot
        .performance
        .clone()
        .expect("initial desktop render should publish performance metrics");
    let gallery = find_node(
        &before_scroll_snapshot,
        SemanticsRole::ScrollView,
        sui_demo_app::widget_book::GALLERY_SCROLL_NAME,
    );
    let before_button = find_node(
        &before_scroll_snapshot,
        SemanticsRole::Button,
        sui_demo_app::widget_book::PRIMARY_BUTTON_LABEL,
    );

    move_cursor(&harness, window_id, node_center(gallery.bounds))?;
    harness.dispatch(
        window_id,
        HostInputEvent::MouseWheel {
            delta: ScrollKind::Pixels(Vector::new(0.0, -360.0)),
        },
    )?;

    let after_frame = harness.capture(window_id)?;
    let after_snapshot = harness.snapshot(window_id)?;
    let after_metrics = after_snapshot
        .performance
        .clone()
        .expect("desktop scroll redraw should publish performance metrics");
    let after_button = find_node(
        &after_snapshot,
        SemanticsRole::Button,
        sui_demo_app::widget_book::PRIMARY_BUTTON_LABEL,
    );

    assert!(
        frame_pixel_diff_count(&before_scroll_frame, &after_frame) > 0,
        "desktop wheel scroll updated semantics but did not change any rendered pixels"
    );
    assert!(after_button.bounds.y() < before_button.bounds.y());
    assert!(after_metrics.frame_index > before_metrics.frame_index);
    assert!(after_metrics.total_time_ms >= 0.0);

    Ok(())
}

#[test]
fn desktop_widget_book_repaints_when_scrolling_with_split_view_visible() -> Result<()> {
    if skip_without_desktop_display(
        "desktop_widget_book_repaints_when_scrolling_with_split_view_visible",
    ) {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    const SCROLL_STEP_PX: f32 = -120.0;
    const MAX_SCROLL_STEPS: usize = 80;
    const MIN_VISIBLE_AREA_RATIO: f32 = 0.2;

    let harness = DesktopHarness::launch(|| {
        build_widget_book_application(default_widget_book_state()).build()
    })?;
    let window_id = harness.main_window_id();

    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let mut snapshot = harness.snapshot(window_id)?;
    let gallery = find_node(
        &snapshot,
        SemanticsRole::ScrollView,
        sui_demo_app::widget_book::GALLERY_SCROLL_NAME,
    );
    let scroll_point = gallery_scroll_point(gallery.bounds);

    move_cursor(&harness, window_id, scroll_point)?;

    let mut split_visible = node_is_mostly_visible(
        &snapshot,
        gallery.bounds,
        SemanticsRole::Splitter,
        sui_demo_app::widget_book::SPLIT_VIEW_NAME,
        MIN_VISIBLE_AREA_RATIO,
    );

    for _ in 0..MAX_SCROLL_STEPS {
        if split_visible {
            break;
        }

        harness.dispatch(
            window_id,
            HostInputEvent::MouseWheel {
                delta: ScrollKind::Pixels(Vector::new(0.0, SCROLL_STEP_PX)),
            },
        )?;
        snapshot = harness.snapshot(window_id)?;
        split_visible = node_is_mostly_visible(
            &snapshot,
            gallery.bounds,
            SemanticsRole::Splitter,
            sui_demo_app::widget_book::SPLIT_VIEW_NAME,
            MIN_VISIBLE_AREA_RATIO,
        );
    }

    assert!(
        split_visible,
        "split view story never became visible while scrolling the gallery"
    );

    let before_frame = harness.capture(window_id)?;
    let before_snapshot = harness.snapshot(window_id)?;
    let before_split = find_node(
        &before_snapshot,
        SemanticsRole::Splitter,
        sui_demo_app::widget_book::SPLIT_VIEW_NAME,
    );

    harness.dispatch(
        window_id,
        HostInputEvent::MouseWheel {
            delta: ScrollKind::Pixels(Vector::new(0.0, SCROLL_STEP_PX)),
        },
    )?;

    let after_frame = harness.capture(window_id)?;
    let after_snapshot = harness.snapshot(window_id)?;
    let after_split = find_node(
        &after_snapshot,
        SemanticsRole::Splitter,
        sui_demo_app::widget_book::SPLIT_VIEW_NAME,
    );

    assert!(
        frame_pixel_diff_count(&before_frame, &after_frame) > 0,
        "desktop wheel scroll with the split view visible did not change any rendered pixels"
    );
    assert!(after_split.bounds.y() < before_split.bounds.y());

    Ok(())
}

#[test]
fn desktop_split_view_table_scroll_repaints_frame() -> Result<()> {
    if skip_without_desktop_display("desktop_split_view_table_scroll_repaints_frame") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let harness = DesktopHarness::launch(|| {
        let table = Table::new("Split table")
            .columns([
                TableColumn::new("Name"),
                TableColumn::new("Passes").width(80.0),
            ])
            .rows([
                TableRow::new(["Glass", "3"]),
                TableRow::new(["Water", "4"]),
                TableRow::new(["Cloud", "2"]),
                TableRow::new(["Foam", "5"]),
                TableRow::new(["Dust", "1"]),
                TableRow::new(["Mist", "6"]),
                TableRow::new(["Lava", "7"]),
                TableRow::new(["Glow", "2"]),
            ]);

        Application::new()
            .window(
                WindowBuilder::new().title("Split scroll repro").root(
                    SizedBox::new().size(Size::new(360.0, 220.0)).with_child(
                        SplitView::horizontal(table, SizedBox::new().size(Size::new(120.0, 220.0)))
                            .ratio(0.68),
                    ),
                ),
            )
            .build()
    })?;
    let window_id = harness.main_window_id();

    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let before_snapshot = harness.snapshot(window_id)?;
    let table = find_node(&before_snapshot, SemanticsRole::Table, "Split table");
    let scroll_point = Point::new(table.bounds.x() + 48.0, table.bounds.y() + 96.0);

    move_cursor(&harness, window_id, scroll_point)?;

    let before_frame = harness.capture(window_id)?;

    harness.dispatch(
        window_id,
        HostInputEvent::MouseWheel {
            delta: ScrollKind::Pixels(Vector::new(0.0, -72.0)),
        },
    )?;

    let after_frame = harness.capture(window_id)?;

    assert!(
        frame_pixel_diff_count(&before_frame, &after_frame) > 0,
        "scrolling a table inside a split view did not change any rendered pixels"
    );

    Ok(())
}

#[test]
fn desktop_split_view_scroll_view_scroll_repaints_frame() -> Result<()> {
    if skip_without_desktop_display("desktop_split_view_scroll_view_scroll_repaints_frame") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let harness = DesktopHarness::launch(|| {
        let scroll = ScrollView::vertical(
            Stack::vertical()
                .with_child(SizedBox::new().height(120.0).with_child(Background::new(
                    Color::rgba(0.82, 0.36, 0.18, 1.0),
                    SizedBox::new().size(Size::new(220.0, 120.0)),
                )))
                .with_child(SizedBox::new().height(120.0).with_child(Background::new(
                    Color::rgba(0.18, 0.54, 0.82, 1.0),
                    SizedBox::new().size(Size::new(220.0, 120.0)),
                )))
                .with_child(SizedBox::new().height(120.0).with_child(Background::new(
                    Color::rgba(0.24, 0.72, 0.36, 1.0),
                    SizedBox::new().size(Size::new(220.0, 120.0)),
                ))),
        )
        .name("Split scroll");

        Application::new()
            .window(
                WindowBuilder::new().title("Split scroll repro").root(
                    SizedBox::new().size(Size::new(360.0, 220.0)).with_child(
                        SplitView::horizontal(
                            scroll,
                            SizedBox::new().size(Size::new(120.0, 220.0)),
                        )
                        .ratio(0.68),
                    ),
                ),
            )
            .build()
    })?;
    let window_id = harness.main_window_id();

    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    let before_snapshot = harness.snapshot(window_id)?;
    let scroll = find_node(&before_snapshot, SemanticsRole::ScrollView, "Split scroll");
    let scroll_point = Point::new(scroll.bounds.x() + 48.0, scroll.bounds.y() + 96.0);

    move_cursor(&harness, window_id, scroll_point)?;

    let before_frame = harness.capture(window_id)?;

    harness.dispatch(
        window_id,
        HostInputEvent::MouseWheel {
            delta: ScrollKind::Pixels(Vector::new(0.0, -72.0)),
        },
    )?;

    let after_frame = harness.capture(window_id)?;

    assert!(
        frame_pixel_diff_count(&before_frame, &after_frame) > 0,
        "scrolling a scroll view inside a split view did not change any rendered pixels"
    );

    Ok(())
}

#[test]
fn desktop_virtual_scroll_render_is_history_independent_for_same_offset() -> Result<()> {
    if skip_without_desktop_display(
        "desktop_virtual_scroll_render_is_history_independent_for_same_offset",
    ) {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    fn capture_after_scroll_steps(steps: &[f32]) -> Result<CapturedFrame> {
        let harness = DesktopHarness::launch(|| build_scroll_history_repro_application().build())?;
        let window_id = harness.main_window_id();

        harness.dispatch(window_id, HostInputEvent::Focused(true))?;

        let initial_snapshot = harness.snapshot(window_id)?;
        let scroll = find_node(
            &initial_snapshot,
            SemanticsRole::ScrollView,
            "History repro scroll",
        );
        let scroll_point = Point::new(scroll.bounds.max_x() - 12.0, scroll.bounds.y() + 40.0);
        move_cursor(&harness, window_id, scroll_point)?;

        for step in steps {
            harness.dispatch(
                window_id,
                HostInputEvent::MouseWheel {
                    delta: ScrollKind::Pixels(Vector::new(0.0, *step)),
                },
            )?;
        }

        harness.capture(window_id)
    }

    let single_frame = capture_after_scroll_steps(&[-48.0])?;
    let single_again_frame = capture_after_scroll_steps(&[-48.0])?;
    let multi_frame = capture_after_scroll_steps(&[-12.0, -12.0, -12.0, -12.0])?;

    let same_history_diff_count = frame_pixel_diff_count(&single_frame, &single_again_frame);
    assert_eq!(
        same_history_diff_count,
        0,
        "fresh launches with the same wheel-event history should produce equivalent pixels (diff pixels: {same_history_diff_count}, diff bounds: {:?})",
        frame_diff_bounds(&single_frame, &single_again_frame),
    );

    let diff_count = frame_pixel_diff_count(&single_frame, &multi_frame);
    assert_eq!(
        diff_count,
        0,
        "the retained renderer produced different pixels for the same final virtual-scroll offset depending on wheel-event history (diff pixels: {diff_count}, diff bounds: {:?})",
        frame_diff_bounds(&single_frame, &multi_frame),
    );

    Ok(())
}

#[test]
fn virtual_scroll_runtime_scene_is_history_independent_for_same_offset() -> Result<()> {
    fn render_after_steps(steps: &[f32]) -> Result<RenderOutput> {
        let mut runtime = build_scroll_history_repro_application().build()?;
        let window_id = runtime.window_ids()[0];
        let _ = runtime.render(window_id)?;

        for step in steps {
            let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(528.0, 40.0));
            scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, *step)));
            runtime.handle_event(window_id, Event::Pointer(scroll))?;
            let _ = runtime.render(window_id)?;
        }

        runtime.render(window_id)
    }

    let single = render_after_steps(&[-48.0])?;
    let multi = render_after_steps(&[-12.0, -12.0, -12.0, -12.0])?;

    assert_eq!(
        normalized_semantics_snapshot(&single.semantics),
        normalized_semantics_snapshot(&multi.semantics),
    );
    assert_eq!(
        normalized_scene_snapshot(&single.frame.scene),
        normalized_scene_snapshot(&multi.frame.scene),
    );
    assert_eq!(
        normalized_layer_updates_snapshot(&single),
        normalized_layer_updates_snapshot(&multi),
    );

    Ok(())
}

#[test]
fn widget_book_scroll_fps_benchmark() -> Result<()> {
    if skip_without_desktop_display("widget_book_scroll_fps_benchmark") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    run_widget_book_scroll_benchmark(|| {
        build_widget_book_application(scroll_benchmark_widget_book_state())
    })
}

#[test]
#[ignore = "diagnostic benchmark for isolating live-overlay cost"]
fn widget_book_scroll_fps_benchmark_without_live_overlay() -> Result<()> {
    if skip_without_desktop_display("widget_book_scroll_fps_benchmark_without_live_overlay") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    run_widget_book_scroll_benchmark(|| {
        build_widget_book_gallery_application(scroll_benchmark_widget_book_state())
    })
}

#[test]
#[ignore = "diagnostic benchmark for cache-invalid repaint cost in overlay-free widget-book gallery"]
fn widget_book_dialog_repaint_benchmark_without_live_overlay() -> Result<()> {
    if skip_without_desktop_display("widget_book_dialog_repaint_benchmark_without_live_overlay") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    run_widget_book_dialog_repaint_benchmark(|| {
        build_widget_book_gallery_application(scroll_benchmark_widget_book_state())
    })
}

#[test]
#[ignore = "diagnostic benchmark for text-heavy retained scroll upload cost"]
fn desktop_retained_text_scroll_upload_benchmark() -> Result<()> {
    if skip_without_desktop_display("desktop_retained_text_scroll_upload_benchmark") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    run_retained_text_scroll_benchmark()
}

#[test]
fn desktop_text_editing_benchmark_reports_frame_samples() -> Result<()> {
    if skip_without_desktop_display("desktop_text_editing_benchmark_reports_frame_samples") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    run_text_editing_benchmark()
}

#[test]
fn desktop_widget_book_overlay_publishes_detailed_scene_stats() -> Result<()> {
    if skip_without_desktop_display("desktop_widget_book_overlay_publishes_detailed_scene_stats") {
        return Ok(());
    }

    let _guard = DESKTOP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let harness = DesktopHarness::launch(|| {
        build_widget_book_application_with_overlay(Rc::new(RefCell::new(WidgetBookState {
            name: "Ada".to_string(),
            subscribed: true,
            theme_preview_comparison: true,
            button_presses: 0,
            icon_button_presses: 0,
            switch_on: true,
            standalone_radio_selected: false,
            radio_choice: "Balanced".to_string(),
            slider_value: 72.0,
            number_value: 12.0,
            notes: "Pinned notes for inspector workflows.\nSupports multiline editing.".to_string(),
            mode: "Normal".to_string(),
            tab_bar_choice: "Canvas".to_string(),
            tabs_choice: "Layout".to_string(),
            last_menu_action: "New tab".to_string(),
            last_context_action: "Rename".to_string(),
            dialog_apply_count: 0,
        })))
        .build()
    })?;
    let window_id = harness.main_window_id();

    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    harness.capture(window_id)?;
    harness.capture(window_id)?;
    let after_snapshot = harness.snapshot(window_id)?;
    let after_performance = after_snapshot.performance.clone().expect(
        "desktop widget book should publish a performance snapshot while overlay is visible",
    );
    let overlay = find_node(
        &after_snapshot,
        SemanticsRole::GenericContainer,
        "Live performance overlay",
    );

    assert!(window_scene_statistics_detail_mode(window_id).is_detailed());
    assert!(after_performance.scene.detail_mode.is_detailed());
    assert!(!after_performance.scene.command_breakdown.is_empty());
    assert!(matches!(overlay.value, Some(SemanticsValue::Text(_))));

    Ok(())
}
