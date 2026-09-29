//! Retained-text, text-editing, and animation benchmark surfaces.

#![forbid(unsafe_code)]

use std::rc::Rc;

use sui::prelude::*;
use sui::{
    PointerEventKind, Rect, SemanticsNode, SemanticsRole, SemanticsValue, TextDirection, TextStyle,
    TextSurface, TextSurfaceOverlayKind, TextSurfaceStyleOverlay, TextSurfaceStyleSpan, Vector,
};
use sui_runtime::{LayerOptions, PaintBoundaryMode};
use sui_scene::{LayerCompositionMode, LayerProperties};

use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style_when};
use crate::demo_support::*;
use crate::live_performance::LivePerformanceRoot;

pub const RETAINED_TEXT_BENCHMARK_TITLE: &str = "SUI Retained Text Scroll Benchmark";
pub const ANIMATION_BENCHMARK_TITLE: &str = "SUI Animation Benchmark";
pub const TEXT_EDITING_BENCHMARK_TITLE: &str = "SUI Text Editing Benchmark";
pub const RETAINED_TEXT_BENCHMARK_SCROLL_NAME: &str = "Retained text benchmark scroll";
pub const RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME: &str =
    "Retained text benchmark vertical scroll bar";

pub const TEXT_EDITING_BENCHMARK_SPLIT_NAME: &str = "Text editing benchmark split";
pub const TEXT_EDITING_BENCHMARK_EDITOR_NAME: &str = "Text editing benchmark editor";
pub const TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME: &str = "Text editing benchmark syntax preview";
pub const ANIMATION_BENCHMARK_RETAINED_NAME: &str = "Animation benchmark retained lane";
pub const ANIMATION_BENCHMARK_REPAINT_NAME: &str = "Animation benchmark repaint lane";
pub const ANIMATION_BENCHMARK_SCALE_NAME: &str = "Animation benchmark scale grid";
pub(crate) const ANIMATION_BENCHMARK_RETAINED_TARGET: &str = "animation-benchmark-retained";
pub(crate) const ANIMATION_BENCHMARK_REPAINT_TARGET: &str = "animation-benchmark-repaint";
pub(crate) const ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX: &str = "animation-benchmark-cell-";
pub(crate) const ANIMATION_BENCHMARK_RADIUS_PATH: &str = "paint.radius";
pub(crate) const ANIMATION_BENCHMARK_ALPHA_PATH: &str = "paint.alpha";
pub(crate) const ANIMATION_BENCHMARK_SCALE_CELLS: usize = 96;
pub(crate) const ANIMATION_BENCHMARK_SCALE_COLUMNS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AnimationBenchmarkRetainedPresentation {
    opacity: f32,
    translation: Vector,
}

impl Default for AnimationBenchmarkRetainedPresentation {
    fn default() -> Self {
        Self {
            opacity: 0.72,
            translation: Vector::new(-24.0, 0.0),
        }
    }
}

impl TimelineBindingSink for AnimationBenchmarkRetainedPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != ANIMATION_BENCHMARK_RETAINED_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.opacity - value).abs() > 0.001;
                self.opacity = value;
                changed
            }
            (AnimationProperty::LayerTranslation, AnimationValue::Vector(value)) => {
                let changed = self.translation != value;
                self.translation = value;
                changed
            }
            _ => false,
        }
    }
}

pub(crate) struct AnimationBenchmarkRetainedLane {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkRetainedPresentation,
}

