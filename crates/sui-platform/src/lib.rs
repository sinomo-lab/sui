#![deny(unsafe_code)]

mod accessibility;
mod desktop;
mod display_capabilities;
mod headless;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod os_clipboard;
#[cfg(any(target_arch = "wasm32", test))]
mod web_interop;
#[cfg(target_os = "windows")]
mod windows_accessibility;

use web_time::Instant;

use sui_core::WindowId;
use sui_render_wgpu::{
    ColorManagementMode, RendererFrameStats, RequestedColorManagementMode,
    RequestedDynamicRangeMode, RequestedOutputColorPrimaries, RequestedToneMappingMode,
    StemDarkening, TextCoveragePolicy, TextHinting, TextSubpixelOrder, WgpuRenderer,
};
use sui_runtime::{
    CacheMetrics, FramePhase, FramePhaseSample, PresentationLatencyDiagnostics, RenderOutput,
    RendererSubmissionDiagnostics, RetainedPacketHotspotDiagnostics,
    RetainedPacketRebuildDiagnostics, SceneStatistics, TextCacheDiagnostics,
    WindowColorManagementMode, WindowDynamicRangeMode, WindowOutputColorPrimaries,
    WindowPerformanceSnapshot, WindowStemDarkening, WindowTextCoveragePolicy, WindowTextHinting,
    WindowTextSubpixelOrder, WindowToneMappingMode, clear_window_performance_snapshot,
    clear_window_performance_snapshots, publish_window_performance_snapshot,
    window_performance_text_caches, window_scene_statistics_detail_mode,
};

pub(crate) use accessibility::AccessibilityBridge;
pub use accessibility::AccessibilitySnapshot;
#[cfg(target_os = "android")]
pub use desktop::AndroidApp;
pub use desktop::{
    DesktopAutomationAction, DesktopAutomationConfig, DesktopPlatform, WakeSignal, Waker,
};
pub use display_capabilities::{
    WindowOutputDiagnostics, clear_window_output_diagnostics, clear_window_output_diagnostics_all,
    detect_window_display_capabilities, publish_window_output_diagnostics,
    resolve_sdr_content_brightness_nits, window_output_diagnostics,
};
pub use headless::{HeadlessPlatform, PlatformWindow};
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use os_clipboard::OsClipboardBackend;

pub(crate) fn reset_window_performance_store() {
    clear_window_performance_snapshots();
    clear_window_output_diagnostics_all();
}

pub(crate) fn map_window_text_hinting(hinting: WindowTextHinting) -> TextHinting {
    match hinting.normalized() {
        WindowTextHinting::None => TextHinting::None,
        WindowTextHinting::Slight { max_ppem } => TextHinting::Slight { max_ppem },
    }
}

pub(crate) fn map_window_stem_darkening(darkening: WindowStemDarkening) -> StemDarkening {
    match darkening.normalized() {
        WindowStemDarkening::None => StemDarkening::None,
        WindowStemDarkening::Enabled { max_ppem, amount } => {
            StemDarkening::Enabled { max_ppem, amount }
        }
    }
}

pub(crate) fn map_window_text_coverage_policy(
    policy: WindowTextCoveragePolicy,
) -> TextCoveragePolicy {
    match policy.normalized() {
        WindowTextCoveragePolicy::Perceptual => TextCoveragePolicy::Perceptual,
        WindowTextCoveragePolicy::Linear => TextCoveragePolicy::Linear,
        WindowTextCoveragePolicy::Gamma(gamma) => TextCoveragePolicy::Gamma(gamma),
        WindowTextCoveragePolicy::CoverageBoost(amount) => {
            TextCoveragePolicy::CoverageBoost(amount)
        }
        WindowTextCoveragePolicy::TwoCoverageMinusCoverageSq => {
            TextCoveragePolicy::TwoCoverageMinusCoverageSq
        }
    }
}

pub(crate) fn map_window_text_subpixel_order(order: WindowTextSubpixelOrder) -> TextSubpixelOrder {
    match order {
        WindowTextSubpixelOrder::None => TextSubpixelOrder::None,
        WindowTextSubpixelOrder::Rgb => TextSubpixelOrder::Rgb,
        WindowTextSubpixelOrder::Bgr => TextSubpixelOrder::Bgr,
    }
}

