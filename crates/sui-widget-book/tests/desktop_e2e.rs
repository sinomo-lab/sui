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
    Error, Event, ImeEvent, Modifiers, Point,
    PointerButton, PointerButtons, PointerEvent, PointerEventKind, PointerKind, Rect, Result,
    ScrollDelta, SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector, WindowEvent,
    WindowId, window_performance_snapshot, WgpuRenderer,
};
use sui_runtime::{
    CacheMetrics, FramePhase, FramePhaseSample, RenderOutput, RendererSubmissionDiagnostics,
    SceneStatistics, TextCacheDiagnostics, WindowPerformanceSnapshot,
    clear_window_performance_snapshots, publish_window_performance_snapshot,
    window_performance_text_caches, window_scene_statistics_detail_mode,
};
use sui_widget_book::{
    BUTTON_GRID_COLUMNS, BUTTON_GRID_ROWS, BUTTON_GRID_BENCHMARK_TITLE, WidgetBookState,
    build_button_grid_benchmark_application, build_widget_book_application,
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
    CursorMoved { position: Point },
    MouseInput { state: ElementState, button: MouseButton },
    MouseWheel { delta: ScrollKind },
    ImeCommit { text: String },
}

enum HarnessCommand {
    Launch {
        build_runtime: RuntimeBuilder,
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

        let proxy = recv_result(&setup_rx, "desktop harness service setup", Duration::from_secs(3))
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
        let proxy = desktop_harness_service().proxy.clone();
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        proxy.send_event(HarnessCommand::Launch {
            build_runtime: Box::new(build_runtime),
            reply: reply_tx,
        })
        .map_err(|_| Error::new("desktop harness service is unavailable"))?;
        let main_window_id = recv_result(&reply_rx, "desktop harness launch", Duration::from_secs(3))?;

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
        let _ = self.proxy.send_event(HarnessCommand::Reset { reply: reply_tx });
        let _ = reply_rx.recv_timeout(Duration::from_secs(1));
    }
}

