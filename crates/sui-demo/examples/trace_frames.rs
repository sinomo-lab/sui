//! Native host frame trace, including Windows' modal move/resize loop.
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use sui::{
    DesktopPlatform, Result, SceneStatisticsDetailMode, set_window_render_options,
    set_window_scene_statistics_detail_mode, window_performance_summary,
};

fn main() -> Result<()> {
    let app = sui_demo_app::build_dev_application();
    let options = app.initial_window_render_options();
    let runtime = app.build()?;
    let registry = sui_render_wgpu::WgpuExternalTextureRegistry::new();
    let platform = DesktopPlatform::new()
        .with_vsync_enabled(true)
        .with_external_texture_registry(registry.clone());
    let window_ids = runtime.window_ids();
    for &id in &window_ids {
        if let Some(options) = options {
            set_window_render_options(id, options);
        }
        set_window_scene_statistics_detail_mode(id, SceneStatisticsDetailMode::Detailed);
    }

    // AboutToWait/desktop-extension callbacks can stop during an OS modal loop.
    // Observe published summaries independently so the trace does not mistake
    // missing observer callbacks for missing presentation.
    let stop = Arc::new(AtomicBool::new(false));
    let observer_stop = Arc::clone(&stop);
    let observer = thread::spawn(move || {
        let started = Instant::now();
        let mut reported_adapter = false;
        let mut previous = HashMap::new();
        while !observer_stop.load(Ordering::Relaxed) {
            if !reported_adapter && let Some(context) = registry.context() {
                println!("ADAPTER {:?}", context.adapter_info());
                reported_adapter = true;
            }
            for &id in &window_ids {
                if let Some(frame) = window_performance_summary(id)
                    && previous.insert(id, frame.frame_index) != Some(frame.frame_index)
                {
                    let stats = &frame.renderer_submission;
                    println!(
                        "TRACE time={:.3} window={} frame={} interval_ms={:?} work_ms={:.3} animating={} wakes={} commands={} upload_us={} encode_us={} submit_us={} acquire_us={} present_us={} packet_us={}",
                        started.elapsed().as_secs_f64(),
                        id.get(),
                        frame.frame_index,
                        frame.frame_interval_ms,
                        frame.total_time_ms,
                        frame.active_animated_widget_count,
                        frame.animation_frame_wake_count,
                        frame.command_count,
                        stats.gpu_upload_time_us,
                        stats.pass_encode_time_us,
                        stats.queue_submit_time_us,
                        stats.surface_acquire_time_us,
                        stats.surface_present_time_us,
                        stats.retained_packet_build_time_us,
                    );
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    });
    let result = platform.run(runtime);
    stop.store(true, Ordering::Relaxed);
    observer.join().expect("frame trace observer panicked");
    result?;
    Ok(())
}
