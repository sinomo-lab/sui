use std::collections::VecDeque;

use sui_core::{AsyncWakeToken, Error, Event, Result, Size, WindowEvent, WindowId};
use sui_render_wgpu::WgpuRenderer;
use sui_runtime::Runtime;

use crate::{AccessibilityBridge, AccessibilitySnapshot};

#[derive(Debug, Clone)]
pub struct PlatformWindow {
    pub id: WindowId,
    pub title: String,
    pub accessibility: Option<AccessibilitySnapshot>,
}

#[derive(Debug, Default)]
pub struct HeadlessPlatform {
    renderer: WgpuRenderer,
    windows: Vec<WindowState>,
    pending_events: VecDeque<QueuedEvent>,
    frame_clock: f64,
}

impl HeadlessPlatform {
    const DEFAULT_WINDOW_SIZE: Size = Size::new(1280.0, 720.0);

    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(&mut self, runtime: &mut Runtime) -> Result<Vec<PlatformWindow>> {
        while self.pump(runtime)? {}

        Ok(self
            .windows
            .iter()
            .filter(|window| window.open)
            .map(WindowState::snapshot)
            .collect())
    }

    pub fn pump(&mut self, runtime: &mut Runtime) -> Result<bool> {
        self.sync_windows(runtime)?;
        runtime.tick(self.frame_clock);
        self.queue_ready_events(runtime);
        self.queue_redraw_requests(runtime)?;

        let mut did_work = false;

        while let Some(queued_event) = self.pending_events.pop_front() {
            did_work = true;
            self.process_event(runtime, queued_event)?;
        }

        Ok(did_work)
    }

    pub fn advance_time(&mut self, delta: f64) {
        self.frame_clock += delta;
    }

    pub fn wake_async(
        &mut self,
        runtime: &mut Runtime,
        window_id: WindowId,
        token: AsyncWakeToken,
    ) -> Result<bool> {
        self.sync_windows(runtime)?;
        runtime.wake_async(window_id, token)
    }

    pub fn current_time(&self) -> f64 {
        self.frame_clock
    }

    pub fn pending_event_count(&self) -> usize {
        self.pending_events.len()
    }

    pub fn has_pending_events(&self) -> bool {
        !self.pending_events.is_empty()
    }

    pub fn dispatch_event(
        &mut self,
        runtime: &Runtime,
        window_id: WindowId,
        event: Event,
    ) -> Result<()> {
        self.sync_windows(runtime)?;

        if !self
            .windows
            .iter()
            .any(|window| window.id == window_id && window.open)
        {
            return Err(Error::new(format!(
                "window {} is not registered with the headless platform",
                window_id.get()
            )));
        }

        self.pending_events
            .push_back(QueuedEvent { window_id, event });
        Ok(())
    }

    pub fn renderer(&self) -> &WgpuRenderer {
        &self.renderer
    }

    pub fn accessibility_snapshot(&self, window_id: WindowId) -> Option<&AccessibilitySnapshot> {
        self.windows
            .iter()
            .find(|window| window.id == window_id && window.open)
            .and_then(|window| window.accessibility.snapshot())
    }

    fn queue_ready_events(&mut self, runtime: &mut Runtime) {
        self.pending_events.extend(
            runtime
                .drain_ready_events()
                .into_iter()
                .map(|(window_id, event)| QueuedEvent { window_id, event }),
        );
    }

    fn sync_windows(&mut self, runtime: &Runtime) -> Result<()> {
        let runtime_window_ids = runtime.window_ids();

        let removed_ids: Vec<_> = self
            .windows
            .iter()
            .map(|window| window.id)
            .filter(|window_id| !runtime_window_ids.contains(window_id))
            .collect();

        for window_id in removed_ids {
            self.renderer.remove_window(window_id);
        }

        self.windows
            .retain(|window| runtime_window_ids.contains(&window.id));
        self.pending_events
            .retain(|queued_event| runtime_window_ids.contains(&queued_event.window_id));

        for window_id in runtime_window_ids {
            if self.windows.iter().any(|window| window.id == window_id) {
                continue;
            }

            self.windows.push(WindowState {
                id: window_id,
                title: runtime.window_title(window_id)?.to_string(),
                open: true,
                redraw_requested: false,
                accessibility: AccessibilityBridge::default(),
            });
            self.pending_events.push_back(QueuedEvent {
                window_id,
                event: Event::Window(WindowEvent::Resized(Self::DEFAULT_WINDOW_SIZE)),
            });
        }

        Ok(())
    }

