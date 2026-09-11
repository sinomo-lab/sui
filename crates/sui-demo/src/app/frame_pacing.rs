use super::*;
use std::time::{Duration, Instant};
use sui::{FramePhase, PointerEvent, SceneStatisticsDetailMode, WindowPerformanceSnapshot};

#[test]
#[ignore = "diagnostic reproduction for drag-and-drop tab frame cadence"]
fn drag_drop_frame_pacing_benchmark() -> Result<()> {
    let app = sui_testing::TestApp::new_with_options(
        || {
            let options = RenderSettingsTab::default_options()
                .with_color_management_mode(WindowColorManagementMode::PreferHdr)
                .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange);
            let shell = DevBrowserShell::new(options);
            shell.state.set_performance_overlay_visible(true);
            let overlay = shell.performance_overlay_reader();
            let commands = shell.command_demo_state();
            finish_dev_application_with_performance_overlay_reader(shell, overlay, commands)
                .with_window_render_options(options)
                .build()
        },
        true,
        true,
    )?;
    let window = app.main_window()?;
    window
        .get_by_role(SemanticsRole::Button)
        .with_name(DRAG_DROP_TAB_LABEL)
        .click()?;
    let snapshot = window.snapshot()?;
    println!(
        "DRAG_NODES {:?}",
        snapshot
            .accessibility
            .nodes
            .iter()
            .filter_map(|node| {
                node.name
                    .as_ref()
                    .filter(|name| {
                        name.contains("Drag")
                            || name.contains("Drop")
                            || name.contains("text")
                            || name.contains("asset")
                    })
                    .map(|name| (name, &node.role, node.bounds))
            })
            .collect::<Vec<_>>()
    );
    let root = window.root();
    let source = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.name.as_deref() == Some("Text source Invoice #1042"))
        .expect("drag source")
        .bounds;
    for phase in ["settle", "idle", "move", "redraw", "drag"] {
        if phase == "drag" {
            let mut down = PointerEvent::new(
                PointerEventKind::Down,
                Point::new(
                    source.x() + source.width() * 0.5,
                    source.y() + source.height() * 0.5,
                ),
            );
            down.button = Some(PointerButton::Primary);
            down.buttons = sui::PointerButtons::new(1);
            root.dispatch_event(Event::Pointer(down))?;
        }
        let mut frames = Vec::new();
        let before = sui::window_performance_snapshot(window.id()).expect("frame");
        let start = Instant::now();
        for index in 0..180 {
            match phase {
                "move" => root.dispatch_event(Event::Pointer(PointerEvent::new(
                    PointerEventKind::Move,
                    Point::new(
                        300.0 + (index % 30) as f32 * 12.0,
                        360.0 + (index % 7) as f32 * 10.0,
                    ),
                )))?,
                "redraw" => root.dispatch_event(Event::Window(WindowEvent::RedrawRequested))?,
                "drag" => {
                    let mut pointer = PointerEvent::new(
                        PointerEventKind::Move,
                        Point::new(
                            source.x() + source.width() * 0.5 + (index % 30) as f32 * 3.0,
                            source.y() + source.height() * 0.5 + 20.0 + (index % 15) as f32 * 4.0,
                        ),
                    );
                    pointer.buttons = sui::PointerButtons::new(1);
                    root.dispatch_event(Event::Pointer(pointer))?;
                }
                _ => {}
            }
            if phase != "redraw" {
                std::thread::sleep(Duration::from_millis(8));
            }
            if let Some(frame) = sui::window_performance_snapshot(window.id())
                && frame.frame_index > before.frame_index
                && frames
                    .last()
                    .is_none_or(|previous: &WindowPerformanceSnapshot| {
                        previous.frame_index != frame.frame_index
                    })
            {
                frames.push(frame);
            }
        }
        let last = sui::window_performance_snapshot(window.id()).expect("frame");
        println!(
            "DRAG_PACING phase={phase} elapsed={:?} frame_delta={} samples={} last_interval={:?} last_work_ms={:.3}",
            start.elapsed(),
            last.frame_index - before.frame_index,
            frames.len(),
            last.frame_interval_ms,
            last.total_time_ms
        );
        frames.sort_by(|a, b| a.total_time_ms.total_cmp(&b.total_time_ms));
        for frame in frames.iter().rev().take(3) {
            println!(
                "DRAG_SLOW phase={phase} frame={} work_ms={:.3} interval={:?} phases={:?} renderer={:?}",
                frame.frame_index,
                frame.total_time_ms,
                frame.frame_interval_ms,
                frame.phase_timings,
                frame.renderer_submission
            );
        }
        if phase == "drag" {
            root.dispatch_event(Event::Pointer(PointerEvent::new(
                PointerEventKind::Cancel,
                Point::ZERO,
            )))?;
        }
    }
    let screenshot = window.capture_screenshot()?;
    screenshot.write_png(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/picker-hover/drag-drop.png"),
    )?;
    Ok(())
}

