#![forbid(unsafe_code)]

use std::{fmt, sync::Arc};

use sui_core::{Color, ColorSpace, Point, Rect, Size, Transform, Vector};
pub use sui_core::{MotionPolicy, MotionPreference};

pub const ANIMATION_DOCUMENT_VERSION: u32 = 1;

pub trait Interpolate: Sized {
    fn interpolate(from: Self, to: Self, t: f32) -> Self;

    /// Like [`Interpolate::interpolate`], but lets `t` leave `0..=1` so a
    /// spring can overshoot its target. Values that cannot extend past their
    /// endpoints, such as colors, clamp instead.
    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        Self::interpolate(from, to, t)
    }
}

impl Interpolate for f32 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Self::extrapolate(from, to, t.clamp(0.0, 1.0))
    }

    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        // Endpoint-exact: `from + (to - from) * 1.0` accumulates float error
        // and lands one bit away from `to`, so settled animations would never
        // exactly reach their target value.
        if t == 0.0 {
            return from;
        }
        if t == 1.0 {
            return to;
        }
        from + ((to - from) * t)
    }
}

impl Interpolate for Point {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Self::extrapolate(from, to, t.clamp(0.0, 1.0))
    }

    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        Point::new(
            f32::extrapolate(from.x, to.x, t),
            f32::extrapolate(from.y, to.y, t),
        )
    }
}

impl Interpolate for Vector {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Self::extrapolate(from, to, t.clamp(0.0, 1.0))
    }

    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        Vector::new(
            f32::extrapolate(from.x, to.x, t),
            f32::extrapolate(from.y, to.y, t),
        )
    }
}

impl Interpolate for Size {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Self::extrapolate(from, to, t.clamp(0.0, 1.0))
    }

    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        Size::new(
            f32::extrapolate(from.width, to.width, t).max(0.0),
            f32::extrapolate(from.height, to.height, t).max(0.0),
        )
    }
}

impl Interpolate for Rect {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Self::extrapolate(from, to, t.clamp(0.0, 1.0))
    }

    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        Rect::from_origin_size(
            Point::extrapolate(from.origin, to.origin, t),
            Size::extrapolate(from.size, to.size, t),
        )
    }
}

/// Blends the translation, rotation, scale, and shear of two transforms
/// separately, the way CSS interpolates matrices. Blending the raw matrix
/// entries would shrink a rotating shape halfway through its turn. Rotation
/// takes the shorter way around. Degenerate transforms (zero scale) cannot be
/// decomposed and fall back to blending the matrix entries.
impl Interpolate for Transform {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Self::extrapolate(from, to, t.clamp(0.0, 1.0))
    }

    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        if t == 0.0 || from == to {
            return from;
        }
        if t == 1.0 {
            return to;
        }
        match (DecomposedTransform::new(from), DecomposedTransform::new(to)) {
            (Some(start), Some(end)) => start.extrapolate(end, t).compose(),
            _ => Transform::new(
                f32::extrapolate(from.xx, to.xx, t),
                f32::extrapolate(from.yx, to.yx, t),
                f32::extrapolate(from.xy, to.xy, t),
                f32::extrapolate(from.yy, to.yy, t),
                f32::extrapolate(from.dx, to.dx, t),
                f32::extrapolate(from.dy, to.dy, t),
            ),
        }
    }
}

/// An affine transform split as `translate * rotate * [[scale_x, shear],
/// [0, scale_y]]`. A mirrored transform has a negative `scale_y`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DecomposedTransform {
    translation: Vector,
    rotation: f32,
    scale_x: f32,
    scale_y: f32,
    shear: f32,
}

impl DecomposedTransform {
    fn new(transform: Transform) -> Option<Self> {
        let scale_x = transform.xx.hypot(transform.yx);
        if !scale_x.is_finite() || scale_x <= f32::EPSILON {
            return None;
        }
        let rotation = transform.yx.atan2(transform.xx);
        let (sin, cos) = rotation.sin_cos();
        Some(Self {
            translation: Vector::new(transform.dx, transform.dy),
            rotation,
            scale_x,
            scale_y: (cos * transform.yy) - (sin * transform.xy),
            shear: (cos * transform.xy) + (sin * transform.yy),
        })
    }

    fn extrapolate(self, to: Self, t: f32) -> Self {
        let turn = (to.rotation - self.rotation + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        Self {
            translation: Vector::extrapolate(self.translation, to.translation, t),
            rotation: self.rotation + turn * t,
            scale_x: f32::extrapolate(self.scale_x, to.scale_x, t),
            scale_y: f32::extrapolate(self.scale_y, to.scale_y, t),
            shear: f32::extrapolate(self.shear, to.shear, t),
        }
    }

    fn compose(self) -> Transform {
        let (sin, cos) = self.rotation.sin_cos();
        Transform::new(
            cos * self.scale_x,
            sin * self.scale_x,
            (cos * self.shear) - (sin * self.scale_y),
            (sin * self.shear) + (cos * self.scale_y),
            self.translation.x,
            self.translation.y,
        )
    }
}

/// Colors blend in premultiplied OKLab (see [`Color::mix_oklab`]): mixes are
/// perceptually even, and fades from a transparent color keep their hue
/// instead of passing through a dark fringe.
impl Interpolate for Color {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        from.mix_oklab(to, t)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
}

impl Easing {
    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - ((-2.0 * t + 2.0).powi(2) * 0.5)
                }
            }
            Self::CubicBezier { x1, y1, x2, y2 } => sample_cubic_bezier(x1, y1, x2, y2, t),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition<T> {
    pub start: T,
    pub end: T,
    pub start_time: f64,
    pub duration: f64,
    pub easing: Easing,
}

impl<T> Transition<T>
where
    T: Copy + Interpolate,
{
    pub fn new(start: T, end: T, start_time: f64, duration: f64, easing: Easing) -> Self {
        Self {
            start,
            end,
            start_time,
            duration: duration.max(0.0),
            easing,
        }
    }

    pub fn progress(&self, time: f64) -> f32 {
        if self.duration <= f64::EPSILON {
            return 1.0;
        }
        let elapsed = time - self.start_time;
        // Snap to completion with a hair of tolerance: durations often
        // originate as f32 theme tokens, so a fixed-step advance can land
        // float dust short of `duration` and would otherwise sample at
        // t = 1 - ε, leaving settled states one bit away from their target.
        if elapsed >= self.duration * (1.0 - 1e-6) {
            return 1.0;
        }
        (elapsed / self.duration).clamp(0.0, 1.0) as f32
    }

    pub fn sample(&self, time: f64) -> T {
        T::interpolate(
            self.start,
            self.end,
            self.easing.sample(self.progress(time)),
        )
    }

    pub fn is_complete(&self, time: f64) -> bool {
        self.progress(time) >= 1.0
    }
}

/// A spring described the way designers tune one: how long it takes to
/// settle, and how much it bounces.
///
/// `duration` is the period of the undamped spring in seconds, which is
/// close to how long the motion reads as taking. `bounce` runs from `-1` to
/// `1`: `0` settles as fast as possible without overshooting, positive values
/// overshoot and oscillate, and negative values approach more gently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringSpec {
    pub duration: f32,
    pub bounce: f32,
}

impl SpringSpec {
    /// Settles without overshoot.
    pub const SMOOTH: Self = Self::new(0.35, 0.0);
    /// Quick, with a hint of overshoot.
    pub const SNAPPY: Self = Self::new(0.3, 0.15);
    /// Visibly overshoots and settles back.
    pub const BOUNCY: Self = Self::new(0.45, 0.4);

    /// How far from rest, as a share of the distance travelled, a spring
    /// counts as settled.
    pub const REST_THRESHOLD: f64 = 1.0e-3;

    pub const fn new(duration: f32, bounce: f32) -> Self {
        Self { duration, bounce }
    }

    /// The spec matching a unit-mass spring with this `stiffness` and
    /// `damping`.
    pub fn from_physics(stiffness: f32, damping: f32) -> Self {
        let stiffness = stiffness.max(f32::EPSILON);
        let ratio = damping.max(0.0) / (2.0 * stiffness.sqrt());
        let bounce = if ratio <= 1.0 {
            1.0 - ratio
        } else {
            1.0 / ratio - 1.0
        };
        Self::new(std::f32::consts::TAU / stiffness.sqrt(), bounce)
    }

    fn resolved_duration(self) -> f64 {
        f64::from(self.duration).max(1.0e-3)
    }

    /// Bounce is kept short of `1`, which would oscillate forever.
    fn resolved_bounce(self) -> f64 {
        f64::from(self.bounce).clamp(-0.95, 0.95)
    }

    /// Stiffness of the equivalent unit-mass spring.
    pub fn stiffness(self) -> f32 {
        self.physics().0 as f32
    }

    /// Damping of the equivalent unit-mass spring.
    pub fn damping(self) -> f32 {
        self.physics().1 as f32
    }

    /// The damping ratio: below 1 oscillates, 1 is critically damped.
    pub fn damping_ratio(self) -> f32 {
        self.resolved_damping_ratio() as f32
    }

    fn resolved_damping_ratio(self) -> f64 {
        let bounce = self.resolved_bounce();
        if bounce >= 0.0 {
            1.0 - bounce
        } else {
            1.0 / (1.0 + bounce)
        }
    }

    fn physics(self) -> (f64, f64) {
        let angular = std::f64::consts::TAU / self.resolved_duration();
        let stiffness = angular * angular;
        let damping = 2.0 * self.resolved_damping_ratio() * angular;
        (stiffness, damping)
    }

    /// Progress from 0 to 1 after `elapsed` seconds of a spring released
    /// from rest. Bouncy springs overshoot past 1 before settling.
    pub fn progress(self, elapsed: f64) -> f32 {
        if elapsed <= 0.0 {
            return 0.0;
        }
        if elapsed >= self.settling_duration() {
            return 1.0;
        }
        let (stiffness, damping) = self.physics();
        let (displacement, _) = spring_state(stiffness, damping, 1.0, 0.0, elapsed);
        (1.0 - displacement) as f32
    }

    /// Seconds until a spring released from rest stays within
    /// [`SpringSpec::REST_THRESHOLD`] of its target.
    pub fn settling_duration(self) -> f64 {
        let (stiffness, damping) = self.physics();
        let ratio = self.resolved_damping_ratio();
        let angular = stiffness.sqrt();
        if ratio < 1.0 - 1.0e-4 {
            // The oscillation stays inside an exponential envelope, whose
            // amplitude for a release from rest is 1 / sqrt(1 - ratio^2).
            let amplitude = 1.0 / (1.0 - ratio * ratio).sqrt();
            return (amplitude / Self::REST_THRESHOLD).ln() / (ratio * angular);
        }
        // Critically damped and overdamped springs released from rest approach
        // the target without crossing it, so the first time within the
        // threshold is final.
        let within =
            |t: f64| spring_state(stiffness, damping, 1.0, 0.0, t).0 <= Self::REST_THRESHOLD;
        let mut high = 1.0 / angular;
        while !within(high) && high < 600.0 {
            high *= 2.0;
        }
        let mut low = 0.0;
        for _ in 0..40 {
            let middle = (low + high) * 0.5;
            if within(middle) {
                high = middle;
            } else {
                low = middle;
            }
        }
        high
    }
}

impl Default for SpringSpec {
    fn default() -> Self {
        Self::SMOOTH
    }
}

