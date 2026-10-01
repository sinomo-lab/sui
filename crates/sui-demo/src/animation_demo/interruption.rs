//! Interruptible motion: the same retargets played with the old
//! restart-from-rest behavior and with momentum, plus a puck that springs
//! home carrying the speed it was flung with.

use std::{cell::Cell, collections::VecDeque, rc::Rc};

use sui::prelude::*;
use sui::{
    AnimationSpec, MotionValue, PointerEventKind, SemanticsNode, SemanticsRole, SemanticsValue,
    SpringF32, SpringSpec, Transition, Vector, motion_policy,
};

use super::{MotionDemoState, draw_text, hairline, paint_card, stroke_polyline};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style_when};

pub(crate) const INTERRUPTION_COMPARISON_NAME: &str = "Interruption comparison";
pub(crate) const FLING_PAD_NAME: &str = "Fling pad";
pub(crate) const FLING_SPRING_NAME: &str = "Fling spring";

/// The comparison's transition, long enough that retargets land mid-flight.
const COMPARISON_DURATION: f64 = 0.5;
/// Seconds between retargets, cycling, at full speed: some land mid-flight,
/// some after the value has settled.
const RETARGET_GAPS: [f64; 5] = [0.26, 0.3, 0.75, 0.22, 0.9];
/// Seconds of history in the comparison's charts, at full speed.
const HISTORY_SECONDS: f64 = 2.6;
const FLING_SPRINGS: [(&str, SpringSpec); 3] = [
    ("Smooth", SpringSpec::SMOOTH),
    ("Snappy", SpringSpec::SNAPPY),
    ("Bouncy", SpringSpec::BOUNCY),
];
const PUCK_RADIUS: f32 = 18.0;
const FLING_TRAIL: usize = 18;

pub(super) fn section(state: MotionDemoState, theme_reader: DevThemeReader) -> impl Widget {
    let spring_choice = Rc::new(Cell::new(2_usize));
    let selected = Rc::clone(&spring_choice);
    let change = Rc::clone(&spring_choice);
    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start)
        .with_item(
            InterruptionComparison::new(Rc::clone(&theme_reader)),
            FlexItem::new()
                .basis_gap_aware_fraction(0.62)
                .min_width(460.0),
        )
        .with_item(
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Flex::horizontal()
                        .gap(10.0)
                        .align_items(Alignment::Center)
                        .with_item(
                            Label::new("Drag the puck and let go").text_style_when(
                                demo_text_style_when(
                                    &theme_reader,
                                    DemoTextRole::Supporting,
                                    |theme| theme.palette.text_muted,
                                ),
                            ),
                            FlexItem::flex(1.0),
                        )
                        .with_child(
                            SegmentedControl::new(FLING_SPRING_NAME)
                                .segments(FLING_SPRINGS.map(|(label, _)| label))
                                .selected_when(move || Some(selected.get()))
                                .theme_when(clone_dev_theme_reader(&theme_reader))
                                .on_change(move |index, _| change.set(index)),
                        ),
                )
                .with_child(FlingPad::new(state, spring_choice, theme_reader)),
            FlexItem::new()
                .basis_gap_aware_fraction(0.38)
                .min_width(300.0),
        )
}

/// The same target changes applied two ways: restarting a transition from
/// the current value at rest (how widgets behaved before), and blending from
/// the running motion with [`MotionValue`].
#[derive(Debug, Clone)]
pub(super) struct RetargetPair {
    target: f32,
    restart: Transition<f32>,
    momentum: MotionValue<f32>,
    next_retarget: f64,
    gap_index: usize,
    history: VecDeque<(f64, f32, f32)>,
    now: f64,
}

impl RetargetPair {
    pub(super) fn new() -> Self {
        Self {
            target: 0.0,
            restart: Transition::new(0.0, 0.0, 0.0, 0.0, Easing::EaseInOut),
            momentum: MotionValue::new(0.0),
            next_retarget: 0.0,
            gap_index: 0,
            history: VecDeque::new(),
            now: 0.0,
        }
    }

    fn spec() -> AnimationSpec {
        AnimationSpec::tween(COMPARISON_DURATION, Easing::EaseInOut)
            .with_movement_policy(motion_policy())
    }

