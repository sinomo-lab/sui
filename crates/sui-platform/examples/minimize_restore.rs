//! Native minimize/restore regression probe. Run with:
//! `cargo run -p sinomo-ui-platform --example minimize_restore`
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use sui_core::{Color, Event, Result, SemanticsNode, SemanticsRole, Size, Vector};
use sui_layout::Constraints;
use sui_platform::{
    DesktopAutomationAction, DesktopAutomationConfig, DesktopExtension, DesktopExtensionContext,
    DesktopPlatform,
};
use sui_runtime::{
    Application, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget, WindowBuilder,
    window_performance_snapshot,
};

struct Probe;
impl Widget for Probe {
    fn measure(&mut self, _: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(400.0, 240.0))
    }
    fn event(&mut self, ctx: &mut EventCtx, _: &Event) {
        ctx.request_paint();
    }
    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(Color::rgba(0.1, 0.3, 0.4, 1.0));
    }
    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::ScrollView, ctx.bounds());
        node.name = Some("Minimize probe".into());
        ctx.push(node);
    }
}

#[derive(Debug, Default)]
struct Evidence {
    minimized: bool,
    restored_and_painted: bool,
}

#[derive(Debug)]
struct MinimizeRestore {
    started: Option<Instant>,
    stage: u8,
    frame_before: u64,
    evidence: Rc<RefCell<Evidence>>,
}

impl DesktopExtension for MinimizeRestore {
    fn poll_interval(&self) -> Option<Duration> {
        Some(Duration::from_millis(50))
    }
    fn update(&mut self, context: DesktopExtensionContext<'_>) -> Result<()> {
        let Some(id) = context.runtime().window_ids().first().copied() else {
            return Ok(());
        };
        let Some(window) = context.window(id) else {
            return Ok(());
        };
        let elapsed = self
            .started
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_secs_f32();
        let host = window.host_window();
        match self.stage {
            0 if elapsed >= 0.5 => {
                self.frame_before = window_performance_snapshot(id)
                    .map(|s| s.frame_index)
                    .unwrap_or(0);
                host.set_minimized(true);
                self.stage = 1;
            }
            1 if elapsed >= 1.0 => {
                self.evidence.borrow_mut().minimized = host.is_minimized() == Some(true);
                // Exercise redraw requests while the native surface is unavailable.
                host.request_redraw();
                if elapsed >= 1.5 {
                    host.set_minimized(false);
                    host.request_redraw();
                    self.stage = 2;
                }
            }
            2 if elapsed >= 2.5 => {
                self.evidence.borrow_mut().restored_and_painted = host.is_minimized() != Some(true)
                    && window_performance_snapshot(id)
                        .is_some_and(|s| s.frame_index > self.frame_before);
                self.stage = 3;
            }
            _ => {}
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let evidence = Rc::new(RefCell::new(Evidence::default()));
    let runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("SUI minimize/restore regression probe")
                .root(Probe),
        )
        .build()?;
    DesktopPlatform::new()
        .with_extension(MinimizeRestore {
            started: None,
            stage: 0,
            frame_before: 0,
            evidence: evidence.clone(),
        })
        .with_automation(DesktopAutomationConfig {
            label: "minimize-restore".into(),
            target_role: SemanticsRole::ScrollView,
            target_name: "Minimize probe".into(),
            action: DesktopAutomationAction::ScrollPixels {
                delta: Vector::new(0.0, 1.0),
            },
            step_interval: Duration::from_millis(100),
            duration: Duration::from_secs(4),
            report_interval: Duration::from_secs(1),
            startup_timeout: Duration::from_secs(10),
        })
        .run(runtime)?;
    let evidence = evidence.borrow();
    assert!(evidence.minimized, "native window did not minimize");
    assert!(
        evidence.restored_and_painted,
        "restored window did not resume painting"
    );
    println!("PASS: native minimize, redraw while minimized, restore, and repaint");
    Ok(())
}
