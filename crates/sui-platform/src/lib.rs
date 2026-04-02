#![forbid(unsafe_code)]

use std::collections::VecDeque;

use sui_core::{Error, Event, Result, Size, WindowEvent, WindowId};
use sui_render_wgpu::WgpuRenderer;
use sui_runtime::Runtime;

#[derive(Debug, Clone)]
pub struct PlatformWindow {
    pub id: WindowId,
    pub title: String,
}

#[derive(Debug, Default)]
pub struct DesktopPlatform {
    renderer: WgpuRenderer,
    windows: Vec<WindowState>,
    pending_events: VecDeque<QueuedEvent>,
    frame_clock: f64,
}

impl DesktopPlatform {
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
        self.queue_redraw_requests(runtime)?;

        let mut did_work = false;

        while let Some(queued_event) = self.pending_events.pop_front() {
            did_work = true;
            self.process_event(runtime, queued_event)?;
        }

        Ok(did_work)
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
                "window {} is not registered with the desktop platform",
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

    fn sync_windows(&mut self, runtime: &Runtime) -> Result<()> {
        for window_id in runtime.window_ids() {
            if self.windows.iter().any(|window| window.id == window_id) {
                continue;
            }

            self.windows.push(WindowState {
                id: window_id,
                title: runtime.window_title(window_id)?.to_string(),
                open: true,
                redraw_requested: false,
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
                self.renderer.render(&output.frame);
            }
        }

        if is_close {
            self.windows[window_index].open = false;
            self.windows[window_index].redraw_requested = false;
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
}

impl WindowState {
    fn snapshot(&self) -> PlatformWindow {
        PlatformWindow {
            id: self.id,
            title: self.title.clone(),
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

    use super::DesktopPlatform;
    use sui_core::{Color, CustomEvent, Event, Rect, Result, SemanticsNode, SemanticsRole};
    use sui_runtime::{
        Application, EventCtx, PaintCtx, Runtime, SemanticsCtx, Widget, WindowBuilder,
    };

    #[derive(Default)]
    struct Counters {
        events: usize,
        paints: usize,
    }

    struct TestRoot {
        counters: Rc<RefCell<Counters>>,
    }

    impl Widget for TestRoot {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Custom(custom) = event
                && custom.kind == "repaint"
            {
                self.counters.borrow_mut().events += 1;
                ctx.request_paint();
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
        let mut platform = DesktopPlatform::new();

        let windows = platform.run(&mut runtime)?;

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "Test");
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
}
