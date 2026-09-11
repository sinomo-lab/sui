use super::*;
use std::time::Instant;
use sui::{FramePhase, PointerEvent, Runtime, SceneStatisticsDetailMode};

fn picker_cards(nodes: &[SemanticsNode]) -> [SemanticsNode; 2] {
    [WIDGET_BOOK_TAB_LABEL, THEMES_TAB_LABEL].map(|name| {
        nodes
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some(name))
            .expect("picker card")
            .clone()
    })
}

fn card_position(card: &SemanticsNode, offset: f32) -> Point {
    Point::new(
        card.bounds.x() + card.bounds.width() * 0.5 + offset,
        card.bounds.y() + card.bounds.height() * 0.5,
    )
}

fn hover_event(cards: &[SemanticsNode; 2], index: usize, moving: bool) -> Event {
    let position = if moving {
        card_position(&cards[(index / 8) % cards.len()], (index % 8) as f32)
    } else {
        Point::new(1.0, 1.0)
    };
    Event::Pointer(PointerEvent::new(PointerEventKind::Move, position))
}

fn advance_clock(runtime: &mut Runtime, time: f64) -> Result<()> {
    runtime.tick(time);
    for (id, event) in runtime.drain_ready_events() {
        runtime.handle_event(id, event)?;
    }
    Ok(())
}

#[test]
fn picker_hover_repaints_only_animating_cards() -> Result<()> {
    let mut runtime = build_dev_application().build()?;
    let window_id = runtime.window_ids()[0];
    let initial = runtime.render(window_id)?;
    let cards = picker_cards(&initial.semantics);
    let move_to = |card: &SemanticsNode| {
        Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            card_position(card, 0.0),
        ))
    };
    runtime.handle_event(window_id, move_to(&cards[0]))?;
    advance_clock(&mut runtime, 0.05)?;
    let hovered = runtime.render(window_id)?;
    let content_updates = |output: &sui::RenderOutput| {
        output
            .frame
            .layer_updates
            .iter()
            .filter(|update| update.kind == sui_scene::SceneLayerUpdateKind::Content)
            .map(|update| update.layer_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        content_updates(&hovered),
        vec![sui_scene::SceneLayerId::from_widget(cards[0].id)],
        "hovering a card must not repaint the picker scroll layer or its neighbors"
    );
    assert_eq!(
        initial.frame.scene.layer_scene(cards[1].id),
        hovered.frame.scene.layer_scene(cards[1].id)
    );
    assert_ne!(
        initial.frame.scene.layer_scene(cards[0].id),
        hovered.frame.scene.layer_scene(cards[0].id)
    );

    runtime.handle_event(window_id, move_to(&cards[1]))?;
    advance_clock(&mut runtime, 0.1)?;
    let crossing = runtime.render(window_id)?;
    let updates = content_updates(&crossing);
    assert_eq!(
        updates.len(),
        2,
        "only the entering and leaving cards should repaint"
    );
    for card in &cards {
        assert!(updates.contains(&sui_scene::SceneLayerId::from_widget(card.id)));
    }

    advance_clock(&mut runtime, 1.0)?;
    runtime.render(window_id)?;
    assert_eq!(
        runtime.next_wakeup_time(window_id)?,
        None,
        "hover motion must settle"
    );
    runtime.handle_event(window_id, hover_event(&cards, 9, true))?;
    assert!(
        !runtime.needs_render(window_id)?,
        "moving within a settled card should not redraw"
    );
    Ok(())
}

#[test]
#[ignore = "diagnostic benchmark for native picker hover presentation"]
#[cfg(not(target_arch = "wasm32"))]
fn picker_card_hover_native_benchmark() -> Result<()> {
    let app = sui_testing::TestApp::new_visible_no_vsync(|| build_dev_application().build())?;
    let window = app.main_window()?;
    sui::set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
    let initial = window.snapshot()?;
    let cards = picker_cards(&initial.accessibility.nodes);
    let root = window.root();
    for moving in [false, true] {
        let mut snapshots = Vec::new();
        let started = Instant::now();
        for index in 0..120 {
            root.dispatch_event(hover_event(&cards, index, moving))?;
            std::thread::sleep(std::time::Duration::from_millis(8));
            if let Some(snapshot) = sui::window_performance_snapshot(window.id())
                && snapshots
                    .last()
                    .is_none_or(|previous: &sui::WindowPerformanceSnapshot| {
                        previous.frame_index != snapshot.frame_index
                    })
            {
                snapshots.push(snapshot);
            }
        }
        assert!(!snapshots.is_empty(), "native host must publish frames");
        let mean = |sample: fn(&sui::WindowPerformanceSnapshot) -> f64| {
            snapshots.iter().map(sample).sum::<f64>() / snapshots.len() as f64
        };
        println!(
            "NATIVE_PICKER moving={moving} elapsed={:?} samples={} total_avg_ms={:.3} packet_build_avg_us={:.1} rebuilt_text_commands_avg={:.1} rebuilt_path_commands_avg={:.1} upload_avg_us={:.1}",
            started.elapsed(),
            snapshots.len(),
            mean(|s| s.total_time_ms),
            mean(|s| s.renderer_submission.retained_packet_build_time_us as f64),
            mean(|s| s.renderer_submission.retained_packet_text_command_count as f64),
            mean(|s| s.renderer_submission.retained_packet_path_command_count as f64),
            mean(|s| s.renderer_submission.gpu_upload_time_us as f64)
        );
    }
    Ok(())
}