/// Displacement and velocity of a unit-mass damped spring `elapsed` seconds
/// after it started at `displacement` with `velocity`. This is the exact
/// solution, so it is stable and gives the same motion at any frame rate.
fn spring_state(
    stiffness: f64,
    damping: f64,
    displacement: f64,
    velocity: f64,
    elapsed: f64,
) -> (f64, f64) {
    let t = elapsed.max(0.0);
    if stiffness <= f64::EPSILON {
        // No spring: damping alone slows the initial velocity.
        if damping <= f64::EPSILON {
            return (displacement + velocity * t, velocity);
        }
        let decay = (-damping * t).exp();
        return (
            displacement + velocity * (1.0 - decay) / damping,
            velocity * decay,
        );
    }

    let angular = stiffness.sqrt();
    let ratio = damping / (2.0 * angular);
    if (ratio - 1.0).abs() <= 1.0e-4 {
        // Critically damped.
        let slope = velocity + angular * displacement;
        let decay = (-angular * t).exp();
        (
            decay * (displacement + slope * t),
            decay * (velocity - angular * slope * t),
        )
    } else if ratio < 1.0 {
        // Underdamped: a decaying oscillation.
        let damped = angular * (1.0 - ratio * ratio).sqrt();
        let decay_rate = ratio * angular;
        let sine_weight = (velocity + decay_rate * displacement) / damped;
        let decay = (-decay_rate * t).exp();
        let (sin, cos) = (damped * t).sin_cos();
        let position = decay * (displacement * cos + sine_weight * sin);
        let slope = decay
            * ((sine_weight * damped - decay_rate * displacement) * cos
                - (displacement * damped + decay_rate * sine_weight) * sin);
        (position, slope)
    } else {
        // Overdamped: two decaying exponentials.
        let root = (ratio * ratio - 1.0).sqrt();
        let fast = -angular * (ratio + root);
        let slow = -angular * (ratio - root);
        let fast_weight = (velocity - slow * displacement) / (fast - slow);
        let slow_weight = displacement - fast_weight;
        let (fast_decay, slow_decay) = ((fast * t).exp(), (slow * t).exp());
        (
            fast_weight * fast_decay + slow_weight * slow_decay,
            fast * fast_weight * fast_decay + slow * slow_weight * slow_decay,
        )
    }
}

