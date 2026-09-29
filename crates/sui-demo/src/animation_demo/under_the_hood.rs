//! Under the hood: what motion asks of the runtime. The same shuttle moves
//! a retained layer on one stage and repaints at a new offset on the other,
//! with counters for what each frame requested, and a graph of animation
//! frame intervals.

use std::{cell::Cell, collections::VecDeque, rc::Rc};

use sui::prelude::*;
use sui::{
    AnimationSpec, LayerOptions, PaintBoundaryMode, SemanticsNode, SemanticsRole, SemanticsValue,
    SingleChild, Vector, WidgetPodMutVisitor, WidgetPodVisitor, motion_policy,
};
use sui_scene::{LayerCompositionMode, LayerProperties};

use super::curves::Shuttle;
use super::{draw_text, hairline, paint_card};
use crate::app::{DemoTextRole, DevThemeReader};

pub(crate) const LAYER_STAGE_NAME: &str = "Layer transform stage";
pub(crate) const REPAINT_STAGE_NAME: &str = "Repaint stage";
pub(crate) const FRAME_PACING_NAME: &str = "Frame pacing";

const STAGE_HEIGHT: f32 = 168.0;
const CHIP: Size = Size::new(76.0, 36.0);
const SHUTTLE_DURATION: f64 = 0.9;
const SHUTTLE_HOLD: f64 = 0.25;
/// Frame intervals kept for the graph.
const PACING_SAMPLES: usize = 120;

/// What one stage's moving part asked of the runtime, frame by frame.
#[derive(Debug, Default)]
pub(super) struct StageCounters {
    pub(super) frames: Cell<u64>,
    pub(super) repaints: Cell<u64>,
    pub(super) layer_moves: Cell<u64>,
}

impl StageCounters {
    fn add(counter: &Cell<u64>) {
        counter.set(counter.get() + 1);
    }

    pub(super) fn summary(&self) -> String {
        format!(
            "{} frames · {} repaints · {} layer moves",
            self.frames.get(),
            self.repaints.get(),
            self.layer_moves.get()
        )
    }
}

fn shuttle_spec(theme: &DefaultTheme) -> AnimationSpec {
    AnimationSpec::tween(SHUTTLE_DURATION, theme.motion.easing_emphasized)
        .with_movement_policy(motion_policy())
}

fn shuttle_hold() -> f64 {
    SHUTTLE_HOLD / f64::from(motion_policy().time_scale())
}

pub(super) fn section(theme_reader: DevThemeReader) -> impl Widget {
    let layer_counters = Rc::new(StageCounters::default());
    let repaint_counters = Rc::new(StageCounters::default());
    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start)
        .with_item(
            StageCard::new(
                LAYER_STAGE_NAME,
                "Layer transform",
                "Each frame moves and scales the chip's layer; nothing is repainted for it.",
                Rc::clone(&layer_counters),
                Rc::clone(&theme_reader),
                LayerStage::new(layer_counters, Rc::clone(&theme_reader)),
            ),
            third(),
        )
        .with_item(
            StageCard::new(
                REPAINT_STAGE_NAME,
                "Repaint",
                "Each frame repaints the chip at a new offset.",
                Rc::clone(&repaint_counters),
                Rc::clone(&theme_reader),
                RepaintStage::new(repaint_counters, Rc::clone(&theme_reader)),
            ),
            third(),
        )
        .with_item(FramePacingGraph::new(theme_reader), third())
}

fn third() -> FlexItem {
    FlexItem::new()
        .basis_gap_aware_fraction(1.0 / 3.0)
        .min_width(280.0)
}

/// A card around a stage: title, explanation, the stage, and its counters.
/// The card repaints its counters on a timer of its own, so reading them
/// never repaints the stage.
struct StageCard {
    name: &'static str,
    title: &'static str,
    detail: &'static str,
    counters: Rc<StageCounters>,
    theme_reader: DevThemeReader,
    stage: SingleChild,
    last_readout: f64,
}

impl StageCard {
    fn new(
        name: &'static str,
        title: &'static str,
        detail: &'static str,
        counters: Rc<StageCounters>,
        theme_reader: DevThemeReader,
        stage: impl Widget + 'static,
    ) -> Self {
        Self {
            name,
            title,
            detail,
            counters,
            theme_reader,
            stage: SingleChild::new(stage),
            last_readout: 0.0,
        }
    }

    fn stage_rect(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + 16.0,
            bounds.y() + 62.0,
            bounds.width() - 32.0,
            STAGE_HEIGHT - 62.0 - 16.0,
        )
    }
}