    fn queue_redraw_requests(&mut self, runtime: &Runtime) -> Result<()> {
        for window in &mut self.windows {
            if !window.open || window.redraw_requested || !runtime.needs_render(window.id)? {
                continue;
            }

            window.redraw_requested = true;
            self.pending_events.push_back(QueuedEvent {
                window_id: window.id,
                event: Event::Window(WindowEvent::RedrawRequested),
            });
        }

        Ok(())
    }

    fn process_event(&mut self, runtime: &mut Runtime, queued_event: QueuedEvent) -> Result<()> {
        let Some(window_index) = self
            .windows
            .iter()
            .position(|window| window.id == queued_event.window_id && window.open)
        else {
            return Ok(());
        };

        let is_redraw = matches!(
            queued_event.event,
            Event::Window(WindowEvent::RedrawRequested)
        );
        let is_close = matches!(
            queued_event.event,
            Event::Window(WindowEvent::CloseRequested)
        );
        let window_id = queued_event.window_id;

        runtime.handle_event(window_id, queued_event.event)?;

        if is_redraw {
            self.windows[window_index].redraw_requested = false;

            if runtime.needs_render(window_id)? {
                self.frame_clock += 1.0;
                runtime.tick(self.frame_clock);

                let output = runtime.render(window_id)?;
                self.windows[window_index].title = output.title;
                self.windows[window_index]
                    .accessibility
                    .update(window_id, output.semantics);
                self.renderer.render(&output.frame)?;
            }
        }

        if is_close {
            runtime.remove_window(window_id)?;
            self.renderer.remove_window(window_id);
            self.pending_events
                .retain(|pending_event| pending_event.window_id != window_id);
            self.windows.swap_remove(window_index);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct WindowState {
    id: WindowId,
    title: String,
    open: bool,
    redraw_requested: bool,
    accessibility: AccessibilityBridge,
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

#[derive(Debug, Clone)]
struct QueuedEvent {
    window_id: WindowId,
    event: Event,
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::HeadlessPlatform;
    use sui_core::{
        AsyncWakeToken, Color, CustomEvent, Event, Rect, Result, SemanticsNode, SemanticsRole,
        TimerToken, WakeEvent, WindowEvent,
    };
    use sui_runtime::{
        Application, EventCtx, PaintCtx, Runtime, SemanticsCtx, Widget, WindowBuilder,
    };

    #[derive(Default)]
    struct Counters {
        events: usize,
        paints: usize,
        timer_wakes: usize,
        async_wakes: usize,
        timer_token: Option<TimerToken>,
        async_token: Option<AsyncWakeToken>,
    }

    struct TestRoot {
        counters: Rc<RefCell<Counters>>,
    }

    impl Widget for TestRoot {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::Custom(custom) if custom.kind == "repaint" => {
                    self.counters.borrow_mut().events += 1;
                    ctx.request_paint();
                }
                Event::Custom(custom) if custom.kind == "arm-wakeups" => {
                    let mut counters = self.counters.borrow_mut();
                    counters.timer_token = Some(ctx.schedule_timer_after(3.0));
                    counters.async_token = Some(ctx.register_async_wakeup());
                }
                Event::Wake(WakeEvent::Timer { token, .. }) => {
                    let mut counters = self.counters.borrow_mut();
                    if counters.timer_token == Some(*token) {
                        counters.timer_wakes += 1;
                        ctx.request_paint();
                        ctx.set_handled();
                    }
                }
                Event::Wake(WakeEvent::Async { token, .. }) => {
                    let mut counters = self.counters.borrow_mut();
                    if counters.async_token == Some(*token) {
                        counters.async_wakes += 1;
                        ctx.request_paint();
                        ctx.set_handled();
                    }
                }
                _ => {}
            }
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.counters.borrow_mut().paints += 1;
            ctx.clear(Color::rgba(0.08, 0.09, 0.11, 1.0));
            ctx.fill_rect(
                Rect::new(24.0, 24.0, 120.0, 48.0),
                Color::rgba(0.16, 0.19, 0.25, 1.0),
            );
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            ctx.push(SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::Window,
                ctx.bounds(),
            ));
        }
    }

    fn build_runtime(counters: Rc<RefCell<Counters>>) -> (Runtime, sui_core::WindowId) {
        let runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Test")
                    .root(TestRoot { counters }),
            )
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    #[test]
    fn queued_events_reenter_the_platform_render_loop() -> Result<()> {
        let counters = Rc::new(RefCell::new(Counters::default()));
        let (mut runtime, window_id) = build_runtime(Rc::clone(&counters));
        let mut platform = HeadlessPlatform::new();

        let windows = platform.run(&mut runtime)?;
        let accessibility = platform.accessibility_snapshot(window_id).unwrap();

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "Test");
        assert_eq!(windows[0].accessibility.as_ref(), Some(accessibility));
        assert_eq!(accessibility.nodes.len(), 1);
        assert_eq!(accessibility.nodes[0].role, SemanticsRole::Window);
        assert_eq!(platform.renderer().frames_rendered(), 1);
        assert_eq!(counters.borrow().paints, 1);

        platform.dispatch_event(
            &runtime,
            window_id,
            Event::Custom(CustomEvent::new("repaint")),
        )?;
        let _ = platform.run(&mut runtime)?;

        assert_eq!(counters.borrow().events, 1);
        assert_eq!(counters.borrow().paints, 2);
        assert_eq!(platform.renderer().frames_rendered(), 2);
        assert!(!runtime.needs_render(window_id)?);

        Ok(())
    }

