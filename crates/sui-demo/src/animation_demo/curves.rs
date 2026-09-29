//! Easing and springs: one looping card per curve, plotting the curve and
//! moving a puck along it. Clicking a card sends the puck back mid-flight.

use std::collections::VecDeque;

use sui::prelude::*;
use sui::{
    AnimationSpec, MotionValue, PointerEventKind, SemanticsNode, SemanticsRole, SpringSpec,
    ThemeMotion, motion_policy,
};

use super::{MotionDemoState, draw_text, hairline, millis, paint_card, stroke_polyline};
use crate::app::{DemoTextRole, DevThemeReader};

const CARD_WIDTH: f32 = 240.0;
const CARD_HEIGHT: f32 = 238.0;
/// How long a puck rests at each end before heading back, at full speed.
const HOLD_SECONDS: f64 = 0.7;
/// Positions kept for the fading trail.
const TRAIL_LENGTH: usize = 14;
/// Vertical range of the plots, wide enough for a bouncy overshoot.
const PLOT_MIN: f32 = -0.12;
const PLOT_MAX: f32 = 1.22;

/// Where a card's motion comes from.
#[derive(Debug, Clone, Copy)]
pub(super) enum Curve {
    /// A theme easing token played over a theme duration token.
    Token {
        name: &'static str,
        use_for: &'static str,
        easing: fn(&ThemeMotion) -> Easing,
        duration: fn(&ThemeMotion) -> f32,
    },
    Spring {
        name: &'static str,
        use_for: &'static str,
        spec: SpringSpec,
    },
}

pub(super) const CURVES: [Curve; 8] = [
    Curve::Token {
        name: "Standard",
        use_for: "Most transitions",
        easing: |motion| motion.easing_standard,
        duration: |motion| motion.duration_slow,
    },
    Curve::Token {
        name: "Emphasized",
        use_for: "Toggles and prominent changes",
        easing: |motion| motion.easing_emphasized,
        duration: |motion| motion.duration_slower,
    },
    Curve::Token {
        name: "Decelerate",
        use_for: "Entering the screen",
        easing: |motion| motion.easing_decelerate,
        duration: |motion| motion.duration_slow,
    },
    Curve::Token {
        name: "Accelerate",
        use_for: "Leaving the screen",
        easing: |motion| motion.easing_accelerate,
        duration: |motion| motion.duration_slow,
    },
    Curve::Token {
        name: "Linear",
        use_for: "Progress and loops",
        easing: |_| Easing::Linear,
        duration: |motion| motion.duration_slow,
    },
    Curve::Spring {
        name: "Smooth spring",
        use_for: "Settles without overshoot",
        spec: SpringSpec::SMOOTH,
    },
    Curve::Spring {
        name: "Snappy spring",
        use_for: "Quick, a hint of overshoot",
        spec: SpringSpec::SNAPPY,
    },
    Curve::Spring {
        name: "Bouncy spring",
        use_for: "Overshoots and settles back",
        spec: SpringSpec::BOUNCY,
    },
];

impl Curve {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Token { name, .. } | Self::Spring { name, .. } => name,
        }
    }

    fn use_for(self) -> &'static str {
        match self {
            Self::Token { use_for, .. } | Self::Spring { use_for, .. } => use_for,
        }
    }

    /// The motion at full speed, before the motion policy applies.
    pub(super) fn spec(self, motion: &ThemeMotion) -> AnimationSpec {
        match self {
            Self::Token {
                easing, duration, ..
            } => AnimationSpec::tween(f64::from(duration(motion)), easing(motion)),
            Self::Spring { spec, .. } => AnimationSpec::spring(spec),
        }
    }

    /// Timing and shape, for the card's subtitle.
    fn detail(self, motion: &ThemeMotion) -> String {
        match self {
            Self::Token {
                easing, duration, ..
            } => format!(
                "{} · {}",
                millis(f64::from(duration(motion))),
                easing_label(easing(motion))
            ),
            Self::Spring { spec, .. } => format!(
                "settles in {} · bounce {}",
                millis(spec.settling_duration()),
                spec.bounce
            ),
        }
    }
}