impl AnimationBenchmarkRetainedLane {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_retained_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkRetainedPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkRetainedLane {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 112.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.10, 0.12, 0.14, 1.0),
        );

        let rail = Rect::new(
            bounds.x() + 38.0,
            bounds.y() + bounds.height() * 0.5 - 3.0,
            bounds.width() - 76.0,
            6.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 3.0),
            Color::rgba(0.42, 0.47, 0.56, 0.40),
        );

        let marker = Rect::new(
            bounds.x() + bounds.width() * 0.5 - 36.0,
            bounds.y() + 28.0,
            72.0,
            44.0,
        );
        ctx.fill(
            Path::rounded_rect(marker, 7.0),
            Color::rgba(0.34, 0.72, 0.88, 0.88),
        );
        ctx.stroke_rect(
            marker,
            Color::rgba(0.86, 0.96, 1.0, 0.78),
            StrokeStyle::new(1.0),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
            .with_opacity(self.presentation.opacity)
            .with_translation(self.presentation.translation)
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_RETAINED_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "opacity {:.2}, x {:.1}",
            self.presentation.opacity, self.presentation.translation.x
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AnimationBenchmarkPaintPresentation {
    fill: Color,
    radius: f32,
    alpha: f32,
}

impl Default for AnimationBenchmarkPaintPresentation {
    fn default() -> Self {
        Self {
            fill: Color::rgba(0.82, 0.33, 0.24, 1.0),
            radius: 18.0,
            alpha: 0.76,
        }
    }
}

impl TimelineBindingSink for AnimationBenchmarkPaintPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != ANIMATION_BENCHMARK_REPAINT_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = self.fill != value;
                self.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_RADIUS_PATH =>
            {
                let value = value.max(3.0);
                let changed = (self.radius - value).abs() > 0.001;
                self.radius = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_ALPHA_PATH =>
            {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.alpha - value).abs() > 0.001;
                self.alpha = value;
                changed
            }
            _ => false,
        }
    }
}

pub(crate) struct AnimationBenchmarkRepaintLane {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkPaintPresentation,
}

impl AnimationBenchmarkRepaintLane {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_repaint_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkPaintPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkRepaintLane {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 136.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.13, 0.12, 0.11, 1.0),
        );

        let lanes = 11;
        for lane in 0..lanes {
            let t = lane as f32 / (lanes - 1) as f32;
            let x = bounds.x() + 38.0 + (bounds.width() - 76.0) * t;
            let y = bounds.y() + bounds.height() * 0.5;
            let radius = self.presentation.radius * (0.56 + 0.045 * lane as f32);
            let alpha = (self.presentation.alpha * (1.0 - t * 0.35)).clamp(0.15, 1.0);
            let color = Color::rgba(
                (self.presentation.fill.red + t * 0.10).min(1.0),
                self.presentation.fill.green,
                (self.presentation.fill.blue + (1.0 - t) * 0.10).min(1.0),
                alpha,
            );
            ctx.fill(Path::circle(Point::new(x, y), radius), color);
            ctx.stroke(
                Path::circle(Point::new(x, y), radius + 3.5),
                Color::rgba(1.0, 1.0, 1.0, 0.20 * alpha),
                StrokeStyle::new(1.0),
            );
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_REPAINT_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "radius {:.1}, alpha {:.2}",
            self.presentation.radius, self.presentation.alpha
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AnimationBenchmarkCellPresentation {
    fill: Color,
    radius: f32,
    alpha: f32,
}

impl Default for AnimationBenchmarkCellPresentation {
    fn default() -> Self {
        Self {
            fill: Color::rgba(0.20, 0.48, 0.86, 1.0),
            radius: 7.0,
            alpha: 0.7,
        }
    }
}

pub(crate) struct AnimationBenchmarkScalePresentation {
    cells: Vec<AnimationBenchmarkCellPresentation>,
}

impl Default for AnimationBenchmarkScalePresentation {
    fn default() -> Self {
        Self {
            cells: vec![
                AnimationBenchmarkCellPresentation::default();
                ANIMATION_BENCHMARK_SCALE_CELLS
            ],
        }
    }
}

impl AnimationBenchmarkScalePresentation {
    fn cell_index(&self, binding: &AnimationBinding) -> Option<usize> {
        let index = binding
            .target
            .as_str()
            .strip_prefix(ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX)?
            .parse::<usize>()
            .ok()?;
        (index < self.cells.len()).then_some(index)
    }
}

impl TimelineBindingSink for AnimationBenchmarkScalePresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        let Some(index) = self.cell_index(binding) else {
            return false;
        };
        let cell = &mut self.cells[index];

        match (&binding.property, value) {
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = cell.fill != value;
                cell.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_RADIUS_PATH =>
            {
                let value = value.max(2.0);
                let changed = (cell.radius - value).abs() > 0.001;
                cell.radius = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == ANIMATION_BENCHMARK_ALPHA_PATH =>
            {
                let value = value.clamp(0.18, 1.0);
                let changed = (cell.alpha - value).abs() > 0.001;
                cell.alpha = value;
                changed
            }
            _ => false,
        }
    }
}