/// A spring that follows a moving target, stepped by elapsed time. Use it for
/// physics-driven motion, such as a dragged handle that springs home carrying
/// the velocity of the fling; for transitions between states prefer
/// [`MotionValue`] with an [`AnimationSpec::Spring`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringF32 {
    pub value: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringF32 {
    /// Distance from the target below which [`SpringF32::is_settled`] reports
    /// the spring at rest; the speed limit is ten times this per second.
    pub const REST_EPSILON: f32 = 1.0e-3;

    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            stiffness: 180.0,
            damping: 24.0,
        }
    }

    /// A spring at rest at `value` with the stiffness and damping of `spec`.
    pub fn from_spec(value: f32, spec: SpringSpec) -> Self {
        Self::new(value).with_spec(spec)
    }

    pub fn with_config(mut self, stiffness: f32, damping: f32) -> Self {
        self.stiffness = stiffness.max(0.0);
        self.damping = damping.max(0.0);
        self
    }

    pub fn with_spec(self, spec: SpringSpec) -> Self {
        self.with_config(spec.stiffness(), spec.damping())
    }

    pub fn with_velocity(mut self, velocity: f32) -> Self {
        self.velocity = velocity;
        self
    }

    /// Advance the spring by `delta` seconds toward `target` and return the
    /// new value. The step is exact, so large or uneven deltas neither
    /// destabilize the spring nor change its path.
    pub fn step(&mut self, target: f32, delta: f64) -> f32 {
        let dt = delta.max(0.0);
        if dt <= f64::EPSILON {
            return self.value;
        }

        let (displacement, velocity) = spring_state(
            f64::from(self.stiffness),
            f64::from(self.damping),
            f64::from(self.value - target),
            f64::from(self.velocity),
            dt,
        );
        self.value = target + displacement as f32;
        self.velocity = velocity as f32;
        self.value
    }

    /// Whether the spring rests at `target`.
    pub fn is_settled(&self, target: f32) -> bool {
        (self.value - target).abs() <= Self::REST_EPSILON
            && self.velocity.abs() <= Self::REST_EPSILON * 10.0
    }

    /// Snap to `target` once the spring has settled there. Returns whether it
    /// is at rest.
    pub fn settle(&mut self, target: f32) -> bool {
        if self.is_settled(target) {
            self.value = target;
            self.velocity = 0.0;
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blink {
    pub period: f64,
    pub duty_cycle: f32,
    pub phase: f64,
}

impl Blink {
    pub fn new(period: f64) -> Self {
        Self {
            period: period.max(f64::EPSILON),
            duty_cycle: 0.5,
            phase: 0.0,
        }
    }

    pub fn with_duty_cycle(mut self, duty_cycle: f32) -> Self {
        self.duty_cycle = duty_cycle.clamp(0.0, 1.0);
        self
    }

    pub fn with_phase(mut self, phase: f64) -> Self {
        self.phase = phase;
        self
    }

    pub fn is_on(&self, time: f64) -> bool {
        let cycle = ((time + self.phase).rem_euclid(self.period)) / self.period;
        cycle < self.duty_cycle as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pulse {
    pub period: f64,
    pub min: f32,
    pub max: f32,
    pub phase: f64,
    pub easing: Easing,
}

impl Pulse {
    pub fn new(period: f64, min: f32, max: f32) -> Self {
        Self {
            period: period.max(f64::EPSILON),
            min,
            max,
            phase: 0.0,
            easing: Easing::EaseInOut,
        }
    }

    pub fn with_phase(mut self, phase: f64) -> Self {
        self.phase = phase;
        self
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn sample(&self, time: f64) -> f32 {
        let cycle = ((time + self.phase).rem_euclid(self.period)) / self.period;
        let triangle = if cycle <= 0.5 {
            (cycle * 2.0) as f32
        } else {
            ((1.0 - cycle) * 2.0) as f32
        };
        f32::interpolate(self.min, self.max, self.easing.sample(triangle))
    }
}

/// How a value travels to a new target: a timed curve or a spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationSpec {
    /// Follow `easing` for `duration` seconds.
    Tween { duration: f64, easing: Easing },
    /// Move like a spring released from rest; see [`SpringSpec`].
    Spring(SpringSpec),
}

impl AnimationSpec {
    /// Jump straight to the target.
    pub const INSTANT: Self = Self::tween(0.0, Easing::Linear);

    pub const fn tween(duration: f64, easing: Easing) -> Self {
        Self::Tween { duration, easing }
    }

    pub const fn spring(spec: SpringSpec) -> Self {
        Self::Spring(spec)
    }

    /// Seconds until the value arrives: the tween's duration, or how long the
    /// spring takes to settle.
    pub fn duration(self) -> f64 {
        match self {
            Self::Tween { duration, .. } => duration.max(0.0),
            Self::Spring(spring) => spring.settling_duration(),
        }
    }

    /// Whether the animation finishes immediately.
    pub fn is_instant(self) -> bool {
        match self {
            Self::Tween { duration, .. } => duration <= f64::EPSILON,
            Self::Spring(_) => false,
        }
    }

    /// Progress toward the target after `elapsed` seconds: 0 at the start and
    /// exactly 1 once complete. Springs may overshoot past 1 on the way.
    pub fn progress(self, elapsed: f64) -> f32 {
        match self {
            Self::Tween { duration, easing } => {
                if self.is_complete(elapsed) {
                    return 1.0;
                }
                easing.sample((elapsed / duration).clamp(0.0, 1.0) as f32)
            }
            Self::Spring(spring) => spring.progress(elapsed),
        }
    }

    pub fn is_complete(self, elapsed: f64) -> bool {
        match self {
            // Snap to completion with a hair of tolerance: durations often
            // originate as f32 theme tokens, so a fixed-step advance can land
            // float dust short of `duration`.
            Self::Tween { duration, .. } => {
                duration <= f64::EPSILON || elapsed >= duration * (1.0 - 1e-6)
            }
            Self::Spring(spring) => elapsed >= spring.settling_duration(),
        }
    }

    /// The same motion played at `time_scale` speed: `0.5` takes twice as long.
    pub fn time_scaled(self, time_scale: f32) -> Self {
        let time_scale = MotionPolicy::FULL.with_time_scale(time_scale).time_scale();
        match self {
            Self::Tween { duration, easing } => Self::Tween {
                duration: duration / f64::from(time_scale),
                easing,
            },
            Self::Spring(spring) => Self::Spring(SpringSpec {
                duration: spring.duration / time_scale,
                ..spring
            }),
        }
    }

    /// The motion to play under `policy`: instant when motion is off,
    /// stretched by the policy's time scale otherwise.
    pub fn with_policy(self, policy: MotionPolicy) -> Self {
        if policy.allows_motion() {
            self.time_scaled(policy.time_scale())
        } else {
            Self::INSTANT
        }
    }

    /// Like [`AnimationSpec::with_policy`], for motion that moves content:
    /// instant unless the policy allows movement.
    pub fn with_movement_policy(self, policy: MotionPolicy) -> Self {
        if policy.allows_movement() {
            self.with_policy(policy)
        } else {
            Self::INSTANT
        }
    }
}

impl From<SpringSpec> for AnimationSpec {
    fn from(spring: SpringSpec) -> Self {
        Self::Spring(spring)
    }
}

/// How many overlapping retargets a [`MotionValue`] blends before folding the
/// oldest into its starting point.
const MOTION_SEGMENTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
struct MotionSegment<T> {
    target: T,
    start_time: f64,
    spec: AnimationSpec,
}

impl<T> MotionSegment<T> {
    fn progress(&self, time: f64) -> f32 {
        self.spec.progress(time - self.start_time)
    }

    fn is_complete(&self, time: f64) -> bool {
        self.spec.is_complete(time - self.start_time)
    }
}

/// A value that animates toward its latest target on an absolute clock.
///
/// Retargeting mid-flight keeps the motion's momentum: the new animation
/// blends from the still-running previous one instead of restarting from a
/// standstill, so a hover that ends halfway through its fade-in turns around
/// smoothly rather than stopping dead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionValue<T> {
    base: T,
    segments: [Option<MotionSegment<T>>; MOTION_SEGMENTS],
}

impl<T: Copy> MotionValue<T> {
    pub const fn new(value: T) -> Self {
        Self {
            base: value,
            segments: [None; MOTION_SEGMENTS],
        }
    }

    /// The value this motion is heading to.
    pub fn target(&self) -> T {
        self.segments
            .iter()
            .rev()
            .flatten()
            .next()
            .map_or(self.base, |segment| segment.target)
    }

    pub fn is_animating(&self) -> bool {
        self.segments[0].is_some()
    }

    /// Stop animating and rest at `value`.
    pub fn jump_to(&mut self, value: T) {
        self.base = value;
        self.segments = [None; MOTION_SEGMENTS];
    }

    fn segment_count(&self) -> usize {
        self.segments.iter().flatten().count()
    }
}

impl<T> MotionValue<T>
where
    T: Interpolate + Copy + PartialEq,
{
    /// The value at `time`.
    pub fn value(&self, time: f64) -> T {
        self.segments
            .iter()
            .flatten()
            .fold(self.base, |value, segment| {
                T::extrapolate(value, segment.target, segment.progress(time))
            })
    }

    /// Start animating from the value at `time` toward `target`. Returns
    /// whether the value is animating afterwards.
    pub fn animate_to(&mut self, target: T, time: f64, spec: AnimationSpec) -> bool {
        self.advance(time);
        if target == self.target() {
            return self.is_animating();
        }
        if spec.is_instant() {
            self.jump_to(target);
            return false;
        }

        if self.segment_count() == MOTION_SEGMENTS {
            // Fold the oldest animation into the starting point where it
            // stands now. The value stays continuous; only that animation's
            // remaining drift is dropped.
            if let Some(oldest) = self.segments[0] {
                self.base = T::extrapolate(self.base, oldest.target, oldest.progress(time));
            }
            self.segments.rotate_left(1);
            self.segments[MOTION_SEGMENTS - 1] = None;
        }
        let slot = self.segment_count();
        self.segments[slot] = Some(MotionSegment {
            target,
            start_time: time,
            spec,
        });
        true
    }

    /// When the running animations will all have finished, or `None` at rest.
    pub fn end_time(&self) -> Option<f64> {
        self.segments
            .iter()
            .flatten()
            .map(|segment| segment.start_time + segment.spec.duration())
            .reduce(f64::max)
    }

    /// Whether an animation is still running at `time`, without dropping
    /// finished ones.
    pub fn is_animating_at(&self, time: f64) -> bool {
        self.segments
            .iter()
            .flatten()
            .any(|segment| !segment.is_complete(time))
    }

    /// Drop animations that have finished by `time`. Returns whether the
    /// value is still animating.
    pub fn advance(&mut self, time: f64) -> bool {
        // A finished animation pins the value to its target, so it and every
        // animation it was blending from can be dropped.
        let finished = self
            .segments
            .iter()
            .rposition(|segment| segment.is_some_and(|segment| segment.is_complete(time)));
        if let Some(index) = finished {
            if let Some(segment) = self.segments[index] {
                self.base = segment.target;
            }
            self.segments.rotate_left(index + 1);
            for slot in &mut self.segments[MOTION_SEGMENTS - index - 1..] {
                *slot = None;
            }
        }
        self.is_animating()
    }
}

/// A value animated by elapsed time: call [`AnimatedValue::tick`] with each
/// frame's delta. Retargeting keeps momentum like [`MotionValue`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimatedValue<T> {
    motion: MotionValue<T>,
    current: T,
    clock: f64,
    spec: AnimationSpec,
}

impl<T> AnimatedValue<T>
where
    T: Interpolate + Copy + PartialEq,
{
    pub fn new(initial: T) -> Self {
        Self {
            motion: MotionValue::new(initial),
            current: initial,
            clock: 0.0,
            spec: AnimationSpec::tween(0.2, Easing::EaseInOut),
        }
    }

    pub fn with_duration(mut self, seconds: f32) -> Self {
        self.set_duration(seconds);
        self
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.set_easing(easing);
        self
    }

    pub fn with_spec(mut self, spec: AnimationSpec) -> Self {
        self.spec = spec;
        self
    }

    /// Use a tween of `seconds` for later targets, keeping the easing.
    pub fn set_duration(&mut self, seconds: f32) {
        let easing = match self.spec {
            AnimationSpec::Tween { easing, .. } => easing,
            AnimationSpec::Spring(_) => Easing::EaseInOut,
        };
        self.spec = AnimationSpec::tween(f64::from(seconds.max(0.0)), easing);
    }

    /// Use a tween with `easing` for later targets, keeping the duration.
    pub fn set_easing(&mut self, easing: Easing) {
        self.spec = AnimationSpec::tween(self.spec.duration(), easing);
    }

    pub fn set_spec(&mut self, spec: AnimationSpec) {
        self.spec = spec;
    }

    pub fn spec(&self) -> AnimationSpec {
        self.spec
    }

    pub fn set_target(&mut self, target: T) {
        self.motion.animate_to(target, self.clock, self.spec);
        self.current = self.motion.value(self.clock);
    }

    pub fn jump_to(&mut self, value: T) {
        self.motion.jump_to(value);
        self.current = value;
    }

    /// Advance by `delta_seconds`. Returns whether the value is still
    /// animating.
    pub fn tick(&mut self, delta_seconds: f32) -> bool {
        if !self.motion.is_animating() {
            return false;
        }
        self.clock += f64::from(delta_seconds.max(0.0));
        let animating = self.motion.advance(self.clock);
        self.current = self.motion.value(self.clock);
        animating
    }

    pub fn value(&self) -> T {
        self.current
    }

    pub fn target(&self) -> T {
        self.motion.target()
    }

    pub fn is_animating(&self) -> bool {
        self.motion.is_animating()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationTargetId(String);

impl AnimationTargetId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AnimationTargetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for AnimationTargetId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for AnimationTargetId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationPropertyPath(String);

impl AnimationPropertyPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AnimationPropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for AnimationPropertyPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for AnimationPropertyPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnimationProperty {
    LayerOpacity,
    LayerTranslation,
    FillColor,
    Bounds,
    Custom(AnimationPropertyPath),
}

impl AnimationProperty {
    pub fn path(&self) -> &str {
        match self {
            Self::LayerOpacity => "layer.opacity",
            Self::LayerTranslation => "layer.translation",
            Self::FillColor => "fill.color",
            Self::Bounds => "bounds",
            Self::Custom(path) => path.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationBinding {
    pub target: AnimationTargetId,
    pub property: AnimationProperty,
}

impl AnimationBinding {
    pub fn new(target: impl Into<AnimationTargetId>, property: AnimationProperty) -> Self {
        Self {
            target: target.into(),
            property,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationValueKind {
    Scalar,
    Point,
    Vector,
    Size,
    Rect,
    Color,
    Transform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationValue {
    Scalar(f32),
    Point(Point),
    Vector(Vector),
    Size(Size),
    Rect(Rect),
    Color(Color),
    Transform(Transform),
}

impl AnimationValue {
    pub fn kind(self) -> AnimationValueKind {
        match self {
            Self::Scalar(_) => AnimationValueKind::Scalar,
            Self::Point(_) => AnimationValueKind::Point,
            Self::Vector(_) => AnimationValueKind::Vector,
            Self::Size(_) => AnimationValueKind::Size,
            Self::Rect(_) => AnimationValueKind::Rect,
            Self::Color(_) => AnimationValueKind::Color,
            Self::Transform(_) => AnimationValueKind::Transform,
        }
    }

    pub fn as_scalar(self) -> Option<f32> {
        match self {
            Self::Scalar(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_vector(self) -> Option<Vector> {
        match self {
            Self::Vector(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_color(self) -> Option<Color> {
        match self {
            Self::Color(value) => Some(value),
            _ => None,
        }
    }
}

impl Interpolate for AnimationValue {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Self::extrapolate(from, to, t.clamp(0.0, 1.0))
    }

    fn extrapolate(from: Self, to: Self, t: f32) -> Self {
        match (from, to) {
            (Self::Scalar(from), Self::Scalar(to)) => Self::Scalar(f32::extrapolate(from, to, t)),
            (Self::Point(from), Self::Point(to)) => Self::Point(Point::extrapolate(from, to, t)),
            (Self::Vector(from), Self::Vector(to)) => {
                Self::Vector(Vector::extrapolate(from, to, t))
            }
            (Self::Size(from), Self::Size(to)) => Self::Size(Size::extrapolate(from, to, t)),
            (Self::Rect(from), Self::Rect(to)) => Self::Rect(Rect::extrapolate(from, to, t)),
            (Self::Color(from), Self::Color(to)) => Self::Color(Color::extrapolate(from, to, t)),
            (Self::Transform(from), Self::Transform(to)) => {
                Self::Transform(Transform::extrapolate(from, to, t))
            }
            (from, to) => {
                if t >= 1.0 {
                    to
                } else {
                    from
                }
            }
        }
    }
}

impl From<f32> for AnimationValue {
    fn from(value: f32) -> Self {
        Self::Scalar(value)
    }
}

impl From<Point> for AnimationValue {
    fn from(value: Point) -> Self {
        Self::Point(value)
    }
}

impl From<Vector> for AnimationValue {
    fn from(value: Vector) -> Self {
        Self::Vector(value)
    }
}

impl From<Size> for AnimationValue {
    fn from(value: Size) -> Self {
        Self::Size(value)
    }
}

impl From<Rect> for AnimationValue {
    fn from(value: Rect) -> Self {
        Self::Rect(value)
    }
}

impl From<Color> for AnimationValue {
    fn from(value: Color) -> Self {
        Self::Color(value)
    }
}

impl From<Transform> for AnimationValue {
    fn from(value: Transform) -> Self {
        Self::Transform(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe<T> {
    pub time: f64,
    pub value: T,
    pub easing: Easing,
}

impl<T> Keyframe<T> {
    pub fn new(time: f64, value: T) -> Self {
        Self {
            time: time.max(0.0),
            value,
            easing: Easing::Linear,
        }
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track<T = AnimationValue> {
    pub binding: AnimationBinding,
    pub keyframes: Vec<Keyframe<T>>,
    pub enabled: bool,
}

impl<T> Track<T> {
    pub fn new(binding: AnimationBinding) -> Self {
        Self {
            binding,
            keyframes: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_keyframes(mut self, keyframes: impl IntoIterator<Item = Keyframe<T>>) -> Self {
        self.keyframes.extend(keyframes);
        self
    }

    pub fn push_keyframe(&mut self, keyframe: Keyframe<T>) {
        self.keyframes.push(keyframe);
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl<T> Track<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Option<T> {
        if !self.enabled || self.keyframes.is_empty() {
            return None;
        }

        let mut first = &self.keyframes[0];
        let mut last = &self.keyframes[0];
        let mut previous = None;
        let mut next = None;

        for keyframe in &self.keyframes {
            if keyframe.time < first.time {
                first = keyframe;
            }
            if keyframe.time > last.time {
                last = keyframe;
            }
            if keyframe.time <= time
                && previous
                    .map(|candidate: &Keyframe<T>| keyframe.time >= candidate.time)
                    .unwrap_or(true)
            {
                previous = Some(keyframe);
            }
            if keyframe.time >= time
                && next
                    .map(|candidate: &Keyframe<T>| keyframe.time <= candidate.time)
                    .unwrap_or(true)
            {
                next = Some(keyframe);
            }
        }

        let Some(previous) = previous else {
            return Some(first.value);
        };
        let Some(next) = next else {
            return Some(last.value);
        };
        if (next.time - previous.time).abs() <= f64::EPSILON {
            return Some(next.value);
        }

        let progress = ((time - previous.time) / (next.time - previous.time)).clamp(0.0, 1.0);
        Some(T::interpolate(
            previous.value,
            next.value,
            previous.easing.sample(progress as f32),
        ))
    }
}

fn sample_sorted_keyframes<T>(keyframes: &[Keyframe<T>], time: f64) -> Option<T>
where
    T: Copy + Interpolate,
{
    let first = keyframes.first()?;
    let last = keyframes.last()?;
    if time <= first.time {
        return Some(first.value);
    }
    if time >= last.time {
        return Some(last.value);
    }

    let next_index = keyframes.partition_point(|keyframe| keyframe.time < time);
    let previous_index = next_index.saturating_sub(1);
    let previous = &keyframes[previous_index];
    let next = &keyframes[next_index];
    if (next.time - previous.time).abs() <= f64::EPSILON {
        return Some(next.value);
    }

    let progress = ((time - previous.time) / (next.time - previous.time)).clamp(0.0, 1.0);
    Some(T::interpolate(
        previous.value,
        next.value,
        previous.easing.sample(progress as f32),
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clip<T = AnimationValue> {
    pub id: String,
    pub start_time: f64,
    pub duration: f64,
    pub tracks: Vec<Track<T>>,
    pub enabled: bool,
}

impl<T> Clip<T> {
    pub fn new(id: impl Into<String>, start_time: f64, duration: f64) -> Self {
        Self {
            id: id.into(),
            start_time: start_time.max(0.0),
            duration: duration.max(0.0),
            tracks: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_track(mut self, track: Track<T>) -> Self {
        self.tracks.push(track);
        self
    }

    pub fn push_track(&mut self, track: Track<T>) {
        self.tracks.push(track);
    }

    pub fn end_time(&self) -> f64 {
        self.start_time + self.duration
    }

    pub fn contains_time(&self, time: f64) -> bool {
        self.enabled && time >= self.start_time && time <= self.end_time()
    }
}

impl<T> Clip<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Vec<SampledAnimationValue<T>> {
        let mut buffer = SampleBuffer::new();
        self.sample_into(time, &mut buffer);
        buffer.into_samples()
    }

    pub fn sample_into<'a>(
        &self,
        time: f64,
        samples: &'a mut SampleBuffer<T>,
    ) -> SampleBatch<'a, T> {
        samples.clear();
        self.append_samples(time, samples.samples_mut());
        samples.batch()
    }

    fn append_samples(&self, time: f64, samples: &mut Vec<SampledAnimationValue<T>>) {
        if !self.contains_time(time) {
            return;
        }

        let local_time = time - self.start_time;
        for track in &self.tracks {
            if let Some(value) = track.sample(local_time) {
                samples.push(SampledAnimationValue {
                    clip_id: self.id.clone(),
                    binding: track.binding.clone(),
                    time,
                    value,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timeline<T = AnimationValue> {
    pub duration: f64,
    pub clips: Vec<Clip<T>>,
}

impl<T> Timeline<T> {
    pub fn new(duration: f64) -> Self {
        Self {
            duration: duration.max(0.0),
            clips: Vec::new(),
        }
    }

    pub fn with_clip(mut self, clip: Clip<T>) -> Self {
        self.clips.push(clip);
        self
    }

    pub fn push_clip(&mut self, clip: Clip<T>) {
        self.clips.push(clip);
    }
}

impl<T> Default for Timeline<T> {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl<T> Timeline<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Vec<SampledAnimationValue<T>> {
        let mut buffer = SampleBuffer::new();
        self.sample_into(time, &mut buffer);
        buffer.into_samples()
    }

    pub fn sample_into<'a>(
        &self,
        time: f64,
        samples: &'a mut SampleBuffer<T>,
    ) -> SampleBatch<'a, T> {
        let clamped_time = time.clamp(0.0, self.duration.max(0.0));
        samples.clear();
        let samples_vec = samples.samples_mut();
        for clip in &self.clips {
            clip.append_samples(clamped_time, samples_vec);
        }
        samples.batch()
    }
}

impl<T> Timeline<T>
where
    T: Clone,
{
    pub fn compile(&self) -> CompiledTimeline<T> {
        CompiledTimeline::from_timeline(self)
    }

    pub fn compile_shared(&self) -> SharedCompiledTimeline<T> {
        self.compile().into_shared()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTimeline<T = AnimationValue> {
    duration: f64,
    clips: Vec<CompiledClip<T>>,
    sample_capacity: usize,
}

pub type SharedCompiledTimeline<T = AnimationValue> = Arc<CompiledTimeline<T>>;

impl<T> CompiledTimeline<T>
where
    T: Clone,
{
    pub fn from_timeline(timeline: &Timeline<T>) -> Self {
        let clips = timeline
            .clips
            .iter()
            .filter(|clip| clip.enabled)
            .filter_map(CompiledClip::from_clip)
            .collect::<Vec<_>>();
        let sample_capacity = clips.iter().map(|clip| clip.tracks.len()).sum();
        Self {
            duration: timeline.duration.max(0.0),
            clips,
            sample_capacity,
        }
    }
}

impl<T> CompiledTimeline<T> {
    pub fn into_shared(self) -> SharedCompiledTimeline<T> {
        Arc::new(self)
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn clips(&self) -> &[CompiledClip<T>] {
        &self.clips
    }

    pub fn sample_capacity(&self) -> usize {
        self.sample_capacity
    }
}

impl<T> CompiledTimeline<T>
where
    T: Clone,
{
    pub fn to_shared(&self) -> SharedCompiledTimeline<T> {
        Arc::new(self.clone())
    }

    pub fn player(&self) -> AnimationPlayer<T> {
        AnimationPlayer::new(self.to_shared())
    }
}

impl<T> CompiledTimeline<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Vec<SampledAnimationValue<T>> {
        let mut buffer = SampleBuffer::with_capacity(self.sample_capacity);
        self.sample_into(time, &mut buffer);
        buffer.into_samples()
    }

    pub fn sample_into<'a>(
        &self,
        time: f64,
        samples: &'a mut SampleBuffer<T>,
    ) -> SampleBatch<'a, T> {
        let clamped_time = time.clamp(0.0, self.duration);
        samples.clear();
        samples.reserve_capacity(self.sample_capacity);
        let samples_vec = samples.samples_mut();
        for clip in &self.clips {
            clip.append_samples(clamped_time, samples_vec);
        }
        samples.batch()
    }
}

impl<T> From<&Timeline<T>> for CompiledTimeline<T>
where
    T: Clone,
{
    fn from(timeline: &Timeline<T>) -> Self {
        Self::from_timeline(timeline)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledClip<T = AnimationValue> {
    id: String,
    start_time: f64,
    duration: f64,
    tracks: Vec<CompiledTrack<T>>,
}

impl<T> CompiledClip<T>
where
    T: Clone,
{
    fn from_clip(clip: &Clip<T>) -> Option<Self> {
        let tracks = clip
            .tracks
            .iter()
            .filter(|track| track.enabled && !track.keyframes.is_empty())
            .map(CompiledTrack::from_track)
            .collect::<Vec<_>>();
        if tracks.is_empty() {
            return None;
        }

        Some(Self {
            id: clip.id.clone(),
            start_time: clip.start_time,
            duration: clip.duration,
            tracks,
        })
    }
}

impl<T> CompiledClip<T> {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn start_time(&self) -> f64 {
        self.start_time
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn end_time(&self) -> f64 {
        self.start_time + self.duration
    }

    pub fn tracks(&self) -> &[CompiledTrack<T>] {
        &self.tracks
    }

    pub fn contains_time(&self, time: f64) -> bool {
        time >= self.start_time && time <= self.end_time()
    }
}

impl<T> CompiledClip<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Vec<SampledAnimationValue<T>> {
        let mut buffer = SampleBuffer::new();
        self.sample_into(time, &mut buffer);
        buffer.into_samples()
    }

    pub fn sample_into<'a>(
        &self,
        time: f64,
        samples: &'a mut SampleBuffer<T>,
    ) -> SampleBatch<'a, T> {
        samples.clear();
        self.append_samples(time, samples.samples_mut());
        samples.batch()
    }

    fn append_samples(&self, time: f64, samples: &mut Vec<SampledAnimationValue<T>>) {
        if !self.contains_time(time) {
            return;
        }

        let local_time = time - self.start_time;
        for track in &self.tracks {
            if let Some(value) = track.sample(local_time) {
                samples.push(SampledAnimationValue {
                    clip_id: self.id.clone(),
                    binding: track.binding.clone(),
                    time,
                    value,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTrack<T = AnimationValue> {
    binding: AnimationBinding,
    keyframes: Vec<Keyframe<T>>,
}

impl<T> CompiledTrack<T>
where
    T: Clone,
{
    fn from_track(track: &Track<T>) -> Self {
        let mut keyframes = track.keyframes.clone();
        keyframes.sort_by(|left, right| left.time.total_cmp(&right.time));
        Self {
            binding: track.binding.clone(),
            keyframes,
        }
    }
}

impl<T> CompiledTrack<T> {
    pub fn binding(&self) -> &AnimationBinding {
        &self.binding
    }

    pub fn keyframes(&self) -> &[Keyframe<T>] {
        &self.keyframes
    }
}

impl<T> CompiledTrack<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Option<T> {
        sample_sorted_keyframes(&self.keyframes, time)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampledAnimationValue<T = AnimationValue> {
    pub clip_id: String,
    pub binding: AnimationBinding,
    pub time: f64,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SampleBuffer<T = AnimationValue> {
    samples: Vec<SampledAnimationValue<T>>,
}

impl<T> SampleBuffer<T> {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.samples.capacity()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn reserve_capacity(&mut self, target_capacity: usize) {
        self.samples
            .reserve(target_capacity.saturating_sub(self.samples.capacity()));
    }

    pub fn samples(&self) -> &[SampledAnimationValue<T>] {
        &self.samples
    }

    pub fn batch(&self) -> SampleBatch<'_, T> {
        SampleBatch {
            samples: &self.samples,
        }
    }

    pub fn into_samples(self) -> Vec<SampledAnimationValue<T>> {
        self.samples
    }

    fn samples_mut(&mut self) -> &mut Vec<SampledAnimationValue<T>> {
        &mut self.samples
    }
}

impl<T> AsRef<[SampledAnimationValue<T>]> for SampleBuffer<T> {
    fn as_ref(&self) -> &[SampledAnimationValue<T>] {
        self.samples()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleBatch<'a, T = AnimationValue> {
    samples: &'a [SampledAnimationValue<T>],
}

impl<'a, T> SampleBatch<'a, T> {
    pub fn samples(self) -> &'a [SampledAnimationValue<T>] {
        self.samples
    }

    pub fn iter(self) -> std::slice::Iter<'a, SampledAnimationValue<T>> {
        self.samples.iter()
    }

    pub fn len(self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(self) -> bool {
        self.samples.is_empty()
    }
}

impl<'a, T> IntoIterator for SampleBatch<'a, T> {
    type Item = &'a SampledAnimationValue<T>;
    type IntoIter = std::slice::Iter<'a, SampledAnimationValue<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopMode {
    #[default]
    Once,
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackState {
    pub playhead: f64,
    pub playback_rate: f64,
    pub playing: bool,
    pub loop_mode: LoopMode,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playhead: 0.0,
            playback_rate: 1.0,
            playing: false,
            loop_mode: LoopMode::Once,
        }
    }
}

impl PlaybackState {
    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.playhead = 0.0;
    }

    pub fn seek(&mut self, time: f64, duration: f64) {
        self.playhead = time.clamp(0.0, duration.max(0.0));
    }

    pub fn tick(&mut self, delta_seconds: f64, duration: f64) -> bool {
        if !self.playing {
            return false;
        }

        let previous_time = self.playhead;
        let duration = duration.max(0.0);
        if duration <= f64::EPSILON {
            self.playhead = 0.0;
            self.playing = false;
            return previous_time != self.playhead;
        }

        self.playhead += delta_seconds.max(0.0) * self.playback_rate;
        if self.playhead > duration {
            match self.loop_mode {
                LoopMode::Once => {
                    self.playhead = duration;
                    self.playing = false;
                }
                LoopMode::Repeat => {
                    self.playhead = self.playhead.rem_euclid(duration);
                }
            }
        } else if self.playhead < 0.0 {
            match self.loop_mode {
                LoopMode::Once => {
                    self.playhead = 0.0;
                    self.playing = false;
                }
                LoopMode::Repeat => {
                    self.playhead = self.playhead.rem_euclid(duration);
                }
            }
        }

        previous_time != self.playhead
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationPlayer<T = AnimationValue> {
    timeline: SharedCompiledTimeline<T>,
    playback: PlaybackState,
}

impl<T> AnimationPlayer<T> {
    pub fn new(timeline: SharedCompiledTimeline<T>) -> Self {
        Self {
            timeline,
            playback: PlaybackState::default(),
        }
    }

    pub fn from_compiled(timeline: CompiledTimeline<T>) -> Self {
        Self::new(timeline.into_shared())
    }

    pub fn timeline(&self) -> &CompiledTimeline<T> {
        self.timeline.as_ref()
    }

    pub fn shared_timeline(&self) -> SharedCompiledTimeline<T> {
        Arc::clone(&self.timeline)
    }

    pub fn playback(&self) -> PlaybackState {
        self.playback
    }

    pub fn playback_mut(&mut self) -> &mut PlaybackState {
        &mut self.playback
    }

    pub fn with_loop_mode(mut self, loop_mode: LoopMode) -> Self {
        self.playback.loop_mode = loop_mode;
        self
    }

    pub fn repeat(self) -> Self {
        self.with_loop_mode(LoopMode::Repeat)
    }

    pub fn once(self) -> Self {
        self.with_loop_mode(LoopMode::Once)
    }

    pub fn with_playback_rate(mut self, playback_rate: f64) -> Self {
        self.playback.playback_rate = playback_rate;
        self
    }

    pub fn play(&mut self) {
        self.playback.play();
    }

    pub fn pause(&mut self) {
        self.playback.pause();
    }

    pub fn stop(&mut self) {
        self.playback.stop();
    }

    pub fn seek(&mut self, time: f64) {
        self.playback.seek(time, self.timeline.duration());
    }
}

impl<T> AnimationPlayer<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self) -> Vec<SampledAnimationValue<T>> {
        self.timeline.sample(self.playback.playhead)
    }

    pub fn sample_into<'a>(&self, samples: &'a mut SampleBuffer<T>) -> SampleBatch<'a, T> {
        self.timeline.sample_into(self.playback.playhead, samples)
    }

    pub fn tick_into<'a>(
        &mut self,
        delta_seconds: f64,
        samples: &'a mut SampleBuffer<T>,
    ) -> AnimationTick<'a, T> {
        let advanced = self.playback.tick(delta_seconds, self.timeline.duration());
        let samples = self.timeline.sample_into(self.playback.playhead, samples);
        AnimationTick {
            samples,
            playback: self.playback,
            advanced,
            should_continue: self.playback.playing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationTick<'a, T = AnimationValue> {
    pub samples: SampleBatch<'a, T>,
    pub playback: PlaybackState,
    pub advanced: bool,
    pub should_continue: bool,
}

impl<'a, T> AnimationTick<'a, T> {
    pub fn samples(self) -> SampleBatch<'a, T> {
        self.samples
    }

    pub fn sample_values(self) -> &'a [SampledAnimationValue<T>] {
        self.samples.samples()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationDocument {
    pub version: u32,
    pub name: String,
    pub timeline: Timeline,
}

impl AnimationDocument {
    pub fn new(name: impl Into<String>, timeline: Timeline) -> Self {
        Self {
            version: ANIMATION_DOCUMENT_VERSION,
            name: name.into(),
            timeline,
        }
    }

    pub fn to_document_format(&self) -> String {
        let mut output = String::new();
        output.push_str("sui-animation-document\t");
        output.push_str(&self.version.to_string());
        output.push('\n');
        output.push_str("name\t");
        output.push_str(&escape_document_field(&self.name));
        output.push('\n');
        output.push_str("duration\t");
        output.push_str(&format_f64(self.timeline.duration));
        output.push('\n');

        for clip in &self.timeline.clips {
            output.push_str("clip\t");
            output.push_str(&escape_document_field(&clip.id));
            output.push('\t');
            output.push_str(&format_f64(clip.start_time));
            output.push('\t');
            output.push_str(&format_f64(clip.duration));
            output.push('\t');
            output.push_str(format_bool(clip.enabled));
            output.push('\n');

            for track in &clip.tracks {
                output.push_str("track\t");
                output.push_str(&escape_document_field(track.binding.target.as_str()));
                output.push('\t');
                output.push_str(&escape_document_field(track.binding.property.path()));
                output.push('\t');
                output.push_str(format_bool(track.enabled));
                output.push('\n');

                for keyframe in &track.keyframes {
                    output.push_str("key\t");
                    output.push_str(&format_f64(keyframe.time));
                    output.push('\t');
                    output.push_str(&format_easing(keyframe.easing));
                    output.push('\t');
                    output.push_str(&format_animation_value(keyframe.value));
                    output.push('\n');
                }

                output.push_str("endtrack\n");
            }

            output.push_str("endclip\n");
        }

        output
    }

    pub fn from_document_format(input: &str) -> Result<Self, AnimationDocumentFormatError> {
        AnimationDocumentFormatParser::new(input).parse()
    }
}

impl Default for AnimationDocument {
    fn default() -> Self {
        Self::new("Untitled animation", Timeline::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationDocumentFormatError {
    pub line: Option<usize>,
    pub message: String,
}

impl AnimationDocumentFormatError {
    fn new(line: Option<usize>, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for AnimationDocumentFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(
                f,
                "animation document format error on line {line}: {}",
                self.message
            ),
            None => write!(f, "animation document format error: {}", self.message),
        }
    }
}

impl std::error::Error for AnimationDocumentFormatError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineSnap {
    pub enabled: bool,
    pub interval: f64,
}

impl TimelineSnap {
    pub fn new(interval: f64) -> Self {
        Self {
            enabled: true,
            interval: interval.max(f64::EPSILON),
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            interval: 1.0 / 60.0,
        }
    }

    pub fn snap_time(self, time: f64) -> f64 {
        if !self.enabled {
            return time.max(0.0);
        }
        ((time.max(0.0) / self.interval).round() * self.interval).max(0.0)
    }
}

impl Default for TimelineSnap {
    fn default() -> Self {
        Self::new(1.0 / 24.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyframeSelection {
    pub clip_index: usize,
    pub track_index: usize,
    pub keyframe_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnimationSelection {
    pub clip_index: Option<usize>,
    pub track_index: Option<usize>,
    pub keyframes: Vec<KeyframeSelection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnimationEditorCommand {
    SetPlayhead(f64),
    SetZoom(f32),
    SetScroll(f32),
    SetSnapping(TimelineSnap),
    ClearSelection,
    SelectClip(usize),
    SelectTrack {
        clip_index: usize,
        track_index: usize,
    },
    SelectKeyframe(KeyframeSelection),
    AddKeyframe {
        clip_index: usize,
        track_index: usize,
        keyframe: Keyframe<AnimationValue>,
    },
    UpdateKeyframeEasing {
        selection: KeyframeSelection,
        easing: Easing,
    },
    /// Move a keyframe to `time`, snapped and kept inside the timeline.
    MoveKeyframe {
        selection: KeyframeSelection,
        time: f64,
    },
    RemoveKeyframe(KeyframeSelection),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEditorState {
    pub document: AnimationDocument,
    pub playback: PlaybackState,
    pub selection: AnimationSelection,
    pub zoom: f32,
    pub scroll: f32,
    pub snap: TimelineSnap,
    undo_stack: Vec<AnimationDocument>,
    redo_stack: Vec<AnimationDocument>,
}

impl AnimationEditorState {
    pub fn new(document: AnimationDocument) -> Self {
        Self {
            document,
            playback: PlaybackState::default(),
            selection: AnimationSelection::default(),
            zoom: 1.0,
            scroll: 0.0,
            snap: TimelineSnap::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn apply_command(&mut self, command: AnimationEditorCommand) -> bool {
        match command {
            AnimationEditorCommand::SetPlayhead(time) => {
                self.playback.seek(time, self.document.timeline.duration);
                true
            }
            AnimationEditorCommand::SetZoom(zoom) => {
                self.zoom = zoom.max(0.05);
                true
            }
            AnimationEditorCommand::SetScroll(scroll) => {
                self.scroll = scroll.max(0.0);
                true
            }
            AnimationEditorCommand::SetSnapping(snap) => {
                self.snap = snap;
                true
            }
            AnimationEditorCommand::ClearSelection => {
                self.selection = AnimationSelection::default();
                true
            }
            AnimationEditorCommand::SelectClip(clip_index) => {
                self.selection.clip_index = Some(clip_index);
                self.selection.track_index = None;
                self.selection.keyframes.clear();
                true
            }
            AnimationEditorCommand::SelectTrack {
                clip_index,
                track_index,
            } => {
                self.selection.clip_index = Some(clip_index);
                self.selection.track_index = Some(track_index);
                self.selection.keyframes.clear();
                true
            }
            AnimationEditorCommand::SelectKeyframe(selection) => {
                self.selection.clip_index = Some(selection.clip_index);
                self.selection.track_index = Some(selection.track_index);
                if !self.selection.keyframes.contains(&selection) {
                    self.selection.keyframes.push(selection);
                }
                true
            }
            AnimationEditorCommand::AddKeyframe {
                clip_index,
                track_index,
                mut keyframe,
            } => {
                let Some(track) = self
                    .document
                    .timeline
                    .clips
                    .get(clip_index)
                    .and_then(|clip| clip.tracks.get(track_index))
                else {
                    return false;
                };
                let mut updated_track = track.clone();
                keyframe.time = self.snap.snap_time(keyframe.time);
                updated_track.push_keyframe(keyframe);

                self.push_undo_snapshot();
                self.document.timeline.clips[clip_index].tracks[track_index] = updated_track;
                self.redo_stack.clear();
                true
            }
            AnimationEditorCommand::UpdateKeyframeEasing { selection, easing } => {
                let Some(track) = self
                    .document
                    .timeline
                    .clips
                    .get(selection.clip_index)
                    .and_then(|clip| clip.tracks.get(selection.track_index))
                else {
                    return false;
                };
                let Some(keyframe) = track.keyframes.get(selection.keyframe_index) else {
                    return false;
                };

                let mut updated_track = track.clone();
                let mut updated_keyframe = *keyframe;
                updated_keyframe.easing = easing;
                updated_track.keyframes[selection.keyframe_index] = updated_keyframe;

                self.push_undo_snapshot();
                self.document.timeline.clips[selection.clip_index].tracks[selection.track_index] =
                    updated_track;
                self.redo_stack.clear();
                true
            }
            AnimationEditorCommand::MoveKeyframe { selection, time } => {
                let duration = self.document.timeline.duration.max(0.0);
                let time = self.snap.snap_time(time).clamp(0.0, duration);
                let Some(keyframe) = self
                    .document
                    .timeline
                    .clips
                    .get(selection.clip_index)
                    .and_then(|clip| clip.tracks.get(selection.track_index))
                    .and_then(|track| track.keyframes.get(selection.keyframe_index))
                else {
                    return false;
                };
                if keyframe.time == time {
                    return false;
                }

                self.push_undo_snapshot();
                self.document.timeline.clips[selection.clip_index].tracks[selection.track_index]
                    .keyframes[selection.keyframe_index]
                    .time = time;
                self.redo_stack.clear();
                true
            }
            AnimationEditorCommand::RemoveKeyframe(selection) => {
                let Some(track) = self
                    .document
                    .timeline
                    .clips
                    .get(selection.clip_index)
                    .and_then(|clip| clip.tracks.get(selection.track_index))
                else {
                    return false;
                };
                if selection.keyframe_index >= track.keyframes.len() {
                    return false;
                }

                let mut updated_track = track.clone();
                updated_track.keyframes.remove(selection.keyframe_index);

                self.push_undo_snapshot();
                self.document.timeline.clips[selection.clip_index].tracks[selection.track_index] =
                    updated_track;
                self.selection
                    .keyframes
                    .retain(|selected| *selected != selection);
                self.redo_stack.clear();
                true
            }
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.document.clone());
        self.document = previous;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.document.clone());
        self.document = next;
        true
    }

    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(self.document.clone());
    }
}

impl Default for AnimationEditorState {
    fn default() -> Self {
        Self::new(AnimationDocument::default())
    }
}

struct AnimationDocumentFormatParser<'a> {
    input: &'a str,
}

impl<'a> AnimationDocumentFormatParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input }
    }

    fn parse(self) -> Result<AnimationDocument, AnimationDocumentFormatError> {
        let mut version = None;
        let mut name = None;
        let mut duration = None;
        let mut clips = Vec::new();
        let mut current_clip: Option<Clip> = None;
        let mut current_track: Option<Track> = None;

        for (line_index, raw_line) in self.input.lines().enumerate() {
            let line_no = line_index + 1;
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }

            let fields = line.split('\t').collect::<Vec<_>>();
            match fields.first().copied().unwrap_or_default() {
                "sui-animation-document" => {
                    expect_field_count(line_no, &fields, 2)?;
                    if version.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "duplicate document header",
                        ));
                    }
                    let parsed_version = parse_u32_field(line_no, fields[1], "version")?;
                    if parsed_version != ANIMATION_DOCUMENT_VERSION {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            format!(
                                "unsupported document version {parsed_version}; expected {ANIMATION_DOCUMENT_VERSION}"
                            ),
                        ));
                    }
                    version = Some(parsed_version);
                }
                "name" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 2)?;
                    ensure_no_open_track_or_clip(line_no, &current_track, &current_clip)?;
                    name = Some(unescape_document_field(fields[1], line_no)?);
                }
                "duration" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 2)?;
                    ensure_no_open_track_or_clip(line_no, &current_track, &current_clip)?;
                    duration = Some(parse_f64_field(line_no, fields[1], "duration")?);
                }
                "clip" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 5)?;
                    if current_clip.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "nested clips are not allowed",
                        ));
                    }
                    let id = unescape_document_field(fields[1], line_no)?;
                    let start_time = parse_f64_field(line_no, fields[2], "clip start time")?;
                    let clip_duration = parse_f64_field(line_no, fields[3], "clip duration")?;
                    let enabled = parse_bool_field(line_no, fields[4], "clip enabled")?;
                    let mut clip = Clip::new(id, start_time, clip_duration);
                    clip.enabled = enabled;
                    current_clip = Some(clip);
                }
                "track" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 4)?;
                    if current_clip.is_none() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "track must appear inside a clip",
                        ));
                    }
                    if current_track.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "nested tracks are not allowed",
                        ));
                    }
                    let target =
                        AnimationTargetId::new(unescape_document_field(fields[1], line_no)?);
                    let property_path = unescape_document_field(fields[2], line_no)?;
                    let enabled = parse_bool_field(line_no, fields[3], "track enabled")?;
                    let mut track = Track::new(AnimationBinding::new(
                        target,
                        animation_property_from_path(property_path),
                    ));
                    track.enabled = enabled;
                    current_track = Some(track);
                }
                "key" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 4)?;
                    let Some(track) = &mut current_track else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "keyframe must appear inside a track",
                        ));
                    };
                    let time = parse_f64_field(line_no, fields[1], "keyframe time")?;
                    let easing = parse_easing(line_no, fields[2])?;
                    let value = parse_animation_value(line_no, fields[3])?;
                    track.push_keyframe(Keyframe::new(time, value).with_easing(easing));
                }
                "endtrack" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 1)?;
                    let Some(track) = current_track.take() else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endtrack without an open track",
                        ));
                    };
                    let Some(clip) = &mut current_clip else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endtrack without an open clip",
                        ));
                    };
                    clip.push_track(track);
                }
                "endclip" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 1)?;
                    if current_track.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endclip reached before endtrack",
                        ));
                    }
                    let Some(clip) = current_clip.take() else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endclip without an open clip",
                        ));
                    };
                    clips.push(clip);
                }
                other => {
                    return Err(AnimationDocumentFormatError::new(
                        Some(line_no),
                        format!("unknown directive {other:?}"),
                    ));
                }
            }
        }

        if version.is_none() {
            return Err(AnimationDocumentFormatError::new(
                None,
                "missing document header",
            ));
        }
        if current_track.is_some() {
            return Err(AnimationDocumentFormatError::new(None, "unclosed track"));
        }
        if current_clip.is_some() {
            return Err(AnimationDocumentFormatError::new(None, "unclosed clip"));
        }

        Ok(AnimationDocument {
            version: ANIMATION_DOCUMENT_VERSION,
            name: name
                .ok_or_else(|| AnimationDocumentFormatError::new(None, "missing document name"))?,
            timeline: Timeline {
                duration: duration.ok_or_else(|| {
                    AnimationDocumentFormatError::new(None, "missing timeline duration")
                })?,
                clips,
            },
        })
    }
}