struct DesktopHarnessApp {
    runtime: sui::Runtime,
    renderer: WgpuRenderer,
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
        self.renderer = WgpuRenderer::default();
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
    ) -> Result<WindowId> {
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
                    frame_index: 0,
                    pending_event_time_ms: 0.0,
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

        for (window_id, window) in &mut self.windows {
            if window.redraw_requested || !self.runtime.needs_render(*window_id)? {
                continue;
            }

            window.redraw_requested = true;
            window.window.request_redraw();
        }

        self.update_control_flow(event_loop)?;
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

        let is_redraw = matches!(event, Event::Window(WindowEvent::RedrawRequested));
        let is_close = matches!(event, Event::Window(WindowEvent::CloseRequested));

        let event_started = Instant::now();
        self.runtime.handle_event(window_id, event)?;
        let event_time_ms = event_started.elapsed().as_secs_f64() * 1000.0;

        if let Some(window) = self.windows.get_mut(&window_id) {
            window.pending_event_time_ms += event_time_ms;
        }

        if is_redraw {
            if let Some(window) = self.windows.get_mut(&window_id) {
                window.redraw_requested = false;
            }

            if self.runtime.needs_render(window_id)? {
                self.update_clock();
                self.runtime.tick(self.frame_clock);

                let output = self.runtime.render(window_id)?;
                let renderer_started = Instant::now();
                self.renderer.render(&output.frame)?;
                let renderer_time_ms = renderer_started.elapsed().as_secs_f64() * 1000.0;

                let mut frame_index = 0;
                let mut pending_event_time_ms = 0.0;

                if let Some(window) = self.windows.get_mut(&window_id) {
                    frame_index = window.frame_index + 1;
                    pending_event_time_ms = std::mem::take(&mut window.pending_event_time_ms);
                    window.frame_index = frame_index;
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

    fn map_host_event(&self, window_id: WindowId, event: HostInputEvent) -> Result<WinitWindowEvent> {
        let window = self
            .windows
            .get(&window_id)
            .ok_or_else(|| Error::new(format!("window {} is not registered in the desktop harness", window_id.get())))?;
        let scale_factor = window.scale_factor;
        let device_id = DeviceId::dummy();

        let event = match event {
            HostInputEvent::Focused(focused) => WinitWindowEvent::Focused(focused),
            HostInputEvent::CursorEntered => WinitWindowEvent::CursorEntered { device_id },
            HostInputEvent::CursorMoved { position } => WinitWindowEvent::CursorMoved {
                device_id,
                position: logical_point_to_physical_position(position, scale_factor),
            },
            HostInputEvent::MouseInput { state, button } => {
                WinitWindowEvent::MouseInput { device_id, state, button }
            }
            HostInputEvent::MouseWheel { delta } => WinitWindowEvent::MouseWheel {
                device_id,
                delta: match delta {
                    ScrollKind::Pixels(delta) => MouseScrollDelta::PixelDelta(
                        logical_vector_to_physical_position(delta, scale_factor),
                    ),
                },
                phase: TouchPhase::Moved,
            },
            HostInputEvent::ImeCommit { text } => WinitWindowEvent::Ime(Ime::Commit(text)),
        };

        Ok(event)
    }

    fn host_id(&self, window_id: WindowId) -> Result<HostWindowId> {
        self.windows
            .get(&window_id)
            .map(|window| window.window.id())
            .ok_or_else(|| Error::new(format!("window {} is not registered in the desktop harness", window_id.get())))
    }

    fn snapshot(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) -> Result<DesktopWindowSnapshot> {
        self.flush_pending_frames(event_loop)?;

        let window = self.windows.get(&window_id).ok_or_else(|| {
            Error::new(format!("window {} is not registered in the desktop harness", window_id.get()))
        })?;

        Ok(DesktopWindowSnapshot {
            title: window.title.clone(),
            semantics: window.semantics.clone(),
            performance: window_performance_snapshot(window_id),
        })
    }

    fn capture(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) -> Result<CapturedFrame> {
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
                reply,
            } => {
                let _ = reply.send(self.launch_runtime(event_loop, build_runtime));
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
    frame_index: u64,
    pending_event_time_ms: f64,
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
    window.set_ime_allowed(rect.is_some());

    if let Some(rect) = rect {
        window.set_ime_cursor_area(
            LogicalPosition::new(rect.x() as f64, rect.y() as f64),
            LogicalSize::new(rect.width().max(1.0) as f64, rect.height().max(1.0) as f64),
        );
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

fn map_ime_event(event: Ime) -> Option<ImeEvent> {
    match event {
        Ime::Enabled => Some(ImeEvent::CompositionStart),
        Ime::Preedit(text, _) => Some(ImeEvent::CompositionUpdate { text }),
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

fn frame_pixel_diff_count(before: &CapturedFrame, after: &CapturedFrame) -> usize {
    assert_eq!(
        (before.width, before.height),
        (after.width, after.height),
        "desktop framebuffer size changed unexpectedly during the test"
    );

    before
        .pixels
        .iter()
        .zip(after.pixels.iter())
        .filter(|(left, right)| left != right)
        .count()
}

fn publish_frame_performance(
    window_id: WindowId,
    frame_index: u64,
    event_time_ms: f64,
    output: &RenderOutput,
    renderer: &WgpuRenderer,
    renderer_time_ms: f64,
) {
    let diagnostics_started = Instant::now();
    let mut phase_timings = Vec::with_capacity(output.diagnostics.phase_timings.len() + 2);
    let renderer_text_cache = renderer.text_cache_snapshot();
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
    };
    let text_cache_deltas = window_performance_text_caches(window_id)
        .map(|previous| text_caches.delta_from(&previous))
        .unwrap_or_else(|| text_caches.delta_from(&TextCacheDiagnostics::default()));
    let renderer_stats = renderer.last_frame_stats(window_id).unwrap_or_default();

    if event_time_ms > 0.0 {
        phase_timings.push(FramePhaseSample::new(FramePhase::Event, event_time_ms));
    }

    phase_timings.extend(output.diagnostics.phase_timings.iter().copied());
    phase_timings.push(FramePhaseSample::new(FramePhase::Renderer, renderer_time_ms));
    phase_timings.push(FramePhaseSample::new(
        FramePhase::Diagnostics,
        diagnostics_started.elapsed().as_secs_f64() * 1000.0,
    ));

    publish_window_performance_snapshot(WindowPerformanceSnapshot::new(
        window_id,
        frame_index,
        phase_timings,
        RendererSubmissionDiagnostics::new(
            renderer_stats.pass_count,
            renderer_stats.draw_count,
            renderer_stats.uploaded_vertex_bytes,
            renderer_stats.visible_layer_count,
            renderer_stats.visible_tile_count,
            renderer_stats.reused_tile_count,
            renderer_stats.regenerated_tile_count,
            renderer_stats.direct_packet_count,
            renderer_stats.tile_memory_bytes,
            renderer_stats.tile_generation_time_us,
            renderer_stats.composition_time_us,
        ),
        text_caches,
        text_cache_deltas,
        SceneStatistics::from_frame_with_mode(
            &output.frame,
            window_scene_statistics_detail_mode(window_id),
        ),
    ));
}

fn node_center(bounds: Rect) -> Point {
    Point::new(
        bounds.x() + (bounds.width() * 0.5),
        bounds.y() + (bounds.height() * 0.5),
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

fn live_performance_toggle_point(snapshot: &DesktopWindowSnapshot) -> Point {
    let overlay = find_node(
        snapshot,
        SemanticsRole::GenericContainer,
        "Live performance overlay",
    );
    Point::new(
        overlay.bounds.max_x() - 12.0 - 38.0,
        overlay.bounds.y() + 10.0 - 1.0 + 9.0,
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

fn text_input_value(snapshot: &DesktopWindowSnapshot, name: &str) -> String {
    let input = find_node(snapshot, SemanticsRole::TextInput, name);
    match input.value {
        Some(SemanticsValue::Text(value)) => value,
        other => panic!("unexpected text input value for {name}: {other:?}"),
    }
}

fn phase_duration_ms(snapshot: &WindowPerformanceSnapshot, phase: FramePhase) -> f64 {
    let total: f64 = snapshot
        .phase_timings
        .iter()
        .filter(|sample| sample.phase == phase)
        .map(|sample| sample.duration_ms)
        .sum();

    if total == 0.0 {
        0.0
    } else {
        total
    }
}

#[test]
fn desktop_widget_book_repaints_and_updates_metrics_from_platform_events() -> Result<()> {
    let _guard = DESKTOP_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

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
    assert_eq!(before_snapshot.title, sui_widget_book::WINDOW_TITLE);
    assert_eq!(text_input_value(&before_snapshot, sui_widget_book::NAME_INPUT_LABEL), "");

    let input = find_node(
        &before_snapshot,
        SemanticsRole::TextInput,
        sui_widget_book::NAME_INPUT_LABEL,
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

    assert_eq!(text_input_value(&after_snapshot, sui_widget_book::NAME_INPUT_LABEL), "Ada");
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
        sui_widget_book::GALLERY_SCROLL_NAME,
    );
    let before_button = find_node(
        &before_scroll_snapshot,
        SemanticsRole::Button,
        sui_widget_book::PRIMARY_BUTTON_LABEL,
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
        sui_widget_book::PRIMARY_BUTTON_LABEL,
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
fn widget_book_scroll_fps_benchmark() -> Result<()> {
    let _guard = DESKTOP_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let harness = DesktopHarness::launch(|| {
        build_widget_book_application(Rc::new(RefCell::new(WidgetBookState {
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
        })))
        .build()
    })?;
    let window_id = harness.main_window_id();

    harness.dispatch(window_id, HostInputEvent::Focused(true))?;

    // Warm-up: initial render + one scroll to prime caches
    let gallery = find_node(
        &harness.snapshot(window_id)?,
        SemanticsRole::ScrollView,
        sui_widget_book::GALLERY_SCROLL_NAME,
    );
    move_cursor(&harness, window_id, node_center(gallery.bounds))?;
    harness.dispatch(
        window_id,
        HostInputEvent::MouseWheel {
            delta: ScrollKind::Pixels(Vector::new(0.0, -40.0)),
        },
    )?;

    // Benchmark: measure frame times over a series of scroll events
    const FRAME_COUNT: usize = 30;
    let mut frame_times_ms = Vec::with_capacity(FRAME_COUNT);
    let mut phase_totals: HashMap<&str, f64> = HashMap::new();

    let benchmark_start = Instant::now();

    for i in 0..FRAME_COUNT {
        let direction = if i % 2 == 0 { -40.0 } else { 40.0 };
        harness.dispatch(
            window_id,
            HostInputEvent::MouseWheel {
                delta: ScrollKind::Pixels(Vector::new(0.0, direction)),
            },
        )?;

        let snapshot = harness.snapshot(window_id)?;
        if let Some(performance) = &snapshot.performance {
            frame_times_ms.push(performance.total_time_ms);

            for sample in &performance.phase_timings {
                *phase_totals
                    .entry(sample.phase.label())
                    .or_insert(0.0) += sample.duration_ms;
            }
        }
    }

    let benchmark_elapsed_ms = benchmark_start.elapsed().as_secs_f64() * 1000.0;

    // Report results
    let valid_count = frame_times_ms.len();
    assert!(
        valid_count >= FRAME_COUNT / 2,
        "expected at least {} valid frame measurements, got {}",
        FRAME_COUNT / 2,
        valid_count,
    );

    let avg_ms: f64 = frame_times_ms.iter().sum::<f64>() / valid_count as f64;
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

    println!("\n=== Widget Book Scroll FPS Benchmark ===");
    println!("frames measured: {valid_count}");
    println!("wall-clock time: {benchmark_elapsed_ms:.1} ms");
    println!("avg frame time:  {avg_ms:.3} ms ({:.0} fps)", 1000.0 / avg_ms);
    println!("min frame time:  {min_ms:.3} ms");
    println!("max frame time:  {max_ms:.3} ms");
    println!("p95 frame time:  {p95_ms:.3} ms ({:.0} fps)", 1000.0 / p95_ms);

    println!("\n--- Phase breakdown (avg per frame) ---");
    let mut phase_entries: Vec<_> = phase_totals.iter().collect();
    phase_entries.sort_by(|a, b| b.1.total_cmp(a.1));
    for (phase, total_ms) in &phase_entries {
        let avg_phase_ms = *total_ms / valid_count as f64;
        let pct = (*total_ms / frame_times_ms.iter().sum::<f64>()) * 100.0;
        println!("  {phase:<22} {avg_phase_ms:>8.3} ms  ({pct:>5.1}%)");
    }

    if let Some(snapshot) = harness.snapshot(window_id)?.performance {
        println!("\n--- Last frame scene stats ---");
        println!("  commands:        {}", snapshot.scene.command_count);
        println!("  dirty regions:   {}", snapshot.scene.dirty_region_count);
        println!("  dirty coverage:  {:.1}%", snapshot.scene.dirty_coverage);
        println!("  gpu draws:       {}", snapshot.renderer_submission.draw_count);
        println!("  gpu passes:      {}", snapshot.renderer_submission.pass_count);
        println!("  tiles visible:   {}", snapshot.renderer_submission.visible_tile_count);
        println!("  tiles reused:    {}", snapshot.renderer_submission.reused_tile_count);
        println!("  tiles regen:     {}", snapshot.renderer_submission.regenerated_tile_count);
        println!("  vertex bytes:    {}", snapshot.renderer_submission.uploaded_vertex_bytes);
    }
    println!("========================================\n");

    assert!(
        avg_ms < 16.67,
        "average frame time {avg_ms:.3} ms exceeds the 16.67 ms budget for 60 fps",
    );

    Ok(())
}

#[test]
fn desktop_button_grid_64_reports_initial_render_time() -> Result<()> {
    let _guard = DESKTOP_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let harness = DesktopHarness::launch(|| build_button_grid_benchmark_application().build())?;
    let snapshot = harness.snapshot(harness.main_window_id())?;
    let performance = snapshot
        .performance
        .clone()
        .expect("64-button grid should publish a performance snapshot after the first paint");
    let button_count = snapshot
        .semantics
        .iter()
        .filter(|node| node.role == SemanticsRole::Button)
        .count();
    let mut row_positions: Vec<i32> = snapshot
        .semantics
        .iter()
        .filter(|node| node.role == SemanticsRole::Button)
        .map(|node| node.bounds.y().round() as i32)
        .collect();
    row_positions.sort_unstable();
    row_positions.dedup();
    let slowest_phase = performance.slowest_phase();

    assert_eq!(snapshot.title, BUTTON_GRID_BENCHMARK_TITLE);
    assert_eq!(button_count, BUTTON_GRID_ROWS * BUTTON_GRID_COLUMNS);
    assert_eq!(row_positions.len(), BUTTON_GRID_ROWS);
    assert!(performance.frame_index > 0);
    assert!(performance.total_time_ms >= 0.0);
    assert!(performance.renderer_submission.draw_count > 0);

    println!(
        "64-button grid first frame: total={:.3} ms, paint={:.3} ms, renderer={:.3} ms, draws={}, commands={}, slowest={} ({:.3} ms)",
        performance.total_time_ms,
        phase_duration_ms(&performance, FramePhase::Paint),
        phase_duration_ms(&performance, FramePhase::Renderer),
        performance.renderer_submission.draw_count,
        performance.scene.command_count,
        slowest_phase
            .map(|sample| sample.phase.label())
            .unwrap_or("none"),
        slowest_phase.map(|sample| sample.duration_ms).unwrap_or(0.0),
    );

    Ok(())
}

#[test]
fn desktop_widget_book_overlay_toggle_publishes_detailed_scene_stats() -> Result<()> {
    let _guard = DESKTOP_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let harness = DesktopHarness::launch(|| {
        build_widget_book_application(Rc::new(RefCell::new(WidgetBookState {
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

    let before_frame = harness.capture(window_id)?;
    let before_snapshot = harness.snapshot(window_id)?;
    let before_performance = before_snapshot
        .performance
        .clone()
        .expect("desktop widget book should publish an initial performance snapshot");

    assert!(!before_performance.scene.detail_mode.is_detailed());

    click_at(
        &harness,
        window_id,
        live_performance_toggle_point(&before_snapshot),
    )?;

    let after_frame = harness.capture(window_id)?;
    let after_snapshot = harness.snapshot(window_id)?;
    let after_performance = after_snapshot
        .performance
        .clone()
        .expect("desktop widget book should publish a performance snapshot after toggling detail mode");
    let overlay = find_node(
        &after_snapshot,
        SemanticsRole::GenericContainer,
        "Live performance overlay",
    );

    assert!(frame_pixel_diff_count(&before_frame, &after_frame) > 0);
    assert!(window_scene_statistics_detail_mode(window_id).is_detailed());
    assert!(after_performance.scene.detail_mode.is_detailed());
    assert!(!after_performance.scene.command_breakdown.is_empty());
    assert_eq!(
        overlay.value,
        Some(SemanticsValue::Text("detail on".to_string()))
    );

    Ok(())
}