#![forbid(unsafe_code)]

mod accessibility;
mod desktop;
mod headless;

use sui_core::WindowId;
use sui_render_wgpu::WgpuRenderer;
use sui_runtime::{
	CacheMetrics, FramePhase, FramePhaseSample, RenderOutput, SceneStatistics,
	TextCacheDiagnostics, WindowPerformanceSnapshot, window_performance_snapshot,
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
	let mut phase_timings = Vec::new();
	let renderer_text_cache = renderer.text_cache_snapshot();
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
	};
	let text_cache_deltas = window_performance_snapshot(window_id)
		.map(|previous| text_caches.delta_from(&previous.text_caches))
		.unwrap_or_else(|| text_caches.delta_from(&TextCacheDiagnostics::default()));

	if event_time_ms > 0.0 {
		phase_timings.push(FramePhaseSample::new(FramePhase::Event, event_time_ms));
	}

	phase_timings.extend(output.diagnostics.phase_timings.iter().copied());
	phase_timings.push(FramePhaseSample::new(FramePhase::Renderer, renderer_time_ms));

	publish_window_performance_snapshot(WindowPerformanceSnapshot::new(
		window_id,
		frame_index,
		phase_timings,
		text_caches,
		text_cache_deltas,
		SceneStatistics::from_frame(&output.frame),
	));
}