impl Widget for StageCard {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Wake(WakeEvent::AnimationFrame { time, .. }) = event {
            if *time - self.last_readout >= 0.25 {
                self.last_readout = *time;
                ctx.request_paint();
                ctx.request_semantics();
            }
            ctx.request_animation_frame();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        ctx.request_animation_frame();
        let size = super::fill_width(constraints, STAGE_HEIGHT + 30.0);
        let stage = Self::stage_rect(Rect::from_origin_size(Point::ZERO, size));
        self.stage.measure(ctx, Constraints::tight(stage.size));
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.stage.arrange(ctx, Self::stage_rect(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        paint_card(ctx, bounds, theme);
        draw_text(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 16.0,
                bounds.y() + 14.0,
                bounds.width() - 32.0,
                20.0,
            ),
            self.title,
            DemoTextRole::CardTitle,
            palette.text,
        );
        draw_text(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 16.0,
                bounds.y() + 36.0,
                bounds.width() - 32.0,
                18.0,
            ),
            self.detail,
            DemoTextRole::Metadata,
            palette.text_muted,
        );
        self.stage.paint(ctx);
        draw_text(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 16.0,
                bounds.max_y() - 30.0,
                bounds.width() - 32.0,
                18.0,
            ),
            &self.counters.summary(),
            DemoTextRole::Metadata,
            palette.text,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        node.description = Some(self.detail.to_string());
        node.value = Some(SemanticsValue::Text(self.counters.summary()));
        ctx.push(node);
        self.stage.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.stage.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.stage.visit_children_mut(visitor);
    }
}

fn paint_rail(ctx: &mut PaintCtx, bounds: Rect, theme: DefaultTheme) {
    let palette = theme.palette;
    ctx.fill(Path::rounded_rect(bounds, 8.0), palette.field);
    let y = bounds.y() + bounds.height() * 0.5;
    hairline(
        ctx,
        Point::new(bounds.x() + 12.0, y),
        Point::new(bounds.max_x() - 12.0, y),
        palette.border,
    );
}

fn chip_rect(bounds: Rect, offset: f32) -> Rect {
    Rect::new(
        bounds.x() + 12.0 + offset,
        bounds.y() + (bounds.height() - CHIP.height) * 0.5,
        CHIP.width,
        CHIP.height,
    )
}

fn chip_travel(bounds: Rect) -> f32 {
    (bounds.width() - 24.0 - CHIP.width).max(0.0)
}

fn paint_chip(ctx: &mut PaintCtx, rect: Rect, theme: DefaultTheme) {
    ctx.fill(Path::rounded_rect(rect, 10.0), theme.palette.accent);
}

/// The layer stage: a static rail with a chip child that moves as a
/// retained layer.
struct LayerStage {
    theme_reader: DevThemeReader,
    chip: SingleChild,
    travel: Rc<Cell<f32>>,
}

impl LayerStage {
    fn new(counters: Rc<StageCounters>, theme_reader: DevThemeReader) -> Self {
        let travel = Rc::new(Cell::new(0.0));
        Self {
            chip: SingleChild::new(LayerChip {
                counters,
                theme_reader: Rc::clone(&theme_reader),
                travel: Rc::clone(&travel),
                shuttle: Shuttle::new(),
            }),
            theme_reader,
            travel,
        }
    }
}

impl Widget for LayerStage {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.chip.measure(ctx, Constraints::tight(CHIP));
        constraints.max
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.travel.set(chip_travel(bounds));
        self.chip.arrange(ctx, chip_rect(bounds, 0.0));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_rail(ctx, ctx.bounds(), (self.theme_reader)());
        self.chip.paint(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.chip.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.chip.visit_children_mut(visitor);
    }
}

/// A chip painted once into its own layer; animation frames only change the
/// layer's translation.
struct LayerChip {
    counters: Rc<StageCounters>,
    theme_reader: DevThemeReader,
    travel: Rc<Cell<f32>>,
    shuttle: Shuttle,
}

impl Widget for LayerChip {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Wake(WakeEvent::AnimationFrame { time, .. }) = event {
            StageCounters::add(&self.counters.frames);
            StageCounters::add(&self.counters.layer_moves);
            let spec = shuttle_spec(&(self.theme_reader)());
            self.shuttle.advance(*time, spec, shuttle_hold());
            ctx.request_transform();
            ctx.request_animation_frame();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        ctx.request_animation_frame();
        constraints.clamp(CHIP)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_chip(ctx, ctx.bounds(), (self.theme_reader)());
    }

    /// The chip's paint only changes with the theme, which invalidates the
    /// whole window, so the runtime can keep it between frames.
    fn supports_output_reuse(&self) -> bool {
        true
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        // The chip swells a little mid-travel: layer scale, like the
        // translation, changes only the composited layer.
        let value = self.shuttle.value();
        LayerProperties::default()
            .with_translation(Vector::new(value * self.travel.get(), 0.0))
            .with_scale(1.0 + 0.18 * (value.clamp(0.0, 1.0) * std::f32::consts::PI).sin())
    }
}

/// The repaint stage: the chip is painted at its offset, so every frame
/// repaints the stage.
struct RepaintStage {
    counters: Rc<StageCounters>,
    theme_reader: DevThemeReader,
    shuttle: Shuttle,
}

impl RepaintStage {
    fn new(counters: Rc<StageCounters>, theme_reader: DevThemeReader) -> Self {
        Self {
            counters,
            theme_reader,
            shuttle: Shuttle::new(),
        }
    }
}