    /// Send both values toward the other end at `time`.
    pub(super) fn retarget(&mut self, time: f64) {
        let spec = Self::spec();
        self.target = 1.0 - self.target;
        let current = self.restart.sample(time);
        self.restart = Transition::new(
            current,
            self.target,
            time,
            spec.duration(),
            Easing::EaseInOut,
        );
        self.momentum.animate_to(self.target, time, spec);
    }

    /// Advance to `time`, retargeting on the cycle of gaps.
    pub(super) fn advance(&mut self, time: f64) {
        self.now = time;
        let time_scale = f64::from(motion_policy().time_scale());
        if time >= self.next_retarget {
            self.retarget(time);
            self.gap_index = (self.gap_index + 1) % RETARGET_GAPS.len();
            self.next_retarget = time + RETARGET_GAPS[self.gap_index] / time_scale;
        }
        self.momentum.advance(time);
        let (restart, momentum) = self.values();
        self.history.push_back((time, restart, momentum));
        let window = HISTORY_SECONDS / time_scale;
        while self
            .history
            .front()
            .is_some_and(|(sampled, _, _)| time - sampled > window)
        {
            self.history.pop_front();
        }
    }

    /// The restart-from-rest value and the momentum-keeping value.
    pub(super) fn values(&self) -> (f32, f32) {
        self.values_at(self.now)
    }

    /// Both values at `time`, without advancing.
    pub(super) fn values_at(&self, time: f64) -> (f32, f32) {
        (self.restart.sample(time), self.momentum.value(time))
    }

    fn history_window(&self) -> f64 {
        HISTORY_SECONDS / f64::from(motion_policy().time_scale())
    }
}

struct InterruptionComparison {
    theme_reader: DevThemeReader,
    pair: RetargetPair,
}

impl InterruptionComparison {
    fn new(theme_reader: DevThemeReader) -> Self {
        Self {
            theme_reader,
            pair: RetargetPair::new(),
        }
    }

    fn paint_lane(
        &self,
        ctx: &mut PaintCtx,
        theme: DefaultTheme,
        area: Rect,
        title: &str,
        detail: &str,
        value_of: fn(&(f64, f32, f32)) -> f32,
        color: Color,
    ) {
        let palette = theme.palette;
        draw_text(
            ctx,
            theme,
            Rect::new(area.x(), area.y(), area.width(), 20.0),
            title,
            DemoTextRole::CardTitle,
            palette.text,
        );
        draw_text(
            ctx,
            theme,
            Rect::new(area.x(), area.y() + 22.0, area.width(), 18.0),
            detail,
            DemoTextRole::Metadata,
            palette.text_muted,
        );

        // The value over the last few seconds, newest on the right.
        let chart = Rect::new(
            area.x(),
            area.y() + 48.0,
            area.width() - 64.0,
            area.height() - 48.0,
        );
        ctx.fill(Path::rounded_rect(chart, 6.0), palette.field);
        let plot = chart.inflate(-6.0, -8.0);
        let guide = palette.border.with_alpha(0.8);
        hairline(
            ctx,
            Point::new(plot.x(), plot.y()),
            Point::new(plot.max_x(), plot.y()),
            guide,
        );
        hairline(
            ctx,
            Point::new(plot.x(), plot.max_y()),
            Point::new(plot.max_x(), plot.max_y()),
            guide,
        );
        let window = self.pair.history_window();
        let now = self.pair.now;
        let point = |sample: &(f64, f32, f32)| {
            let age = ((now - sample.0) / window).clamp(0.0, 1.0) as f32;
            Point::new(
                plot.max_x() - plot.width() * age,
                plot.max_y() - plot.height() * value_of(sample),
            )
        };
        stroke_polyline(ctx, self.pair.history.iter().map(point), color, 2.0);

        // The value itself, on a vertical track beside the chart.
        let track = Rect::new(chart.max_x() + 30.0, plot.y(), 4.0, plot.height());
        ctx.fill(
            Path::rounded_rect(track, 2.0),
            palette.border.with_alpha(0.8),
        );
        let value = self.pair.history.back().map_or(0.0, value_of);
        ctx.fill(
            Path::circle(
                Point::new(track.x() + 2.0, track.max_y() - track.height() * value),
                9.0,
            ),
            color,
        );
    }
}