pub(crate) struct AnimationBenchmarkScaleGrid {
    player: TimelinePlayer,
    presentation: AnimationBenchmarkScalePresentation,
}

impl AnimationBenchmarkScaleGrid {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(animation_benchmark_scale_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        let mut presentation = AnimationBenchmarkScalePresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
            ctx.request_animation_frame();
        }
    }
}

impl Widget for AnimationBenchmarkScaleGrid {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(920.0, 296.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill(
            Path::rounded_rect(bounds, 8.0),
            Color::rgba(0.085, 0.095, 0.11, 1.0),
        );

        let rows = ANIMATION_BENCHMARK_SCALE_CELLS / ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let grid = Rect::new(
            bounds.x() + 20.0,
            bounds.y() + 18.0,
            bounds.width() - 40.0,
            bounds.height() - 36.0,
        );
        let cell_width = grid.width() / ANIMATION_BENCHMARK_SCALE_COLUMNS as f32;
        let cell_height = grid.height() / rows as f32;

        for (index, cell) in self.presentation.cells.iter().enumerate() {
            let column = index % ANIMATION_BENCHMARK_SCALE_COLUMNS;
            let row = index / ANIMATION_BENCHMARK_SCALE_COLUMNS;
            let center = Point::new(
                grid.x() + cell_width * (column as f32 + 0.5),
                grid.y() + cell_height * (row as f32 + 0.5),
            );
            let bounds = Rect::new(
                center.x - cell_width * 0.34,
                center.y - cell_height * 0.30,
                cell_width * 0.68,
                cell_height * 0.60,
            );
            ctx.fill(
                Path::rounded_rect(bounds, 5.0),
                Color::rgba(0.14, 0.16, 0.19, 0.92),
            );
            ctx.fill(
                Path::circle(center, cell.radius),
                cell.fill.with_alpha(cell.alpha),
            );
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(ANIMATION_BENCHMARK_SCALE_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "{} animated cells",
            self.presentation.cells.len()
        )));
        ctx.push(node);
    }
}

pub(crate) fn animation_benchmark_retained_timeline() -> Timeline {
    let target = AnimationTargetId::new(ANIMATION_BENCHMARK_RETAINED_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.4).with_clip(
        Clip::new("retained-lane", 0.0, 1.4)
            .with_track(
                Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.44)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Scalar(0.44)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-32.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Vector(Vector::new(32.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Vector(Vector::new(-32.0, 0.0))),
                ]),
            ),
    )
}

