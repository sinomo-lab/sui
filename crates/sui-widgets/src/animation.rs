use sui_core::{Color, Vector};

pub trait Interpolate: Sized {
    fn interpolate(from: Self, to: Self, t: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        from + ((to - from) * t.clamp(0.0, 1.0))
    }
}

impl Interpolate for Vector {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Vector::new(
            f32::interpolate(from.x, to.x, t),
            f32::interpolate(from.y, to.y, t),
        )
    }
}

impl Interpolate for Color {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Color::new(
            from.space,
            f32::interpolate(from.red, to.red, t),
            f32::interpolate(from.green, to.green, t),
            f32::interpolate(from.blue, to.blue, t),
            f32::interpolate(from.alpha, to.alpha, t),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
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
        ((time - self.start_time) / self.duration).clamp(0.0, 1.0) as f32
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringF32 {
    pub value: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringF32 {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            stiffness: 180.0,
            damping: 24.0,
        }
    }

    pub fn with_config(mut self, stiffness: f32, damping: f32) -> Self {
        self.stiffness = stiffness.max(0.0);
        self.damping = damping.max(0.0);
        self
    }

    pub fn step(&mut self, target: f32, delta: f64) -> f32 {
        let dt = delta.max(0.0) as f32;
        if dt <= f32::EPSILON {
            return self.value;
        }

        let displacement = target - self.value;
        let acceleration = (displacement * self.stiffness) - (self.velocity * self.damping);
        self.velocity += acceleration * dt;
        self.value += self.velocity * dt;
        self.value
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

/// A self-contained, delta-driven transition driver for a single value.
///
/// `AnimatedValue` removes the boilerplate of hand-wiring a [`Transition`] plus
/// `request_animation_frame`: hold one in widget/app state, call
/// [`set_target`](Self::set_target) when the goal changes, and drive it from each
/// `WakeEvent::AnimationFrame` by calling [`tick`](Self::tick) with the frame's
/// `delta` (in **seconds**). [`tick`](Self::tick) returns `true` while the value
/// is still moving, which is exactly the signal to call
/// `EventCtx::request_animation_frame` again for the next frame.
///
/// Unlike [`Transition`], which is sampled against an absolute clock,
/// `AnimatedValue` accumulates elapsed time internally, so callers only need the
/// per-frame `delta`.
///
/// # Example
///
/// ```
/// use sui_widgets::animation::{AnimatedValue, Easing};
///
/// // Fade-in opacity over 0.2s.
/// let mut opacity = AnimatedValue::new(0.0_f32)
///     .with_duration(0.2)
///     .with_easing(Easing::EaseOut);
/// opacity.set_target(1.0);
///
/// // In each animation frame, advance by the frame delta:
/// let still_animating = opacity.tick(1.0 / 60.0);
/// assert!(still_animating);
/// assert!(opacity.value() > 0.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimatedValue<T> {
    start: T,
    target: T,
    current: T,
    elapsed: f32,
    duration: f32,
    easing: Easing,
    animating: bool,
}

impl<T> AnimatedValue<T>
where
    T: Interpolate + Copy,
{
    /// Creates a value that starts (and rests) at `initial`.
    pub fn new(initial: T) -> Self {
        Self {
            start: initial,
            target: initial,
            current: initial,
            elapsed: 0.0,
            duration: 0.2,
            easing: Easing::EaseInOut,
            animating: false,
        }
    }

    /// Sets the transition duration in **seconds** (builder form).
    ///
    /// A duration `<= 0.0` makes [`set_target`](Self::set_target) snap instantly.
    pub fn with_duration(mut self, seconds: f32) -> Self {
        self.duration = seconds.max(0.0);
        self
    }

    /// Sets the easing curve applied while interpolating (builder form).
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Replaces the transition duration in **seconds**.
    pub fn set_duration(&mut self, seconds: f32) {
        self.duration = seconds.max(0.0);
    }

    /// Replaces the easing curve applied while interpolating.
    pub fn set_easing(&mut self, easing: Easing) {
        self.easing = easing;
    }

    /// Aims the value at `target`, animating from wherever it currently sits.
    ///
    /// If the configured duration is non-positive the value jumps immediately.
    /// After calling this, drive the animation by calling [`tick`](Self::tick)
    /// each frame until it returns `false`.
    pub fn set_target(&mut self, target: T) {
        self.target = target;
        if self.duration <= f32::EPSILON {
            self.start = target;
            self.current = target;
            self.elapsed = 0.0;
            self.animating = false;
            return;
        }
        self.start = self.current;
        self.elapsed = 0.0;
        self.animating = true;
    }

    /// Immediately sets the current value (and target) without animating.
    pub fn jump_to(&mut self, value: T) {
        self.start = value;
        self.target = value;
        self.current = value;
        self.elapsed = 0.0;
        self.animating = false;
    }

    /// Advances the animation by `delta_seconds` and updates the current value.
    ///
    /// Returns `true` while the value is still in motion (the caller should
    /// request another animation frame), and `false` once it has settled on the
    /// target.
    pub fn tick(&mut self, delta_seconds: f32) -> bool {
        if !self.animating {
            return false;
        }
        self.elapsed += delta_seconds.max(0.0);
        let progress = if self.duration <= f32::EPSILON {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        };
        let eased = self.easing.sample(progress);
        self.current = T::interpolate(self.start, self.target, eased);
        if progress >= 1.0 {
            self.current = self.target;
            self.animating = false;
            return false;
        }
        true
    }

    /// The value at the current point in the animation.
    pub fn value(&self) -> T {
        self.current
    }

    /// The value the animation is heading toward.
    pub fn target(&self) -> T {
        self.target
    }

    /// Whether the value is currently animating toward its target.
    pub fn is_animating(&self) -> bool {
        self.animating
    }
}

fn sample_cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    let sample_curve = |a: f32, b: f32, c: f32, u: f32| {
        let inv = 1.0 - u;
        (3.0 * inv * inv * u * a) + (3.0 * inv * u * u * b) + (u * u * u * c)
    };

    let mut low = 0.0;
    let mut high = 1.0;
    let mut u = t;
    for _ in 0..10 {
        u = (low + high) * 0.5;
        let x = sample_curve(x1, x2, 1.0, u);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            low = u;
        } else {
            high = u;
        }
    }