pub(super) fn easing_label(easing: Easing) -> String {
    match easing {
        Easing::Linear => "linear".to_string(),
        Easing::EaseIn => "ease in".to_string(),
        Easing::EaseOut => "ease out".to_string(),
        Easing::EaseInOut => "ease in-out".to_string(),
        Easing::CubicBezier { x1, y1, x2, y2 } => {
            format!("cubic-bezier({x1}, {y1}, {x2}, {y2})")
        }
    }
}

pub(super) fn gallery(state: MotionDemoState, theme_reader: DevThemeReader) -> impl Widget {
    CURVES.into_iter().fold(
        Flex::horizontal()
            .gap(14.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Start),
        |row, curve| {
            row.with_item(
                CurveCard::new(curve, state.clone(), theme_reader.clone()),
                FlexItem::new()
                    .basis_gap_aware_fraction(0.25)
                    .min_width(CARD_WIDTH),
            )
        },
    )
}

/// A value that shuttles between 0 and 1, resting at each end.
#[derive(Debug, Clone)]
pub(super) struct Shuttle {
    motion: MotionValue<f32>,
    target: f32,
    leg_start: f64,
    rest_until: Option<f64>,
    now: f64,
}

impl Shuttle {
    pub(super) fn new() -> Self {
        Self {
            motion: MotionValue::new(0.0),
            target: 0.0,
            leg_start: 0.0,
            // Set off on the first frame.
            rest_until: Some(f64::NEG_INFINITY),
            now: 0.0,
        }
    }

    /// Advance to `time`, heading back once the value has rested for `hold`
    /// seconds. `spec` is the motion for the next leg.
    pub(super) fn advance(&mut self, time: f64, spec: AnimationSpec, hold: f64) {
        self.now = time;
        if self.motion.advance(time) {
            return;
        }
        match self.rest_until {
            None => self.rest_until = Some(time + hold),
            Some(until) if time >= until => self.turn(time, spec),
            Some(_) => {}
        }
    }

    /// Head for the other end now, from wherever the value is.
    pub(super) fn turn(&mut self, time: f64, spec: AnimationSpec) {
        self.now = time;
        self.target = 1.0 - self.target;
        self.motion.animate_to(self.target, time, spec);
        self.leg_start = time;
        self.rest_until = None;
    }

    pub(super) fn value(&self) -> f32 {
        self.motion.value(self.now)
    }

    /// Seconds since the current leg started.
    pub(super) fn leg_elapsed(&self) -> f64 {
        (self.now - self.leg_start).max(0.0)
    }

    pub(super) fn heading_up(&self) -> bool {
        self.target >= 0.5
    }
}

struct CurveCard {
    curve: Curve,
    state: MotionDemoState,
    theme_reader: DevThemeReader,
    shuttle: Shuttle,
    trail: VecDeque<f32>,
}

impl CurveCard {
    fn new(curve: Curve, state: MotionDemoState, theme_reader: DevThemeReader) -> Self {
        Self {
            curve,
            state,
            theme_reader,
            shuttle: Shuttle::new(),
            trail: VecDeque::with_capacity(TRAIL_LENGTH),
        }
    }

    /// The next leg's motion under the app's motion policy. The puck moves,
    /// so reduced motion makes it jump.
    fn policy_spec(&self) -> AnimationSpec {
        self.curve
            .spec(&(self.theme_reader)().motion)
            .with_movement_policy(motion_policy())
    }

    fn layout(bounds: Rect) -> CardLayout {
        let inner = bounds.inflate(-14.0, -14.0);
        let plot = Rect::new(inner.x(), inner.y() + 50.0, inner.width(), 108.0);
        let track = Rect::new(
            inner.x() + 10.0,
            plot.max_y() + 26.0,
            inner.width() - 20.0,
            4.0,
        );
        CardLayout { inner, plot, track }
    }
}

struct CardLayout {
    inner: Rect,
    plot: Rect,
    track: Rect,
}

fn plot_point(plot: Rect, x: f32, y: f32) -> Point {
    let y = (y - PLOT_MIN) / (PLOT_MAX - PLOT_MIN);
    Point::new(
        plot.x() + plot.width() * x,
        plot.max_y() - plot.height() * y,
    )
}