fn format_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn format_f64(value: f64) -> String {
    value.to_string()
}

fn format_f32(value: f32) -> String {
    value.to_string()
}

fn escape_document_field(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn unescape_document_field(
    value: &str,
    line_no: usize,
) -> Result<String, AnimationDocumentFormatError> {
    let mut unescaped = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            unescaped.push(ch);
            continue;
        }

        match chars.next() {
            Some('\\') => unescaped.push('\\'),
            Some('n') => unescaped.push('\n'),
            Some('r') => unescaped.push('\r'),
            Some('t') => unescaped.push('\t'),
            Some(other) => {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    format!("unsupported escape sequence \\{other}"),
                ));
            }
            None => {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    "unterminated escape sequence",
                ));
            }
        }
    }
    Ok(unescaped)
}

fn expect_field_count(
    line_no: usize,
    fields: &[&str],
    expected: usize,
) -> Result<(), AnimationDocumentFormatError> {
    if fields.len() == expected {
        return Ok(());
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        format!("expected {expected} fields, found {}", fields.len()),
    ))
}

fn ensure_document_header(
    line_no: usize,
    version: Option<u32>,
) -> Result<(), AnimationDocumentFormatError> {
    if version.is_some() {
        return Ok(());
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        "document header must be the first directive",
    ))
}

