use std::{
    collections::{BTreeMap, HashMap},
    sync::{OnceLock, RwLock},
    time::Duration,
};

use sui_core::{DirtyRegion, Size, WindowId};
use sui_scene::{SceneCommand, SceneFrame, SceneLayerUpdateKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    Event,
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderDiagnostics {
    pub phase_timings: Vec<FramePhaseSample>,
    pub text_caches: TextCacheDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererSubmissionDiagnostics {
    pub pass_count: usize,
    pub draw_count: usize,
    pub uploaded_vertex_bytes: u64,
    pub visible_layer_count: usize,
    pub visible_tile_count: usize,
    pub reused_tile_count: usize,
    pub regenerated_tile_count: usize,
    pub direct_packet_count: usize,
    pub tile_memory_bytes: u64,
    pub tile_generation_time_us: u64,
    pub composition_time_us: u64,
}

impl RendererSubmissionDiagnostics {
    pub const fn new(
        pass_count: usize,
        draw_count: usize,
        uploaded_vertex_bytes: u64,
        visible_layer_count: usize,
        visible_tile_count: usize,
        reused_tile_count: usize,
        regenerated_tile_count: usize,
        direct_packet_count: usize,
        tile_memory_bytes: u64,
        tile_generation_time_us: u64,
        composition_time_us: u64,
    ) -> Self {
        Self {
            pass_count,
            draw_count,
            uploaded_vertex_bytes,
            visible_layer_count,
            visible_tile_count,
            reused_tile_count,
            regenerated_tile_count,
            direct_packet_count,
            tile_memory_bytes,
            tile_generation_time_us,
            composition_time_us,
        }
    }
}

impl RenderDiagnostics {
    pub fn push(&mut self, phase: FramePhase, duration: Duration) {
        self.phase_timings
            .push(FramePhaseSample::from_duration(phase, duration));
    }

    pub fn total_time_ms(&self) -> f64 {
        self.phase_timings.iter().map(|sample| sample.duration_ms).sum()
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextCacheDeltaDiagnostics {
    pub runtime_layout: CacheMetricsDelta,
    pub renderer_layout: CacheMetricsDelta,
    pub renderer_glyph: CacheMetricsDelta,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowPerformanceSummary {
    pub window_id: WindowId,
    pub frame_index: u64,
    pub total_time_ms: f64,
    pub slowest_phase: Option<FramePhaseSample>,
    pub renderer_submission: RendererSubmissionDiagnostics,
    pub text_caches: TextCacheDiagnostics,
    pub dirty_region_count: usize,
    pub dirty_coverage: f32,
    pub command_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneStatistics {
    pub detail_mode: SceneStatisticsDetailMode,
    pub viewport: Size,
    pub dirty_region_count: usize,
    pub dirty_regions: Vec<DirtyRegion>,
    pub dirty_area: f32,
    pub dirty_coverage: f32,
    pub command_count: usize,
    pub command_breakdown: Vec<(String, usize)>,
    pub layer_count: usize,
    pub layer_update_count: usize,
    pub layer_update_breakdown: Vec<(String, usize)>,
    pub text_command_count: usize,
    pub image_command_count: usize,
    pub clip_command_count: usize,
    pub transform_command_count: usize,
}

impl SceneStatistics {
    pub fn from_frame(frame: &SceneFrame) -> Self {
        Self::from_frame_with_mode(frame, SceneStatisticsDetailMode::Lightweight)
    }

    pub fn from_frame_with_mode(
        frame: &SceneFrame,
        detail_mode: SceneStatisticsDetailMode,
    ) -> Self {
        let detailed = detail_mode.is_detailed();
        let mut command_breakdown = detailed.then(BTreeMap::<String, usize>::new);
        let mut text_command_count = 0usize;
        let mut image_command_count = 0usize;
        let mut clip_command_count = 0usize;
        let mut transform_command_count = 0usize;
        let mut command_count = 0usize;
        let mut layer_count = 0usize;

        frame.scene.visit_commands(&mut |command| {
            command_count += 1;
            if let Some(breakdown) = &mut command_breakdown {
                *breakdown.entry(command_kind(command).to_string()).or_default() += 1;
            }

            match command {
                SceneCommand::DrawText(_)
                | SceneCommand::DrawShapedText(_)
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
                SceneCommand::Layer(_) => {
                    layer_count += 1;
                }
                SceneCommand::Clear(_)
                | SceneCommand::FillRect { .. }
                | SceneCommand::StrokeRect { .. }
                | SceneCommand::FillPath { .. }
                | SceneCommand::StrokePath { .. } => {}
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

        Self {
            detail_mode,
            viewport: frame.viewport,
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
            layer_count,
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

#[derive(Debug, Clone, PartialEq)]
pub struct WindowPerformanceSnapshot {
    pub window_id: WindowId,
    pub frame_index: u64,
    pub total_time_ms: f64,
    pub phase_timings: Vec<FramePhaseSample>,
    pub renderer_submission: RendererSubmissionDiagnostics,
    pub text_caches: TextCacheDiagnostics,
    pub text_cache_deltas: TextCacheDeltaDiagnostics,
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

        Self {
            window_id,
            frame_index,
            total_time_ms,
            phase_timings,
            renderer_submission,
            text_caches,
            text_cache_deltas,
            scene,
        }
    }

    pub fn slowest_phase(&self) -> Option<FramePhaseSample> {
        self.phase_timings.iter().copied().max_by(|left, right| {
            left.duration_ms.total_cmp(&right.duration_ms)
        })
    }

    pub fn summary(&self) -> WindowPerformanceSummary {
        WindowPerformanceSummary {
            window_id: self.window_id,
            frame_index: self.frame_index,
            total_time_ms: self.total_time_ms,
            slowest_phase: self.slowest_phase(),
            renderer_submission: self.renderer_submission,
            text_caches: self.text_caches,
            dirty_region_count: self.scene.dirty_region_count,
            dirty_coverage: self.scene.dirty_coverage,
            command_count: self.scene.command_count,
        }
    }
}

static WINDOW_PERFORMANCE_SNAPSHOTS: OnceLock<RwLock<HashMap<WindowId, WindowPerformanceSnapshot>>> =
    OnceLock::new();
static WINDOW_SCENE_STATISTICS_DETAIL_MODES: OnceLock<
    RwLock<HashMap<WindowId, SceneStatisticsDetailMode>>,
> = OnceLock::new();

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
    store.get(&window_id).map(WindowPerformanceSnapshot::summary)
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

pub fn clear_window_performance_snapshot(window_id: WindowId) {
    let mut store = window_performance_store()
        .write()
        .expect("window performance snapshot store lock should not be poisoned");
    store.remove(&window_id);

    let mut detail_modes = window_scene_statistics_detail_mode_store()
        .write()
        .expect("scene statistics detail mode store lock should not be poisoned");
    detail_modes.remove(&window_id);
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
}

fn window_performance_store() -> &'static RwLock<HashMap<WindowId, WindowPerformanceSnapshot>> {
    WINDOW_PERFORMANCE_SNAPSHOTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn window_scene_statistics_detail_mode_store(
) -> &'static RwLock<HashMap<WindowId, SceneStatisticsDetailMode>> {
    WINDOW_SCENE_STATISTICS_DETAIL_MODES.get_or_init(|| RwLock::new(HashMap::new()))
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
        SceneStatistics, SceneStatisticsDetailMode, TextCacheDeltaDiagnostics,
        TextCacheDiagnostics, WindowPerformanceSnapshot, clear_window_performance_snapshot,
        set_window_scene_statistics_detail_mode, window_scene_statistics_detail_mode,
    };
    use sui_core::{Color, DirtyRegion, InvalidationKind, Rect, Size, WindowId};
    use sui_scene::{SceneCommand, SceneFrame};

    #[test]
    fn text_cache_deltas_are_derived_from_prior_counters() {
        let previous = TextCacheDiagnostics {
            runtime_layout: CacheMetrics::new(2, 10, 4),
            renderer_layout: CacheMetrics::new(3, 6, 2),
            renderer_glyph: CacheMetrics::new(5, 12, 3),
        };
        let current = TextCacheDiagnostics {
            runtime_layout: CacheMetrics::new(4, 15, 6),
            renderer_layout: CacheMetrics::new(3, 9, 3),
            renderer_glyph: CacheMetrics::new(7, 18, 5),
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
    }

    #[test]
    fn window_performance_snapshot_preserves_renderer_submission_stats() {
        let snapshot = WindowPerformanceSnapshot::new(
            WindowId::new(5),
            17,
            vec![FramePhaseSample::new(FramePhase::Renderer, 2.5)],
            RendererSubmissionDiagnostics::new(3, 9, 4096, 4, 12, 10, 2, 7, 16384, 230, 90),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: SceneStatisticsDetailMode::Lightweight,
                viewport: Size::new(640.0, 360.0),
                dirty_region_count: 0,
                dirty_regions: Vec::new(),
                dirty_area: 0.0,
                dirty_coverage: 0.0,
                command_count: 0,
                command_breakdown: Vec::new(),
                layer_count: 0,
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
        assert_eq!(snapshot.renderer_submission.visible_tile_count, 12);
        assert_eq!(snapshot.renderer_submission.reused_tile_count, 10);
        assert_eq!(snapshot.renderer_submission.regenerated_tile_count, 2);
        assert_eq!(snapshot.renderer_submission.tile_memory_bytes, 16384);
        assert_eq!(snapshot.total_time_ms, 2.5);
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

        let lightweight = SceneStatistics::from_frame(&frame);
        assert_eq!(
            lightweight.detail_mode,
            SceneStatisticsDetailMode::Lightweight
        );
        assert_eq!(lightweight.dirty_region_count, 1);
        assert_eq!(lightweight.command_count, 2);
        assert!(lightweight.dirty_regions.is_empty());
        assert!(lightweight.command_breakdown.is_empty());

        let detailed =
            SceneStatistics::from_frame_with_mode(&frame, SceneStatisticsDetailMode::Detailed);
        assert_eq!(detailed.detail_mode, SceneStatisticsDetailMode::Detailed);
        assert_eq!(detailed.dirty_region_count, 1);
        assert_eq!(detailed.dirty_regions.len(), 1);
        assert!(detailed.command_breakdown.contains(&("Clear".to_string(), 1)));
        assert!(detailed.command_breakdown.contains(&("Label".to_string(), 1)));
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
}