pub(crate) fn animation_benchmark_repaint_timeline() -> Timeline {
    let target = AnimationTargetId::new(ANIMATION_BENCHMARK_REPAINT_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.2).with_clip(
        Clip::new("repaint-lane", 0.0, 1.2)
            .with_track(
                Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
                    Keyframe::new(
                        0.0,
                        AnimationValue::Color(Color::rgba(0.86, 0.30, 0.22, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        0.6,
                        AnimationValue::Color(Color::rgba(0.22, 0.66, 0.82, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        1.2,
                        AnimationValue::Color(Color::rgba(0.86, 0.30, 0.22, 1.0)),
                    ),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(ANIMATION_BENCHMARK_RADIUS_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(14.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.6, AnimationValue::Scalar(26.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.2, AnimationValue::Scalar(14.0)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(ANIMATION_BENCHMARK_ALPHA_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.52)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.6, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.2, AnimationValue::Scalar(0.52)),
                ]),
            ),
    )
}

pub(crate) fn animation_benchmark_scale_timeline() -> Timeline {
    let mut clip = Clip::new("scale-grid", 0.0, 1.8);
    for index in 0..ANIMATION_BENCHMARK_SCALE_CELLS {
        let target =
            AnimationTargetId::new(format!("{ANIMATION_BENCHMARK_SCALE_TARGET_PREFIX}{index}"));
        let column = index % ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let row = index / ANIMATION_BENCHMARK_SCALE_COLUMNS;
        let phase = ((column + row) % 6) as f32 / 6.0;
        let low_radius = 4.0 + (index % 5) as f32 * 0.35;
        let high_radius = 9.0 + (index % 7) as f32 * 0.45;
        let cool = Color::rgba(0.16 + phase * 0.16, 0.42 + phase * 0.16, 0.84, 1.0);
        let warm = Color::rgba(0.84, 0.36 + phase * 0.18, 0.20 + phase * 0.18, 1.0);

        clip.push_track(
            Track::new(AnimationBinding::new(
                target.clone(),
                AnimationProperty::Custom(AnimationPropertyPath::new(
                    ANIMATION_BENCHMARK_RADIUS_PATH,
                )),
            ))
            .with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(low_radius))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(0.9, AnimationValue::Scalar(high_radius))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(1.8, AnimationValue::Scalar(low_radius)),
            ]),
        );
        clip.push_track(
            Track::new(AnimationBinding::new(
                target.clone(),
                AnimationProperty::Custom(AnimationPropertyPath::new(
                    ANIMATION_BENCHMARK_ALPHA_PATH,
                )),
            ))
            .with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(0.38 + phase * 0.24))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(0.9, AnimationValue::Scalar(0.82 + phase * 0.14))
                    .with_easing(Easing::EaseInOut),
                Keyframe::new(1.8, AnimationValue::Scalar(0.38 + phase * 0.24)),
            ]),
        );
        clip.push_track(
            Track::new(AnimationBinding::new(target, AnimationProperty::FillColor)).with_keyframes(
                [
                    Keyframe::new(0.0, AnimationValue::Color(cool)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Color(warm)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Color(cool)),
                ],
            ),
        );
    }

    Timeline::new(1.8).with_clip(clip)
}

pub fn build_animation_benchmark() -> impl Widget {
    Padding::all(
        24.0,
        Stack::vertical()
            .spacing(18.0)
            .alignment(Alignment::Stretch)
            .with_child(AnimationBenchmarkRetainedLane::new())
            .with_child(AnimationBenchmarkRepaintLane::new())
            .with_child(AnimationBenchmarkScaleGrid::new()),
    )
}

pub fn build_animation_benchmark_application() -> Application {
    App::new()
        .window(Window::new(ANIMATION_BENCHMARK_TITLE).root(build_animation_benchmark()))
        .into_application()
}

pub fn build_retained_text_benchmark() -> impl Widget {
    build_retained_text_benchmark_with_theme(default_theme_reader())
}