fn ensure_no_open_track_or_clip(
    line_no: usize,
    track: &Option<Track>,
    clip: &Option<Clip>,
) -> Result<(), AnimationDocumentFormatError> {
    if track.is_none() && clip.is_none() {
        return Ok(());
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        "document metadata must appear before clips",
    ))
}

fn parse_u32_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<u32, AnimationDocumentFormatError> {
    value.parse::<u32>().map_err(|_| {
        AnimationDocumentFormatError::new(Some(line_no), format!("invalid {label}: {value:?}"))
    })
}

fn parse_f64_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<f64, AnimationDocumentFormatError> {
    let parsed = value.parse::<f64>().map_err(|_| {
        AnimationDocumentFormatError::new(Some(line_no), format!("invalid {label}: {value:?}"))
    })?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("{label} must be finite"),
        ))
    }
}

fn parse_f32_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<f32, AnimationDocumentFormatError> {
    let parsed = value.parse::<f32>().map_err(|_| {
        AnimationDocumentFormatError::new(Some(line_no), format!("invalid {label}: {value:?}"))
    })?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("{label} must be finite"),
        ))
    }
}

fn parse_bool_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<bool, AnimationDocumentFormatError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("{label} must be true or false"),
        )),
    }
}

fn format_easing(easing: Easing) -> String {
    match easing {
        Easing::Linear => "linear".to_string(),
        Easing::EaseIn => "ease-in".to_string(),
        Easing::EaseOut => "ease-out".to_string(),
        Easing::EaseInOut => "ease-in-out".to_string(),
        Easing::CubicBezier { x1, y1, x2, y2 } => format!(
            "cubic-bezier:{},{},{},{}",
            format_f32(x1),
            format_f32(y1),
            format_f32(x2),
            format_f32(y2)
        ),
    }
}

