//! Runtime-driven motion: the motion policy shared by every widget on a UI
//! thread, [`Motion`] values the runtime animates for widgets, and how a
//! window's animation frames are timed.
//!
//! The platform reports the operating system's motion preference, an app can
//! override it (for example from a settings page), and a time scale slows
//! every transition down for debugging. Widgets read the combined
//! [`MotionPolicy`] when they start a transition, so changes apply to the
//! next animation rather than cutting running ones short.

use std::cell::Cell;

use sui_animation::{AnimationSpec, Interpolate, MotionValue};
use sui_core::{InvalidationKind, MotionPolicy, MotionPreference};

#[derive(Debug, Clone, Copy, PartialEq)]
struct MotionSettings {
    system: MotionPreference,
    app: Option<MotionPreference>,
    time_scale: f32,
}

impl MotionSettings {
    const DEFAULT: Self = Self {
        system: MotionPreference::Full,
        app: None,
        time_scale: 1.0,
    };
}

thread_local! {
    static MOTION_SETTINGS: Cell<MotionSettings> = const { Cell::new(MotionSettings::DEFAULT) };
}

fn update(change: impl FnOnce(&mut MotionSettings)) {
    MOTION_SETTINGS.with(|settings| {
        let mut next = settings.get();
        change(&mut next);
        settings.set(next);
    });
}

fn settings() -> MotionSettings {
    MOTION_SETTINGS.with(Cell::get)
}

/// The motion policy in effect on this UI thread: the app's preference if it
/// set one, otherwise the system's, with the current time scale.
pub fn motion_policy() -> MotionPolicy {
    let settings = settings();
    MotionPolicy::new(settings.app.unwrap_or(settings.system)).with_time_scale(settings.time_scale)
}

/// The operating system's motion preference, as reported by the platform.
pub fn system_motion_preference() -> MotionPreference {
    settings().system
}

/// Record the operating system's motion preference. Platform backends call
/// this at startup and when the setting may have changed.
pub fn set_system_motion_preference(preference: MotionPreference) {
    update(|settings| settings.system = preference);
}

/// The app's motion preference override, if any.
pub fn app_motion_preference() -> Option<MotionPreference> {
    settings().app
}

/// Override the system's motion preference for this app, or pass `None` to
/// follow the system again.
pub fn set_app_motion_preference(preference: Option<MotionPreference>) {
    update(|settings| settings.app = preference);
}

/// How fast transitions play relative to their nominal durations.
pub fn motion_time_scale() -> f32 {
    settings().time_scale
}

/// Play transitions at `time_scale` speed: `0.25` shows them at quarter
/// speed. Meant for inspecting motion while developing.
pub fn set_motion_time_scale(time_scale: f32) {
    let time_scale = MotionPolicy::FULL.with_time_scale(time_scale).time_scale();
    update(|settings| settings.time_scale = time_scale);
}

/// Forget the system preference, app override, and time scale. Test
/// harnesses call this so each app starts from full motion.
pub fn reset_motion_settings() {
    MOTION_SETTINGS.with(|settings| settings.set(MotionSettings::DEFAULT));
}

/// How a window's animation frames are timed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FramePacing {
    /// The runtime times animation frames itself, up to 120 per second. Used
    /// by headless hosts and by windows that are not presenting.
    #[default]
    Timer,
    /// The platform starts each animation frame when the display is ready
    /// for one, with [`crate::Runtime::begin_animation_frame`], stamped with
    /// the time the frame is expected on screen.
    Display,
}

/// Something that knows the time of the frame being produced: widget
/// contexts, or a plain time in seconds.
pub trait FrameClock {
    fn frame_time(&self) -> f64;
}

impl FrameClock for f64 {
    fn frame_time(&self) -> f64 {
        *self
    }
}

/// A context that can start [`Motion`] transitions: event, measure, and
/// arrange contexts.
pub trait AnimateCtx: FrameClock {
    /// See [`crate::EventCtx::animate`].
    fn animate<T>(&mut self, motion: &mut Motion<T>, target: T, spec: AnimationSpec) -> bool
    where
        T: Interpolate + Copy + PartialEq;
}

/// A value the runtime animates for a widget.
///
/// Start a transition with `ctx.animate(&mut motion, target, spec)` and read
/// the value while painting with `motion.get(ctx)`. The runtime invalidates the
/// widget every frame until the transition ends, so the widget needs no
/// animation-frame handling of its own. The value is a function of time, so
/// reading it at a frame's time gives what is on screen then.
///
/// Transitions follow the [`MotionPolicy`]; retargeting mid-flight keeps
/// momentum (see [`MotionValue`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion<T> {
    value: MotionValue<T>,
    invalidation: InvalidationKind,
    movement: bool,
}