    sample_curve(y1, y2, 1.0, u)
}

#[cfg(test)]
mod tests {
    use super::{AnimatedValue, Blink, Easing, Interpolate, Pulse, SpringF32, Transition};
    use sui_core::{Color, Vector};

    #[test]
    fn interpolate_supports_f32_color_and_vector() {
        assert!((f32::interpolate(2.0, 6.0, 0.25) - 3.0).abs() < 1e-6);
        assert_eq!(
            Vector::interpolate(Vector::new(0.0, 4.0), Vector::new(8.0, 12.0), 0.5),
            Vector::new(4.0, 8.0)
        );
        let interpolated = Color::interpolate(
            Color::rgba(0.2, 0.4, 0.6, 1.0),
            Color::rgba(0.6, 0.8, 1.0, 0.0),
            0.5,
        );
        let expected = Color::rgba(0.4, 0.6, 0.8, 0.5);
        assert_eq!(interpolated.space, expected.space);
        assert!((interpolated.red - expected.red).abs() < 1e-6);
        assert!((interpolated.green - expected.green).abs() < 1e-6);
        assert!((interpolated.blue - expected.blue).abs() < 1e-6);
        assert!((interpolated.alpha - expected.alpha).abs() < 1e-6);
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
        assert!(a >= 0.2 && a <= 1.0);
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

        // Halfway through a linear transition lands near the midpoint.
        assert!(animated.tick(0.1));
        assert!((animated.value() - 0.5).abs() < 1e-4);

        // Completing the duration settles exactly on the target and stops.
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
}