#[test]
#[ignore = "diagnostic benchmark for VSync/HDR frame timing and idle input"]
fn picker_frame_pacing_vsync_hdr_benchmark() -> Result<()> {
    let app = sui_testing::TestApp::new_with_options(
        || {
            let options = RenderSettingsTab::default_options()
                .with_color_management_mode(WindowColorManagementMode::PreferHdr)
                .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange);
            let shell = DevBrowserShell::new(options);
            shell.state.set_performance_overlay_visible(true);
            let overlay = shell.performance_overlay_reader();
            let commands = shell.command_demo_state();
            finish_dev_application_with_performance_overlay_reader(shell, overlay, commands)
                .with_window_render_options(options)
                .build()
        },
        true,
        true,
    )?;
    let window = app.main_window()?;
    sui::set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
    let initial = window.snapshot()?;
    if let Some(output) = window_output_diagnostics(window.id()) {
        println!("PACING_OUTPUT {:?}", output.active_output_strategy);
    }
    let cards = [WIDGET_BOOK_TAB_LABEL, THEMES_TAB_LABEL].map(|name| {
        initial
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some(name))
            .expect("picker card")
            .bounds
    });
    let root = window.root();
    for phase in ["redraw", "background", "hover", "idle"] {
        let mut frames: Vec<WindowPerformanceSnapshot> = Vec::new();
        let started = Instant::now();
        let count = if phase == "background" { 30000 } else { 180 };
        for index in 0..count {
            let event = match phase {
                "hover" => {
                    let card = cards[(index / 8) % cards.len()];
                    Event::Pointer(PointerEvent::new(
                        PointerEventKind::Move,
                        Point::new(
                            card.x() + card.width() * 0.5 + (index % 8) as f32,
                            card.y() + card.height() * 0.5,
                        ),
                    ))
                }
                "background" => Event::Pointer(PointerEvent::new(
                    PointerEventKind::Move,
                    Point::new(200.0 + (index % 50) as f32, 80.0),
                )),
                _ => Event::Window(WindowEvent::RedrawRequested),
            };
            root.dispatch_event(event)?;
            if phase == "hover" {
                std::thread::sleep(Duration::from_millis(8));
            }
            if phase == "idle" {
                std::thread::sleep(Duration::from_millis(40));
            }
            if let Some(snapshot) = sui::window_performance_snapshot(window.id())
                && frames
                    .last()
                    .is_none_or(|last| last.frame_index != snapshot.frame_index)
            {
                frames.push(snapshot);
            }
        }
        root.dispatch_event(Event::Window(WindowEvent::RedrawRequested))?;
        let final_frame = window.performance_snapshot()?;
        if phase == "background" {
            assert!(
                final_frame
                    .phase_timings
                    .iter()
                    .all(|sample| sample.phase != FramePhase::Event || sample.duration_ms == 0.0),
                "idle mouse input must not inflate a later frame's FPS reading: {:?}",
                final_frame.phase_timings
            );
        }
        frames.push(final_frame);
        let intervals = frames
            .iter()
            .filter_map(|frame| frame.frame_interval_ms)
            .collect::<Vec<_>>();
        let cadence_ms = intervals.iter().sum::<f64>() / intervals.len().max(1) as f64;
        frames.sort_by(|left, right| left.total_time_ms.total_cmp(&right.total_time_ms));
        let percentile = |p: usize| frames[(frames.len() - 1) * p / 100].total_time_ms;
        println!(
            "PACING phase={phase} elapsed={:?} samples={} cadence_avg_ms={cadence_ms:.3} cadence_fps={:.1} work_p50={:.3} work_p95={:.3} work_p99={:.3} work_max={:.3}",
            started.elapsed(),
            frames.len(),
            1000.0 / cadence_ms,
            percentile(50),
            percentile(95),
            percentile(99),
            percentile(100)
        );
        for frame in frames.iter().rev().take(3) {
            let phase_ms = |phase| {
                frame
                    .phase_timings
                    .iter()
                    .filter(|s| s.phase == phase)
                    .map(|s| s.duration_ms)
                    .sum::<f64>()
            };
            let stats = &frame.renderer_submission;
            println!(
                "SLOW frame={} total_ms={:.3} event_ms={:.3} redraw_ms={:.3} renderer_ms={:.3} wait_ms={:.3} packet_us={} upload_us={} acquire_us={} present_us={} text_misses={} misses_us={}",
                frame.frame_index,
                frame.total_time_ms,
                phase_ms(FramePhase::Event),
                phase_ms(FramePhase::Redraw),
                phase_ms(FramePhase::Renderer),
                phase_ms(FramePhase::SurfaceWait),
                stats.retained_packet_build_time_us,
                stats.gpu_upload_time_us,
                stats.surface_acquire_time_us,
                stats.surface_present_time_us,
                stats.text_atlas_miss_count,
                stats.text_atlas_miss_time_us
            );
        }
    }
    window
        .get_by_role(SemanticsRole::Button)
        .with_name(ANIMATION_DEMO_TAB_LABEL)
        .click()?;
    std::thread::sleep(Duration::from_millis(300));
    let before = sui::window_performance_snapshot(window.id()).expect("animation frame");
    let started = Instant::now();
    std::thread::sleep(Duration::from_secs(2));
    let after = sui::window_performance_snapshot(window.id()).expect("animation frame");
    let elapsed = started.elapsed().as_secs_f64();
    let frame_count = after.frame_index - before.frame_index;
    assert!(
        frame_count > 0,
        "the animation must present without mouse input"
    );
    println!(
        "PACING_ANIMATION frames={frame_count} elapsed_s={elapsed:.3} observed_fps={:.1} last_interval_ms={:?} last_work_ms={:.3}",
        frame_count as f64 / elapsed,
        after.frame_interval_ms,
        after.total_time_ms
    );
    Ok(())
}