pub fn build_retained_text_benchmark_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    const SECTION_COUNT: usize = 72;
    const PARAGRAPHS_PER_SECTION: usize = 4;

    let scroll_state = ScrollState::new();
    let mut content = Stack::vertical()
        .spacing(18.0)
        .alignment(Alignment::Stretch)
        .with_child(panel(
        "Retained text wall",
        "Focused benchmark surface for measuring text-heavy cached scroll regeneration without the live overlay or mixed control chrome.",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(
                        "The outer scroll view stays retained, the visible content stays dominated by wrapped labels, and the benchmark scrolls through enough sections to keep retained packet rebuilds focused on atlas text payloads.",
                    )
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.38, 0.46, 0.56, 1.0),
                    )),
                ),
            )
            .with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(
                        "Each section deliberately uses several long paragraphs so the per-frame upload delta is shaped by text submission rather than button chrome, icons, or image content.",
                    )
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.42, 0.49, 0.58, 1.0),
                    )),
                ),
            ),
    ));

    for section_index in 0..SECTION_COUNT {
        let (title, subtitle) = retained_text_benchmark_section(section_index);
        let mut body = Stack::vertical().spacing(8.0).alignment(Alignment::Stretch);

        for paragraph_index in 0..PARAGRAPHS_PER_SECTION {
            body = body.with_child(
                SizedBox::new().width(900.0).with_child(
                    Label::new(retained_text_benchmark_paragraph(
                        section_index,
                        paragraph_index,
                    ))
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.36, 0.44, 0.53, 1.0),
                    )),
                ),
            );
        }

        content = content.with_child(Background::new(
            Color::rgba(0.985, 0.99, 1.0, 1.0),
            Padding::all(
                18.0,
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(Label::new(title).style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::SectionTitle,
                        |_| Color::rgba(0.11, 0.15, 0.21, 1.0),
                    )))
                    .with_child(Label::new(subtitle).style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Body,
                        |_| Color::rgba(0.44, 0.51, 0.60, 1.0),
                    )))
                    .with_child(body),
            ),
        ));
    }

    VerticalScrollPane::new(
        ScrollView::vertical(Padding::all(
            24.0,
            SizedBox::new().width(948.0).with_child(content),
        ))
        .state(scroll_state.clone())
        .overlay_scroll_bars(false)
        .name(RETAINED_TEXT_BENCHMARK_SCROLL_NAME),
        ScrollBar::vertical(scroll_state)
            .name(RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
}

pub fn build_retained_text_benchmark_application() -> Application {
    App::new()
        .window(Window::new(RETAINED_TEXT_BENCHMARK_TITLE).root(build_retained_text_benchmark()))
        .into_application()
}

pub fn build_text_editing_benchmark() -> impl Widget {
    build_text_editing_benchmark_with_theme(default_theme_reader())
}

pub fn build_text_editing_benchmark_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let editor_document = text_editing_benchmark_document();
    let editor_style_spans = text_editing_benchmark_style_spans(&editor_document);
    let editor_style_overlays = text_editing_benchmark_style_overlays(&editor_document);
    let editor_panel = panel_with_theme(
        Rc::clone(&theme_reader),
        "Editable styled code surface",
        "Benchmark typing, selection, IME preedit, wheel scrolling, and syntax-overlay churn against one long text surface.",
        SizedBox::new().width(560.0).height(700.0).with_child(
            TextSurface::new(TEXT_EDITING_BENCHMARK_EDITOR_NAME)
                .value(editor_document)
                .direction(TextDirection::LeftToRight)
                .min_width(560.0)
                .min_height(700.0)
                .style_spans(editor_style_spans)
                .style_overlays(editor_style_overlays)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .text_style_when(|theme| {
                    theme_mono_text_style(theme, theme.text.sm, theme.palette.text)
                }),
        ),
    );
    let syntax_panel = panel_with_theme(
        Rc::clone(&theme_reader),
        "Syntax-highlight preview",
        "A scrollable code-preview column keeps the benchmark honest about syntax-color churn instead of measuring only plain-text editing.",
        SizedBox::new().width(520.0).height(700.0).with_child(
            build_text_editing_syntax_preview_with_theme(Rc::clone(&theme_reader)),
        ),
    );

    Padding::all(
        24.0,
        SplitView::horizontal(editor_panel, syntax_panel)
            .name(TEXT_EDITING_BENCHMARK_SPLIT_NAME)
            .ratio(0.54)
            .min_first(420.0)
            .min_second(360.0)
            .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
}

pub fn build_text_editing_benchmark_application() -> Application {
    App::new()
        .window(Window::new(TEXT_EDITING_BENCHMARK_TITLE).root(
            LivePerformanceRoot::new(
                TEXT_EDITING_BENCHMARK_TITLE,
                "Focused benchmark surface for editor-style typing, selection, scrolling, and syntax-highlight preview cost.",
                build_text_editing_benchmark(),
            ),
        ))
        .into_application()
}

pub(crate) fn retained_text_benchmark_section(section_index: usize) -> (String, String) {
    const THEMES: [(&str, &str); 6] = [
        (
            "Atlas residency",
            "Repeated prose keeps the retained packet mix biased toward atlas glyph work.",
        ),
        (
            "Viewport churn",
            "Small scroll deltas expose new wrapped lines while leaving most state unchanged.",
        ),
        (
            "Packet rebuilds",
            "Retained packets should stay text-heavy instead of expanding glyph quads into generic geometry.",
        ),
        (
            "Glyph density",
            "Wide paragraphs keep each visible retained surface loaded with enough glyph instances to show byte deltas clearly.",
        ),
        (
            "Cache locality",
            "Stable content and repeated vocabulary encourage glyph atlas reuse after the initial warmup.",
        ),
        (
            "Scroll pacing",
            "No-vsync harness runs isolate renderer prep and upload cost from present back-pressure.",
        ),
    ];

    let (topic, subtitle) = THEMES[section_index % THEMES.len()];
    (
        format!("Section {:02} · {topic}", section_index + 1),
        subtitle.to_string(),
    )
}

pub(crate) fn retained_text_benchmark_paragraph(
    section_index: usize,
    paragraph_index: usize,
) -> String {
    const OPENERS: [&str; 6] = [
        "Atlas uploads should now track per-glyph instance payloads instead of six transient vertices per shaped glyph.",
        "This retained scroll surface keeps the scene composition simple so upload accounting is easier to read.",
        "Visible paragraphs change a little on each wheel tick, which keeps retained packet rebuilds centered on text.",
        "Repeated headings and body copy help stabilize glyph atlas misses after the initial scroll warmup.",
        "The benchmark is intentionally prose-heavy because text submission is the renderer path under inspection.",
        "Scroll delta size is fixed so frame samples stay comparable across runs and git revisions.",
    ];
    const DETAILS: [&str; 6] = [
        "Long wrapped lines are useful here because they raise glyph count without introducing extra widget complexity.",
        "Retained caches should still avoid rebuilding unrelated packets while the scroll layer reveals fresh text bands.",
        "Frame summaries can then compare upload bytes, glyph counts, and timing without guessing how much non-text work leaked into the sample.",
        "The same prose appears in varied combinations so the atlas can reuse cached glyph shapes while the instance buffer still changes per frame.",
        "This also mirrors the next line-window phase, where large text surfaces should only submit visible lines to the renderer.",
        "Running the harness with vsync disabled keeps the benchmark focused on renderer cost rather than swapchain pacing.",
    ];

    let opener = OPENERS[(section_index + paragraph_index) % OPENERS.len()];
    let detail = DETAILS[(section_index * 3 + paragraph_index) % DETAILS.len()];
    let cadence = 12 + ((section_index + paragraph_index) % 9);
    let packet_hint = 4 + ((section_index * 5 + paragraph_index) % 7);

    format!(
        "Section {:02}, paragraph {}. {} {} The visible cadence in this sample targets about {} wrapped lines per viewport slice, while adjacent retained packets typically contribute around {} neighboring text blocks before the next wheel event moves the window again.",
        section_index + 1,
        paragraph_index + 1,
        opener,
        detail,
        cadence,
        packet_hint,
    )
}

pub(crate) fn text_editing_benchmark_document() -> String {
    let mut lines = Vec::new();
    lines.push("// Text editing benchmark: long code-like document with mixed comments and repeated glyph traffic".to_string());
    lines.push("mod editor_benchmark {".to_string());
    for index in 0..240 {
        let indent = if index % 6 == 0 { "        " } else { "    " };
        let keyword = ["let", "if", "match", "while", "for", "return"][index % 6];
        let symbol = [
            "shape_visible_window",
            "apply_incremental_edit",
            "measure_selection_overlay",
            "resolve_fallback_face",
            "update_syntax_cache",
            "record_scroll_sample",
        ][(index * 3) % 6];
        let comment = [
            "// atlas reuse should stay warm 🙂",
            "// bidi note: abc אבג 123 مرحبا",
            "// syntax colors keep changing across the preview pane",
            "// fallback sample includes Ж, 中, and नमस्ते in comments",
            "// selection overlays should repaint locally",
            "// retained packets should not rebuild unrelated code blocks",
        ][(index * 5) % 6];
        lines.push(format!(
            "{indent}{keyword} row_{index:03} = {symbol}(cursor + {delta}, viewport_height - {trim}); {comment}",
            delta = 3 + (index % 17),
            trim = 1 + (index % 7),
        ));
        if index % 8 == 7 {
            lines.push(format!(
                "        // folded section {:02}: syntax_color = accent::{:?}; ime = \"候補{}\";",
                (index / 8) + 1,
                ["Keyword", "Type", "Comment", "Number"][index % 4],
                index
            ));
        }
    }
    lines.push("}".to_string());
    lines.join("\n")
}

pub(crate) fn text_editing_benchmark_style_spans(document: &str) -> Vec<TextSurfaceStyleSpan> {
    let keyword_style = text_editing_benchmark_span_style(Color::rgba(0.78, 0.34, 0.16, 1.0));
    let symbol_style = text_editing_benchmark_span_style(Color::rgba(0.09, 0.43, 0.58, 1.0));
    let string_style = text_editing_benchmark_span_style(Color::rgba(0.42, 0.32, 0.74, 1.0));
    let comment_style = text_editing_benchmark_span_style(Color::rgba(0.36, 0.45, 0.25, 1.0));
    let number_style = text_editing_benchmark_span_style(Color::rgba(0.14, 0.49, 0.24, 1.0));
    let keywords = ["mod", "let", "if", "match", "while", "for", "return"];
    let symbols = [
        "editor_benchmark",
        "shape_visible_window",
        "apply_incremental_edit",
        "measure_selection_overlay",
        "resolve_fallback_face",
        "update_syntax_cache",
        "record_scroll_sample",
    ];
    let mut spans = Vec::new();
    let mut line_offset = 0usize;

    for line_with_break in document.split_inclusive('\n') {
        let line = line_with_break
            .strip_suffix('\n')
            .unwrap_or(line_with_break);
        let comment_start = line.find("//");
        let code_end = comment_start.unwrap_or(line.len());

        for keyword in keywords {
            collect_text_editing_word_spans(
                &mut spans,
                line_offset,
                &line[..code_end],
                keyword,
                keyword_style.clone(),
            );
        }
        for symbol in symbols {
            collect_text_editing_word_spans(
                &mut spans,
                line_offset,
                &line[..code_end],
                symbol,
                symbol_style.clone(),
            );
        }
        collect_text_editing_number_spans(
            &mut spans,
            line_offset,
            &line[..code_end],
            number_style.clone(),
        );
        collect_text_editing_string_spans(
            &mut spans,
            line_offset,
            &line[..code_end],
            string_style.clone(),
        );
        if let Some(comment_start) = comment_start {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + comment_start..line_offset + line.len(),
                comment_style.clone(),
            ));
        }

        line_offset += line_with_break.len();
    }

    spans
}

