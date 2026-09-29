//! Live frame-timing overlay shared by the dev shell and demo windows.

#![forbid(unsafe_code)]

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{
    FramePhase, InvalidationKind, InvalidationRequest, InvalidationTarget, Rect,
    SceneStatisticsDetailMode, SemanticsNode, SemanticsRole, SemanticsValue, TextStyle,
    WidgetPodMutVisitor, WidgetPodVisitor, WindowEvent, WindowPerformanceSnapshot,
    paint_single_line_aligned_text, set_window_scene_statistics_detail_mode,
    window_performance_snapshot, window_scene_statistics_detail_mode,
};
use sui_runtime::{LayerOptions, PaintBoundaryMode};
use sui_scene::LayerCompositionMode;

use crate::app::dev_text_style;

pub struct LivePerformanceRoot {
    content: SingleChild,
    performance_overlay: SingleChild,
    performance_display: Rc<RefCell<LivePerformanceDisplay>>,
    window_title: String,
    window_description: String,
    overlay_enabled: bool,
    overlay_enabled_reader: Option<Rc<dyn Fn() -> bool>>,
    last_overlay_enabled: bool,
    owns_detail_mode: bool,
}

impl LivePerformanceRoot {
    const OVERLAY_MARGIN: Insets = Insets {
        left: 0.0,
        top: 18.0,
        right: 18.0,
        bottom: 0.0,
    };

    pub fn new<Content>(
        window_title: impl Into<String>,
        window_description: impl Into<String>,
        content: Content,
    ) -> Self
    where
        Content: Widget + 'static,
    {
        let performance_display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        Self {
            content: SingleChild::new(content),
            performance_overlay: SingleChild::new(LivePerformancePanel::with_display(Rc::clone(
                &performance_display,
            ))),
            performance_display,
            window_title: window_title.into(),
            window_description: window_description.into(),
            overlay_enabled: false,
            overlay_enabled_reader: None,
            last_overlay_enabled: false,
            owns_detail_mode: false,
        }
    }

    pub fn show_performance_overlay(mut self) -> Self {
        self.overlay_enabled = true;
        self
    }

