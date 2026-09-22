//! Observe native presentations through the normal DesktopPlatform, without
//! test-harness redraw injection or per-frame display-capability discovery.
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
use sui::{
    DesktopAutomationAction, DesktopAutomationConfig, DesktopPlatform, FramePhase,
    SceneStatisticsDetailMode, SemanticsRole, Vector, WindowPerformanceSnapshot,
};
use sui_platform::{DesktopExtension, DesktopExtensionContext, DesktopFramePresented};

#[derive(Debug, Default)]
struct Samples {
    frames: Vec<(Instant, WindowPerformanceSnapshot)>,
    first_presented: Option<Instant>,
}

#[derive(Debug)]
struct Probe(Rc<RefCell<Samples>>);

impl DesktopExtension for Probe {
    fn frame_presented(
        &mut self,
        context: DesktopExtensionContext<'_>,
        frame: DesktopFramePresented,
    ) -> sui::Result<()> {
        // Native window registration can reset diagnostics configured before
        // launch. Enable detail on the live window before the warmup interval.
        sui::set_window_scene_statistics_detail_mode(
            frame.window_id,
            SceneStatisticsDetailMode::Detailed,
        );
        let mut samples = self.0.borrow_mut();
        if samples.first_presented.is_none() {
            samples.first_presented = Some(frame.presented_at);
            let host = context.window(frame.window_id).unwrap().host_window();
            println!(
                "SHRINKWRAP_DESKTOP physical={:?} scale={} adapter={:?} output={:?} validation={:?} debug={:?}",
                host.inner_size(),
                host.scale_factor(),
                context.renderer().adapter_info(),
                sui::window_output_diagnostics(frame.window_id)
                    .map(|output| output.active_output_strategy),
                std::env::var_os("WGPU_VALIDATION"),
                std::env::var_os("WGPU_DEBUG")
            );
        }
        let elapsed = frame
            .presented_at
            .duration_since(samples.first_presented.unwrap())
            .as_secs_f64();
        if (2.0..15.0).contains(&elapsed) {
            if let Some(snapshot) = sui::window_performance_snapshot(frame.window_id) {
                assert_eq!(snapshot.frame_index, frame.frame_index);
                assert!(
                    !snapshot.phase_timings.is_empty(),
                    "native stage diagnostics were not collected"
                );
                samples.frames.push((frame.presented_at, snapshot));
            }
        }
        Ok(())
    }
}

fn phase(frame: &WindowPerformanceSnapshot, wanted: FramePhase) -> f64 {
    frame
        .phase_timings
        .iter()
        .filter(|sample| sample.phase == wanted)
        .map(|sample| sample.duration_ms)
        .sum()
}

fn percentile(values: &mut [f64], p: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() - 1) * p / 100]
}

fn main() -> sui::Result<()> {
    let runtime = sui_demo_app::build_shrinkwrap_application().build()?;
    for id in runtime.window_ids() {
        sui::set_window_scene_statistics_detail_mode(id, SceneStatisticsDetailMode::Detailed);
    }
    let samples = Rc::new(RefCell::new(Samples::default()));
    DesktopPlatform::new()
        .with_vsync_enabled(true)
        .with_extension(Probe(Rc::clone(&samples)))
        .with_automation(DesktopAutomationConfig {
            label: "shrinkwrap-native-observation".into(),
            target_role: SemanticsRole::GenericContainer,
            target_name: "Animated shrinkwrap conversation".into(),
            // A single zero-delta action arms timed shutdown. No repeated input
            // or redraw requests are injected during the measurement interval.
            action: DesktopAutomationAction::ScrollPixels {
                delta: Vector::ZERO,
            },
            step_interval: Duration::from_secs(3600),
            duration: Duration::from_secs(18),
            report_interval: Duration::from_secs(60),
            startup_timeout: Duration::from_secs(10),
        })
        .run(runtime)?;
    let samples = samples.borrow();
    assert!(
        samples.frames.len() > 2,
        "native animation produced no samples"
    );
    let first = samples.frames.first().unwrap();
    let last = samples.frames.last().unwrap();
    let elapsed = last.0.duration_since(first.0).as_secs_f64();
    let frames = last.1.frame_index - first.1.frame_index;
    let mut total = samples
        .frames
        .iter()
        .map(|(_, frame)| frame.total_time_ms)
        .collect::<Vec<_>>();
    let mut work = samples
        .frames
        .iter()
        .map(|(_, frame)| (frame.total_time_ms - phase(frame, FramePhase::SurfaceWait)).max(0.0))
        .collect::<Vec<_>>();
    let mut cadence = samples
        .frames
        .windows(2)
        .map(|pair| pair[1].0.duration_since(pair[0].0).as_secs_f64() * 1000.0)
        .collect::<Vec<_>>();
    println!(
        "SHRINKWRAP_DESKTOP samples={} host_fps={:.2} interval_p50_ms={:.3} interval_p95_ms={:.3} work_p50_ms={:.3} work_p95_ms={:.3} total_p50_ms={:.3} total_p95_ms={:.3}",
        samples.frames.len(),
        frames as f64 / elapsed,
        percentile(&mut cadence, 50),
        percentile(&mut cadence, 95),
        percentile(&mut work, 50),
        percentile(&mut work, 95),
        percentile(&mut total, 50),
        percentile(&mut total, 95)
    );
    for stage in [
        FramePhase::Event,
        FramePhase::Redraw,
        FramePhase::MeasureArrange,
        FramePhase::Paint,
        FramePhase::Semantics,
        FramePhase::Renderer,
        FramePhase::SurfaceWait,
    ] {
        println!(
            "  {}_avg_ms={:.3}",
            stage.label(),
            samples
                .frames
                .iter()
                .map(|(_, frame)| phase(frame, stage))
                .sum::<f64>()
                / samples.frames.len() as f64
        );
    }
    println!(
        "  text_avg_ms={:.3}",
        samples
            .frames
            .iter()
            .map(|(_, frame)| frame.runtime_text_timing.total_time_us as f64 / 1000.0)
            .sum::<f64>()
            / samples.frames.len() as f64
    );
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shrinkwrap-demo");
    std::fs::create_dir_all(&directory).unwrap();
    let mut csv = String::from(
        "frame,total_ms,work_ms,interval_ms,redraw_ms,paint_ms,text_us,renderer_ms,surface_wait_ms\n",
    );
    for (_, frame) in &samples.frames {
        let wait = phase(frame, FramePhase::SurfaceWait);
        csv.push_str(&format!(
            "{},{:.6},{:.6},{:.6},{:.6},{:.6},{},{:.6},{wait:.6}\n",
            frame.frame_index,
            frame.total_time_ms,
            (frame.total_time_ms - wait).max(0.0),
            frame.frame_interval_ms.unwrap_or(0.0),
            phase(frame, FramePhase::Redraw),
            phase(frame, FramePhase::Paint),
            frame.runtime_text_timing.total_time_us,
            phase(frame, FramePhase::Renderer)
        ));
    }
    std::fs::write(directory.join("desktop-profile.csv"), csv).unwrap();
    Ok(())
}