pub(crate) fn map_window_color_management(
    mode: WindowColorManagementMode,
    primaries: WindowOutputColorPrimaries,
    dynamic_range: WindowDynamicRangeMode,
    tone_mapping: WindowToneMappingMode,
    sdr_content_brightness_nits: f32,
) -> ColorManagementMode {
    ColorManagementMode {
        mode: match mode {
            WindowColorManagementMode::Automatic => RequestedColorManagementMode::Automatic,
            WindowColorManagementMode::ForceSdr => RequestedColorManagementMode::ForceSdr,
            WindowColorManagementMode::PreferWideGamut => {
                RequestedColorManagementMode::PreferWideGamut
            }
            WindowColorManagementMode::PreferHdr => RequestedColorManagementMode::PreferHdr,
        },
        output_primaries: match primaries {
            WindowOutputColorPrimaries::Automatic => RequestedOutputColorPrimaries::Automatic,
            WindowOutputColorPrimaries::Srgb => RequestedOutputColorPrimaries::Srgb,
            WindowOutputColorPrimaries::DisplayP3 => RequestedOutputColorPrimaries::DisplayP3,
        },
        dynamic_range: match dynamic_range {
            WindowDynamicRangeMode::Automatic => RequestedDynamicRangeMode::Automatic,
            WindowDynamicRangeMode::StandardDynamicRange => {
                RequestedDynamicRangeMode::StandardDynamicRange
            }
            WindowDynamicRangeMode::HighDynamicRange => RequestedDynamicRangeMode::HighDynamicRange,
        },
        tone_mapping: match tone_mapping {
            WindowToneMappingMode::Automatic => RequestedToneMappingMode::Automatic,
            WindowToneMappingMode::Clamp => RequestedToneMappingMode::Clamp,
            WindowToneMappingMode::Reinhard => RequestedToneMappingMode::Reinhard,
        },
        sdr_content_brightness_nits,
    }
}

pub(crate) fn clear_window_performance(window_id: WindowId) {
    clear_window_performance_snapshot(window_id);
    clear_window_output_diagnostics(window_id);
}

fn retained_packet_rebuild_diagnostics(
    rebuilds: sui_render_wgpu::RetainedPacketRebuildStats,
) -> RetainedPacketRebuildDiagnostics {
    RetainedPacketRebuildDiagnostics::new(
        rebuilds.new_count,
        rebuilds.coordinate_space_count,
        rebuilds.signature_count,
        rebuilds.scene_count,
        rebuilds.state_count,
    )
}

fn renderer_submission_diagnostics_from_frame_stats(
    renderer_stats: &RendererFrameStats,
) -> RendererSubmissionDiagnostics {
    RendererSubmissionDiagnostics::new(
        renderer_stats.pass_count,
        renderer_stats.draw_count,
        renderer_stats.uploaded_vertex_bytes,
        renderer_stats.text_glyph_instance_count,
        renderer_stats.text_vertex_bytes,
        renderer_stats.visible_layer_count,
        renderer_stats.direct_packet_count,
        renderer_stats.retained_state_update_time_us,
        renderer_stats.composition_time_us,
        renderer_stats.retained_scene_traversal_time_us,
        renderer_stats.retained_packet_build_time_us,
        renderer_stats.retained_packet_build_count,
        retained_packet_rebuild_diagnostics(renderer_stats.retained_packet_rebuilds),
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
    )
}

fn retained_packet_hotspot_diagnostics_from_frame_stats(
    renderer_stats: &RendererFrameStats,
) -> Option<RetainedPacketHotspotDiagnostics> {
    renderer_stats
        .retained_packet_hotspot
        .clone()
        .map(|hotspot| RetainedPacketHotspotDiagnostics {
            container_layer_id: hotspot.container_layer_id,
            owner_widget_id: hotspot.owner_widget_id,
            segment_index: hotspot.segment_index,
            total_time_us: hotspot.total_time_us,
            scene_build_time_us: hotspot.scene_build_time_us,
            command_count: hotspot.command_count,
            text_command_count: hotspot.text_command_count,
            path_command_count: hotspot.path_command_count,
            rect_command_count: hotspot.rect_command_count,
            text_command_time_us: hotspot.text_command_time_us,
            path_command_time_us: hotspot.path_command_time_us,
            rect_command_time_us: hotspot.rect_command_time_us,
            text_sample: hotspot.text_sample,
        })
}

fn split_renderer_phase_times_ms(
    renderer_time_ms: f64,
    renderer_stats: &RendererFrameStats,
) -> (f64, f64) {
    let renderer_time_ms = renderer_time_ms.max(0.0);
    let surface_wait_time_ms = ((renderer_stats
        .surface_acquire_time_us
        .saturating_add(renderer_stats.surface_present_time_us))
        as f64
        / 1000.0)
        .min(renderer_time_ms);
    let renderer_work_time_ms = (renderer_time_ms - surface_wait_time_ms).max(0.0);
    (renderer_work_time_ms, surface_wait_time_ms)
}