impl Widget for RepaintStage {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Wake(WakeEvent::AnimationFrame { time, .. }) = event {
            StageCounters::add(&self.counters.frames);
            StageCounters::add(&self.counters.repaints);
            let spec = shuttle_spec(&(self.theme_reader)());
            self.shuttle.advance(*time, spec, shuttle_hold());
            ctx.request_paint();
            ctx.request_animation_frame();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        ctx.request_animation_frame();
        constraints.max
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        paint_rail(ctx, bounds, theme);
        let offset = self.shuttle.value() * chip_travel(bounds);
        paint_chip(ctx, chip_rect(bounds, offset), theme);
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }
}

/// Frame interval statistics, in milliseconds.
#[derive(Debug, Clone, Default)]
pub(super) struct FrameIntervals {
    samples: VecDeque<f64>,
}

impl FrameIntervals {
    /// Record a frame `delta` seconds after the previous one. The first frame
    /// of a run reports no interval.
    pub(super) fn record(&mut self, delta: f64) {
        if delta <= 0.0 {
            return;
        }
        if self.samples.len() == PACING_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(delta * 1000.0);
    }

    pub(super) fn average(&self) -> Option<f64> {
        (!self.samples.is_empty())
            .then(|| self.samples.iter().sum::<f64>() / self.samples.len() as f64)
    }

    pub(super) fn worst(&self) -> Option<f64> {
        self.samples.iter().copied().reduce(f64::max)
    }

    /// The typical interval. When frames are paced by the display, this is
    /// its refresh period.
    pub(super) fn median(&self) -> Option<f64> {
        let mut sorted = self.samples.iter().copied().collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        sorted.get(sorted.len() / 2).copied()
    }

    pub(super) fn summary(&self) -> String {
        match (self.median(), self.average(), self.worst()) {
            (Some(median), Some(average), Some(worst)) => format!(
                "typically {median:.1} ms ({:.0} Hz) · average {average:.1} ms · worst {worst:.1} ms",
                1000.0 / median
            ),
            _ => "waiting for frames".to_string(),
        }
    }
}

struct FramePacingGraph {
    theme_reader: DevThemeReader,
    intervals: FrameIntervals,
    last_summary: f64,
}

impl FramePacingGraph {
    fn new(theme_reader: DevThemeReader) -> Self {
        Self {
            theme_reader,
            intervals: FrameIntervals::default(),
            last_summary: 0.0,
        }
    }
}

impl Widget for FramePacingGraph {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Wake(WakeEvent::AnimationFrame { time, delta, .. }) = event {
            self.intervals.record(*delta);
            ctx.request_paint();
            // The accessible summary changes every frame; refresh it now and
            // then rather than rebuilding semantics at the frame rate.
            if *time - self.last_summary >= 0.5 {
                self.last_summary = *time;
                ctx.request_semantics();
            }
            ctx.request_animation_frame();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        ctx.request_animation_frame();
        super::fill_width(constraints, STAGE_HEIGHT + 30.0)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        paint_card(ctx, bounds, theme);
        draw_text(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 16.0,
                bounds.y() + 14.0,
                bounds.width() - 32.0,
                20.0,
            ),
            "Frame intervals",
            DemoTextRole::CardTitle,
            palette.text,
        );
        draw_text(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 16.0,
                bounds.y() + 36.0,
                bounds.width() - 32.0,
                18.0,
            ),
            &self.intervals.summary(),
            DemoTextRole::Metadata,
            palette.text_muted,
        );

        // Bars for the last intervals against 120 Hz and 60 Hz guides, on a
        // scale up to 33 ms.
        let chart = Rect::new(
            bounds.x() + 16.0,
            bounds.y() + 62.0,
            bounds.width() - 32.0,
            bounds.height() - 78.0,
        );
        ctx.fill(Path::rounded_rect(chart, 8.0), palette.field);
        let plot = chart.inflate(-8.0, -8.0);
        let scale = 100.0 / 3.0;
        let y_of = |millis: f64| plot.max_y() - plot.height() * (millis / scale).min(1.0) as f32;
        for (millis, label) in [(1000.0 / 120.0, "120 Hz"), (1000.0 / 60.0, "60 Hz")] {
            let y = y_of(millis);
            hairline(
                ctx,
                Point::new(plot.x(), y),
                Point::new(plot.max_x() - 44.0, y),
                palette.border,
            );
            draw_text(
                ctx,
                theme,
                Rect::new(plot.max_x() - 40.0, y - 9.0, 40.0, 18.0),
                label,
                DemoTextRole::Metadata,
                palette.text_muted,
            );
        }
        let bar_width = (plot.width() - 48.0) / PACING_SAMPLES as f32;
        for (index, millis) in self.intervals.samples.iter().enumerate() {
            let top = y_of(*millis);
            let color = if *millis > 1000.0 / 50.0 {
                palette.warning
            } else {
                palette.accent
            };
            ctx.fill_rect(
                Rect::new(
                    plot.x() + index as f32 * bar_width,
                    top,
                    (bar_width - 1.0).max(1.0),
                    plot.max_y() - top,
                ),
                color,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(FRAME_PACING_NAME.to_string());
        node.value = Some(SemanticsValue::Text(self.intervals.summary()));
        ctx.push(node);
    }
}