impl Widget for InterruptionComparison {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.pair.retarget(ctx.current_time());
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.pair.advance(*time);
                ctx.request_paint();
                ctx.request_animation_frame();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        ctx.request_animation_frame();
        super::fill_width(constraints, 300.0)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        paint_card(ctx, bounds, theme);
        let inner = bounds.inflate(-16.0, -16.0);
        let lane_height = (inner.height() - 16.0) * 0.5;
        self.paint_lane(
            ctx,
            theme,
            Rect::new(inner.x(), inner.y(), inner.width(), lane_height),
            "Restart from rest",
            "Each retarget starts over at zero speed: a visible kink.",
            |sample| sample.1,
            theme.palette.text_muted,
        );
        self.paint_lane(
            ctx,
            theme,
            Rect::new(
                inner.x(),
                inner.y() + lane_height + 16.0,
                inner.width(),
                lane_height,
            ),
            "Keep momentum",
            "The new target blends from the running motion, so speed carries over.",
            |sample| sample.2,
            theme.palette.accent,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(INTERRUPTION_COMPARISON_NAME.to_string());
        node.description =
            Some("Two lanes receive the same retargets; click to retarget now.".to_string());
        ctx.push(node);
    }
}

/// A puck that springs back to the middle of its pad, starting with the
/// velocity it was released at.
#[derive(Debug, Clone)]
pub(super) struct FlingPuck {
    offset: Vector,
    velocity: Vector,
    springs: Option<(SpringF32, SpringF32)>,
    last_move: Option<(f64, Point)>,
}

impl FlingPuck {
    pub(super) fn new() -> Self {
        Self {
            offset: Vector::ZERO,
            velocity: Vector::ZERO,
            springs: None,
            last_move: None,
        }
    }

    pub(super) fn offset(&self) -> Vector {
        self.offset
    }

    pub(super) fn is_springing(&self) -> bool {
        self.springs.is_some()
    }

    /// Start dragging at `position` (the pad's center is the origin).
    pub(super) fn grab(&mut self, time: f64, position: Point) {
        self.springs = None;
        self.velocity = Vector::ZERO;
        self.last_move = Some((time, position));
    }

    /// Follow the pointer, estimating its velocity from recent moves.
    pub(super) fn drag(&mut self, time: f64, position: Point, offset: Vector) {
        if let Some((last_time, last_position)) = self.last_move {
            let dt = (time - last_time) as f32;
            if dt > 1.0e-4 {
                let instant = Vector::new(
                    (position.x - last_position.x) / dt,
                    (position.y - last_position.y) / dt,
                );
                self.velocity = Vector::new(
                    self.velocity.x * 0.3 + instant.x * 0.7,
                    self.velocity.y * 0.3 + instant.y * 0.7,
                );
            }
        }
        self.last_move = Some((time, position));
        self.offset = offset;
    }

    /// Let go: spring home with the drag's velocity. With movement off the
    /// puck jumps home.
    pub(super) fn release(&mut self, spec: SpringSpec) {
        self.last_move = None;
        if !motion_policy().allows_movement() {
            self.offset = Vector::ZERO;
            self.velocity = Vector::ZERO;
            return;
        }
        self.springs = Some((
            SpringF32::from_spec(self.offset.x, spec).with_velocity(self.velocity.x),
            SpringF32::from_spec(self.offset.y, spec).with_velocity(self.velocity.y),
        ));
    }

    /// Advance the springs by `delta` seconds of animation time. Returns
    /// whether the puck is still moving.
    pub(super) fn step(&mut self, delta: f64) -> bool {
        let Some((mut x, mut y)) = self.springs else {
            return false;
        };
        x.step(0.0, delta);
        y.step(0.0, delta);
        self.offset = Vector::new(x.value, y.value);
        if x.settle(0.0) && y.settle(0.0) {
            self.springs = None;
            self.offset = Vector::ZERO;
            return false;
        }
        self.springs = Some((x, y));
        true
    }
}

struct FlingPad {
    state: MotionDemoState,
    spring_choice: Rc<Cell<usize>>,
    theme_reader: DevThemeReader,
    puck: FlingPuck,
    dragging: Option<(u64, Vector)>,
    trail: VecDeque<Vector>,
}

impl FlingPad {
    fn new(
        state: MotionDemoState,
        spring_choice: Rc<Cell<usize>>,
        theme_reader: DevThemeReader,
    ) -> Self {
        Self {
            state,
            spring_choice,
            theme_reader,
            puck: FlingPuck::new(),
            dragging: None,
            trail: VecDeque::with_capacity(FLING_TRAIL),
        }
    }