pub(crate) fn text_editing_benchmark_style_overlays(
    document: &str,
) -> Vec<TextSurfaceStyleOverlay> {
    let search_style = text_editing_benchmark_span_style(Color::rgba(0.08, 0.38, 0.72, 1.0));
    let diagnostic_style = text_editing_benchmark_span_style(Color::rgba(0.70, 0.14, 0.20, 1.0));
    let rich_preview_style = text_editing_benchmark_span_style(Color::rgba(0.46, 0.23, 0.66, 1.0));
    let mut overlays = Vec::new();

    collect_text_editing_overlays(
        &mut overlays,
        document,
        "shape_visible_window",
        TextSurfaceOverlayKind::SearchMatch,
        search_style,
    );
    collect_text_editing_overlays(
        &mut overlays,
        document,
        "fallback",
        TextSurfaceOverlayKind::Diagnostic,
        diagnostic_style,
    );
    collect_text_editing_overlays(
        &mut overlays,
        document,
        "🙂",
        TextSurfaceOverlayKind::RichTextPreview,
        rich_preview_style,
    );

    overlays
}

pub(crate) fn text_editing_benchmark_span_style(color: Color) -> TextStyle {
    let text = DefaultTheme::default().text;
    mono_text_style(text.sm, color)
}

pub(crate) fn collect_text_editing_word_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    word: &str,
    style: TextStyle,
) {
    let mut search_offset = 0usize;
    while let Some(relative_start) = line[search_offset..].find(word) {
        let start = search_offset + relative_start;
        let end = start + word.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if text_editing_word_boundary(before) && text_editing_word_boundary(after) {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + start..line_offset + end,
                style.clone(),
            ));
        }
        search_offset = end;
    }
}