    pub fn performance_overlay_enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.overlay_enabled_reader = Some(Rc::new(enabled));
        self
    }

    fn overlay_enabled(&self) -> bool {
        self.overlay_enabled
            || self
                .overlay_enabled_reader
                .as_ref()
                .is_some_and(|enabled| enabled())
    }

    fn set_performance_display(
        &mut self,
        snapshot: Option<WindowPerformanceSnapshot>,
        idle: bool,
    ) -> bool {
        let mut samples = self.performance_display.borrow().samples.clone();
        if let Some(snapshot) = &snapshot {
            if samples
                .last()
                .is_none_or(|sample| sample.frame_index != snapshot.frame_index)
            {
                samples.push(LivePerformanceFrameSample::from_snapshot(snapshot));
                if samples.len() > LIVE_PERFORMANCE_HISTORY_LIMIT {
                    let overflow = samples.len() - LIVE_PERFORMANCE_HISTORY_LIMIT;
                    samples.drain(0..overflow);
                }
            }
        } else {
            samples.clear();
        }

        let next = LivePerformanceDisplay {
            snapshot,
            idle,
            samples,
        };
        let mut display = self.performance_display.borrow_mut();
        if *display == next {
            return false;
        }

        *display = next;
        true
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct LivePerformanceDisplay {
    snapshot: Option<WindowPerformanceSnapshot>,
    idle: bool,
    samples: Vec<LivePerformanceFrameSample>,
}

pub(crate) const LIVE_PERFORMANCE_HISTORY_LIMIT: usize = 72;
pub(crate) const LIVE_PERFORMANCE_STAGE_COUNT: usize = 9;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LivePerformanceFrameSample {
    frame_index: u64,
    total_time_ms: f32,
    stage_costs: [f32; LIVE_PERFORMANCE_STAGE_COUNT],
}

impl LivePerformanceFrameSample {
    fn from_snapshot(snapshot: &WindowPerformanceSnapshot) -> Self {
        let mut stage_costs = [0.0; LIVE_PERFORMANCE_STAGE_COUNT];
        for timing in &snapshot.phase_timings {
            stage_costs[frame_phase_index(timing.phase)] += timing.duration_ms.max(0.0) as f32;
        }

        if snapshot.phase_timings.is_empty() {
            stage_costs[frame_phase_index(FramePhase::Renderer)] = snapshot.total_time_ms as f32;
        }

        Self {
            frame_index: snapshot.frame_index,
            total_time_ms: snapshot.total_time_ms.max(0.0) as f32,
            stage_costs,
        }
    }
}

pub(crate) const fn frame_phase_index(phase: FramePhase) -> usize {
    match phase {
        FramePhase::Event => 0,
        FramePhase::Redraw => 1,
        FramePhase::MeasureArrange => 2,
        FramePhase::HitTest => 3,
        FramePhase::Paint => 4,
        FramePhase::Semantics => 5,
        FramePhase::Renderer => 6,
        FramePhase::SurfaceWait => 7,
        FramePhase::Diagnostics => 8,
    }
}

impl Widget for LivePerformanceRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            let overlay_enabled = self.overlay_enabled();
            if overlay_enabled != self.last_overlay_enabled {
                self.last_overlay_enabled = overlay_enabled;
                ctx.request_measure();
                ctx.request_semantics();
                ctx.request_paint();

                if !overlay_enabled {
                    self.set_performance_display(None, true);
                    if self.owns_detail_mode {
                        set_window_scene_statistics_detail_mode(
                            ctx.window_id(),
                            SceneStatisticsDetailMode::Lightweight,
                        );
                        self.owns_detail_mode = false;
                    }
                }
            }

            if overlay_enabled {
                if !window_scene_statistics_detail_mode(ctx.window_id()).is_detailed() {
                    set_window_scene_statistics_detail_mode(
                        ctx.window_id(),
                        SceneStatisticsDetailMode::Detailed,
                    );
                    self.owns_detail_mode = true;
                }

                if let Some(snapshot) = window_performance_snapshot(ctx.window_id())
                    && self.set_performance_display(Some(snapshot), false)
                {
                    let overlay_id = self.performance_overlay.child().id();
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(overlay_id),
                            InvalidationKind::Paint,
                        )
                        .with_region(self.performance_overlay.child().bounds()),
                    );
                }
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                1280.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                720.0
            },
        ));
        self.content.measure(ctx, Constraints::tight(viewport));
        if self.overlay_enabled() {
            self.performance_overlay
                .measure(ctx, Constraints::new(Size::ZERO, viewport));
        }
        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content
            .arrange(ctx, Rect::from_origin_size(bounds.origin, bounds.size));

        if self.overlay_enabled() {
            let overlay_size = self.performance_overlay.child().measured_size();
            let overlay_x = (bounds.max_x() - overlay_size.width - Self::OVERLAY_MARGIN.right)
                .max(bounds.x() + Self::OVERLAY_MARGIN.left);
            let overlay_y = bounds.y() + Self::OVERLAY_MARGIN.top;
            self.performance_overlay.arrange(
                ctx,
                Rect::from_origin_size(Point::new(overlay_x, overlay_y), overlay_size),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(ThemeColors::light().neutrals.subtle);
        self.content.paint(ctx);
        if self.overlay_enabled() {
            self.performance_overlay.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some(self.window_title.clone());
        root.description = Some(self.window_description.clone());
        ctx.push(root);
        self.content.semantics(ctx);
        if self.overlay_enabled() {
            self.performance_overlay.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        if self.overlay_enabled() {
            self.performance_overlay.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        if self.overlay_enabled() {
            self.performance_overlay.visit_children_mut(visitor);
        }
    }
}

pub(crate) struct LivePerformancePanel {
    display: Rc<RefCell<LivePerformanceDisplay>>,
}

impl LivePerformancePanel {
    const WIDTH: f32 = 340.0;
    const HEIGHT: f32 = 162.0;
    const PADDING_X: f32 = 12.0;
    const PADDING_Y: f32 = 10.0;
    const CORNER_RADIUS: f32 = 8.0;
    const HEADER_HEIGHT: f32 = 36.0;
    const GRAPH_HEIGHT: f32 = 72.0;
    const LEGEND_HEIGHT: f32 = 28.0;
    const BAR_GAP: f32 = 1.0;

    #[cfg(test)]
    fn new() -> Self {
        Self::with_display(Rc::new(RefCell::new(LivePerformanceDisplay::default())))
    }

    fn with_display(display: Rc<RefCell<LivePerformanceDisplay>>) -> Self {
        Self { display }
    }

    fn caption_text_style(color: Color) -> TextStyle {
        let theme = DefaultTheme::default();
        dev_text_style(theme, theme.text.xs, color)
    }

    fn headline_text_style(color: Color) -> TextStyle {
        let theme = DefaultTheme::default();
        dev_text_style(theme, theme.text._2xl, color)
    }

    fn graph_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + Self::PADDING_X,
            bounds.y() + Self::PADDING_Y + Self::HEADER_HEIGHT,
            (bounds.width() - Self::PADDING_X * 2.0).max(1.0),
            Self::GRAPH_HEIGHT,
        )
    }

    fn legend_bounds(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + Self::PADDING_X,
            bounds.max_y() - Self::PADDING_Y - Self::LEGEND_HEIGHT,
            (bounds.width() - Self::PADDING_X * 2.0).max(1.0),
            Self::LEGEND_HEIGHT,
        )
    }

    fn frame_cost_scale(samples: &[LivePerformanceFrameSample]) -> f32 {
        samples
            .iter()
            .map(|sample| sample.total_time_ms)
            .fold(16.67, f32::max)
            .clamp(16.67, 66.67)
    }

    fn stage_short_label(phase: FramePhase) -> &'static str {
        match phase {
            FramePhase::Event => "evt",
            FramePhase::Redraw => "redraw",
            FramePhase::MeasureArrange => "layout",
            FramePhase::HitTest => "hit",
            FramePhase::Paint => "paint",
            FramePhase::Semantics => "a11y",
            FramePhase::Renderer => "rend",
            FramePhase::SurfaceWait => "wait",
            FramePhase::Diagnostics => "diag",
        }
    }

    fn stage_color(phase: FramePhase, alpha: f32) -> Color {
        let alpha = alpha.clamp(0.0, 1.0);
        match phase {
            FramePhase::Event => Color::rgba(0.24, 0.78, 0.68, alpha),
            FramePhase::Redraw => Color::rgba(0.50, 0.68, 0.95, alpha),
            FramePhase::MeasureArrange => Color::rgba(0.96, 0.68, 0.30, alpha),
            FramePhase::HitTest => Color::rgba(0.68, 0.56, 0.92, alpha),
            FramePhase::Paint => Color::rgba(0.94, 0.42, 0.54, alpha),
            FramePhase::Semantics => Color::rgba(0.72, 0.82, 0.36, alpha),
            FramePhase::Renderer => Color::rgba(0.36, 0.78, 0.96, alpha),
            FramePhase::SurfaceWait => Color::rgba(0.68, 0.72, 0.78, alpha),
            FramePhase::Diagnostics => Color::rgba(0.78, 0.80, 0.86, alpha),
        }
    }

    fn paint_budget_line(ctx: &mut PaintCtx, graph: Rect, scale_ms: f32, budget_ms: f32) {
        if budget_ms > scale_ms {
            return;
        }

        let y = graph.max_y() - graph.height() * (budget_ms / scale_ms);
        let mut path = Path::builder();
        path.move_to(Point::new(graph.x(), y));
        path.line_to(Point::new(graph.max_x(), y));
        ctx.stroke(
            path.build(),
            Color::rgba(0.98, 1.0, 1.0, 0.28),
            StrokeStyle::new(1.0),
        );
    }

    fn paint_graph(&self, ctx: &mut PaintCtx, display: &LivePerformanceDisplay, graph: Rect) {
        ctx.fill_rect(graph, Color::rgba(0.0, 0.0, 0.0, 0.24));
        ctx.stroke_rect(
            graph,
            Color::rgba(0.98, 1.0, 1.0, 0.22),
            StrokeStyle::new(1.0),
        );

        let scale_ms = Self::frame_cost_scale(&display.samples);
        Self::paint_budget_line(ctx, graph, scale_ms, 16.67);
        Self::paint_budget_line(ctx, graph, scale_ms, 33.33);

        if display.samples.is_empty() {
            let style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.72));
            paint_single_line_aligned_text(
                ctx,
                graph,
                "waiting for frames",
                &style,
                style.line_height,
                0.5,
            );
            return;
        }

        let slot_width = (graph.width() / LIVE_PERFORMANCE_HISTORY_LIMIT as f32).max(2.0);
        let bar_width = (slot_width - Self::BAR_GAP).max(1.0);
        let visible_count = ((graph.width() / slot_width).floor() as usize)
            .min(display.samples.len())
            .max(1);
        let samples = &display.samples[display.samples.len() - visible_count..];
        let start_x = graph.max_x() - slot_width * samples.len() as f32;

        ctx.push_clip_rect(graph);
        for (sample_index, sample) in samples.iter().enumerate() {
            let x = start_x + sample_index as f32 * slot_width;
            let mut y = graph.max_y();
            for phase in LIVE_PERFORMANCE_GRAPH_PHASES {
                let duration = sample.stage_costs[frame_phase_index(phase)];
                if duration <= 0.0 {
                    continue;
                }

                let height = (graph.height() * (duration / scale_ms)).max(0.5);
                y = (y - height).max(graph.y());
                ctx.fill_rect(
                    Rect::new(x, y, bar_width, (graph.max_y() - y).min(height)),
                    Self::stage_color(phase, 0.88),
                );
            }
        }
        ctx.pop_clip();

        let scale_style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.62));
        let scale_label = format!("{scale_ms:.0} ms");
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                graph.x() + 4.0,
                graph.y() + 2.0,
                48.0,
                scale_style.line_height,
            ),
            &scale_label,
            &scale_style,
            scale_style.line_height,
            0.0,
        );
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                graph.x() + 4.0,
                graph.max_y() - scale_style.line_height - 2.0,
                56.0,
                scale_style.line_height,
            ),
            "16.7 ms",
            &scale_style,
            scale_style.line_height,
            0.0,
        );
    }

    fn paint_legend(&self, ctx: &mut PaintCtx, bounds: Rect) {
        let mut x = bounds.x();
        let label_style = Self::caption_text_style(Color::rgba(0.94, 0.97, 1.0, 0.74));
        let y = bounds.y() + (bounds.height() - label_style.line_height) * 0.5;
        for phase in LIVE_PERFORMANCE_GRAPH_PHASES {
            let label = Self::stage_short_label(phase);
            let label_width = match phase {
                FramePhase::MeasureArrange => 38.0,
                FramePhase::Redraw => 42.0,
                FramePhase::Renderer | FramePhase::SurfaceWait => 34.0,
                _ => 30.0,
            };
            if x + label_width > bounds.max_x() {
                break;
            }

            ctx.fill_rect(
                Rect::new(x, y + 4.0, 7.0, 7.0),
                Self::stage_color(phase, 0.95),
            );
            paint_single_line_aligned_text(
                ctx,
                Rect::new(x + 10.0, y, label_width - 10.0, label_style.line_height),
                label,
                &label_style,
                label_style.line_height,
                0.0,
            );
            x += label_width;
        }
    }

    fn snapshot_phase_duration(snapshot: &WindowPerformanceSnapshot, phase: FramePhase) -> f64 {
        snapshot
            .phase_timings
            .iter()
            .filter(|sample| sample.phase == phase)
            .map(|sample| sample.duration_ms)
            .sum()
    }
}