    fn center(bounds: Rect) -> Point {
        Point::new(
            bounds.x() + bounds.width() * 0.5,
            bounds.y() + bounds.height() * 0.5,
        )
    }

    /// Keep the puck inside the pad.
    fn clamp_offset(bounds: Rect, offset: Vector) -> Vector {
        let reach_x = (bounds.width() * 0.5 - PUCK_RADIUS - 6.0).max(0.0);
        let reach_y = (bounds.height() * 0.5 - PUCK_RADIUS - 6.0).max(0.0);
        Vector::new(
            offset.x.clamp(-reach_x, reach_x),
            offset.y.clamp(-reach_y, reach_y),
        )
    }

    fn record_trail(&mut self) {
        if self.trail.len() == FLING_TRAIL {
            self.trail.pop_front();
        }
        self.trail.push_back(self.puck.offset());
    }
}

impl Widget for FlingPad {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let bounds = ctx.bounds();
        let center = Self::center(bounds);
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Down => {
                let puck = center + self.puck.offset();
                let reach = PUCK_RADIUS + 10.0;
                if (pointer.position.x - puck.x).hypot(pointer.position.y - puck.y) <= reach {
                    let grab =
                        Vector::new(pointer.position.x - puck.x, pointer.position.y - puck.y);
                    self.dragging = Some((pointer.pointer_id, grab));
                    self.puck.grab(ctx.current_time(), pointer.position);
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                if let Some((pointer_id, grab)) = self.dragging
                    && pointer.pointer_id == pointer_id
                {
                    let offset = Vector::new(
                        pointer.position.x - grab.x - center.x,
                        pointer.position.y - grab.y - center.y,
                    );
                    self.puck.drag(
                        ctx.current_time(),
                        pointer.position,
                        Self::clamp_offset(bounds, offset),
                    );
                    self.record_trail();
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Up => {
                if let Some((pointer_id, _)) = self.dragging
                    && pointer.pointer_id == pointer_id
                {
                    self.dragging = None;
                    let (_, spec) = FLING_SPRINGS[self.spring_choice.get().min(2)];
                    self.puck.release(spec);
                    ctx.release_pointer_capture(pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.request_animation_frame();
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let moving = self.puck.step(motion_policy().scale_delta(*delta));
                self.record_trail();
                if moving {
                    ctx.request_animation_frame();
                } else {
                    self.trail.clear();
                    ctx.request_semantics();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        super::fill_width(constraints, 252.0)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        paint_card(ctx, bounds, theme);
        let center = Self::center(bounds);

        // Crosshair at home.
        let guide = palette.border.with_alpha(0.9);
        hairline(
            ctx,
            Point::new(center.x - 14.0, center.y),
            Point::new(center.x + 14.0, center.y),
            guide,
        );
        hairline(
            ctx,
            Point::new(center.x, center.y - 14.0),
            Point::new(center.x, center.y + 14.0),
            guide,
        );

        // A hard fling can carry the spring past the pad's edge; keep the
        // puck on the pad.
        let on_pad = |offset: Vector| center + Self::clamp_offset(bounds, offset);
        if self.state.show_traces() {
            let count = self.trail.len().max(1) as f32;
            for (index, offset) in self.trail.iter().enumerate() {
                let fresh = (index + 1) as f32 / count;
                ctx.fill(
                    Path::circle(on_pad(*offset), PUCK_RADIUS * (0.4 + 0.5 * fresh)),
                    palette.accent.with_alpha(0.18 * fresh),
                );
            }
        }
        let puck = on_pad(self.puck.offset());
        ctx.fill(Path::circle(puck, PUCK_RADIUS), palette.accent);
        if self.dragging.is_some() {
            ctx.stroke(
                Path::circle(puck, PUCK_RADIUS + 3.0),
                palette.border_focus,
                StrokeStyle::new(2.0),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(FLING_PAD_NAME.to_string());
        node.description = Some("Drag the puck and let go; it springs home.".to_string());
        node.value = Some(SemanticsValue::Text(
            if self.puck.is_springing() {
                "springing home"
            } else {
                "at rest"
            }
            .to_string(),
        ));
        ctx.push(node);
    }
}
