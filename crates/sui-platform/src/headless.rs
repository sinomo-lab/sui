use std::{collections::VecDeque, time::Instant};

use sui_core::{AsyncWakeToken, Error, Event, Result, Size, WindowEvent, WindowId};
use sui_render_wgpu::{
    DebugCaptureArtifact, DebugCaptureRequest, FeatheringOptions, RgbaImage, WgpuRenderer,
};
use sui_runtime::{
    PresentationLatencyDiagnostics, Runtime, window_render_options,
    window_scene_statistics_detail_mode,
};

use crate::{
    AccessibilityBridge, AccessibilitySnapshot, map_window_stem_darkening, map_window_text_hinting,
    map_window_text_render_policy,
};

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

    pub fn run(&mut self, runtime: &mut Runtime) -> Result<Vec<PlatformWindow>> {
        loop {
            if self.pump(runtime)? {
                continue;
            }
            if !self.advance_to_next_wakeup(runtime)? {
                break;
            }
        }

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

    fn advance_to_next_wakeup(&mut self, runtime: &Runtime) -> Result<bool> {
        let mut next: Option<f64> = None;
        for window_id in runtime.window_ids() {
            let candidate = runtime.next_wakeup_time(window_id)?;
            next = match (next, candidate) {
                (Some(current), Some(candidate)) => Some(current.min(candidate)),
                (None, Some(candidate)) => Some(candidate),
                (current, None) => current,
            };
        }

        let Some(next) = next else {
            return Ok(false);
        };
        if next <= self.frame_clock {
            return Ok(false);
        }

        self.frame_clock = next;
        Ok(true)
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

    pub fn feather_width(&self) -> f32 {
        self.renderer.feather_width()
    }

    pub fn feathering_enabled(&self) -> bool {
        self.renderer.feathering_enabled()
    }

    pub fn set_feather_width(&mut self, feather_width: f32) {
        self.renderer.set_feather_width(feather_width);
    }

    pub fn set_feathering_enabled(&mut self, enabled: bool) {
        self.renderer.set_feathering_enabled(enabled);
    }

    pub fn renderer_mut(&mut self) -> &mut WgpuRenderer {
        &mut self.renderer
    }

    pub fn capture_rgba(&self, window_id: WindowId) -> Result<RgbaImage> {
        self.renderer.capture_rgba(window_id)
    }

    pub fn capture_debug_frame(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        self.renderer.capture_last_frame_debug(window_id, request)
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
            crate::clear_window_performance(window_id);
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
                awaiting_performance_bootstrap: true,
                redraw_requested: false,
                redraw_requested_at_ms: None,
                frame_index: 0,
                pending_event_time_ms: 0.0,
                last_non_redraw_event_at_ms: None,
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
        let now_ms = self.current_time() * 1000.0;

        for window in &mut self.windows {
            if !window.open || window.redraw_requested || !runtime.needs_render(window.id)? {
                continue;
            }

            window.redraw_requested = true;
            window.redraw_requested_at_ms = Some(now_ms);
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
        let event_arrived_at_ms = self.current_time() * 1000.0;

        let event_started = Instant::now();
        runtime.handle_event(window_id, queued_event.event)?;
        let event_time_ms = event_started.elapsed().as_secs_f64() * 1000.0;
        if !is_redraw {
            self.windows[window_index].pending_event_time_ms += event_time_ms;
        }
        if !is_redraw && !is_close {
            self.windows[window_index].last_non_redraw_event_at_ms = Some(event_arrived_at_ms);
        }

        if is_redraw {
            self.windows[window_index].redraw_requested = false;

            if runtime.needs_render(window_id)? {
                runtime.tick(self.frame_clock);

                let render_started_at_ms = self.current_time() * 1000.0;
                let mut presentation_latency = PresentationLatencyDiagnostics::new(
                    self.windows[window_index]
                        .last_non_redraw_event_at_ms
                        .map(|timestamp| (render_started_at_ms - timestamp).max(0.0))
                        .unwrap_or(0.0),
                    0.0,
                    self.windows[window_index]
                        .redraw_requested_at_ms
                        .map(|timestamp| (render_started_at_ms - timestamp).max(0.0))
                        .unwrap_or(0.0),
                );

                let runtime_started = Instant::now();
                let output = runtime.render(window_id)?;
                let runtime_time_ms = runtime_started.elapsed().as_secs_f64() * 1000.0;
                let renderer_started = Instant::now();
                let diagnostics_enabled =
                    window_scene_statistics_detail_mode(window_id).is_detailed();
                self.renderer
                    .set_runtime_diagnostics_enabled(diagnostics_enabled);
                let render_options = window_render_options(window_id);
                self.renderer
                    .set_runtime_feathering_override(render_options.map(|options| {
                        FeatheringOptions::new(options.feathering_enabled, options.feather_width)
                    }));
                self.renderer.set_runtime_text_coverage_policy_override(
                    render_options
                        .map(|options| map_window_text_render_policy(options.text_render_policy)),
                );
                self.renderer.set_runtime_text_hinting_override(
                    render_options.map(|options| map_window_text_hinting(options.text_hinting)),
                );
                self.renderer.set_runtime_stem_darkening_override(
                    render_options.map(|options| map_window_stem_darkening(options.stem_darkening)),
                );
                self.renderer.set_runtime_glyph_pixel_alignment_override(
                    render_options.map(|options| options.glyph_pixel_alignment_enabled),
                );
                self.renderer.render(&output.frame)?;
                let renderer_time_ms = renderer_started.elapsed().as_secs_f64() * 1000.0;
                let presented_at_ms = self.current_time() * 1000.0;
                presentation_latency.event_to_present_ms = self.windows[window_index]
                    .last_non_redraw_event_at_ms
                    .map(|timestamp| (presented_at_ms - timestamp).max(0.0))
                    .unwrap_or(0.0);

                self.windows[window_index].frame_index += 1;
                let frame_index = self.windows[window_index].frame_index;
                let pending_event_time_ms =
                    std::mem::take(&mut self.windows[window_index].pending_event_time_ms);
                self.windows[window_index].last_non_redraw_event_at_ms = None;
                self.windows[window_index].redraw_requested_at_ms = None;

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

                if self.windows[window_index].awaiting_performance_bootstrap {
                    self.windows[window_index].awaiting_performance_bootstrap = false;
                    if !self.windows[window_index].redraw_requested {
                        self.windows[window_index].redraw_requested = true;
                        self.windows[window_index].redraw_requested_at_ms =
                            Some(self.current_time() * 1000.0);
                        self.pending_events.push_back(QueuedEvent {
                            window_id,
                            event: Event::Window(WindowEvent::RedrawRequested),
                        });
                    }
                }

                self.windows[window_index].title = output.title;
                self.windows[window_index]
                    .accessibility
                    .update(window_id, output.semantics);
            }
        }

        if is_close {
            runtime.remove_window(window_id)?;
            self.renderer.remove_window(window_id);
            crate::clear_window_performance(window_id);
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
    awaiting_performance_bootstrap: bool,
    redraw_requested: bool,
    redraw_requested_at_ms: Option<f64>,
    frame_index: u64,
    pending_event_time_ms: f64,
    last_non_redraw_event_at_ms: Option<f64>,
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
    use std::{cell::RefCell, f32::consts::TAU, rc::Rc};

    use super::HeadlessPlatform;
    use sui_core::{
        AsyncWakeToken, Color, CustomEvent, Event, Rect, Result, SemanticsNode, SemanticsRole,
        Size, TimerToken, WakeEvent, WindowEvent,
    };
    use sui_layout::Constraints;
    use sui_runtime::{
        Application, ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, Runtime,
        SceneStatisticsDetailMode, SemanticsCtx, Widget, WindowBuilder, WindowColorManagementMode,
        WindowDynamicRangeMode, WindowOutputColorPrimaries, WindowRenderOptions,
        WindowToneMappingMode, set_window_render_options, set_window_scene_statistics_detail_mode,
        window_performance_snapshot,
    };

    #[derive(Default)]
    struct Counters {
        events: usize,
        paints: usize,
        timer_wakes: usize,
        async_wakes: usize,
        animation_wakes: usize,
        pending_animation_frames: usize,
        last_animation_delta: Option<f64>,
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
                Event::Custom(custom) if custom.kind == "arm-animation" => {
                    let mut counters = self.counters.borrow_mut();
                    counters.pending_animation_frames = 3;
                    counters.last_animation_delta = None;
                    ctx.request_animation_frame();
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
                Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                    let mut counters = self.counters.borrow_mut();
                    if counters.pending_animation_frames > 0 {
                        counters.pending_animation_frames -= 1;
                        counters.animation_wakes += 1;
                        counters.last_animation_delta = Some(*delta);
                        ctx.request_paint();
                        if counters.pending_animation_frames > 0 {
                            ctx.request_animation_frame();
                        }
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
        assert_eq!(platform.renderer().frames_rendered(), 2);
        assert_eq!(counters.borrow().paints, 1);
        assert!(!platform.has_pending_events());

        platform.dispatch_event(
            &runtime,
            window_id,
            Event::Custom(CustomEvent::new("repaint")),
        )?;
        let _ = platform.run(&mut runtime)?;

        assert_eq!(counters.borrow().events, 1);
        assert_eq!(counters.borrow().paints, 2);
        assert_eq!(platform.renderer().frames_rendered(), 3);
        assert!(!platform.has_pending_events());
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
        assert!(platform.pump(&mut runtime)?);

        let async_token = counters.borrow().async_token.unwrap();
        assert_eq!(counters.borrow().paints, 1);
        assert_eq!(counters.borrow().timer_wakes, 0);

        assert!(platform.wake_async(&mut runtime, window_id, async_token)?);
        assert!(platform.pump(&mut runtime)?);

        assert_eq!(counters.borrow().async_wakes, 1);
        assert_eq!(counters.borrow().paints, 1);
        assert!(platform.pump(&mut runtime)?);
        assert_eq!(counters.borrow().paints, 2);

        platform.advance_time(3.0);
        assert!(platform.pump(&mut runtime)?);

        assert_eq!(counters.borrow().timer_wakes, 1);
        assert_eq!(counters.borrow().paints, 2);
        assert!(platform.pump(&mut runtime)?);
        assert_eq!(counters.borrow().paints, 3);

        Ok(())
    }

    #[test]
    fn animation_frame_request_keeps_headless_pumping_until_completion() -> Result<()> {
        let counters = Rc::new(RefCell::new(Counters::default()));
        let (mut runtime, window_id) = build_runtime(Rc::clone(&counters));
        let mut platform = HeadlessPlatform::new();

        let _ = platform.run(&mut runtime)?;
        assert_eq!(counters.borrow().paints, 1);

        platform.dispatch_event(
            &runtime,
            window_id,
            Event::Custom(CustomEvent::new("arm-animation")),
        )?;
        let _ = platform.run(&mut runtime)?;

        let counters = counters.borrow();
        assert_eq!(counters.animation_wakes, 3);
        assert_eq!(counters.pending_animation_frames, 0);
        assert_eq!(counters.paints, 4);
        assert_eq!(counters.last_animation_delta, Some(1.0 / 120.0));
        assert!((platform.current_time() - (2.0 / 120.0)).abs() < 1e-9);

        Ok(())
    }

    #[test]
    fn feather_width_is_configurable_without_renderer_access() {
        let mut platform = HeadlessPlatform::new()
            .with_feathering_enabled(false)
            .with_feather_width(2.5);

        assert!(!platform.feathering_enabled());
        assert_eq!(platform.feather_width(), 2.5);

        platform.set_feather_width(-4.0);
        platform.set_feathering_enabled(true);

        assert!(platform.feathering_enabled());
        assert_eq!(platform.feather_width(), 0.0);
    }

    const HDR_BENCH_STEP_S: f64 = 1.0 / 120.0;
    const HDR_BENCH_FRAMES: usize = 120;
    const HDR_BENCH_WIDTH: f32 = 1280.0;
    const HDR_BENCH_HEIGHT: f32 = 720.0;
    const HDR_BENCH_COLS: usize = 48;
    const HDR_BENCH_ROWS: usize = 27;

    struct AnimatedHdrGrid {
        phase: f32,
        timer: Option<TimerToken>,
    }

    impl AnimatedHdrGrid {
        fn new() -> Self {
            Self {
                phase: 0.0,
                timer: None,
            }
        }

        fn arm(&mut self, ctx: &mut EventCtx) {
            if self.timer.is_none() {
                self.timer = Some(ctx.schedule_timer_after(HDR_BENCH_STEP_S));
            }
        }
    }

    impl Widget for AnimatedHdrGrid {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::Window(WindowEvent::Resized(_)) => {
                    self.arm(ctx);
                    ctx.request_paint();
                }
                Event::Wake(WakeEvent::Timer { token, .. }) if self.timer == Some(*token) => {
                    self.phase = (self.phase + 0.035) % TAU;
                    self.timer = Some(ctx.schedule_timer_after(HDR_BENCH_STEP_S));
                    ctx.request_paint();
                    ctx.set_handled();
                }
                _ => {}
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(HDR_BENCH_WIDTH, HDR_BENCH_HEIGHT))
        }

        fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: Rect) {}

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.clear(Color::linear_rgba(0.08, 0.09, 0.11, 1.0));
            let cell_w = HDR_BENCH_WIDTH / HDR_BENCH_COLS as f32;
            let cell_h = HDR_BENCH_HEIGHT / HDR_BENCH_ROWS as f32;
            for row in 0..HDR_BENCH_ROWS {
                for col in 0..HDR_BENCH_COLS {
                    let x = col as f32 * cell_w;
                    let y = row as f32 * cell_h;
                    let t = self.phase + row as f32 * 0.19 + col as f32 * 0.11;
                    let r = 0.5 + 3.5 * (0.5 + 0.5 * t.sin());
                    let g = 0.25 + 2.25 * (0.5 + 0.5 * (t * 1.3).cos());
                    let b = 0.15 + 1.85 * (0.5 + 0.5 * (t * 0.7 + 1.2).sin());
                    ctx.fill_rect(
                        Rect::new(x, y, cell_w + 1.0, cell_h + 1.0),
                        Color::linear_rgba(r, g, b, 1.0),
                    );
                }
            }
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
            node.name = Some("Animated HDR Grid".to_string());
            ctx.push(node);
        }
    }

    fn mean(values: &[f64]) -> f64 {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    }

    fn percentile(sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
        sorted[rank]
    }

    #[test]
    #[ignore = "hardware Vulkan benchmark; run explicitly on a machine with a working headless GPU path"]
    fn steady_state_headless_hdr_grid_benchmark_emits_advancing_frames() -> Result<()> {
        let options = WindowRenderOptions::new(true, 1.0)
            .with_color_management_mode(WindowColorManagementMode::PreferHdr)
            .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
            .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
            .with_tone_mapping_mode(WindowToneMappingMode::Automatic);

        let runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Animated HDR Grid")
                    .root(AnimatedHdrGrid::new()),
            )
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        set_window_render_options(window_id, options);
        set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);

        let mut runtime = runtime;
        let mut platform = HeadlessPlatform::new();
        let mut totals = Vec::with_capacity(HDR_BENCH_FRAMES);
        let mut commands = Vec::with_capacity(HDR_BENCH_FRAMES);
        let mut dirty = Vec::with_capacity(HDR_BENCH_FRAMES);
        let mut last_frame = 0u64;
        let mut iterations = 0usize;
        let max_iterations = 20_000usize;

        while totals.len() < HDR_BENCH_FRAMES && iterations < max_iterations {
            let _ = platform.pump(&mut runtime)?;
            if let Some(snapshot) = window_performance_snapshot(window_id) {
                if snapshot.frame_index != last_frame {
                    last_frame = snapshot.frame_index;
                    totals.push(snapshot.total_time_ms);
                    commands.push(snapshot.scene.command_count as f64);
                    dirty.push(snapshot.scene.dirty_coverage as f64);
                }
            }
            platform.advance_time(HDR_BENCH_STEP_S);
            iterations += 1;
        }

        let mut totals_sorted = totals.clone();
        totals_sorted.sort_by(|a, b| a.total_cmp(b));

        println!("benchmark=steady-state-headless-hdr-grid");
        println!("iterations={iterations}");
        println!("frames={}", totals.len());
        println!("nominal_fps={:.2}", 1.0 / HDR_BENCH_STEP_S);
        println!("frame_total_ms_avg={:.3}", mean(&totals));
        println!("frame_total_ms_p50={:.3}", percentile(&totals_sorted, 0.50));
        println!("frame_total_ms_p95={:.3}", percentile(&totals_sorted, 0.95));
        println!(
            "frame_total_ms_max={:.3}",
            totals_sorted.last().copied().unwrap_or(0.0)
        );
        println!("command_count_avg={:.1}", mean(&commands));
        println!("dirty_coverage_avg={:.2}", mean(&dirty));

        assert_eq!(
            totals.len(),
            HDR_BENCH_FRAMES,
            "expected {} benchmark frames, got {} after {} iterations",
            HDR_BENCH_FRAMES,
            totals.len(),
            iterations
        );
        assert!(mean(&totals) > 0.0, "expected positive frame timings");

        Ok(())
    }
}