    #[test]
    fn close_requested_removes_window_from_platform_and_runtime() -> Result<()> {
        let counters = Rc::new(RefCell::new(Counters::default()));
        let (mut runtime, window_id) = build_runtime(counters);
        let mut platform = HeadlessPlatform::new();

        let _ = platform.run(&mut runtime)?;

        platform.dispatch_event(
            &runtime,
            window_id,
            Event::Window(WindowEvent::CloseRequested),
        )?;
        let windows = platform.run(&mut runtime)?;

        assert!(windows.is_empty());
        assert!(runtime.window_ids().is_empty());
        assert!(runtime.needs_render(window_id).is_err());

        Ok(())
    }

    #[test]
    fn wakeups_reenter_the_platform_pump_and_trigger_repaint() -> Result<()> {
        let counters = Rc::new(RefCell::new(Counters::default()));
        let (mut runtime, window_id) = build_runtime(Rc::clone(&counters));
        let mut platform = HeadlessPlatform::new();

        let _ = platform.run(&mut runtime)?;

        platform.dispatch_event(
            &runtime,
            window_id,
            Event::Custom(CustomEvent::new("arm-wakeups")),
        )?;
        let _ = platform.run(&mut runtime)?;

        let async_token = counters.borrow().async_token.unwrap();
        assert_eq!(counters.borrow().paints, 1);

        assert!(platform.wake_async(&mut runtime, window_id, async_token)?);
        let _ = platform.run(&mut runtime)?;

        assert_eq!(counters.borrow().async_wakes, 1);
        assert_eq!(counters.borrow().paints, 2);

        platform.advance_time(3.0);
        let _ = platform.run(&mut runtime)?;

        assert_eq!(counters.borrow().timer_wakes, 1);
        assert_eq!(counters.borrow().paints, 3);

        Ok(())
    }
}
