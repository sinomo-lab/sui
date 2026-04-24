use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    sync::{OnceLock, RwLock},
    time::Duration,
};

use sui_core::{DirtyRegion, Size, WidgetId, WindowId};
use sui_scene::{LayerCompositionMode, SceneCommand, SceneFrame, SceneLayerUpdateKind};
use sui_text::RuntimeTextTimingDiagnostics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    Event,
    Redraw,
    MeasureArrange,
    HitTest,
    Paint,
    Semantics,
    Renderer,
    Diagnostics,
}

impl FramePhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Event => "Event handling",
            Self::Redraw => "Redraw callback",
            Self::MeasureArrange => "Measure and arrange",
            Self::HitTest => "Graph and hit test",
            Self::Paint => "Paint",
            Self::Semantics => "Semantics",
            Self::Renderer => "Renderer",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramePhaseSample {
    pub phase: FramePhase,
    pub duration_ms: f64,
}

impl FramePhaseSample {
    pub const fn new(phase: FramePhase, duration_ms: f64) -> Self {
        Self { phase, duration_ms }
    }

    pub fn from_duration(phase: FramePhase, duration: Duration) -> Self {
        Self::new(phase, duration.as_secs_f64() * 1000.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WidgetTimingPhase {
    Measure,
    Arrange,
    Paint,
    Semantics,
}

impl WidgetTimingPhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Measure => "Measure",
            Self::Arrange => "Arrange",
            Self::Paint => "Paint",
            Self::Semantics => "Semantics",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetTimingSample {
    pub widget_id: WidgetId,
    pub widget_name: &'static str,
    pub phase: WidgetTimingPhase,
    pub duration_ms: f64,
    pub calls: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderDiagnostics {
    pub phase_timings: Vec<FramePhaseSample>,
    pub text_caches: TextCacheDiagnostics,
    pub runtime_text_timing: RuntimeTextTimingDiagnostics,
    pub widget_timings: Vec<WidgetTimingSample>,
    pub widget_count: usize,
    pub active_animated_widget_count: usize,
    pub animation_frame_wake_count: usize,
    pub animation_repaint_frame_count: usize,
    pub animation_transform_effect_only_frame_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct WidgetTimingKey {
    widget_id: WidgetId,
    widget_name: &'static str,
    phase: WidgetTimingPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct WidgetTimingAccum {
    duration_ms: f64,
    calls: usize,
}

thread_local! {
    static WIDGET_TIMING_COLLECTOR: RefCell<Option<BTreeMap<WidgetTimingKey, WidgetTimingAccum>>> =
        const { RefCell::new(None) };
}

fn widget_timing_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SUI_PROFILE_WIDGET_TIMINGS").is_some())
}

pub(crate) fn begin_widget_timing_collection() {
    if !widget_timing_enabled() {
        return;
    }

    WIDGET_TIMING_COLLECTOR.with(|collector| {
        *collector.borrow_mut() = Some(BTreeMap::new());
    });
}

pub(crate) fn record_widget_timing(
    widget_id: WidgetId,
    widget_name: &'static str,
    phase: WidgetTimingPhase,
    duration: Duration,
) {
    if !widget_timing_enabled() {
        return;
    }

    let duration_ms = duration.as_secs_f64() * 1000.0;
    WIDGET_TIMING_COLLECTOR.with(|collector| {
        let mut collector = collector.borrow_mut();
        let Some(entries) = collector.as_mut() else {
            return;
        };

        let entry = entries
            .entry(WidgetTimingKey {
                widget_id,
                widget_name,
                phase,
            })
            .or_default();
        entry.duration_ms += duration_ms;
        entry.calls += 1;
    });
}

pub(crate) fn take_widget_timing_collection() -> Vec<WidgetTimingSample> {
    if !widget_timing_enabled() {
        return Vec::new();
    }

    let mut samples = WIDGET_TIMING_COLLECTOR
        .with(|collector| collector.borrow_mut().take())
        .unwrap_or_default()
        .into_iter()
        .map(|(key, accum)| WidgetTimingSample {
            widget_id: key.widget_id,
            widget_name: key.widget_name,
            phase: key.phase,
            duration_ms: accum.duration_ms,
            calls: accum.calls,
        })
        .collect::<Vec<_>>();
    samples.sort_by(|left, right| {
        right
            .duration_ms
            .total_cmp(&left.duration_ms)
            .then_with(|| left.phase.cmp(&right.phase))
            .then_with(|| left.widget_id.get().cmp(&right.widget_id.get()))
    });
    samples
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RetainedPacketRebuildDiagnostics {
    pub new_count: usize,
    pub coordinate_space_count: usize,
    pub signature_count: usize,
    pub scene_count: usize,
    pub state_count: usize,
}

impl RetainedPacketRebuildDiagnostics {
    pub const fn new(
        new_count: usize,
        coordinate_space_count: usize,
        signature_count: usize,
        scene_count: usize,
        state_count: usize,
    ) -> Self {
        Self {
            new_count,
            coordinate_space_count,
            signature_count,
            scene_count,
            state_count,
        }
    }

    pub const fn total_count(&self) -> usize {
        self.new_count
            + self.coordinate_space_count
            + self.signature_count
            + self.scene_count
            + self.state_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererSubmissionDiagnostics {
    pub pass_count: usize,
    pub draw_count: usize,
    pub uploaded_vertex_bytes: u64,
    pub text_glyph_instance_count: usize,
    pub text_vertex_bytes: u64,
    pub visible_layer_count: usize,
    pub direct_packet_count: usize,
    pub retained_state_update_time_us: u64,
    pub composition_time_us: u64,
    pub retained_scene_traversal_time_us: u64,
    pub retained_packet_build_time_us: u64,
    pub retained_packet_build_count: usize,
    pub retained_packet_rebuilds: RetainedPacketRebuildDiagnostics,
    pub retained_packet_normalize_time_us: u64,
    pub retained_packet_signature_time_us: u64,
    pub retained_packet_raster_state_init_time_us: u64,
    pub retained_packet_scene_build_time_us: u64,
    pub retained_packet_command_count: usize,
    pub retained_packet_text_command_count: usize,
    pub retained_packet_path_command_count: usize,
    pub retained_packet_clip_path_command_count: usize,
    pub retained_packet_image_command_count: usize,
    pub retained_packet_rect_command_count: usize,
    pub retained_packet_text_command_time_us: u64,
    pub retained_packet_path_command_time_us: u64,
    pub retained_packet_clip_path_command_time_us: u64,
    pub retained_packet_image_command_time_us: u64,
    pub retained_packet_rect_command_time_us: u64,
    pub text_atlas_miss_count: usize,
    pub text_atlas_miss_time_us: u64,
    pub surface_acquire_time_us: u64,
    pub resource_collection_time_us: u64,
    pub bind_group_prepare_time_us: u64,
    pub image_bind_group_time_us: u64,
    pub analytic_path_bind_group_time_us: u64,
    pub analytic_path_bind_group_miss_count: usize,
    pub analytic_path_bind_group_upload_bytes: u64,
    pub text_atlas_bind_group_time_us: u64,
    pub text_atlas_upload_copy_time_us: u64,
    pub text_atlas_upload_write_time_us: u64,
    pub text_atlas_upload_bytes: u64,
    pub batch_prepare_time_us: u64,
    pub gpu_upload_time_us: u64,
    pub pass_encode_time_us: u64,
    pub queue_submit_time_us: u64,
    pub surface_present_time_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetainedPacketHotspotDiagnostics {
    pub container_layer_id: Option<u64>,
    pub owner_widget_id: Option<u64>,
    pub segment_index: u32,
    pub total_time_us: u64,
    pub scene_build_time_us: u64,
    pub command_count: usize,
    pub text_command_count: usize,
    pub path_command_count: usize,
    pub rect_command_count: usize,
    pub text_command_time_us: u64,
    pub path_command_time_us: u64,
    pub rect_command_time_us: u64,
    pub text_sample: Option<String>,
}

impl RendererSubmissionDiagnostics {
    pub const fn new(
        pass_count: usize,
        draw_count: usize,
        uploaded_vertex_bytes: u64,
        text_glyph_instance_count: usize,
        text_vertex_bytes: u64,
        visible_layer_count: usize,
        direct_packet_count: usize,
        retained_state_update_time_us: u64,
        composition_time_us: u64,
        retained_scene_traversal_time_us: u64,
        retained_packet_build_time_us: u64,
        retained_packet_build_count: usize,
        retained_packet_rebuilds: RetainedPacketRebuildDiagnostics,
        text_atlas_miss_count: usize,
        text_atlas_miss_time_us: u64,
        surface_acquire_time_us: u64,
        resource_collection_time_us: u64,
        bind_group_prepare_time_us: u64,
        image_bind_group_time_us: u64,
        analytic_path_bind_group_time_us: u64,
        analytic_path_bind_group_miss_count: usize,
        analytic_path_bind_group_upload_bytes: u64,
        text_atlas_bind_group_time_us: u64,
        text_atlas_upload_copy_time_us: u64,
        text_atlas_upload_write_time_us: u64,
        text_atlas_upload_bytes: u64,
        batch_prepare_time_us: u64,
        gpu_upload_time_us: u64,
        pass_encode_time_us: u64,
        queue_submit_time_us: u64,
        surface_present_time_us: u64,
    ) -> Self {
        Self {
            pass_count,
            draw_count,
            uploaded_vertex_bytes,
            text_glyph_instance_count,
            text_vertex_bytes,
            visible_layer_count,
            direct_packet_count,
            retained_state_update_time_us,
            composition_time_us,
            retained_scene_traversal_time_us,
            retained_packet_build_time_us,
            retained_packet_build_count,
            retained_packet_rebuilds,
            retained_packet_normalize_time_us: 0,
            retained_packet_signature_time_us: 0,
            retained_packet_raster_state_init_time_us: 0,
            retained_packet_scene_build_time_us: 0,
            retained_packet_command_count: 0,
            retained_packet_text_command_count: 0,
            retained_packet_path_command_count: 0,
            retained_packet_clip_path_command_count: 0,
            retained_packet_image_command_count: 0,
            retained_packet_rect_command_count: 0,
            retained_packet_text_command_time_us: 0,
            retained_packet_path_command_time_us: 0,
            retained_packet_clip_path_command_time_us: 0,
            retained_packet_image_command_time_us: 0,
            retained_packet_rect_command_time_us: 0,
            text_atlas_miss_count,
            text_atlas_miss_time_us,
            surface_acquire_time_us,
            resource_collection_time_us,
            bind_group_prepare_time_us,
            image_bind_group_time_us,
            analytic_path_bind_group_time_us,
            analytic_path_bind_group_miss_count,
            analytic_path_bind_group_upload_bytes,
            text_atlas_bind_group_time_us,
            text_atlas_upload_copy_time_us,
            text_atlas_upload_write_time_us,
            text_atlas_upload_bytes,
            batch_prepare_time_us,
            gpu_upload_time_us,
            pass_encode_time_us,
            queue_submit_time_us,
            surface_present_time_us,
        }
    }

    pub fn with_retained_packet_breakdown(
        mut self,
        retained_packet_normalize_time_us: u64,
        retained_packet_signature_time_us: u64,
        retained_packet_raster_state_init_time_us: u64,
        retained_packet_scene_build_time_us: u64,
        retained_packet_command_count: usize,
        retained_packet_text_command_count: usize,
        retained_packet_path_command_count: usize,
        retained_packet_clip_path_command_count: usize,
        retained_packet_image_command_count: usize,
        retained_packet_rect_command_count: usize,
        retained_packet_text_command_time_us: u64,
        retained_packet_path_command_time_us: u64,
        retained_packet_clip_path_command_time_us: u64,
        retained_packet_image_command_time_us: u64,
        retained_packet_rect_command_time_us: u64,
    ) -> Self {
        self.retained_packet_normalize_time_us = retained_packet_normalize_time_us;
        self.retained_packet_signature_time_us = retained_packet_signature_time_us;
        self.retained_packet_raster_state_init_time_us = retained_packet_raster_state_init_time_us;
        self.retained_packet_scene_build_time_us = retained_packet_scene_build_time_us;
        self.retained_packet_command_count = retained_packet_command_count;
        self.retained_packet_text_command_count = retained_packet_text_command_count;
        self.retained_packet_path_command_count = retained_packet_path_command_count;
        self.retained_packet_clip_path_command_count = retained_packet_clip_path_command_count;
        self.retained_packet_image_command_count = retained_packet_image_command_count;
        self.retained_packet_rect_command_count = retained_packet_rect_command_count;
        self.retained_packet_text_command_time_us = retained_packet_text_command_time_us;
        self.retained_packet_path_command_time_us = retained_packet_path_command_time_us;
        self.retained_packet_clip_path_command_time_us = retained_packet_clip_path_command_time_us;
        self.retained_packet_image_command_time_us = retained_packet_image_command_time_us;
        self.retained_packet_rect_command_time_us = retained_packet_rect_command_time_us;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PresentationLatencyDiagnostics {
    pub event_to_render_start_ms: f64,
    pub event_to_present_ms: f64,
    pub redraw_request_to_callback_ms: f64,
}

impl PresentationLatencyDiagnostics {
    pub const fn new(
        event_to_render_start_ms: f64,
        event_to_present_ms: f64,
        redraw_request_to_callback_ms: f64,
    ) -> Self {
        Self {
            event_to_render_start_ms,
            event_to_present_ms,
            redraw_request_to_callback_ms,
        }
    }
}

impl RenderDiagnostics {
    pub fn push(&mut self, phase: FramePhase, duration: Duration) {
        self.phase_timings
            .push(FramePhaseSample::from_duration(phase, duration));
    }

    pub fn total_time_ms(&self) -> f64 {
        self.phase_timings
            .iter()
            .map(|sample| sample.duration_ms)
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheMetrics {
    pub entries: usize,
    pub hits: usize,
    pub misses: usize,
}

impl CacheMetrics {
    pub const fn new(entries: usize, hits: usize, misses: usize) -> Self {
        Self {
            entries,
            hits,
            misses,
        }
    }

    pub const fn requests(self) -> usize {
        self.hits + self.misses
    }

    pub fn hit_rate(self) -> f64 {
        let requests = self.requests();
        if requests == 0 {
            0.0
        } else {
            self.hits as f64 / requests as f64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheMetricsDelta {
    pub entries_delta: isize,
    pub hits: usize,
    pub misses: usize,
}

impl CacheMetricsDelta {
    pub fn from_counters(current: CacheMetrics, previous: CacheMetrics) -> Self {
        Self {
            entries_delta: current.entries as isize - previous.entries as isize,
            hits: current.hits.saturating_sub(previous.hits),
            misses: current.misses.saturating_sub(previous.misses),
        }
    }

    pub const fn requests(self) -> usize {
        self.hits + self.misses
    }

    pub fn hit_rate(self) -> f64 {
        let requests = self.requests();
        if requests == 0 {
            0.0
        } else {
            self.hits as f64 / requests as f64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextCacheDiagnostics {
    pub runtime_layout: CacheMetrics,
    pub renderer_layout: CacheMetrics,
    pub renderer_glyph: CacheMetrics,
    pub renderer_path: CacheMetrics,
}

impl TextCacheDiagnostics {
    pub fn delta_from(&self, previous: &Self) -> TextCacheDeltaDiagnostics {
        TextCacheDeltaDiagnostics {
            runtime_layout: CacheMetricsDelta::from_counters(
                self.runtime_layout,
                previous.runtime_layout,
            ),
            renderer_layout: CacheMetricsDelta::from_counters(
                self.renderer_layout,
                previous.renderer_layout,
            ),
            renderer_glyph: CacheMetricsDelta::from_counters(
                self.renderer_glyph,
                previous.renderer_glyph,
            ),
            renderer_path: CacheMetricsDelta::from_counters(
                self.renderer_path,
                previous.renderer_path,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextCacheDeltaDiagnostics {
    pub runtime_layout: CacheMetricsDelta,
    pub renderer_layout: CacheMetricsDelta,
    pub renderer_glyph: CacheMetricsDelta,
    pub renderer_path: CacheMetricsDelta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneStatisticsDetailMode {
    #[default]
    Lightweight,
    Detailed,
}

impl SceneStatisticsDetailMode {
    pub const fn is_detailed(self) -> bool {
        matches!(self, Self::Detailed)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Lightweight => "lightweight",
            Self::Detailed => "detailed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowTextRenderPolicy {
    AutomaticByTextLuminance,
    Linear,
    Gamma(f32),
    TwoCoverageMinusCoverageSq,
}

impl Default for WindowTextRenderPolicy {
    fn default() -> Self {
        Self::AutomaticByTextLuminance
    }
}

impl WindowTextRenderPolicy {
    pub fn normalized(self) -> Self {
        match self {
            Self::AutomaticByTextLuminance => Self::AutomaticByTextLuminance,
            Self::Linear => Self::Linear,
            Self::Gamma(gamma) if gamma.is_finite() && gamma > 0.0 => Self::Gamma(gamma),
            Self::Gamma(_) => Self::Linear,
            Self::TwoCoverageMinusCoverageSq => Self::TwoCoverageMinusCoverageSq,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowTextHinting {
    None,
    Slight { max_ppem: f32 },
}

impl Default for WindowTextHinting {
    fn default() -> Self {
        Self::None
    }
}

impl WindowTextHinting {
    pub fn normalized(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Slight { max_ppem } if max_ppem.is_finite() && max_ppem > 0.0 => {
                Self::Slight { max_ppem }
            }
            Self::Slight { .. } => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowStemDarkening {
    None,
    Enabled { max_ppem: f32, amount: f32 },
}

impl Default for WindowStemDarkening {
    fn default() -> Self {
        Self::None
    }
}

impl WindowStemDarkening {
    pub fn normalized(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Enabled { max_ppem, amount }
                if max_ppem.is_finite() && max_ppem > 0.0 && amount.is_finite() && amount > 0.0 =>
            {
                Self::Enabled {
                    max_ppem,
                    amount: amount.clamp(0.0, 1.0),
                }
            }
            Self::Enabled { .. } => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowOutputColorPrimaries {
    #[default]
    Automatic,
    Srgb,
    DisplayP3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowDynamicRangeMode {
    #[default]
    Automatic,
    StandardDynamicRange,
    HighDynamicRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowToneMappingMode {
    #[default]
    Automatic,
    Clamp,
    Reinhard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowColorManagementMode {
    #[default]
    Automatic,
    ForceSdr,
    PreferWideGamut,
    PreferHdr,
}

pub const DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS: f32 = 203.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowRenderOptions {
    pub feathering_enabled: bool,
    pub feather_width: f32,
    pub optical_vertical_text_alignment_enabled: bool,
    pub glyph_pixel_alignment_enabled: bool,
    pub text_render_policy: WindowTextRenderPolicy,
    pub text_hinting: WindowTextHinting,
    pub stem_darkening: WindowStemDarkening,
    pub output_color_primaries: WindowOutputColorPrimaries,
    pub dynamic_range_mode: WindowDynamicRangeMode,
    pub tone_mapping_mode: WindowToneMappingMode,
    pub color_management_mode: WindowColorManagementMode,
    pub sdr_content_brightness_nits: f32,
}

impl WindowRenderOptions {
    pub const fn new(feathering_enabled: bool, feather_width: f32) -> Self {
        Self {
            feathering_enabled,
            feather_width,
            optical_vertical_text_alignment_enabled: true,
            glyph_pixel_alignment_enabled: true,
            text_render_policy: WindowTextRenderPolicy::AutomaticByTextLuminance,
            text_hinting: WindowTextHinting::None,
            stem_darkening: WindowStemDarkening::None,
            output_color_primaries: WindowOutputColorPrimaries::Automatic,
            dynamic_range_mode: WindowDynamicRangeMode::Automatic,
            tone_mapping_mode: WindowToneMappingMode::Automatic,
            color_management_mode: WindowColorManagementMode::Automatic,
            sdr_content_brightness_nits: DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS,
        }
    }

    pub const fn with_optical_vertical_text_alignment_enabled(mut self, enabled: bool) -> Self {
        self.optical_vertical_text_alignment_enabled = enabled;
        self
    }

    pub const fn with_glyph_pixel_alignment_enabled(mut self, enabled: bool) -> Self {
        self.glyph_pixel_alignment_enabled = enabled;
        self
    }

    pub const fn with_text_render_policy(mut self, policy: WindowTextRenderPolicy) -> Self {
        self.text_render_policy = policy;
        self
    }

    pub const fn with_text_hinting(mut self, hinting: WindowTextHinting) -> Self {
        self.text_hinting = hinting;
        self
    }

    pub const fn with_stem_darkening(mut self, darkening: WindowStemDarkening) -> Self {
        self.stem_darkening = darkening;
        self
    }

    pub const fn with_output_color_primaries(
        mut self,
        primaries: WindowOutputColorPrimaries,
    ) -> Self {
        self.output_color_primaries = primaries;
        self
    }

    pub const fn with_dynamic_range_mode(mut self, mode: WindowDynamicRangeMode) -> Self {
        self.dynamic_range_mode = mode;
        self
    }

    pub const fn with_tone_mapping_mode(mut self, mode: WindowToneMappingMode) -> Self {
        self.tone_mapping_mode = mode;
        self
    }

    pub const fn with_color_management_mode(mut self, mode: WindowColorManagementMode) -> Self {
        self.color_management_mode = mode;
        self
    }

    pub const fn with_sdr_content_brightness_nits(mut self, brightness_nits: f32) -> Self {
        self.sdr_content_brightness_nits = brightness_nits;
        self
    }

    pub fn clamped(self) -> Self {
        Self {
            feathering_enabled: self.feathering_enabled,
            feather_width: self.feather_width.max(0.0),
            optical_vertical_text_alignment_enabled: self.optical_vertical_text_alignment_enabled,
            glyph_pixel_alignment_enabled: self.glyph_pixel_alignment_enabled,
            text_render_policy: self.text_render_policy.normalized(),
            text_hinting: self.text_hinting.normalized(),
            stem_darkening: self.stem_darkening.normalized(),
            output_color_primaries: self.output_color_primaries,
            dynamic_range_mode: self.dynamic_range_mode,
            tone_mapping_mode: self.tone_mapping_mode,
            color_management_mode: self.color_management_mode,
            sdr_content_brightness_nits: if self.sdr_content_brightness_nits.is_finite()
                && self.sdr_content_brightness_nits > 0.0
            {
                self.sdr_content_brightness_nits
            } else {
                DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowPerformanceSummary {
    pub window_id: WindowId,
    pub frame_index: u64,
    pub total_time_ms: f64,
    pub slowest_phase: Option<FramePhaseSample>,
    pub presentation_latency: PresentationLatencyDiagnostics,
    pub renderer_submission: RendererSubmissionDiagnostics,
    pub text_caches: TextCacheDiagnostics,
    pub total_widget_count: usize,
    pub active_animated_widget_count: usize,
    pub animation_frame_wake_count: usize,
    pub animation_repaint_frame_count: usize,
    pub animation_transform_effect_only_frame_count: usize,
    pub repaint_boundary_count: usize,
    pub scene_layer_count: usize,
    pub stack_surface_count: usize,
    pub overlay_layer_count: usize,
    pub dirty_region_count: usize,
    pub dirty_coverage: f32,
    pub command_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneStatistics {
    pub detail_mode: SceneStatisticsDetailMode,
    pub viewport: Size,
    pub total_widget_count: usize,
    pub active_animated_widget_count: usize,
    pub animation_frame_wake_count: usize,
    pub animation_repaint_frame_count: usize,
    pub animation_transform_effect_only_frame_count: usize,
    pub dirty_region_count: usize,
    pub dirty_regions: Vec<DirtyRegion>,
    pub dirty_area: f32,
    pub dirty_coverage: f32,
    pub command_count: usize,
    pub command_breakdown: Vec<(String, usize)>,
    /// Current repaint granularity as observed by the runtime today.
    ///
    /// During the layer-boundary transition this still tracks emitted scene
    /// layers, because the runtime has not yet decoupled explicit repaint
    /// boundaries from per-widget `SceneLayer` emission.
    pub repaint_boundary_count: usize,
    pub scene_layer_count: usize,
    pub stack_surface_count: usize,
    pub overlay_layer_count: usize,
    pub layer_update_count: usize,
    pub layer_update_breakdown: Vec<(String, usize)>,
    pub text_command_count: usize,
    pub image_command_count: usize,
    pub clip_command_count: usize,
    pub transform_command_count: usize,
}

impl SceneStatistics {
    pub fn with_animation_counters(
        mut self,
        active_animated_widget_count: usize,
        animation_frame_wake_count: usize,
        animation_repaint_frame_count: usize,
        animation_transform_effect_only_frame_count: usize,
    ) -> Self {
        self.active_animated_widget_count = active_animated_widget_count;
        self.animation_frame_wake_count = animation_frame_wake_count;
        self.animation_repaint_frame_count = animation_repaint_frame_count;
        self.animation_transform_effect_only_frame_count =
            animation_transform_effect_only_frame_count;
        self
    }

    pub fn from_frame(frame: &SceneFrame, total_widget_count: usize) -> Self {
        Self::from_frame_with_mode(
            frame,
            total_widget_count,
            SceneStatisticsDetailMode::Lightweight,
        )
    }

    pub fn minimal(
        frame: &SceneFrame,
        total_widget_count: usize,
        detail_mode: SceneStatisticsDetailMode,
    ) -> Self {
        let layer_totals = scene_layer_totals(frame);
        Self {
            detail_mode,
            viewport: frame.viewport,
            total_widget_count,
            active_animated_widget_count: 0,
            animation_frame_wake_count: 0,
            animation_repaint_frame_count: 0,
            animation_transform_effect_only_frame_count: 0,
            dirty_region_count: 0,
            dirty_regions: Vec::new(),
            dirty_area: 0.0,
            dirty_coverage: 0.0,
            command_count: 0,
            command_breakdown: Vec::new(),
            repaint_boundary_count: layer_totals.repaint_boundary_count,
            scene_layer_count: layer_totals.scene_layer_count,
            stack_surface_count: layer_totals.stack_surface_count,
            overlay_layer_count: layer_totals.overlay_layer_count,
            layer_update_count: 0,
            layer_update_breakdown: Vec::new(),
            text_command_count: 0,
            image_command_count: 0,
            clip_command_count: 0,
            transform_command_count: 0,
        }
    }

    pub fn from_frame_with_mode(
        frame: &SceneFrame,
        total_widget_count: usize,
        detail_mode: SceneStatisticsDetailMode,
    ) -> Self {
        let detailed = detail_mode.is_detailed();
        let mut command_breakdown = detailed.then(BTreeMap::<String, usize>::new);
        let mut text_command_count = 0usize;
        let mut image_command_count = 0usize;
        let mut clip_command_count = 0usize;
        let mut transform_command_count = 0usize;
        let mut command_count = 0usize;

        frame.scene.visit_commands(&mut |command| {
            command_count += 1;
            if let Some(breakdown) = &mut command_breakdown {
                *breakdown
                    .entry(command_kind(command).to_string())
                    .or_default() += 1;
            }

            match command {
                SceneCommand::DrawText(_)
                | SceneCommand::DrawShapedText(_)
                | SceneCommand::DrawShapedTextWindow(_)
                | SceneCommand::Label { .. } => {
                    text_command_count += 1;
                }
                SceneCommand::DrawImage { .. } => {
                    image_command_count += 1;
                }
                SceneCommand::PushClip { .. }
                | SceneCommand::PushClipPath { .. }
                | SceneCommand::PopClip => {
                    clip_command_count += 1;
                }
                SceneCommand::PushTransform { .. } | SceneCommand::PopTransform => {
                    transform_command_count += 1;
                }
                SceneCommand::Clear(_)
                | SceneCommand::FillRect { .. }
                | SceneCommand::StrokeRect { .. }
                | SceneCommand::FillPath { .. }
                | SceneCommand::StrokePath { .. }
                | SceneCommand::Layer(_) => {}
            }
        });

        let mut layer_update_breakdown = detailed.then(BTreeMap::<String, usize>::new);
        if let Some(breakdown) = &mut layer_update_breakdown {
            for update in &frame.layer_updates {
                *breakdown
                    .entry(layer_update_kind(update.kind).to_string())
                    .or_default() += 1;
            }
        }

        let dirty_area: f32 = frame
            .dirty_regions
            .iter()
            .map(|region| region.area.width().max(0.0) * region.area.height().max(0.0))
            .sum();
        let viewport_area = frame.viewport.width.max(0.0) * frame.viewport.height.max(0.0);
        let dirty_coverage = if viewport_area > 0.0 {
            ((dirty_area / viewport_area) * 100.0).min(100.0)
        } else {
            0.0
        };
        let dirty_region_count = frame.dirty_regions.len();
        let layer_totals = scene_layer_totals(frame);

        Self {
            detail_mode,
            viewport: frame.viewport,
            total_widget_count,
            active_animated_widget_count: 0,
            animation_frame_wake_count: 0,
            animation_repaint_frame_count: 0,
            animation_transform_effect_only_frame_count: 0,
            dirty_region_count,
            dirty_regions: if detailed {
                frame.dirty_regions.clone()
            } else {
                Vec::new()
            },
            dirty_area,
            dirty_coverage,
            command_count,
            command_breakdown: command_breakdown
                .map(|breakdown| breakdown.into_iter().collect())
                .unwrap_or_default(),
            repaint_boundary_count: layer_totals.repaint_boundary_count,
            scene_layer_count: layer_totals.scene_layer_count,
            stack_surface_count: layer_totals.stack_surface_count,
            overlay_layer_count: layer_totals.overlay_layer_count,
            layer_update_count: frame.layer_updates.len(),
            layer_update_breakdown: layer_update_breakdown
                .map(|breakdown| breakdown.into_iter().collect())
                .unwrap_or_default(),
            text_command_count,
            image_command_count,
            clip_command_count,
            transform_command_count,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct SceneLayerTotals {
    repaint_boundary_count: usize,
    scene_layer_count: usize,
    stack_surface_count: usize,
    overlay_layer_count: usize,
}

fn scene_layer_totals(frame: &SceneFrame) -> SceneLayerTotals {
    let mut totals = SceneLayerTotals::default();
    frame.scene.visit_layers(&mut |layer| {
        totals.scene_layer_count += 1;
        if layer.descriptor.is_stack_surface {
            totals.stack_surface_count += 1;
        }
        if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
            totals.overlay_layer_count += 1;
        }
    });
    // Current runtime behavior still uses emitted scene layers as repaint
    // granularity. Keep the two metrics separate in the API so later slices can
    // decouple them without another diagnostics rename, but document the
    // equality clearly for this transitional phase.
    totals.repaint_boundary_count = totals.scene_layer_count;
    totals
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowPerformanceSnapshot {
    pub window_id: WindowId,
    pub frame_index: u64,
    pub total_time_ms: f64,
    pub phase_timings: Vec<FramePhaseSample>,
    pub presentation_latency: PresentationLatencyDiagnostics,
    pub renderer_submission: RendererSubmissionDiagnostics,
    pub text_caches: TextCacheDiagnostics,
    pub text_cache_deltas: TextCacheDeltaDiagnostics,
    pub runtime_text_timing: RuntimeTextTimingDiagnostics,
    pub widget_timings: Vec<WidgetTimingSample>,
    pub retained_packet_hotspot: Option<RetainedPacketHotspotDiagnostics>,
    pub scene: SceneStatistics,
}

impl WindowPerformanceSnapshot {
    pub fn new(
        window_id: WindowId,
        frame_index: u64,
        phase_timings: Vec<FramePhaseSample>,
        renderer_submission: RendererSubmissionDiagnostics,
        text_caches: TextCacheDiagnostics,
        text_cache_deltas: TextCacheDeltaDiagnostics,
        scene: SceneStatistics,
    ) -> Self {
        let total_time_ms = phase_timings.iter().map(|sample| sample.duration_ms).sum();

        Self::with_total_time_ms(
            window_id,
            frame_index,
            total_time_ms,
            phase_timings,
            renderer_submission,
            text_caches,
            text_cache_deltas,
            scene,
        )
    }

    pub fn with_total_time_ms(
        window_id: WindowId,
        frame_index: u64,
        total_time_ms: f64,
        phase_timings: Vec<FramePhaseSample>,
        renderer_submission: RendererSubmissionDiagnostics,
        text_caches: TextCacheDiagnostics,
        text_cache_deltas: TextCacheDeltaDiagnostics,
        scene: SceneStatistics,
    ) -> Self {
        Self {
            window_id,
            frame_index,
            total_time_ms,
            phase_timings,
            presentation_latency: PresentationLatencyDiagnostics::default(),
            renderer_submission,
            text_caches,
            text_cache_deltas,
            runtime_text_timing: RuntimeTextTimingDiagnostics::default(),
            widget_timings: Vec::new(),
            retained_packet_hotspot: None,
            scene,
        }
    }

    pub fn with_presentation_latency(
        mut self,
        presentation_latency: PresentationLatencyDiagnostics,
    ) -> Self {
        self.presentation_latency = presentation_latency;
        self
    }

    pub fn with_widget_timings(mut self, widget_timings: Vec<WidgetTimingSample>) -> Self {
        self.widget_timings = widget_timings;
        self
    }

    pub fn with_runtime_text_timing(
        mut self,
        runtime_text_timing: RuntimeTextTimingDiagnostics,
    ) -> Self {
        self.runtime_text_timing = runtime_text_timing;
        self
    }

    pub fn with_retained_packet_hotspot(
        mut self,
        retained_packet_hotspot: Option<RetainedPacketHotspotDiagnostics>,
    ) -> Self {
        self.retained_packet_hotspot = retained_packet_hotspot;
        self
    }

    pub fn slowest_phase(&self) -> Option<FramePhaseSample> {
        self.phase_timings
            .iter()
            .copied()
            .max_by(|left, right| left.duration_ms.total_cmp(&right.duration_ms))
    }

    pub fn summary(&self) -> WindowPerformanceSummary {
        WindowPerformanceSummary {
            window_id: self.window_id,
            frame_index: self.frame_index,
            total_time_ms: self.total_time_ms,
            slowest_phase: self.slowest_phase(),
            presentation_latency: self.presentation_latency,
            renderer_submission: self.renderer_submission,
            text_caches: self.text_caches,
            total_widget_count: self.scene.total_widget_count,
            active_animated_widget_count: self.scene.active_animated_widget_count,
            animation_frame_wake_count: self.scene.animation_frame_wake_count,
            animation_repaint_frame_count: self.scene.animation_repaint_frame_count,
            animation_transform_effect_only_frame_count: self
                .scene
                .animation_transform_effect_only_frame_count,
            repaint_boundary_count: self.scene.repaint_boundary_count,
            scene_layer_count: self.scene.scene_layer_count,
            stack_surface_count: self.scene.stack_surface_count,
            overlay_layer_count: self.scene.overlay_layer_count,
            dirty_region_count: self.scene.dirty_region_count,
            dirty_coverage: self.scene.dirty_coverage,
            command_count: self.scene.command_count,
        }
    }
}

static WINDOW_PERFORMANCE_SNAPSHOTS: OnceLock<
    RwLock<HashMap<WindowId, WindowPerformanceSnapshot>>,
> = OnceLock::new();
static WINDOW_SCENE_STATISTICS_DETAIL_MODES: OnceLock<
    RwLock<HashMap<WindowId, SceneStatisticsDetailMode>>,
> = OnceLock::new();
static WINDOW_RENDER_OPTIONS: OnceLock<RwLock<HashMap<WindowId, WindowRenderOptions>>> =
    OnceLock::new();

pub fn publish_window_performance_snapshot(snapshot: WindowPerformanceSnapshot) {
    let mut store = window_performance_store()
        .write()
        .expect("window performance snapshot store lock should not be poisoned");
    store.insert(snapshot.window_id, snapshot);
}

pub fn window_performance_snapshot(window_id: WindowId) -> Option<WindowPerformanceSnapshot> {
    let store = window_performance_store()
        .read()
        .expect("window performance snapshot store lock should not be poisoned");
    store.get(&window_id).cloned()
}

pub fn window_performance_summary(window_id: WindowId) -> Option<WindowPerformanceSummary> {
    let store = window_performance_store()
        .read()
        .expect("window performance snapshot store lock should not be poisoned");
    store
        .get(&window_id)
        .map(WindowPerformanceSnapshot::summary)
}

pub fn window_performance_text_caches(window_id: WindowId) -> Option<TextCacheDiagnostics> {
    let store = window_performance_store()
        .read()
        .expect("window performance snapshot store lock should not be poisoned");
    store.get(&window_id).map(|snapshot| snapshot.text_caches)
}

pub fn set_window_scene_statistics_detail_mode(
    window_id: WindowId,
    detail_mode: SceneStatisticsDetailMode,
) {
    let mut store = window_scene_statistics_detail_mode_store()
        .write()
        .expect("scene statistics detail mode store lock should not be poisoned");
    if detail_mode == SceneStatisticsDetailMode::Lightweight {
        store.remove(&window_id);
    } else {
        store.insert(window_id, detail_mode);
    }
}

pub fn window_scene_statistics_detail_mode(window_id: WindowId) -> SceneStatisticsDetailMode {
    let store = window_scene_statistics_detail_mode_store()
        .read()
        .expect("scene statistics detail mode store lock should not be poisoned");
    store.get(&window_id).copied().unwrap_or_default()
}

pub fn set_window_render_options(window_id: WindowId, options: WindowRenderOptions) {
    let mut store = window_render_options_store()
        .write()
        .expect("window render options store lock should not be poisoned");
    store.insert(window_id, options.clamped());
}

pub fn window_render_options(window_id: WindowId) -> Option<WindowRenderOptions> {
    let store = window_render_options_store()
        .read()
        .expect("window render options store lock should not be poisoned");
    store.get(&window_id).copied()
}

pub fn clear_window_render_options(window_id: WindowId) {
    let mut store = window_render_options_store()
        .write()
        .expect("window render options store lock should not be poisoned");
    store.remove(&window_id);
}

pub fn clear_window_performance_snapshot(window_id: WindowId) {
    let mut store = window_performance_store()
        .write()
        .expect("window performance snapshot store lock should not be poisoned");
    store.remove(&window_id);

    let mut detail_modes = window_scene_statistics_detail_mode_store()
        .write()
        .expect("scene statistics detail mode store lock should not be poisoned");
    detail_modes.remove(&window_id);

    clear_window_render_options(window_id);
}

pub fn clear_window_performance_snapshots() {
    let mut store = window_performance_store()
        .write()
        .expect("window performance snapshot store lock should not be poisoned");
    store.clear();

    let mut detail_modes = window_scene_statistics_detail_mode_store()
        .write()
        .expect("scene statistics detail mode store lock should not be poisoned");
    detail_modes.clear();

    let mut render_options = window_render_options_store()
        .write()
        .expect("window render options store lock should not be poisoned");
    render_options.clear();
}

fn window_performance_store() -> &'static RwLock<HashMap<WindowId, WindowPerformanceSnapshot>> {
    WINDOW_PERFORMANCE_SNAPSHOTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn window_scene_statistics_detail_mode_store()
-> &'static RwLock<HashMap<WindowId, SceneStatisticsDetailMode>> {
    WINDOW_SCENE_STATISTICS_DETAIL_MODES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn window_render_options_store() -> &'static RwLock<HashMap<WindowId, WindowRenderOptions>> {
    WINDOW_RENDER_OPTIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn command_kind(command: &SceneCommand) -> &'static str {
    match command {
        SceneCommand::Clear(_) => "Clear",
        SceneCommand::FillRect { .. } => "FillRect",
        SceneCommand::StrokeRect { .. } => "StrokeRect",
        SceneCommand::FillPath { .. } => "FillPath",
        SceneCommand::StrokePath { .. } => "StrokePath",
        SceneCommand::DrawText(_) => "DrawText",
        SceneCommand::DrawShapedText(_) => "DrawShapedText",
        SceneCommand::DrawShapedTextWindow(_) => "DrawShapedTextWindow",
        SceneCommand::DrawImage { .. } => "DrawImage",
        SceneCommand::PushClip { .. } => "PushClip",
        SceneCommand::PushClipPath { .. } => "PushClipPath",
        SceneCommand::PopClip => "PopClip",
        SceneCommand::PushTransform { .. } => "PushTransform",
        SceneCommand::PopTransform => "PopTransform",
        SceneCommand::Layer(_) => "Layer",
        SceneCommand::Label { .. } => "Label",
    }
}

fn layer_update_kind(kind: SceneLayerUpdateKind) -> &'static str {
    match kind {
        SceneLayerUpdateKind::Ordering => "Ordering",
        SceneLayerUpdateKind::Content => "Content",
        SceneLayerUpdateKind::Transform => "Transform",
        SceneLayerUpdateKind::Clip => "Clip",
        SceneLayerUpdateKind::Effect => "Effect",
        SceneLayerUpdateKind::Visibility => "Visibility",
        SceneLayerUpdateKind::Resources => "Resources",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CacheMetrics, FramePhase, FramePhaseSample, RendererSubmissionDiagnostics,
        RetainedPacketRebuildDiagnostics, SceneStatistics, SceneStatisticsDetailMode,
        TextCacheDeltaDiagnostics, TextCacheDiagnostics, WindowColorManagementMode,
        WindowDynamicRangeMode, WindowOutputColorPrimaries, WindowPerformanceSnapshot,
        WindowRenderOptions, WindowTextRenderPolicy, WindowToneMappingMode,
        clear_window_performance_snapshot, set_window_render_options,
        set_window_scene_statistics_detail_mode, window_render_options,
        window_scene_statistics_detail_mode,
    };
    use sui_core::{Color, DirtyRegion, InvalidationKind, Rect, Size, WidgetId, WindowId};
    use sui_scene::{
        LayerCompositionMode, Scene, SceneCommand, SceneFrame, SceneLayer, SceneLayerDescriptor,
    };

    #[test]
    fn text_cache_deltas_are_derived_from_prior_counters() {
        let previous = TextCacheDiagnostics {
            runtime_layout: CacheMetrics::new(2, 10, 4),
            renderer_layout: CacheMetrics::new(3, 6, 2),
            renderer_glyph: CacheMetrics::new(5, 12, 3),
            renderer_path: CacheMetrics::new(7, 14, 5),
        };
        let current = TextCacheDiagnostics {
            runtime_layout: CacheMetrics::new(4, 15, 6),
            renderer_layout: CacheMetrics::new(3, 9, 3),
            renderer_glyph: CacheMetrics::new(7, 18, 5),
            renderer_path: CacheMetrics::new(8, 21, 6),
        };

        let delta = current.delta_from(&previous);

        assert_eq!(delta.runtime_layout.entries_delta, 2);
        assert_eq!(delta.runtime_layout.hits, 5);
        assert_eq!(delta.runtime_layout.misses, 2);
        assert_eq!(delta.renderer_layout.entries_delta, 0);
        assert_eq!(delta.renderer_layout.hits, 3);
        assert_eq!(delta.renderer_layout.misses, 1);
        assert_eq!(delta.renderer_glyph.entries_delta, 2);
        assert_eq!(delta.renderer_glyph.hits, 6);
        assert_eq!(delta.renderer_glyph.misses, 2);
        assert_eq!(delta.renderer_path.entries_delta, 1);
        assert_eq!(delta.renderer_path.hits, 7);
        assert_eq!(delta.renderer_path.misses, 1);
    }

    #[test]
    fn scene_statistics_diagnostics_minimal_preserves_split_count_totals() {
        let mut frame = SceneFrame::new(WindowId::new(8), Size::new(320.0, 180.0));
        frame
            .scene
            .push(SceneCommand::Layer(SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    WidgetId::new(10).into(),
                    WidgetId::new(10),
                    Rect::new(0.0, 0.0, 120.0, 80.0),
                )
                .with_is_stack_surface(true),
                Scene::new(),
            )));
        frame
            .scene
            .push(SceneCommand::Layer(SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    WidgetId::new(11).into(),
                    WidgetId::new(11),
                    Rect::new(12.0, 10.0, 80.0, 48.0),
                )
                .with_composition_mode(LayerCompositionMode::Overlay),
                Scene::new(),
            )));

        let stats = SceneStatistics::minimal(&frame, 5, SceneStatisticsDetailMode::Lightweight);

        assert_eq!(stats.total_widget_count, 5);
        assert_eq!(stats.repaint_boundary_count, 2);
        assert_eq!(stats.scene_layer_count, 2);
        assert_eq!(stats.stack_surface_count, 1);
        assert_eq!(stats.overlay_layer_count, 1);
        assert!(stats.command_breakdown.is_empty());
        assert!(stats.layer_update_breakdown.is_empty());
    }

    #[test]
    fn window_performance_snapshot_preserves_renderer_submission_stats() {
        let snapshot = WindowPerformanceSnapshot::new(
            WindowId::new(5),
            17,
            vec![FramePhaseSample::new(FramePhase::Renderer, 2.5)],
            RendererSubmissionDiagnostics::new(
                3,
                9,
                4096,
                128,
                3584,
                4,
                7,
                230,
                90,
                310,
                120,
                2,
                RetainedPacketRebuildDiagnostics::new(1, 0, 1, 1, 0),
                5,
                80,
                440,
                210,
                130,
                15,
                95,
                4,
                32768,
                115,
                85,
                22,
                16384,
                920,
                640,
                180,
                70,
                560,
            ),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: SceneStatisticsDetailMode::Lightweight,
                viewport: Size::new(640.0, 360.0),
                total_widget_count: 9,
                active_animated_widget_count: 2,
                animation_frame_wake_count: 1,
                animation_repaint_frame_count: 1,
                animation_transform_effect_only_frame_count: 0,
                dirty_region_count: 0,
                dirty_regions: Vec::new(),
                dirty_area: 0.0,
                dirty_coverage: 0.0,
                command_count: 0,
                command_breakdown: Vec::new(),
                repaint_boundary_count: 4,
                scene_layer_count: 4,
                stack_surface_count: 1,
                overlay_layer_count: 1,
                layer_update_count: 0,
                layer_update_breakdown: Vec::new(),
                text_command_count: 0,
                image_command_count: 0,
                clip_command_count: 0,
                transform_command_count: 0,
            },
        );

        assert_eq!(snapshot.renderer_submission.pass_count, 3);
        assert_eq!(snapshot.renderer_submission.draw_count, 9);
        assert_eq!(snapshot.renderer_submission.uploaded_vertex_bytes, 4096);
        assert_eq!(snapshot.renderer_submission.direct_packet_count, 7);
        assert_eq!(
            snapshot.renderer_submission.retained_state_update_time_us,
            230
        );
        assert_eq!(
            snapshot
                .renderer_submission
                .retained_scene_traversal_time_us,
            310
        );
        assert_eq!(
            snapshot.renderer_submission.retained_packet_build_time_us,
            120
        );
        assert_eq!(snapshot.renderer_submission.retained_packet_build_count, 2);
        assert_eq!(
            snapshot
                .renderer_submission
                .retained_packet_rebuilds
                .total_count(),
            3
        );
        assert_eq!(
            snapshot
                .renderer_submission
                .retained_packet_rebuilds
                .new_count,
            1
        );
        assert_eq!(
            snapshot
                .renderer_submission
                .retained_packet_rebuilds
                .signature_count,
            1
        );
        assert_eq!(
            snapshot
                .renderer_submission
                .retained_packet_rebuilds
                .scene_count,
            1
        );
        assert_eq!(snapshot.renderer_submission.text_atlas_miss_count, 5);
        assert_eq!(snapshot.renderer_submission.text_atlas_miss_time_us, 80);
        assert_eq!(snapshot.renderer_submission.surface_acquire_time_us, 440);
        assert_eq!(
            snapshot.renderer_submission.resource_collection_time_us,
            210
        );
        assert_eq!(snapshot.renderer_submission.bind_group_prepare_time_us, 130);
        assert_eq!(snapshot.renderer_submission.image_bind_group_time_us, 15);
        assert_eq!(
            snapshot
                .renderer_submission
                .analytic_path_bind_group_time_us,
            95
        );
        assert_eq!(
            snapshot
                .renderer_submission
                .analytic_path_bind_group_miss_count,
            4
        );
        assert_eq!(
            snapshot
                .renderer_submission
                .analytic_path_bind_group_upload_bytes,
            32768
        );
        assert_eq!(
            snapshot.renderer_submission.text_atlas_bind_group_time_us,
            115
        );
        assert_eq!(
            snapshot.renderer_submission.text_atlas_upload_copy_time_us,
            85
        );
        assert_eq!(
            snapshot.renderer_submission.text_atlas_upload_write_time_us,
            22
        );
        assert_eq!(snapshot.renderer_submission.text_atlas_upload_bytes, 16384);
        assert_eq!(snapshot.renderer_submission.batch_prepare_time_us, 920);
        assert_eq!(snapshot.renderer_submission.gpu_upload_time_us, 640);
        assert_eq!(snapshot.renderer_submission.pass_encode_time_us, 180);
        assert_eq!(snapshot.renderer_submission.queue_submit_time_us, 70);
        assert_eq!(snapshot.renderer_submission.surface_present_time_us, 560);
        assert_eq!(snapshot.total_time_ms, 2.5);

        let summary = snapshot.summary();
        assert_eq!(summary.total_widget_count, 9);
        assert_eq!(summary.active_animated_widget_count, 2);
        assert_eq!(summary.animation_frame_wake_count, 1);
        assert_eq!(summary.animation_repaint_frame_count, 1);
        assert_eq!(summary.animation_transform_effect_only_frame_count, 0);
        assert_eq!(summary.repaint_boundary_count, 4);
        assert_eq!(summary.scene_layer_count, 4);
        assert_eq!(summary.stack_surface_count, 1);
        assert_eq!(summary.overlay_layer_count, 1);
    }

    #[test]
    fn animation_frame_counters_distinguish_repaint_from_transform_only_frames() {
        let base_scene = SceneStatistics::minimal(
            &SceneFrame::new(WindowId::new(12), Size::new(320.0, 180.0)),
            3,
            SceneStatisticsDetailMode::Detailed,
        );

        let repaint_summary = WindowPerformanceSnapshot::new(
            WindowId::new(12),
            3,
            vec![FramePhaseSample::new(FramePhase::Paint, 0.8)],
            RendererSubmissionDiagnostics::default(),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            base_scene.clone().with_animation_counters(2, 1, 1, 0),
        )
        .summary();
        let transform_only_summary = WindowPerformanceSnapshot::new(
            WindowId::new(12),
            4,
            vec![FramePhaseSample::new(FramePhase::Renderer, 0.4)],
            RendererSubmissionDiagnostics::default(),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            base_scene.with_animation_counters(1, 1, 0, 1),
        )
        .summary();

        assert_eq!(repaint_summary.active_animated_widget_count, 2);
        assert_eq!(repaint_summary.animation_frame_wake_count, 1);
        assert_eq!(repaint_summary.animation_repaint_frame_count, 1);
        assert_eq!(
            repaint_summary.animation_transform_effect_only_frame_count,
            0
        );
        assert_eq!(transform_only_summary.active_animated_widget_count, 1);
        assert_eq!(transform_only_summary.animation_frame_wake_count, 1);
        assert_eq!(transform_only_summary.animation_repaint_frame_count, 0);
        assert_eq!(
            transform_only_summary.animation_transform_effect_only_frame_count,
            1
        );
    }

    #[test]
    fn retained_packet_rebuild_diagnostics_total_count_sums_reasons() {
        let rebuilds = RetainedPacketRebuildDiagnostics::new(2, 3, 5, 7, 11);

        assert_eq!(rebuilds.total_count(), 28);
    }

    #[test]
    fn scene_statistics_skip_expensive_breakdowns_in_lightweight_mode() {
        let mut frame = SceneFrame::new(WindowId::new(8), Size::new(320.0, 180.0));
        frame.dirty_regions.push(DirtyRegion::new(
            Rect::new(0.0, 0.0, 64.0, 48.0),
            InvalidationKind::Paint,
        ));
        frame.scene.push(SceneCommand::Clear(Color::BLACK));
        frame.scene.push(SceneCommand::Label {
            rect: Rect::new(12.0, 12.0, 96.0, 24.0),
            text: "frame".to_string(),
            color: Color::WHITE,
        });

        let lightweight = SceneStatistics::from_frame(&frame, 2);
        assert_eq!(
            lightweight.detail_mode,
            SceneStatisticsDetailMode::Lightweight
        );
        assert_eq!(lightweight.dirty_region_count, 1);
        assert_eq!(lightweight.command_count, 2);
        assert_eq!(lightweight.total_widget_count, 2);
        assert!(lightweight.dirty_regions.is_empty());
        assert!(lightweight.command_breakdown.is_empty());

        let detailed =
            SceneStatistics::from_frame_with_mode(&frame, 2, SceneStatisticsDetailMode::Detailed);
        assert_eq!(detailed.detail_mode, SceneStatisticsDetailMode::Detailed);
        assert_eq!(detailed.dirty_region_count, 1);
        assert_eq!(detailed.dirty_regions.len(), 1);
        assert!(
            detailed
                .command_breakdown
                .contains(&("Clear".to_string(), 1))
        );
        assert!(
            detailed
                .command_breakdown
                .contains(&("Label".to_string(), 1))
        );
    }

    #[test]
    fn scene_statistics_diagnostics_split_widget_boundary_and_scene_layer_counts() {
        let mut frame = SceneFrame::new(WindowId::new(18), Size::new(320.0, 180.0));
        frame.scene.push(SceneCommand::Layer(SceneLayer::new(
            WidgetId::new(20),
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Scene::new(),
        )));
        frame
            .scene
            .push(SceneCommand::Layer(SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    WidgetId::new(21).into(),
                    WidgetId::new(21),
                    Rect::new(12.0, 10.0, 80.0, 48.0),
                )
                .with_is_stack_surface(true),
                Scene::new(),
            )));
        frame
            .scene
            .push(SceneCommand::Layer(SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    WidgetId::new(22).into(),
                    WidgetId::new(22),
                    Rect::new(24.0, 20.0, 64.0, 40.0),
                )
                .with_composition_mode(LayerCompositionMode::Overlay),
                Scene::new(),
            )));

        let stats =
            SceneStatistics::from_frame_with_mode(&frame, 7, SceneStatisticsDetailMode::Detailed);

        assert_eq!(stats.total_widget_count, 7);
        assert_eq!(stats.repaint_boundary_count, 3);
        assert_eq!(stats.scene_layer_count, 3);
        assert_eq!(stats.stack_surface_count, 1);
        assert_eq!(stats.overlay_layer_count, 1);
        assert!(stats.command_breakdown.contains(&("Layer".to_string(), 3)));
    }

    #[test]
    fn scene_statistics_detail_mode_defaults_to_lightweight() {
        let window_id = WindowId::new(77);
        assert_eq!(
            window_scene_statistics_detail_mode(window_id),
            SceneStatisticsDetailMode::Lightweight
        );

        set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);
        assert_eq!(
            window_scene_statistics_detail_mode(window_id),
            SceneStatisticsDetailMode::Detailed
        );

        clear_window_performance_snapshot(window_id);
        assert_eq!(
            window_scene_statistics_detail_mode(window_id),
            SceneStatisticsDetailMode::Lightweight
        );
    }

    #[test]
    fn window_render_options_are_clamped_and_cleared_with_window_state() {
        let window_id = WindowId::new(91);

        set_window_render_options(
            window_id,
            WindowRenderOptions::new(false, -2.0)
                .with_text_render_policy(WindowTextRenderPolicy::Gamma(-1.0))
                .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
                .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
                .with_tone_mapping_mode(WindowToneMappingMode::Reinhard)
                .with_color_management_mode(WindowColorManagementMode::PreferHdr)
                .with_sdr_content_brightness_nits(-25.0),
        );

        assert_eq!(
            window_render_options(window_id),
            Some(
                WindowRenderOptions::new(false, 0.0)
                    .with_text_render_policy(WindowTextRenderPolicy::Linear)
                    .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
                    .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
                    .with_tone_mapping_mode(WindowToneMappingMode::Reinhard)
                    .with_color_management_mode(WindowColorManagementMode::PreferHdr)
                    .with_sdr_content_brightness_nits(203.0)
            )
        );

        set_window_render_options(
            window_id,
            WindowRenderOptions::new(true, 1.0)
                .with_optical_vertical_text_alignment_enabled(false)
                .with_glyph_pixel_alignment_enabled(false)
                .with_text_render_policy(WindowTextRenderPolicy::TwoCoverageMinusCoverageSq)
                .with_output_color_primaries(WindowOutputColorPrimaries::Srgb)
                .with_dynamic_range_mode(WindowDynamicRangeMode::StandardDynamicRange)
                .with_tone_mapping_mode(WindowToneMappingMode::Clamp)
                .with_color_management_mode(WindowColorManagementMode::ForceSdr),
        );

        assert_eq!(
            window_render_options(window_id),
            Some(
                WindowRenderOptions::new(true, 1.0)
                    .with_glyph_pixel_alignment_enabled(false)
                    .with_optical_vertical_text_alignment_enabled(false)
                    .with_text_render_policy(WindowTextRenderPolicy::TwoCoverageMinusCoverageSq)
                    .with_output_color_primaries(WindowOutputColorPrimaries::Srgb)
                    .with_dynamic_range_mode(WindowDynamicRangeMode::StandardDynamicRange)
                    .with_tone_mapping_mode(WindowToneMappingMode::Clamp)
                    .with_color_management_mode(WindowColorManagementMode::ForceSdr)
            )
        );

        clear_window_performance_snapshot(window_id);

        assert_eq!(window_render_options(window_id), None);
    }
}
