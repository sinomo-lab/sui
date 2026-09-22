//! Isolate packet granularity from widget/layout and text costs.
use std::time::{Duration, Instant};

use crate::WgpuRenderer;
use crate::tests::support::assert_rgba_images_match;
use sui_core::{Color, Rect, Size, WindowId};
use sui_scene::{SceneCommand, SceneFrame};

fn grid_frame(window: WindowId, changed: bool) -> SceneFrame {
    let mut frame = SceneFrame::new(window, Size::new(1050.0, 806.0));
    frame.scene.push(SceneCommand::Clear(Color::BLACK));
    for index in 0..1024 {
        frame.scene.push(SceneCommand::FillRoundedRect {
            rect: Rect::new(
                (index % 32) as f32 * 32.0 + 8.0,
                (index / 32) as f32 * 24.0 + 8.0,
                2.5,
                2.5,
            ),
            radii: [1.25; 4],
            brush: if changed && index == 513 {
                Color::WHITE
            } else {
                Color::rgba(0.18, 0.5, 0.74, 0.18)
            }
            .into(),
            border: None,
            shadow: None,
        });
    }
    frame
}

#[test]
#[ignore = "controlled renderer packet granularity profile; run serially"]
fn retained_pipeline_granularity_profile() {
    const FRAMES: usize = 40;
    let window = WindowId::new(9081);
    let frames = [grid_frame(window, false), grid_frame(window, true)];
    let mut renderer = WgpuRenderer::new();
    renderer.render(&frames[0]).unwrap();
    let reference = renderer.capture_rgba(window).unwrap();
    renderer.render(&frames[1]).unwrap();
    let edited_reference = renderer.capture_rgba(window).unwrap();
    println!(
        "PIPELINE_ADAPTER {:?} validation={:?} debug={:?}",
        renderer.adapter_info(),
        std::env::var_os("WGPU_VALIDATION"),
        std::env::var_os("WGPU_DEBUG")
    );
    // Forward/reverse order helps identify warmup and machine-load effects.
    for limit in [16, 64, 256, 256, 64, 16] {
        renderer.compositors.remove(&window);
        renderer.frame_resources.fragments.remove(&window);
        renderer
            .compositors
            .entry(window)
            .or_default()
            .packet_draw_limit = limit;
        renderer.render(&frames[0]).unwrap();
        assert_rgba_images_match(&reference, &renderer.capture_rgba(window).unwrap());
        for changed in [false, true] {
            let mut times = Vec::new();
            let mut sums = [0u64; 10];
            for index in 0..FRAMES + 10 {
                std::thread::sleep(Duration::from_millis(17));
                let frame = &frames[usize::from(changed && index % 2 == 1)];
                let started = Instant::now();
                renderer.render(frame).unwrap();
                let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                if index < 10 {
                    continue;
                }
                let stats = renderer.last_frame_stats(window).unwrap();
                times.push(elapsed);
                for (sum, value) in sums.iter_mut().zip([
                    stats.draw_count as u64,
                    stats.direct_packet_count as u64,
                    stats.retained_packet_command_count as u64,
                    stats.retained_scene_traversal_time_us,
                    stats.retained_state_update_time_us,
                    stats.composition_time_us,
                    stats.batch_prepare_time_us,
                    stats.command_finish_time_us,
                    stats.queue_submit_time_us,
                    stats.uploaded_vertex_bytes,
                ]) {
                    *sum += value;
                }
                if !changed {
                    assert_eq!(stats.retained_packet_build_count, 0);
                    assert_eq!(stats.uploaded_vertex_bytes, 0);
                }
            }
            let buffers = renderer.frame_resources.fragments[&window]
                .values()
                .map(|b| {
                    [
                        &b.solid,
                        &b.scene,
                        &b.analytic,
                        &b.extended,
                        &b.clip,
                        &b.text,
                    ]
                    .iter()
                    .filter(|v| v.buffer.is_some())
                    .count()
                })
                .sum::<usize>();
            assert_rgba_images_match(
                if changed {
                    &edited_reference
                } else {
                    &reference
                },
                &renderer.capture_rgba(window).unwrap(),
            );
            times.sort_by(f64::total_cmp);
            println!(
                "PIPELINE limit={limit} edit={changed} p50_ms={:.3} p95_ms={:.3} buffers={buffers}",
                times[FRAMES / 2],
                times[FRAMES * 95 / 100]
            );
            for (name, sum) in [
                "draws",
                "packets",
                "rebuilt_commands",
                "traversal_us",
                "state_us",
                "compose_us",
                "batch_us",
                "finish_us",
                "submit_us",
                "upload_bytes",
            ]
            .into_iter()
            .zip(sums)
            {
                println!("  {name}={:.3}", sum as f64 / FRAMES as f64);
            }
        }
    }
}