pub(crate) fn collect_text_editing_number_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    style: TextStyle,
) {
    let mut span_start = None;
    for (index, ch) in line.char_indices() {
        let number_char = ch.is_ascii_digit() || ch == '.';
        match (span_start, number_char) {
            (None, true) => span_start = Some(index),
            (Some(start), false) => {
                spans.push(TextSurfaceStyleSpan::new(
                    line_offset + start..line_offset + index,
                    style.clone(),
                ));
                span_start = None;
            }
            _ => {}
        }
    }

    if let Some(start) = span_start {
        spans.push(TextSurfaceStyleSpan::new(
            line_offset + start..line_offset + line.len(),
            style,
        ));
    }
}

pub(crate) fn collect_text_editing_string_spans(
    spans: &mut Vec<TextSurfaceStyleSpan>,
    line_offset: usize,
    line: &str,
    style: TextStyle,
) {
    let mut string_start = None;
    for (index, ch) in line.char_indices() {
        if ch != '"' {
            continue;
        }
        if let Some(start) = string_start.take() {
            spans.push(TextSurfaceStyleSpan::new(
                line_offset + start..line_offset + index + ch.len_utf8(),
                style.clone(),
            ));
        } else {
            string_start = Some(index);
        }
    }
}

pub(crate) fn collect_text_editing_overlays(
    overlays: &mut Vec<TextSurfaceStyleOverlay>,
    document: &str,
    needle: &str,
    kind: TextSurfaceOverlayKind,
    style: TextStyle,
) {
    let mut search_offset = 0usize;
    while let Some(relative_start) = document[search_offset..].find(needle) {
        let start = search_offset + relative_start;
        let end = start + needle.len();
        overlays.push(TextSurfaceStyleOverlay::new(
            start..end,
            style.clone(),
            kind.clone(),
        ));
        search_offset = end;
    }
}

