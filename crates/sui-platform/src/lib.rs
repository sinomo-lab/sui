#![forbid(unsafe_code)]

mod accessibility;
mod desktop;
mod headless;

use std::time::Instant;

use sui_core::WindowId;
use sui_render_wgpu::WgpuRenderer;
use sui_runtime::{
	CacheMetrics, FramePhase, FramePhaseSample, RenderOutput, SceneStatistics,
	RendererSubmissionDiagnostics, TextCacheDiagnostics, WindowPerformanceSnapshot,
	window_performance_text_caches, window_scene_statistics_detail_mode,
	clear_window_performance_snapshot, clear_window_performance_snapshots,
	publish_window_performance_snapshot,
};

pub(crate) use accessibility::AccessibilityBridge;
pub use accessibility::AccessibilitySnapshot;
pub use desktop::DesktopPlatform;
pub use headless::{HeadlessPlatform, PlatformWindow};

pub(crate) fn reset_window_performance_store() {
	clear_window_performance_snapshots();
}

pub(crate) fn clear_window_performance(window_id: WindowId) {
	clear_window_performance_snapshot(window_id);
}

pub(crate) fn publish_frame_performance(
	window_id: WindowId,
	frame_index: u64,
	event_time_ms: f64,
	output: &RenderOutput,
	renderer: &WgpuRenderer,
	renderer_time_ms: f64,
) {
	let diagnostics_started = Instant::now();
	let mut phase_timings = Vec::with_capacity(output.diagnostics.phase_timings.len() + 2);
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

	phase_timings.extend(output.diagnostics.phase_timings.iter().copied());
	phase_timings.push(FramePhaseSample::new(FramePhase::Renderer, renderer_time_ms));
	phase_timings.push(FramePhaseSample::new(
		FramePhase::Diagnostics,
		diagnostics_started.elapsed().as_secs_f64() * 1000.0,
	));

	publish_window_performance_snapshot(WindowPerformanceSnapshot::new(
		window_id,
		frame_index,
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
			renderer_stats.text_atlas_miss_count,
			renderer_stats.text_atlas_miss_time_us,
			renderer_stats.text_atlas_fallback_count,
		),
		text_caches,
		text_cache_deltas,
		SceneStatistics::from_frame_with_mode(
			&output.frame,
			window_scene_statistics_detail_mode(window_id),
		),
	));
}