fn parse_easing(line_no: usize, value: &str) -> Result<Easing, AnimationDocumentFormatError> {
    match value {
        "linear" => Ok(Easing::Linear),
        "ease-in" => Ok(Easing::EaseIn),
        "ease-out" => Ok(Easing::EaseOut),
        "ease-in-out" => Ok(Easing::EaseInOut),
        _ => {
            let Some(params) = value.strip_prefix("cubic-bezier:") else {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    format!("unknown easing {value:?}"),
                ));
            };
            let values = parse_f32_list(line_no, params, "cubic-bezier")?;
            if values.len() != 4 {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    "cubic-bezier easing requires four values",
                ));
            }
            Ok(Easing::CubicBezier {
                x1: values[0],
                y1: values[1],
                x2: values[2],
                y2: values[3],
            })
        }
    }
}

fn format_animation_value(value: AnimationValue) -> String {
    match value {
        AnimationValue::Scalar(value) => format!("scalar:{}", format_f32(value)),
        AnimationValue::Point(value) => {
            format!("point:{},{}", format_f32(value.x), format_f32(value.y))
        }
        AnimationValue::Vector(value) => {
            format!("vector:{},{}", format_f32(value.x), format_f32(value.y))
        }
        AnimationValue::Size(value) => {
            format!(
                "size:{},{}",
                format_f32(value.width),
                format_f32(value.height)
            )
        }
        AnimationValue::Rect(value) => {
            format!(
                "rect:{},{},{},{}",
                format_f32(value.x()),
                format_f32(value.y()),
                format_f32(value.width()),
                format_f32(value.height())
            )
        }
        AnimationValue::Color(value) => {
            format!(
                "color:{},{},{},{},{}",
                format_color_space(value.space),
                format_f32(value.red),
                format_f32(value.green),
                format_f32(value.blue),
                format_f32(value.alpha)
            )
        }
        AnimationValue::Transform(value) => {
            format!(
                "transform:{},{},{},{},{},{}",
                format_f32(value.xx),
                format_f32(value.yx),
                format_f32(value.xy),
                format_f32(value.yy),
                format_f32(value.dx),
                format_f32(value.dy)
            )
        }
    }
}

fn parse_animation_value(
    line_no: usize,
    value: &str,
) -> Result<AnimationValue, AnimationDocumentFormatError> {
    let Some((kind, payload)) = value.split_once(':') else {
        return Err(AnimationDocumentFormatError::new(
            Some(line_no),
            "animation value must include a kind prefix",
        ));
    };

    match kind {
        "scalar" => Ok(AnimationValue::Scalar(parse_f32_field(
            line_no,
            payload,
            "scalar value",
        )?)),
        "point" => {
            let values = parse_fixed_f32_list(line_no, payload, "point", 2)?;
            Ok(AnimationValue::Point(Point::new(values[0], values[1])))
        }
        "vector" => {
            let values = parse_fixed_f32_list(line_no, payload, "vector", 2)?;
            Ok(AnimationValue::Vector(Vector::new(values[0], values[1])))
        }
        "size" => {
            let values = parse_fixed_f32_list(line_no, payload, "size", 2)?;
            Ok(AnimationValue::Size(Size::new(values[0], values[1])))
        }
        "rect" => {
            let values = parse_fixed_f32_list(line_no, payload, "rect", 4)?;
            Ok(AnimationValue::Rect(Rect::new(
                values[0], values[1], values[2], values[3],
            )))
        }
        "color" => {
            let parts = payload.split(',').collect::<Vec<_>>();
            if parts.len() != 5 {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    "color value requires color space plus four channels",
                ));
            }
            Ok(AnimationValue::Color(Color::new(
                parse_color_space(line_no, parts[0])?,
                parse_f32_field(line_no, parts[1], "red channel")?,
                parse_f32_field(line_no, parts[2], "green channel")?,
                parse_f32_field(line_no, parts[3], "blue channel")?,
                parse_f32_field(line_no, parts[4], "alpha channel")?,
            )))
        }
        "transform" => {
            let values = parse_fixed_f32_list(line_no, payload, "transform", 6)?;
            Ok(AnimationValue::Transform(Transform::new(
                values[0], values[1], values[2], values[3], values[4], values[5],
            )))
        }
        _ => Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("unknown animation value kind {kind:?}"),
        )),
    }
}

fn parse_f32_list(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<Vec<f32>, AnimationDocumentFormatError> {
    value
        .split(',')
        .map(|field| parse_f32_field(line_no, field, label))
        .collect()
}

fn parse_fixed_f32_list(
    line_no: usize,
    value: &str,
    label: &str,
    expected: usize,
) -> Result<Vec<f32>, AnimationDocumentFormatError> {
    let values = parse_f32_list(line_no, value, label)?;
    if values.len() == expected {
        return Ok(values);
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        format!("{label} value requires {expected} numbers"),
    ))
}

fn format_color_space(color_space: ColorSpace) -> &'static str {
    match color_space {
        ColorSpace::Srgb => "srgb",
        ColorSpace::LinearSrgb => "linear-srgb",
        ColorSpace::DisplayP3 => "display-p3",
        ColorSpace::LinearDisplayP3 => "linear-display-p3",
    }
}

fn parse_color_space(
    line_no: usize,
    value: &str,
) -> Result<ColorSpace, AnimationDocumentFormatError> {
    match value {
        "srgb" => Ok(ColorSpace::Srgb),
        "linear-srgb" => Ok(ColorSpace::LinearSrgb),
        "display-p3" => Ok(ColorSpace::DisplayP3),
        "linear-display-p3" => Ok(ColorSpace::LinearDisplayP3),
        _ => Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("unknown color space {value:?}"),
        )),
    }
}

fn animation_property_from_path(path: String) -> AnimationProperty {
    match path.as_str() {
        "layer.opacity" => AnimationProperty::LayerOpacity,
        "layer.translation" => AnimationProperty::LayerTranslation,
        "fill.color" => AnimationProperty::FillColor,
        "bounds" => AnimationProperty::Bounds,
        _ => AnimationProperty::Custom(AnimationPropertyPath::new(path)),
    }
}

/// The CSS `cubic-bezier()` timing function: solve the curve's x for `t`,
/// then return its y. Newton's method converges in a few steps on typical
/// curves; bisection takes over where the slope is too flat for it.
fn sample_cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    // Polynomial coefficients of each coordinate, in f64 for accuracy.
    let coefficients = |p1: f32, p2: f32| {
        let c = 3.0 * f64::from(p1);
        let b = 3.0 * (f64::from(p2) - f64::from(p1)) - c;
        let a = 1.0 - c - b;
        (a, b, c)
    };
    let (ax, bx, cx) = coefficients(x1, x2);
    let (ay, by, cy) = coefficients(y1, y2);
    let curve_x = |u: f64| ((ax * u + bx) * u + cx) * u;
    let slope_x = |u: f64| (3.0 * ax * u + 2.0 * bx) * u + cx;
    let target = f64::from(t);
    const EPSILON: f64 = 1e-7;

    let mut u = target;
    let mut solved = false;
    for _ in 0..8 {
        let error = curve_x(u) - target;
        if error.abs() < EPSILON {
            solved = true;
            break;
        }
        let slope = slope_x(u);
        if slope.abs() < 1e-6 {
            break;
        }
        u -= error / slope;
    }
    if !solved {
        let (mut low, mut high) = (0.0, 1.0);
        u = target;
        for _ in 0..64 {
            let x = curve_x(u);
            if (x - target).abs() < EPSILON {
                break;
            }
            if x < target {
                low = u;
            } else {
                high = u;
            }
            u = (low + high) * 0.5;
        }
    }

    (((ay * u + by) * u + cy) * u) as f32
}

#[cfg(test)]
mod tests {
    use super::{
        AnimatedValue, AnimationBinding, AnimationDocument, AnimationEditorCommand,
        AnimationEditorState, AnimationPlayer, AnimationProperty, AnimationPropertyPath,
        AnimationSpec, AnimationTargetId, AnimationValue, Blink, Clip, Easing, Interpolate,
        Keyframe, KeyframeSelection, LoopMode, MotionPolicy, MotionPreference, MotionValue,
        PlaybackState, Pulse, SampleBuffer, SpringF32, SpringSpec, Timeline, Track, Transition,
    };
    use sui_core::{Color, ColorSpace, Rect, Transform, Vector};

    fn opacity_binding() -> AnimationBinding {
        AnimationBinding::new(
            AnimationTargetId::new("hero-card"),
            AnimationProperty::LayerOpacity,
        )
    }