pub(crate) fn text_editing_word_boundary(ch: Option<char>) -> bool {
    match ch {
        Some(ch) => !ch.is_alphanumeric() && ch != '_',
        None => true,
    }
}

pub(crate) fn build_text_editing_syntax_preview_with_theme(
    theme_reader: DevThemeReader,
) -> impl Widget {
    let theme = theme_reader();
    let (document, style_spans) = text_editing_syntax_preview_content(theme);

    TextSurface::new(TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME)
        .value(document)
        .read_only()
        .min_width(520.0)
        .min_height(700.0)
        .style_spans(style_spans)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .text_style_when(|theme| theme_mono_text_style(theme, theme.text.sm, theme.palette.text))
}

pub(crate) fn text_editing_syntax_preview_content(
    theme: DefaultTheme,
) -> (String, Vec<TextSurfaceStyleSpan>) {
    let text = theme_mono_text_style(theme, theme.text.sm, theme.palette.text);
    let muted = theme_mono_text_style(theme, theme.text.sm, theme.palette.text_muted);
    let accent = theme_mono_text_style(theme, theme.text.sm, theme.palette.accent);
    let success = theme_mono_text_style(theme, theme.text.sm, theme.palette.success);
    let mut document = String::new();
    let mut spans = Vec::with_capacity(220 * 8);

    for line_index in 0..220 {
        let keyword = ["fn", "let", "match", "if", "while", "return"][line_index % 6];
        let type_name =
            ["Editor", "Glyphs", "Select", "Syntax", "Window", "Frame"][(line_index * 7) % 6];
        let method =
            ["shape", "cache", "cursor", "paint", "fallback", "commit"][(line_index * 11) % 6];
        let color_name = ["keyword", "type", "comment", "number"][line_index % 4];

        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("{:>3} ", line_index + 1),
            &muted,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("{keyword} "),
            &accent,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("sample_{line_index:03}"),
            &text,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!(": {type_name}"),
            &muted,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!(" = {method}("),
            &text,
        );
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("{:.2}", 0.5 + ((line_index % 17) as f32 * 0.125)),
            &success,
        );
        push_text_editing_syntax_segment(&mut document, &mut spans, "); ", &text);
        push_text_editing_syntax_segment(
            &mut document,
            &mut spans,
            &format!("// {color_name} glyph {}", line_index % 9),
            &muted,
        );
        if line_index + 1 < 220 {
            document.push('\n');
        }
    }

    (document, spans)
}

pub(crate) fn push_text_editing_syntax_segment(
    document: &mut String,
    spans: &mut Vec<TextSurfaceStyleSpan>,
    segment: &str,
    style: &TextStyle,
) {
    let start = document.len();
    document.push_str(segment);
    spans.push(TextSurfaceStyleSpan::new(
        start..document.len(),
        style.clone(),
    ));
}

#[cfg(test)]
mod tests;
