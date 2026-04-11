#![forbid(unsafe_code)]

mod accessibility;
mod desktop;
mod headless;

use std::time::Instant;

use sui_core::WindowId;
use sui_render_wgpu::{TextCoveragePolicy, WgpuRenderer};
use sui_runtime::{
    CacheMetrics, FramePhase, FramePhaseSample, PresentationLatencyDiagnostics, RenderOutput,
    RendererSubmissionDiagnostics, SceneStatistics, TextCacheDiagnostics,
    WindowPerformanceSnapshot, WindowTextRenderPolicy, clear_window_performance_snapshot,
    clear_window_performance_snapshots, publish_window_performance_snapshot,
    window_performance_text_caches, window_scene_statistics_detail_mode,
};

pub(crate) use accessibility::AccessibilityBridge;
pub use accessibility::AccessibilitySnapshot;
pub use desktop::DesktopPlatform;
pub use headless::{HeadlessPlatform, PlatformWindow};

pub(crate) fn reset_window_performance_store() {
    clear_window_performance_snapshots();
}

pub(crate) fn map_window_text_render_policy(policy: WindowTextRenderPolicy) -> TextCoveragePolicy {
    match policy.normalized() {
        WindowTextRenderPolicy::AutomaticByTextLuminance => {
            TextCoveragePolicy::AutomaticByTextLuminance
        }
        WindowTextRenderPolicy::Linear => TextCoveragePolicy::Linear,
        WindowTextRenderPolicy::Gamma(gamma) => TextCoveragePolicy::Gamma(gamma),
        WindowTextRenderPolicy::TwoCoverageMinusCoverageSq => {
            TextCoveragePolicy::TwoCoverageMinusCoverageSq
        }
    }
}

pub(crate) fn clear_window_performance(window_id: WindowId) {
    clear_window_performance_snapshot(window_id);
}