    #[test]
    fn interpolate_supports_common_sui_values() {
        assert!((f32::interpolate(2.0, 6.0, 0.25) - 3.0).abs() < 1e-6);
        assert_eq!(
            Vector::interpolate(Vector::new(0.0, 4.0), Vector::new(8.0, 12.0), 0.5),
            Vector::new(4.0, 8.0)
        );
        assert_eq!(
            Rect::interpolate(
                Rect::new(0.0, 2.0, 10.0, 20.0),
                Rect::new(10.0, 12.0, 30.0, 40.0),
                0.5
            ),
            Rect::new(5.0, 7.0, 20.0, 30.0)
        );
        let interpolated = Color::interpolate(
            Color::rgba(0.2, 0.4, 0.6, 1.0),
            Color::rgba(0.6, 0.8, 1.0, 0.0),
            0.5,
        );
        // Premultiplied blending: fading toward a transparent color keeps
        // the visible color and only lowers its opacity.
        let expected = Color::rgba(0.2, 0.4, 0.6, 0.5);
        assert_eq!(interpolated.space, expected.space);
        assert!((interpolated.red - expected.red).abs() < 1e-4);
        assert!((interpolated.green - expected.green).abs() < 1e-4);
        assert!((interpolated.blue - expected.blue).abs() < 1e-4);
        assert!((interpolated.alpha - expected.alpha).abs() < 1e-6);
    }

    #[test]
    fn transform_interpolation_rotates_without_shrinking() {
        let quarter_turn = Transform::rotation(std::f32::consts::FRAC_PI_2);

        let halfway = Transform::interpolate(Transform::IDENTITY, quarter_turn, 0.5);

        let eighth = Transform::rotation(std::f32::consts::FRAC_PI_4);
        for (actual, expected) in [
            (halfway.xx, eighth.xx),
            (halfway.yx, eighth.yx),
            (halfway.xy, eighth.xy),
            (halfway.yy, eighth.yy),
        ] {
            assert!((actual - expected).abs() < 1e-5, "{actual} vs {expected}");
        }
        // Blending the matrix entries would give a scale of about 0.707.
        assert!((halfway.xx.hypot(halfway.yx) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn transform_interpolation_blends_scale_translation_and_short_rotation() {
        let from = Transform::scale(1.0, 2.0).then(Transform::translation(10.0, 0.0));
        let to = Transform::scale(3.0, 4.0).then(Transform::translation(30.0, 20.0));

        let halfway = Transform::interpolate(from, to, 0.5);

        assert!((halfway.xx - 2.0).abs() < 1e-5);
        assert!((halfway.yy - 3.0).abs() < 1e-5);
        assert!((halfway.dx - 20.0).abs() < 1e-5);
        assert!((halfway.dy - 10.0).abs() < 1e-5);
        assert_eq!(Transform::interpolate(from, to, 0.0), from);
        assert_eq!(Transform::interpolate(from, to, 1.0), to);

        // From 170 degrees to -170 degrees turns 20 degrees through 180.
        let near_half = Transform::rotation(170_f32.to_radians());
        let past_half = Transform::rotation((-170_f32).to_radians());
        let middle = Transform::interpolate(near_half, past_half, 0.5);
        assert!((middle.xx + 1.0).abs() < 1e-5, "{middle:?}");
    }

    #[test]
    fn extrapolation_lets_scalars_overshoot_but_clamps_colors() {
        assert_eq!(f32::extrapolate(0.0, 10.0, 1.2), 12.0);
        assert_eq!(f32::interpolate(0.0, 10.0, 1.2), 10.0);
        assert_eq!(
            Color::extrapolate(Color::BLACK, Color::WHITE, 1.5),
            Color::WHITE
        );
    }

    #[test]
    fn transition_samples_ease_in_out_curve() {
        let transition = Transition::new(0.0_f32, 1.0, 10.0, 2.0, Easing::EaseInOut);

        assert_eq!(transition.sample(10.0), 0.0);
        assert!(transition.sample(11.0) > 0.45 && transition.sample(11.0) < 0.55);
        assert_eq!(transition.sample(12.0), 1.0);
        assert!(transition.is_complete(12.0));
    }

    #[test]
    fn cubic_bezier_easing_has_stable_endpoints() {
        let easing = Easing::CubicBezier {
            x1: 0.4,
            y1: 0.0,
            x2: 0.2,
            y2: 1.0,
        };

        assert_eq!(easing.sample(0.0), 0.0);
        assert_eq!(easing.sample(1.0), 1.0);
        let midpoint = easing.sample(0.5);
        assert!(midpoint > 0.0 && midpoint < 1.0);
    }

    #[test]
    fn track_samples_keyframes_without_requiring_sorted_input() {
        let track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(1.0, 1.0_f32),
            Keyframe::new(0.0, 0.0_f32).with_easing(Easing::Linear),
        ]);

        assert_eq!(track.sample(-1.0), Some(0.0));
        assert_eq!(track.sample(0.5), Some(0.5));
        assert_eq!(track.sample(2.0), Some(1.0));
    }

    #[test]
    fn timeline_samples_clip_tracks_in_global_time() {
        let opacity_track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(0.0, AnimationValue::Scalar(0.25)),
            Keyframe::new(2.0, AnimationValue::Scalar(1.0)),
        ]);
        let timeline =
            Timeline::new(4.0).with_clip(Clip::new("intro", 1.0, 2.0).with_track(opacity_track));

