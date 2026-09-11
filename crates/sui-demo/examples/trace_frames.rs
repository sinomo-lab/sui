//! Native host frame trace for reproducing interactive presentation issues.
use std::{collections::HashMap, time::Instant};

use sui::{
    DesktopExtension, DesktopExtensionContext, DesktopPlatform, Result, SceneStatisticsDetailMode,
    set_window_render_options, set_window_scene_statistics_detail_mode,
    window_performance_snapshot,
};

#[derive(Debug)]
struct FrameTrace {
    previous: HashMap<sui::WindowId, u64>,
    started: Instant,
}

impl DesktopExtension for FrameTrace {
    fn update(&mut self, context: DesktopExtensionContext<'_>) -> Result<()> {
        for id in context.runtime().window_ids() {
            if let Some(frame) = window_performance_snapshot(id)
                && self.previous.insert(id, frame.frame_index) != Some(frame.frame_index)
            {
                let stats = &frame.renderer_submission;
                println!(
                    "TRACE time={:.3} window={} frame={} interval_ms={:?} work_ms={:.3} animating={} commands={} upload_us={} encode_us={} submit_us={} acquire_us={} present_us={} packet_us={}",
                    self.started.elapsed().as_secs_f64(),
                    id.get(),
                    frame.frame_index,
                    frame.frame_interval_ms,
                    frame.total_time_ms,
                    frame.scene.active_animated_widget_count,
                    frame.scene.command_count,
                    stats.gpu_upload_time_us,
                    stats.pass_encode_time_us,
                    stats.queue_submit_time_us,
                    stats.surface_acquire_time_us,
                    stats.surface_present_time_us,
                    stats.retained_packet_build_time_us,
                );
            }
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let app = sui_demo_app::build_dev_application();
    let options = app.initial_window_render_options();
    let runtime = app.build()?;
    let platform = DesktopPlatform::new()
        .with_vsync_enabled(true)
        .with_extension(FrameTrace {
            previous: HashMap::new(),
            started: Instant::now(),
        });
    for id in runtime.window_ids() {
        if let Some(options) = options {
            set_window_render_options(id, options);
        }
        set_window_scene_statistics_detail_mode(id, SceneStatisticsDetailMode::Detailed);
    }
    platform.run(runtime)?;
    Ok(())
}