#[test]
#[ignore = "diagnostic benchmark for new-tab picker card hover frames"]
fn picker_card_hover_benchmark() -> Result<()> {
    for scale in [1.0, 2.0] {
        run_picker_hover_benchmark(scale)?;
    }
    Ok(())
}

fn run_picker_hover_benchmark(scale: f64) -> Result<()> {
    const WARMUP: usize = 60;
    const FRAMES: usize = 240;
    let mut runtime = build_dev_application().build()?;
    let window_id = runtime.window_ids()[0];
    runtime.handle_event(
        window_id,
        Event::Window(WindowEvent::ScaleFactorChanged {
            scale_factor: scale,
            raw_dpi: None,
            suggested_size: Some(Size::new(1920.0, 1080.0)),
        }),
    )?;
    sui::set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);
    let initial = runtime.render(window_id)?;
    let cards = picker_cards(&initial.semantics);
    let mut renderer = WgpuRenderer::new();
    renderer.render(&initial.frame)?;
    for moving in [false, true] {
        let mut event_ms = 0.0;
        let mut runtime_ms = 0.0;
        let mut renderer_ms = 0.0;
        let mut phases = [0.0; 4];
        let mut renderer_us = [0u64; 8];
        let mut packets = 0;
        let mut draws = 0;
        let mut animations = 0;
        for index in 0..WARMUP + FRAMES {
            let started = Instant::now();
            advance_clock(
                &mut runtime,
                (index + if moving { WARMUP + FRAMES } else { 0 }) as f64 / 60.0,
            )?;
            runtime.handle_event(window_id, hover_event(&cards, index, moving))?;
            // Force both lanes to draw, measuring cached presentation versus
            // hover updates without counting intentional idle time as slow FPS.
            runtime.handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))?;
            let event_elapsed = started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            let output = runtime.render(window_id)?;
            let runtime_elapsed = started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            renderer.render(&output.frame)?;
            if index < WARMUP {
                continue;
            }
            event_ms += event_elapsed;
            runtime_ms += runtime_elapsed;
            renderer_ms += started.elapsed().as_secs_f64() * 1000.0;
            for timing in &output.diagnostics.phase_timings {
                let slot = match timing.phase {
                    FramePhase::MeasureArrange => 0,
                    FramePhase::HitTest => 1,
                    FramePhase::Paint => 2,
                    FramePhase::Semantics => 3,
                    _ => continue,
                };
                phases[slot] += timing.duration_ms;
            }
            animations += output.diagnostics.animation_frame_wake_count;
            let stats = renderer
                .last_frame_stats(window_id)
                .expect("renderer stats");
            packets += stats.retained_packet_build_count;
            draws += stats.draw_count;
            for (sum, value) in renderer_us.iter_mut().zip([
                stats.retained_packet_build_time_us,
                stats.retained_packet_text_command_time_us,
                stats.retained_packet_path_command_time_us,
                stats.resource_collection_time_us,
                stats.bind_group_prepare_time_us,
                stats.batch_prepare_time_us,
                stats.pass_encode_time_us,
                stats.queue_submit_time_us,
            ]) {
                *sum += value;
            }
        }
        println!(
            "PICKER_HOVER scale={scale} moving={moving} frames={FRAMES} event_avg_ms={:.3} runtime_avg_ms={:.3} renderer_avg_ms={:.3} total_avg_ms={:.3} phases_ms={:?} renderer_us={:?} packets_avg={:.2} draws_avg={:.2} animation_wakes_avg={:.2}",
            event_ms / FRAMES as f64,
            runtime_ms / FRAMES as f64,
            renderer_ms / FRAMES as f64,
            (event_ms + runtime_ms + renderer_ms) / FRAMES as f64,
            ["layout", "hit_test", "paint", "semantics"]
                .into_iter()
                .zip(phases.map(|v| v / FRAMES as f64))
                .collect::<Vec<_>>(),
            [
                "packet_build",
                "text",
                "paths",
                "resources",
                "bind_groups",
                "batches",
                "encode",
                "submit"
            ]
            .into_iter()
            .zip(renderer_us.map(|v| v as f64 / FRAMES as f64))
            .collect::<Vec<_>>(),
            packets as f64 / FRAMES as f64,
            draws as f64 / FRAMES as f64,
            animations as f64 / FRAMES as f64
        );
        if let Some(directory) = std::env::var_os("SUI_HOVER_BENCH_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            let captured = renderer.capture_rgba(window_id)?;
            let file =
                std::fs::File::create(directory.join(format!("scale-{scale}-moving-{moving}.png")))
                    .unwrap();
            let mut encoder = png::Encoder::new(file, captured.width(), captured.height());
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(captured.pixels())
                .unwrap();
        }
    }
    Ok(())
}
