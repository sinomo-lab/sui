use std::{
    collections::{BTreeMap, HashMap},
    sync::{OnceLock, RwLock},
    time::Duration,
};

use sui_core::{DirtyRegion, Size, WindowId};
use sui_scene::{SceneCommand, SceneFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    Event,
    Layout,
    HitTest,
    Paint,
    Semantics,
    Renderer,
}

impl FramePhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Event => "Event handling",
            Self::Layout => "Layout",
            Self::HitTest => "Graph and hit test",
            Self::Paint => "Paint",
            Self::Semantics => "Semantics",
            Self::Renderer => "Renderer",
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextCacheDiagnostics {
    pub runtime_layout: CacheMetrics,
    pub renderer_layout: CacheMetrics,
    pub renderer_glyph: CacheMetrics,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneStatistics {
    pub viewport: Size,
    pub dirty_regions: Vec<DirtyRegion>,
    pub dirty_area: f32,
    pub dirty_coverage: f32,
    pub command_count: usize,
    pub command_breakdown: Vec<(String, usize)>,
    pub text_command_count: usize,
    pub image_command_count: usize,
    pub clip_command_count: usize,
    pub transform_command_count: usize,
}

impl SceneStatistics {
    pub fn from_frame(frame: &SceneFrame) -> Self {
        let mut command_breakdown = BTreeMap::<String, usize>::new();
        let mut text_command_count = 0usize;
        let mut image_command_count = 0usize;
        let mut clip_command_count = 0usize;
        let mut transform_command_count = 0usize;

        for command in frame.scene.commands() {
            *command_breakdown
                .entry(command_kind(command).to_string())
                .or_default() += 1;

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
                SceneCommand::Clear(_)
                | SceneCommand::FillRect { .. }
                | SceneCommand::StrokeRect { .. }
                | SceneCommand::FillPath { .. }
                | SceneCommand::StrokePath { .. } => {}
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

        Self {
            viewport: frame.viewport,
            dirty_regions: frame.dirty_regions.clone(),
            dirty_area,
            dirty_coverage,
            command_count: frame.scene.commands().len(),
            command_breakdown: command_breakdown.into_iter().collect(),
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
    pub text_caches: TextCacheDiagnostics,
    pub scene: SceneStatistics,
}

impl WindowPerformanceSnapshot {
    pub fn new(
        window_id: WindowId,
        frame_index: u64,
        phase_timings: Vec<FramePhaseSample>,
        text_caches: TextCacheDiagnostics,
        scene: SceneStatistics,
    ) -> Self {
        let total_time_ms = phase_timings.iter().map(|sample| sample.duration_ms).sum();

        Self {
            window_id,
            frame_index,
            total_time_ms,
            phase_timings,
            text_caches,
            scene,
        }
    }

    pub fn slowest_phase(&self) -> Option<FramePhaseSample> {
        self.phase_timings.iter().copied().max_by(|left, right| {
            left.duration_ms.total_cmp(&right.duration_ms)
        })
    }
}

static WINDOW_PERFORMANCE_SNAPSHOTS: OnceLock<RwLock<HashMap<WindowId, WindowPerformanceSnapshot>>> =
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

pub fn clear_window_performance_snapshot(window_id: WindowId) {
    let mut store = window_performance_store()
        .write()
        .expect("window performance snapshot store lock should not be poisoned");
    store.remove(&window_id);
}

pub fn clear_window_performance_snapshots() {
    let mut store = window_performance_store()
        .write()
        .expect("window performance snapshot store lock should not be poisoned");
    store.clear();
}

fn window_performance_store() -> &'static RwLock<HashMap<WindowId, WindowPerformanceSnapshot>> {
    WINDOW_PERFORMANCE_SNAPSHOTS.get_or_init(|| RwLock::new(HashMap::new()))
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
        SceneCommand::Label { .. } => "Label",
    }
}