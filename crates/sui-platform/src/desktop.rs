use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use sui_core::{
    Error, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
    PointerButtons, PointerEvent, PointerEventKind, PointerKind, Result, ScrollDelta, Size, Vector,
    WindowEvent, WindowId,
};
use sui_render_wgpu::WgpuRenderer;
use sui_runtime::Runtime;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    error::{EventLoopError, OsError},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent as WinitWindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey, PhysicalKey},
    window::{Window, WindowAttributes, WindowId as HostWindowId},
};

use crate::{AccessibilityBridge, headless::PlatformWindow};

#[derive(Debug, Default)]
pub struct DesktopPlatform {
    renderer: WgpuRenderer,
}

impl DesktopPlatform {
    const DEFAULT_WINDOW_SIZE: Size = Size::new(1280.0, 720.0);

    pub fn new() -> Self {
        Self::default()
    }

    pub fn renderer(&self) -> &WgpuRenderer {
        &self.renderer
    }

    pub fn run(&mut self, runtime: &mut Runtime) -> Result<Vec<PlatformWindow>> {
        let event_loop = EventLoop::new().map_err(map_event_loop_error)?;
        let mut app = DesktopApp::new(runtime, &mut self.renderer);

        event_loop.run_app(&mut app).map_err(map_event_loop_error)?;

        if let Some(error) = app.last_error.take() {
            return Err(error);
        }

        Ok(app.snapshot_windows())
    }
}

struct DesktopApp<'a> {
    runtime: &'a mut Runtime,
    renderer: &'a mut WgpuRenderer,
    started_at: Instant,
    frame_clock: f64,
    windows: HashMap<WindowId, WindowState>,
    host_to_runtime: HashMap<HostWindowId, WindowId>,
    last_error: Option<Error>,
}

impl<'a> DesktopApp<'a> {
    fn new(runtime: &'a mut Runtime, renderer: &'a mut WgpuRenderer) -> Self {
        Self {
            runtime,
            renderer,
            started_at: Instant::now(),
            frame_clock: 0.0,
            windows: HashMap::new(),
            host_to_runtime: HashMap::new(),
            last_error: None,
        }
    }

    fn snapshot_windows(&self) -> Vec<PlatformWindow> {
        self.windows.values().map(WindowState::snapshot).collect()
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
                            .with_title(title.clone())
                            .with_inner_size(LogicalSize::new(
                                DesktopPlatform::DEFAULT_WINDOW_SIZE.width,
                                DesktopPlatform::DEFAULT_WINDOW_SIZE.height,
                            )),
                    )
                    .map_err(map_os_error)?,
            );
            window.set_ime_allowed(false);

            let host_id = window.id();
            let size = physical_size_to_size(window.inner_size());
            self.renderer
                .register_window(window_id, Arc::clone(&window))?;

            self.host_to_runtime.insert(host_id, window_id);
            self.windows.insert(
                window_id,
                WindowState {
                    id: window_id,
                    title,
                    redraw_requested: false,
                    accessibility: AccessibilityBridge::default(),
                    pointer: PointerState::default(),
                    window,
                },
            );

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

    fn update_clock(&mut self) {
        self.frame_clock = self.started_at.elapsed().as_secs_f64();
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

        let is_redraw = matches!(event, Event::Window(WindowEvent::RedrawRequested));
        let is_close = matches!(event, Event::Window(WindowEvent::CloseRequested));

        self.runtime.handle_event(window_id, event)?;

        if is_redraw {
            if let Some(window) = self.windows.get_mut(&window_id) {
                window.redraw_requested = false;
            }

            if self.runtime.needs_render(window_id)? {
                self.update_clock();
                self.runtime.tick(self.frame_clock);

                let output = self.runtime.render(window_id)?;
                let semantics = output.semantics.clone();
                self.renderer.render(&output.frame)?;

                if let Some(window) = self.windows.get_mut(&window_id) {
                    if window.title != output.title {
                        window.title = output.title.clone();
                        window.window.set_title(&output.title);
                    }

                    window.accessibility.update(window_id, semantics);

                    apply_ime_composition_rect(window.window.as_ref(), output.ime_composition_rect);
                }
            }
        }

        if is_close {
            self.runtime.remove_window(window_id)?;
            self.sync_windows(event_loop)?;
        }

        if self.windows.is_empty() {
            event_loop.exit();
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
                Event::Window(WindowEvent::Resized(physical_size_to_size(size))),
            ),
            WinitWindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let suggested_size = self
                    .windows
                    .get(&window_id)
                    .map(|window| physical_size_to_size(window.window.inner_size()));
                self.process_event(
                    event_loop,
                    window_id,
                    Event::Window(WindowEvent::ScaleFactorChanged {
                        scale_factor,
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
                    let next_position = physical_position_to_point(position);
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
                                Vector::new(position.x as f32, position.y as f32),
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
}

impl ApplicationHandler for DesktopApp<'_> {
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

struct WindowState {
    id: WindowId,
    title: String,
    redraw_requested: bool,
    accessibility: AccessibilityBridge,
    pointer: PointerState,
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

fn physical_size_to_size(size: PhysicalSize<u32>) -> Size {
    Size::new(size.width as f32, size.height as f32)
}

fn physical_position_to_point(position: PhysicalPosition<f64>) -> Point {
    Point::new(position.x as f32, position.y as f32)
}

fn apply_ime_composition_rect(window: &Window, rect: Option<sui_core::Rect>) {
    window.set_ime_allowed(rect.is_some());

    if let Some(rect) = rect {
        window.set_ime_cursor_area(
            LogicalPosition::new(rect.x() as f64, rect.y() as f64),
            LogicalSize::new(rect.width().max(1.0) as f64, rect.height().max(1.0) as f64),
        );
    }
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