impl<T: Copy> Motion<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: MotionValue::new(value),
            invalidation: InvalidationKind::Paint,
            movement: false,
        }
    }

    /// What the runtime invalidates each frame while this animates: `Paint`
    /// (the default) for values read while painting, `Transform` or `Effect`
    /// for layer properties, `Measure` for sizes.
    pub const fn invalidating(mut self, kind: InvalidationKind) -> Self {
        self.invalidation = kind;
        self
    }

    /// Mark this as movement, such as a sliding indicator: reduced motion
    /// finishes it immediately instead of animating it.
    pub const fn movement(mut self) -> Self {
        self.movement = true;
        self
    }

    pub const fn invalidation(&self) -> InvalidationKind {
        self.invalidation
    }

    pub const fn is_movement(&self) -> bool {
        self.movement
    }

    /// The value this motion is heading to.
    pub fn target(&self) -> T {
        self.value.target()
    }

    /// Stop animating and rest at `value`.
    pub fn jump_to(&mut self, value: T) {
        self.value.jump_to(value);
    }
}

impl<T> Motion<T>
where
    T: Interpolate + Copy + PartialEq,
{
    /// The value at the time of the frame `clock` is producing.
    pub fn get(&self, clock: &impl FrameClock) -> T {
        self.at(clock.frame_time())
    }

    /// The value at `time`.
    pub fn at(&self, time: f64) -> T {
        self.value.value(time)
    }

    /// Whether a transition is running at the time of `clock`'s frame.
    pub fn is_animating(&self, clock: &impl FrameClock) -> bool {
        self.value.is_animating_at(clock.frame_time())
    }

    /// When the running transition ends, or `None` at rest.
    pub fn end_time(&self) -> Option<f64> {
        self.value.end_time()
    }

    /// Start toward `target` at `time` with `spec` under the motion policy.
    /// Returns when the transition ends if one started. Contexts' `animate`
    /// methods call this and keep the widget invalidated until then.
    pub fn start(&mut self, target: T, time: f64, spec: AnimationSpec) -> Option<f64> {
        let policy = motion_policy();
        let spec = if self.movement {
            spec.with_movement_policy(policy)
        } else {
            spec.with_policy(policy)
        };
        // Drop finished transitions so the end time reflects only what runs.
        self.value.advance(time);
        if self.value.animate_to(target, time, spec) {
            self.value.end_time()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_preference_overrides_the_system_until_cleared() {
        reset_motion_settings();
        set_system_motion_preference(MotionPreference::Reduced);
        assert_eq!(motion_policy().preference, MotionPreference::Reduced);

        set_app_motion_preference(Some(MotionPreference::Full));
        assert_eq!(motion_policy().preference, MotionPreference::Full);
        assert_eq!(system_motion_preference(), MotionPreference::Reduced);

        set_app_motion_preference(None);
        assert_eq!(motion_policy().preference, MotionPreference::Reduced);
        reset_motion_settings();
    }

    #[test]
    fn motion_reads_its_value_at_the_frame_time() {
        reset_motion_settings();
        let mut motion = Motion::new(0.0_f32);
        let until = motion
            .start(
                1.0,
                10.0,
                AnimationSpec::tween(0.2, sui_animation::Easing::Linear),
            )
            .expect("a transition starts");

        assert!((until - 10.2).abs() < 1e-9);
        assert!((motion.get(&10.1) - 0.5).abs() < 1e-5);
        assert!(motion.is_animating(&10.1));
        assert_eq!(motion.get(&10.3), 1.0);
        assert!(!motion.is_animating(&10.3));
        assert_eq!(motion.target(), 1.0);
    }

    #[test]
    fn motion_follows_the_policy_for_movement() {
        reset_motion_settings();
        set_app_motion_preference(Some(MotionPreference::Reduced));
        let spec = AnimationSpec::tween(0.2, sui_animation::Easing::Linear);

        let mut fade = Motion::new(0.0_f32);
        assert!(fade.start(1.0, 0.0, spec).is_some());
        let mut slide = Motion::new(0.0_f32).movement();
        assert!(slide.start(1.0, 0.0, spec).is_none());
        assert_eq!(slide.get(&0.0), 1.0);
        reset_motion_settings();
    }

    #[test]
    fn time_scale_is_kept_positive() {
        reset_motion_settings();
        set_motion_time_scale(0.25);
        assert_eq!(motion_policy().time_scale, 0.25);

        set_motion_time_scale(-1.0);
        assert_eq!(motion_time_scale(), MotionPolicy::MIN_TIME_SCALE);
        reset_motion_settings();
        assert_eq!(motion_policy(), MotionPolicy::FULL);
    }
}