pub(crate) const LIVE_PERFORMANCE_GRAPH_PHASES: [FramePhase; LIVE_PERFORMANCE_STAGE_COUNT] = [
    FramePhase::Event,
    FramePhase::Redraw,
    FramePhase::MeasureArrange,
    FramePhase::HitTest,
    FramePhase::Paint,
    FramePhase::Semantics,
    FramePhase::Renderer,
    FramePhase::SurfaceWait,
    FramePhase::Diagnostics,
];

impl Widget for LivePerformancePanel {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.min(Self::WIDTH)
        } else {
            Self::WIDTH
        };
        constraints.clamp(Size::new(width, Self::HEIGHT))
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Overlay,
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let display = self.display.borrow().clone();
        let frame = rounded_rect_path(ctx.bounds(), Self::CORNER_RADIUS);
        ctx.fill(frame.clone(), Color::rgba(0.015, 0.025, 0.035, 0.50));
        ctx.stroke(
            frame,
            Color::rgba(0.98, 1.0, 1.0, 0.18),
            StrokeStyle::new(1.0),
        );

        let header_y = ctx.bounds().y() + Self::PADDING_Y;
        let (fps_text, frame_text, slowest_text) = if let Some(snapshot) = &display.snapshot {
            let fps = if display.idle {
                "0 fps".to_string()
            } else {
                format_fps(snapshot.frame_interval_ms)
            };
            let frame = if display.idle {
                "idle".to_string()
            } else {
                format!(
                    "frame {} | work {}",
                    snapshot.frame_index,
                    format_duration_ms(snapshot.total_time_ms)
                )
            };
            let renderer_work_ms = Self::snapshot_phase_duration(snapshot, FramePhase::Renderer);
            let surface_wait_ms = Self::snapshot_phase_duration(snapshot, FramePhase::SurfaceWait);
            let slowest = if renderer_work_ms > 0.0 || surface_wait_ms > 0.0 {
                format!(
                    "rend {} | wait {}",
                    format_duration_ms(renderer_work_ms),
                    format_duration_ms(surface_wait_ms),
                )
            } else {
                snapshot
                    .slowest_phase()
                    .map(|sample| {
                        format!(
                            "{} {}",
                            Self::stage_short_label(sample.phase),
                            format_duration_ms(sample.duration_ms)
                        )
                    })
                    .unwrap_or_else(|| "waiting for phases".to_string())
            };
            (fps, frame, slowest)
        } else {
            (
                "0 fps".to_string(),
                "waiting".to_string(),
                "waiting for first frame".to_string(),
            )
        };

        let fps_style = Self::headline_text_style(Color::rgba(0.98, 1.0, 1.0, 0.96));
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                ctx.bounds().x() + Self::PADDING_X,
                header_y,
                118.0,
                Self::HEADER_HEIGHT,
            ),
            &fps_text,
            &fps_style,
            fps_style.line_height,
            0.0,
        );
        let detail_style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.76));
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                ctx.bounds().x() + 136.0,
                header_y,
                ctx.bounds().width() - 148.0,
                detail_style.line_height,
            ),
            &frame_text,
            &detail_style,
            detail_style.line_height,
            0.0,
        );
        let muted_detail_style = Self::caption_text_style(Color::rgba(0.92, 0.96, 1.0, 0.66));
        paint_single_line_aligned_text(
            ctx,
            Rect::new(
                ctx.bounds().x() + 136.0,
                header_y + detail_style.line_height,
                ctx.bounds().width() - 148.0,
                muted_detail_style.line_height,
            ),
            &slowest_text,
            &muted_detail_style,
            muted_detail_style.line_height,
            0.0,
        );

        self.paint_graph(ctx, &display, Self::graph_bounds(ctx.bounds()));
        self.paint_legend(ctx, Self::legend_bounds(ctx.bounds()));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let display = self.display.borrow();
        let value = display
            .snapshot
            .as_ref()
            .map(|snapshot| {
                format!(
                    "{} | {} | {} samples",
                    if display.idle {
                        "0 fps".to_string()
                    } else {
                        format_fps(snapshot.frame_interval_ms)
                    },
                    format_duration_ms(snapshot.total_time_ms),
                    display.samples.len()
                )
            })
            .unwrap_or_else(|| "waiting for frames".to_string());
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some("Live performance overlay".to_string());
        node.description =
            Some("Host frame cadence with rolling stacked frame-work costs.".to_string());
        node.value = Some(SemanticsValue::Text(value));
        ctx.push(node);
    }
}

pub(crate) fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(rect, radius.min(rect.width().min(rect.height()) * 0.5))
}

pub(crate) fn format_fps(frame_interval_ms: Option<f64>) -> String {
    match frame_interval_ms.filter(|interval| interval.is_finite() && *interval > 0.0) {
        Some(interval) => format!("{:.0} fps", 1000.0 / interval),
        None => "-- fps".to_string(),
    }
}

pub(crate) fn format_duration_ms(duration_ms: f64) -> String {
    format!("{duration_ms:.1} ms")
}

#[cfg(test)]
mod tests;