#[allow(clippy::too_many_arguments)]
pub fn publish_frame_performance(
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
        let scene =
            SceneStatistics::minimal(&output.frame, output.diagnostics.widget_count, detail_mode)
                .with_animation_counters(
                    output.diagnostics.active_animated_widget_count,
                    output.diagnostics.animation_frame_wake_count,
                    output.diagnostics.animation_repaint_frame_count,
                    output
                        .diagnostics
                        .animation_transform_effect_only_frame_count,
                );
        publish_window_performance_snapshot(
            WindowPerformanceSnapshot::with_total_time_ms(
                window_id,
                frame_index,
                total_time_ms,
                Vec::new(),
                RendererSubmissionDiagnostics::default(),
                TextCacheDiagnostics::default(),
                Default::default(),
                scene,
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
    let (renderer_work_time_ms, surface_wait_time_ms) =
        split_renderer_phase_times_ms(renderer_time_ms, &renderer_stats);
    if renderer_work_time_ms > 0.0 {
        phase_timings.push(FramePhaseSample::new(
            FramePhase::Renderer,
            renderer_work_time_ms,
        ));
    }
    if surface_wait_time_ms > 0.0 {
        phase_timings.push(FramePhaseSample::new(
            FramePhase::SurfaceWait,
            surface_wait_time_ms,
        ));
    }
    phase_timings.push(FramePhaseSample::new(
        FramePhase::Diagnostics,
        diagnostics_started.elapsed().as_secs_f64() * 1000.0,
    ));
    let total_time_ms = event_time_ms
        + redraw_time_ms
        + runtime_time_ms
        + renderer_time_ms
        + diagnostics_started.elapsed().as_secs_f64() * 1000.0;

    let scene = SceneStatistics::from_frame_with_mode(
        &output.frame,
        output.diagnostics.widget_count,
        detail_mode,
    )
    .with_animation_counters(
        output.diagnostics.active_animated_widget_count,
        output.diagnostics.animation_frame_wake_count,
        output.diagnostics.animation_repaint_frame_count,
        output
            .diagnostics
            .animation_transform_effect_only_frame_count,
    );

    publish_window_performance_snapshot(
        WindowPerformanceSnapshot::with_total_time_ms(
            window_id,
            frame_index,
            total_time_ms,
            phase_timings,
            renderer_submission_diagnostics_from_frame_stats(&renderer_stats),
            text_caches,
            text_cache_deltas,
            scene,
        )
        .with_presentation_latency(presentation_latency)
        .with_runtime_text_timing(output.diagnostics.runtime_text_timing)
        .with_retained_packet_hotspot(retained_packet_hotspot_diagnostics_from_frame_stats(
            &renderer_stats,
        ))
        .with_widget_timings(output.diagnostics.widget_timings.clone()),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        renderer_submission_diagnostics_from_frame_stats,
        retained_packet_hotspot_diagnostics_from_frame_stats, retained_packet_rebuild_diagnostics,
        split_renderer_phase_times_ms,
    };
    use sui_render_wgpu::{RendererFrameStats, RendererPacketHotspot, RetainedPacketRebuildStats};

    #[test]
    fn retained_packet_rebuild_diagnostics_bridge_preserves_named_buckets() {
        let rebuilds = RetainedPacketRebuildStats::new(2, 3, 5, 7, 11);

        let diagnostics = retained_packet_rebuild_diagnostics(rebuilds);

        assert_eq!(diagnostics.new_count, 2);
        assert_eq!(diagnostics.coordinate_space_count, 3);
        assert_eq!(diagnostics.signature_count, 5);
        assert_eq!(diagnostics.scene_count, 7);
        assert_eq!(diagnostics.state_count, 11);
    }

    #[test]
    fn renderer_submission_diagnostics_from_frame_stats_preserves_retained_details() {
        let renderer_stats = RendererFrameStats {
            pass_count: 3,
            draw_count: 9,
            uploaded_vertex_bytes: 4096,
            text_glyph_instance_count: 42,
            text_vertex_bytes: 2048,
            visible_layer_count: 4,
            direct_packet_count: 6,
            retained_state_update_time_us: 31,
            composition_time_us: 32,
            retained_scene_traversal_time_us: 33,
            retained_packet_build_time_us: 34,
            retained_packet_build_count: 2,
            retained_packet_rebuilds: RetainedPacketRebuildStats::new(1, 0, 1, 1, 0),
            retained_packet_normalize_time_us: 41,
            retained_packet_signature_time_us: 42,
            retained_packet_raster_state_init_time_us: 43,
            retained_packet_scene_build_time_us: 44,
            retained_packet_command_count: 45,
            retained_packet_text_command_count: 46,
            retained_packet_path_command_count: 47,
            retained_packet_clip_path_command_count: 48,
            retained_packet_image_command_count: 49,
            retained_packet_rect_command_count: 50,
            retained_packet_text_command_time_us: 51,
            retained_packet_path_command_time_us: 52,
            retained_packet_clip_path_command_time_us: 53,
            retained_packet_image_command_time_us: 54,
            retained_packet_rect_command_time_us: 55,
            text_atlas_miss_count: 7,
            text_atlas_miss_time_us: 56,
            surface_acquire_time_us: 57,
            resource_collection_time_us: 58,
            bind_group_prepare_time_us: 59,
            image_bind_group_time_us: 60,
            analytic_path_bind_group_time_us: 61,
            analytic_path_bind_group_miss_count: 8,
            analytic_path_bind_group_upload_bytes: 62,
            text_atlas_bind_group_time_us: 63,
            text_atlas_upload_copy_time_us: 64,
            text_atlas_upload_write_time_us: 65,
            text_atlas_upload_bytes: 66,
            batch_prepare_time_us: 67,
            gpu_upload_time_us: 68,
            pass_encode_time_us: 69,
            queue_submit_time_us: 70,
            surface_present_time_us: 71,
            retained_packet_hotspot: Some(RendererPacketHotspot {
                container_layer_id: Some(101),
                owner_widget_id: Some(202),
                segment_index: 3,
                total_time_us: 72,
                scene_build_time_us: 73,
                command_count: 74,
                text_command_count: 75,
                path_command_count: 76,
                rect_command_count: 77,
                text_command_time_us: 78,
                path_command_time_us: 79,
                rect_command_time_us: 80,
                text_sample: Some("packet text".to_string()),
            }),
        };

        let diagnostics = renderer_submission_diagnostics_from_frame_stats(&renderer_stats);

        assert_eq!(diagnostics.pass_count, 3);
        assert_eq!(diagnostics.retained_packet_build_count, 2);
        assert_eq!(diagnostics.retained_packet_rebuilds.new_count, 1);
        assert_eq!(diagnostics.retained_packet_rebuilds.signature_count, 1);
        assert_eq!(diagnostics.retained_packet_rebuilds.scene_count, 1);
        assert_eq!(diagnostics.retained_packet_normalize_time_us, 41);
        assert_eq!(diagnostics.retained_packet_command_count, 45);
        assert_eq!(diagnostics.text_atlas_miss_count, 7);
    }

    #[test]
    fn renderer_phase_split_separates_surface_wait_from_work() {
        let renderer_stats = RendererFrameStats {
            surface_acquire_time_us: 4_000,
            surface_present_time_us: 8_000,
            ..Default::default()
        };

        let (renderer_work_time_ms, surface_wait_time_ms) =
            split_renderer_phase_times_ms(20.0, &renderer_stats);

        assert_eq!(renderer_work_time_ms, 8.0);
        assert_eq!(surface_wait_time_ms, 12.0);
    }

    #[test]
    fn renderer_phase_split_caps_wait_at_renderer_wall_time() {
        let renderer_stats = RendererFrameStats {
            surface_acquire_time_us: 9_000,
            surface_present_time_us: 9_000,
            ..Default::default()
        };

        let (renderer_work_time_ms, surface_wait_time_ms) =
            split_renderer_phase_times_ms(10.0, &renderer_stats);

        assert_eq!(renderer_work_time_ms, 0.0);
        assert_eq!(surface_wait_time_ms, 10.0);
    }

    #[test]
    fn retained_packet_hotspot_diagnostics_bridge_preserves_hotspot_fields() {
        let renderer_stats = RendererFrameStats {
            retained_packet_hotspot: Some(RendererPacketHotspot {
                container_layer_id: Some(11),
                owner_widget_id: Some(22),
                segment_index: 3,
                total_time_us: 44,
                scene_build_time_us: 55,
                command_count: 66,
                text_command_count: 77,
                path_command_count: 88,
                rect_command_count: 99,
                text_command_time_us: 111,
                path_command_time_us: 222,
                rect_command_time_us: 333,
                text_sample: Some("sample".to_string()),
            }),
            ..Default::default()
        };

        let hotspot = retained_packet_hotspot_diagnostics_from_frame_stats(&renderer_stats)
            .expect("expected hotspot diagnostics");

        assert_eq!(hotspot.container_layer_id, Some(11));
        assert_eq!(hotspot.owner_widget_id, Some(22));
        assert_eq!(hotspot.segment_index, 3);
        assert_eq!(hotspot.total_time_us, 44);
        assert_eq!(hotspot.scene_build_time_us, 55);
        assert_eq!(hotspot.command_count, 66);
        assert_eq!(hotspot.text_command_count, 77);
        assert_eq!(hotspot.path_command_count, 88);
        assert_eq!(hotspot.rect_command_count, 99);
        assert_eq!(hotspot.text_command_time_us, 111);
        assert_eq!(hotspot.path_command_time_us, 222);
        assert_eq!(hotspot.rect_command_time_us, 333);
        assert_eq!(hotspot.text_sample.as_deref(), Some("sample"));
    }
}