        assert!(timeline.sample(0.5).is_empty());
        let samples = timeline.sample(2.0);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].clip_id, "intro");
        let opacity = samples[0]
            .value
            .as_scalar()
            .expect("opacity sample should be scalar");
        assert!((opacity - 0.625).abs() < 1e-6);
    }

    #[test]
    fn timeline_sample_into_reuses_output_storage() {
        let opacity_track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(0.0, AnimationValue::Scalar(0.25)),
            Keyframe::new(1.0, AnimationValue::Scalar(0.75)),
        ]);
        let timeline =
            Timeline::new(1.0).with_clip(Clip::new("intro", 0.0, 1.0).with_track(opacity_track));
        let mut samples = SampleBuffer::with_capacity(8);
        let initial_capacity = samples.capacity();

        {
            let batch = timeline.sample_into(0.5, &mut samples);
            assert_eq!(batch.len(), 1);
            assert_eq!(batch.samples()[0].clip_id, "intro");
            assert_eq!(batch.samples()[0].value.as_scalar(), Some(0.5));
        }
        assert_eq!(samples.capacity(), initial_capacity);

        {
            let batch = timeline.sample_into(2.0, &mut samples);
            assert_eq!(batch.len(), 1);
            assert_eq!(batch.samples()[0].value.as_scalar(), Some(0.75));
        }
        assert_eq!(samples.capacity(), initial_capacity);
    }

    #[test]
    fn compiled_timeline_sorts_tracks_and_omits_disabled_data() {
        let opacity_track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(1.0, AnimationValue::Scalar(1.0)),
            Keyframe::new(0.0, AnimationValue::Scalar(0.0)).with_easing(Easing::Linear),
        ]);
        let disabled_track = Track::new(AnimationBinding::new(
            AnimationTargetId::new("hero-card"),
            AnimationProperty::FillColor,
        ))
        .with_enabled(false)
        .with_keyframes([
            Keyframe::new(0.0, AnimationValue::Color(Color::rgba(1.0, 0.0, 0.0, 1.0))),
            Keyframe::new(1.0, AnimationValue::Color(Color::rgba(0.0, 0.0, 1.0, 1.0))),
        ]);
        let disabled_clip = Clip::new("disabled", 0.0, 1.0).with_track(
            Track::new(opacity_binding()).with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(0.0)),
                Keyframe::new(1.0, AnimationValue::Scalar(1.0)),
            ]),
        );
        let mut disabled_clip = disabled_clip;
        disabled_clip.enabled = false;
        let timeline = Timeline::new(1.0)
            .with_clip(
                Clip::new("intro", 0.0, 1.0)
                    .with_track(opacity_track)
                    .with_track(disabled_track),
            )
            .with_clip(disabled_clip);

        let compiled = timeline.compile();
        let mut samples = SampleBuffer::new();
        let batch = compiled.sample_into(0.5, &mut samples);

        assert_eq!(compiled.clips().len(), 1);
        assert_eq!(compiled.sample_capacity(), 1);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.samples()[0].clip_id, "intro");
        assert_eq!(
            batch.samples()[0].binding.property,
            AnimationProperty::LayerOpacity
        );
        assert_eq!(batch.samples()[0].value.as_scalar(), Some(0.5));
    }

    #[test]
    fn animation_player_ticks_shared_compiled_timeline_into_sample_buffer() {
        let timeline = Timeline::new(1.0).with_clip(Clip::new("intro", 0.0, 1.0).with_track(
            Track::new(opacity_binding()).with_keyframes([
                Keyframe::new(0.0, AnimationValue::Scalar(0.0)).with_easing(Easing::Linear),
                Keyframe::new(1.0, AnimationValue::Scalar(1.0)),
            ]),
        ));
        let compiled = timeline.compile_shared();
        let mut player = AnimationPlayer::new(compiled.clone()).repeat();
        let mut sibling = AnimationPlayer::new(compiled);
        let mut samples = SampleBuffer::with_capacity(1);
        player.play();

        let tick = player.tick_into(0.5, &mut samples);

        assert!(tick.advanced);
        assert!(tick.should_continue);
        assert_eq!(tick.playback.playhead, 0.5);
        assert_eq!(tick.sample_values()[0].value.as_scalar(), Some(0.5));

        sibling.seek(0.25);
        let batch = sibling.sample_into(&mut samples);
        assert_eq!(batch.samples()[0].value.as_scalar(), Some(0.25));
    }

    #[test]
    fn playback_state_advances_and_repeats() {
        let mut playback = PlaybackState {
            loop_mode: LoopMode::Repeat,
            ..PlaybackState::default()
        };
        playback.play();

        assert!(playback.tick(1.25, 1.0));
        assert!((playback.playhead - 0.25).abs() < 1e-6);
        assert!(playback.playing);
    }

    #[test]
    fn blink_is_deterministic_for_same_time_inputs() {
        let blink = Blink::new(0.8).with_duty_cycle(0.25).with_phase(0.1);

        assert_eq!(blink.is_on(1.25), blink.is_on(1.25));
        assert!(blink.is_on(0.0));
        assert!(!blink.is_on(0.35));
    }

    #[test]
    fn pulse_samples_repeatable_range() {
        let pulse = Pulse::new(1.0, 0.2, 1.0);
        let a = pulse.sample(0.25);
        let b = pulse.sample(0.25);
        assert_eq!(a, b);
        assert!((0.2..=1.0).contains(&a));
    }

    #[test]
    fn spring_steps_are_exact_at_any_frame_rate() {
        let spec = SpringSpec::BOUNCY;
        let mut fine = SpringF32::from_spec(0.0, spec);
        let mut coarse = SpringF32::from_spec(0.0, spec);
        for _ in 0..240 {
            fine.step(1.0, 1.0 / 240.0);
        }
        for _ in 0..10 {
            coarse.step(1.0, 0.1);
        }

        assert!((fine.value - coarse.value).abs() < 1e-4);
        assert!((fine.velocity - coarse.velocity).abs() < 1e-3);
    }

    #[test]
    fn stiff_springs_stay_stable_across_long_frames() {
        let mut spring = SpringF32::new(0.0).with_config(4000.0, 20.0);

        // Explicit integration would blow up at this step size.
        for _ in 0..20 {
            spring.step(1.0, 0.25);
        }

        assert!(spring.value.is_finite());
        assert!(spring.settle(1.0));
        assert_eq!(spring.value, 1.0);
        assert_eq!(spring.velocity, 0.0);
    }

    #[test]
    fn spring_carries_fling_velocity() {
        let mut flung = SpringF32::from_spec(0.0, SpringSpec::SMOOTH).with_velocity(20.0);

        flung.step(0.0, 0.05);

        assert!(flung.value > 0.3, "a fling travels before springing back");
        assert!(!flung.is_settled(0.0));
    }

    #[test]
    fn spring_spec_maps_bounce_to_damping() {
        assert!((SpringSpec::SMOOTH.damping_ratio() - 1.0).abs() < 1e-6);
        assert!((SpringSpec::new(0.3, 0.25).damping_ratio() - 0.75).abs() < 1e-6);
        assert!(SpringSpec::new(0.3, -0.5).damping_ratio() > 1.0);

        let round_trip =
            SpringSpec::from_physics(SpringSpec::BOUNCY.stiffness(), SpringSpec::BOUNCY.damping());
        assert!((round_trip.duration - SpringSpec::BOUNCY.duration).abs() < 1e-4);
        assert!((round_trip.bounce - SpringSpec::BOUNCY.bounce).abs() < 1e-4);
    }

    #[test]
    fn spring_progress_overshoots_only_when_bouncy_and_settles_exactly() {
        let peak = |spec: SpringSpec| {
            (0..400)
                .map(|step| spec.progress(f64::from(step) / 200.0))
                .fold(0.0_f32, f32::max)
        };

        assert!(peak(SpringSpec::BOUNCY) > 1.05);
        assert!(peak(SpringSpec::SMOOTH) <= 1.0);
        assert!(peak(SpringSpec::new(0.3, -0.4)) <= 1.0);
        for spec in [SpringSpec::SMOOTH, SpringSpec::SNAPPY, SpringSpec::BOUNCY] {
            let settle = spec.settling_duration();
            assert!(settle > f64::from(spec.duration) * 0.5 && settle < 3.0);
            assert_eq!(spec.progress(settle), 1.0);
            assert!((spec.progress(settle * 0.999) - 1.0).abs() < 2e-3);
            assert_eq!(spec.progress(0.0), 0.0);
        }
    }

    #[test]
    fn animation_spec_follows_the_motion_policy() {
        let spec = AnimationSpec::tween(0.2, Easing::EaseOut);
        let slow = MotionPolicy::FULL.with_time_scale(0.5);

        assert_eq!(spec.with_policy(slow).duration(), 0.4);
        assert!(
            spec.with_policy(MotionPolicy::new(MotionPreference::Off))
                .is_instant()
        );
        let reduced = MotionPolicy::new(MotionPreference::Reduced);
        assert_eq!(spec.with_policy(reduced), spec);
        assert!(spec.with_movement_policy(reduced).is_instant());
        let spring = AnimationSpec::spring(SpringSpec::SMOOTH).with_policy(slow);
        let nominal = AnimationSpec::spring(SpringSpec::SMOOTH).duration();
        assert!((spring.duration() - nominal * 2.0).abs() < 1e-3);
    }

    fn velocity(motion: &MotionValue<f32>, time: f64) -> f32 {
        let step = 1.0e-4;
        (motion.value(time + step) - motion.value(time - step)) / (2.0 * step) as f32
    }

    #[test]
    fn motion_value_reaches_its_target_exactly() {
        let mut motion = MotionValue::new(0.0_f32);

        assert!(motion.animate_to(1.0, 10.0, AnimationSpec::tween(0.2, Easing::Linear)));
        assert!((motion.value(10.1) - 0.5).abs() < 1e-5);
        assert!(motion.advance(10.1));
        assert!(!motion.advance(10.2));
        assert_eq!(motion.value(10.2), 1.0);
        assert!(!motion.animate_to(1.0, 10.3, AnimationSpec::tween(0.2, Easing::Linear)));
    }

    #[test]
    fn retargeting_keeps_velocity_instead_of_restarting_from_rest() {
        let spec = AnimationSpec::tween(0.3, Easing::EaseInOut);
        let mut motion = MotionValue::new(0.0_f32);
        motion.animate_to(1.0, 0.0, spec);
        let before = velocity(&motion, 0.15);
        assert!(before > 1.0);

        motion.animate_to(0.0, 0.15, spec);

        // The value keeps moving the way it was going and turns around
        // smoothly, rather than stopping dead at the retarget.
        assert!((velocity(&motion, 0.15) - before).abs() < 0.05 * before);
        assert!(motion.value(0.2) > motion.value(0.15));
        assert!(!motion.advance(0.45));
        assert_eq!(motion.value(0.45), 0.0);
    }

    #[test]
    fn spring_retargets_keep_momentum_too() {
        let spec = AnimationSpec::spring(SpringSpec::SNAPPY);
        let mut motion = MotionValue::new(0.0_f32);
        motion.animate_to(100.0, 0.0, spec);
        let before = velocity(&motion, 0.08);

        motion.animate_to(-100.0, 0.08, spec);

        assert!((velocity(&motion, 0.08) - before).abs() < 0.05 * before.abs());
        assert!(motion.animate_to(-100.0, 0.1, spec));
        let settled = 0.08 + spec.duration() + 0.01;
        assert!(!motion.advance(settled));
        assert_eq!(motion.value(settled), -100.0);
    }

    #[test]
    fn rapid_retargets_fold_old_motion_without_jumping() {
        let spec = AnimationSpec::tween(1.0, Easing::EaseInOut);
        let mut motion = MotionValue::new(0.0_f32);
        let mut time = 0.0;
        for step in 0..12 {
            let before = motion.value(time);
            motion.animate_to(if step % 2 == 0 { 1.0 } else { 0.0 }, time, spec);
            assert!((motion.value(time) - before).abs() < 1e-5);
            time += 0.05;
        }

        assert_eq!(motion.target(), 0.0);
        assert!(!motion.advance(time + 1.0));
        assert_eq!(motion.value(time + 1.0), 0.0);
    }

    #[test]
    fn animated_value_supports_springs() {
        let mut value = AnimatedValue::new(0.0_f32).with_spec(SpringSpec::BOUNCY.into());
        value.set_target(1.0);

        let mut peak = 0.0_f32;
        while value.tick(1.0 / 60.0) {
            peak = peak.max(value.value());
        }

        assert!(peak > 1.05);
        assert_eq!(value.value(), 1.0);
    }

    #[test]
    fn spring_helpers_converge_toward_target_values() {
        let mut spring = SpringF32::new(0.0).with_config(140.0, 22.0);
        let mut value = 0.0;
        for _ in 0..120 {
            value = spring.step(1.0, 1.0 / 120.0);
        }

        assert!(value > 0.95);
        assert!((value - 1.0).abs() < 0.05);
    }

    #[test]
    fn animated_value_reaches_target_and_reports_completion() {
        let mut animated = AnimatedValue::new(0.0_f32)
            .with_duration(0.2)
            .with_easing(Easing::Linear);
        animated.set_target(1.0);
        assert!(animated.is_animating());

        assert!(animated.tick(0.1));
        assert!((animated.value() - 0.5).abs() < 1e-4);

        assert!(!animated.tick(0.1));
        assert_eq!(animated.value(), 1.0);
        assert!(!animated.is_animating());
        assert!(!animated.tick(0.1));
    }

    #[test]
    fn animated_value_with_zero_duration_snaps_immediately() {
        let mut animated = AnimatedValue::new(2.0_f32).with_duration(0.0);
        animated.set_target(9.0);

        assert!(!animated.is_animating());
        assert_eq!(animated.value(), 9.0);
        assert!(!animated.tick(1.0));
    }

    #[test]
    fn editor_state_adds_keyframes_and_undoes_document_changes() {
        let track = Track::new(opacity_binding());
        let document = AnimationDocument::new(
            "Editor test",
            Timeline::new(2.0).with_clip(Clip::new("intro", 0.0, 2.0).with_track(track)),
        );
        let mut editor = AnimationEditorState::new(document);

        assert!(editor.apply_command(AnimationEditorCommand::AddKeyframe {
            clip_index: 0,
            track_index: 0,
            keyframe: Keyframe::new(0.51, AnimationValue::Scalar(0.8)),
        }));
        assert_eq!(editor.undo_len(), 1);
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes[0].time,
            0.5
        );

        assert!(editor.undo());
        assert!(
            editor.document.timeline.clips[0].tracks[0]
                .keyframes
                .is_empty()
        );
        assert!(editor.redo());
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes.len(),
            1
        );
    }

    #[test]
    fn editor_moves_keyframes_with_snapping_and_undo() {
        let mut editor = AnimationEditorState::new(AnimationDocument::new(
            "move",
            Timeline::new(1.0).with_clip(Clip::new("intro", 0.0, 1.0).with_track(
                Track::new(opacity_binding()).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.0)),
                    Keyframe::new(0.5, AnimationValue::Scalar(1.0)),
                ]),
            )),
        ));
        let selection = KeyframeSelection {
            clip_index: 0,
            track_index: 0,
            keyframe_index: 1,
        };
        let keyframe_time = |editor: &AnimationEditorState| {
            editor.document.timeline.clips[0].tracks[0].keyframes[1].time
        };

        assert!(editor.apply_command(AnimationEditorCommand::MoveKeyframe {
            selection,
            time: 0.74,
        }));
        assert!((keyframe_time(&editor) - 0.75).abs() < 1e-9);
        assert!(editor.apply_command(AnimationEditorCommand::MoveKeyframe {
            selection,
            time: 4.0,
        }));
        assert_eq!(keyframe_time(&editor), 1.0);
        assert!(!editor.apply_command(AnimationEditorCommand::MoveKeyframe {
            selection,
            time: 1.0,
        }));
        assert_eq!(editor.undo_len(), 2);
        assert!(editor.undo());
        assert!((keyframe_time(&editor) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn editor_selection_tracks_keyframes_without_mutating_document() {
        let mut editor = AnimationEditorState::default();
        let selection = KeyframeSelection {
            clip_index: 2,
            track_index: 1,
            keyframe_index: 3,
        };

        assert!(editor.apply_command(AnimationEditorCommand::SelectKeyframe(selection)));
        assert_eq!(editor.selection.keyframes, vec![selection]);
        assert_eq!(editor.undo_len(), 0);
    }

    #[test]
    fn editor_updates_keyframe_easing_with_undo() {
        let track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(0.0, AnimationValue::Scalar(0.0)),
            Keyframe::new(1.0, AnimationValue::Scalar(1.0)),
        ]);
        let document = AnimationDocument::new(
            "Easing test",
            Timeline::new(1.0).with_clip(Clip::new("intro", 0.0, 1.0).with_track(track)),
        );
        let mut editor = AnimationEditorState::new(document);
        let selection = KeyframeSelection {
            clip_index: 0,
            track_index: 0,
            keyframe_index: 0,
        };

        assert!(
            editor.apply_command(AnimationEditorCommand::UpdateKeyframeEasing {
                selection,
                easing: Easing::EaseInOut,
            })
        );
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes[0].easing,
            Easing::EaseInOut
        );
        assert!(editor.undo());
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes[0].easing,
            Easing::Linear
        );
    }

    #[test]
    fn animation_document_format_round_trips_timeline_data() {
        let target = AnimationTargetId::new("preview card\tmain");
        let binding = |property| AnimationBinding::new(target.clone(), property);
        let mut fill_track = Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
            Keyframe::new(
                0.0,
                AnimationValue::Color(Color::new(ColorSpace::DisplayP3, 0.1, 0.2, 0.3, 0.4)),
            )
            .with_easing(Easing::CubicBezier {
                x1: 0.4,
                y1: 0.0,
                x2: 0.2,
                y2: 1.0,
            }),
            Keyframe::new(
                1.0,
                AnimationValue::Color(Color::new(ColorSpace::LinearDisplayP3, 0.7, 0.6, 0.5, 1.0)),
            ),
        ]);
        fill_track.enabled = false;
        let mut clip = Clip::new("intro\nclip", 0.25, 2.0)
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-12.0, 4.5)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.0, AnimationValue::Vector(Vector::new(22.0, -8.0))),
                ]),
            )
            .with_track(fill_track)
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new("paint.radius"),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(6.0)).with_easing(Easing::EaseOut),
                    Keyframe::new(1.0, AnimationValue::Scalar(18.0)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new("local.transform"),
                )))
                .with_keyframes([Keyframe::new(
                    0.5,
                    AnimationValue::Transform(Transform::translation(8.0, 9.0)),
                )]),
            );
        clip.enabled = true;
        let document = AnimationDocument::new(
            "Demo\tAnimation\nDocument",
            Timeline::new(3.5).with_clip(clip),
        );

        let serialized = document.to_document_format();
        assert!(serialized.starts_with("sui-animation-document\t1\n"));
        assert!(serialized.contains("paint.radius"));

        let parsed = AnimationDocument::from_document_format(&serialized)
            .expect("serialized document should parse");
        assert_eq!(parsed, document);
        assert_eq!(parsed.to_document_format(), serialized);
    }

    #[test]
    fn animation_document_format_rejects_unsupported_versions() {
        let err = AnimationDocument::from_document_format(
            "sui-animation-document\t99\nname\tBad\nduration\t1\n",
        )
        .expect_err("unsupported version should fail");

        assert_eq!(err.line, Some(1));
        assert!(err.message.contains("unsupported document version"));
    }

    #[test]
    fn animation_document_format_rejects_unclosed_tracks() {
        let err = AnimationDocument::from_document_format(
            "sui-animation-document\t1\nname\tBad\nduration\t1\nclip\tintro\t0\t1\ttrue\ntrack\ttarget\tlayer.opacity\ttrue\n",
        )
        .expect_err("unclosed track should fail");

        assert!(err.message.contains("unclosed track"));
    }
}