pub(crate) fn publish_frame_performance(
    window_id: WindowId,
    frame_index: u64,
    event_time_ms: f64,
    redraw_time_ms: f64,
    runtime_time_ms: f64,
    presentation_latency: PresentationLatencyDiagnostics,
    output: &RenderOutput,
    renderer: &WgpuRenderer,
    renderer_time_ms: f64,
) {
    let detail_mode = window_scene_statistics_detail_mode(window_id);
    let total_time_ms = event_time_ms + redraw_time_ms + runtime_time_ms + renderer_time_ms;

    if !detail_mode.is_detailed() {
        publish_window_performance_snapshot(
            WindowPerformanceSnapshot::with_total_time_ms(
                window_id,
                frame_index,
                total_time_ms,
                Vec::new(),
                RendererSubmissionDiagnostics::default(),
                TextCacheDiagnostics::default(),
                Default::default(),
                SceneStatistics::minimal(&output.frame, detail_mode),
            )
            .with_presentation_latency(presentation_latency)
            .with_runtime_text_timing(output.diagnostics.runtime_text_timing)
            .with_widget_timings(output.diagnostics.widget_timings.clone()),
        );
        return;
    }

    let diagnostics_started = Instant::now();
    let mut phase_timings = Vec::with_capacity(output.diagnostics.phase_timings.len() + 3);
    let renderer_text_cache = renderer.text_cache_snapshot(window_id);
    let text_caches = TextCacheDiagnostics {
        runtime_layout: output.diagnostics.text_caches.runtime_layout,
        renderer_layout: CacheMetrics::new(
            renderer_text_cache.layout.entries,
            renderer_text_cache.layout.hits,
            renderer_text_cache.layout.misses,
        ),
        renderer_glyph: CacheMetrics::new(
            renderer_text_cache.glyph.entries,
            renderer_text_cache.glyph.hits,
            renderer_text_cache.glyph.misses,
        ),
        renderer_path: CacheMetrics::new(
            renderer_text_cache.path.entries,
            renderer_text_cache.path.hits,
            renderer_text_cache.path.misses,
        ),
    };
    let text_cache_deltas = window_performance_text_caches(window_id)
        .map(|previous| text_caches.delta_from(&previous))
        .unwrap_or_else(|| text_caches.delta_from(&TextCacheDiagnostics::default()));
    let renderer_stats = renderer.last_frame_stats(window_id).unwrap_or_default();

    if event_time_ms > 0.0 {
        phase_timings.push(FramePhaseSample::new(FramePhase::Event, event_time_ms));
    }

    if redraw_time_ms > 0.0 {
        phase_timings.push(FramePhaseSample::new(FramePhase::Redraw, redraw_time_ms));
    }

    phase_timings.extend(output.diagnostics.phase_timings.iter().copied());
    phase_timings.push(FramePhaseSample::new(
        FramePhase::Renderer,
        renderer_time_ms,
    ));
    phase_timings.push(FramePhaseSample::new(
        FramePhase::Diagnostics,
        diagnostics_started.elapsed().as_secs_f64() * 1000.0,
    ));
    let total_time_ms = event_time_ms
        + redraw_time_ms
        + runtime_time_ms
        + renderer_time_ms
        + diagnostics_started.elapsed().as_secs_f64() * 1000.0;

    publish_window_performance_snapshot(
        WindowPerformanceSnapshot::with_total_time_ms(
            window_id,
            frame_index,
            total_time_ms,
            phase_timings,
            RendererSubmissionDiagnostics::new(
                renderer_stats.pass_count,
                renderer_stats.draw_count,
                renderer_stats.uploaded_vertex_bytes,
                renderer_stats.text_glyph_instance_count,
                renderer_stats.text_vertex_bytes,
                renderer_stats.visible_layer_count,
                renderer_stats.visible_tile_count,
                renderer_stats.reused_tile_count,
                renderer_stats.regenerated_tile_count,
                renderer_stats.direct_packet_count,
                renderer_stats.tile_memory_bytes,
                renderer_stats.tile_generation_time_us,
                renderer_stats.composition_time_us,
                renderer_stats.retained_scene_traversal_time_us,
                renderer_stats.retained_packet_build_time_us,
                renderer_stats.retained_packet_build_count,
                renderer_stats.retained_packet_rebuild_new_count,
                renderer_stats.retained_packet_rebuild_coordinate_space_count,
                renderer_stats.retained_packet_rebuild_signature_count,
                renderer_stats.retained_packet_rebuild_scene_count,
                renderer_stats.retained_packet_rebuild_state_count,
                renderer_stats.text_atlas_miss_count,
                renderer_stats.text_atlas_miss_time_us,
                renderer_stats.surface_acquire_time_us,
                renderer_stats.resource_collection_time_us,
                renderer_stats.bind_group_prepare_time_us,
                renderer_stats.image_bind_group_time_us,
                renderer_stats.analytic_path_bind_group_time_us,
                renderer_stats.analytic_path_bind_group_miss_count,
                renderer_stats.analytic_path_bind_group_upload_bytes,
                renderer_stats.text_atlas_bind_group_time_us,
                renderer_stats.text_atlas_upload_copy_time_us,
                renderer_stats.text_atlas_upload_write_time_us,
                renderer_stats.text_atlas_upload_bytes,
                renderer_stats.batch_prepare_time_us,
                renderer_stats.gpu_upload_time_us,
                renderer_stats.pass_encode_time_us,
                renderer_stats.queue_submit_time_us,
                renderer_stats.surface_present_time_us,
            )
            .with_retained_packet_breakdown(
                renderer_stats.retained_packet_normalize_time_us,
                renderer_stats.retained_packet_signature_time_us,
                renderer_stats.retained_packet_raster_state_init_time_us,
                renderer_stats.retained_packet_scene_build_time_us,
                renderer_stats.retained_packet_command_count,
                renderer_stats.retained_packet_text_command_count,
                renderer_stats.retained_packet_path_command_count,
                renderer_stats.retained_packet_clip_path_command_count,
                renderer_stats.retained_packet_image_command_count,
                renderer_stats.retained_packet_rect_command_count,
                renderer_stats.retained_packet_text_command_time_us,
                renderer_stats.retained_packet_path_command_time_us,
                renderer_stats.retained_packet_clip_path_command_time_us,
                renderer_stats.retained_packet_image_command_time_us,
                renderer_stats.retained_packet_rect_command_time_us,
            ),
            text_caches,
            text_cache_deltas,
            SceneStatistics::from_frame_with_mode(&output.frame, detail_mode),
        )
        .with_presentation_latency(presentation_latency)
        .with_runtime_text_timing(output.diagnostics.runtime_text_timing)
        .with_widget_timings(output.diagnostics.widget_timings.clone()),
    );
}