impl Widget for CurveCard {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.shuttle.turn(ctx.current_time(), self.policy_spec());
                ctx.request_paint();
                ctx.request_animation_frame();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let hold = HOLD_SECONDS / f64::from(motion_policy().time_scale());
                self.shuttle.advance(*time, self.policy_spec(), hold);
                if self.trail.len() == TRAIL_LENGTH {
                    self.trail.pop_front();
                }
                self.trail.push_back(self.shuttle.value());
                ctx.request_paint();
                ctx.request_animation_frame();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        // Resume looping whenever the card is laid out again, such as after
        // switching back to this tab.
        ctx.request_animation_frame();
        constraints.clamp(Size::new(CARD_WIDTH, CARD_HEIGHT))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        let layout = Self::layout(bounds);
        paint_card(ctx, bounds, theme);

        draw_text(
            ctx,
            theme,
            Rect::new(
                layout.inner.x(),
                layout.inner.y(),
                layout.inner.width(),
                20.0,
            ),
            self.curve.name(),
            DemoTextRole::CardTitle,
            palette.text,
        );
        draw_text(
            ctx,
            theme,
            Rect::new(
                layout.inner.x(),
                layout.inner.y() + 22.0,
                layout.inner.width(),
                18.0,
            ),
            &self.curve.detail(&theme.motion),
            DemoTextRole::Metadata,
            palette.text_muted,
        );

        // The curve: progress over the motion's own duration.
        let plot = layout.plot;
        let guide = palette.border.with_alpha(0.8);
        hairline(
            ctx,
            plot_point(plot, 0.0, 0.0),
            plot_point(plot, 1.0, 0.0),
            guide,
        );
        hairline(
            ctx,
            plot_point(plot, 0.0, 1.0),
            plot_point(plot, 1.0, 1.0),
            guide,
        );
        let spec = self.curve.spec(&theme.motion);
        let duration = spec.duration().max(f64::EPSILON);
        let steps = 64;
        stroke_polyline(
            ctx,
            (0..=steps).map(|step| {
                let x = step as f32 / steps as f32;
                plot_point(plot, x, spec.progress(duration * f64::from(x)))
            }),
            palette.accent.with_alpha(0.9),
            2.0,
        );

        // Where the puck is on the curve. Heading down mirrors the curve, so
        // plot the distance covered instead of the value. A retarget
        // mid-flight leaves the curve: that is the momentum being kept.
        let value = self.shuttle.value();
        let covered = if self.shuttle.heading_up() {
            value
        } else {
            1.0 - value
        };
        let live_duration = self.policy_spec().duration().max(f64::EPSILON);
        let x = (self.shuttle.leg_elapsed() / live_duration).min(1.0) as f32;
        ctx.fill(
            Path::circle(plot_point(plot, x, covered), 4.5),
            palette.accent,
        );

        // The puck on its track, with a fading trail of where it was.
        let track = layout.track;
        ctx.fill(
            Path::rounded_rect(track, 2.0),
            palette.border.with_alpha(0.8),
        );
        let puck_at = |value: f32| {
            Point::new(
                track.x() + track.width() * value,
                track.y() + track.height() * 0.5,
            )
        };
        if self.state.show_traces() {
            let count = self.trail.len().max(1) as f32;
            for (index, value) in self.trail.iter().enumerate() {
                let age = 1.0 - index as f32 / count;
                ctx.fill(
                    Path::circle(puck_at(*value), 9.0),
                    palette.accent.with_alpha(0.22 * (1.0 - age)),
                );
            }
        }
        ctx.fill(Path::circle(puck_at(value), 9.0), palette.accent);

        draw_text(
            ctx,
            theme,
            Rect::new(
                layout.inner.x(),
                layout.inner.max_y() - 18.0,
                layout.inner.width(),
                18.0,
            ),
            self.curve.use_for(),
            DemoTextRole::Metadata,
            palette.text_muted,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(format!("{} curve", self.curve.name()));
        node.description = Some(format!(
            "{}. Click to send the puck back mid-flight.",
            self.curve.use_for()
        ));
        ctx.push(node);
    }
}
