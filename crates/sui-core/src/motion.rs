/// How much motion the user wants to see.
///
/// Operating systems expose this as an accessibility setting (Windows "Show
/// animations in Windows", macOS "Reduce motion", the web's
/// `prefers-reduced-motion`). Motion can trigger discomfort, so widgets honor
/// it by dropping movement rather than just shortening it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MotionPreference {
    /// Play every animation.
    #[default]
    Full,
    /// Keep fades and color changes, but drop movement: surfaces fade in place
    /// instead of sliding, and sliding indicators jump to their destination.
    Reduced,
    /// Skip animations: every transition finishes immediately.
    Off,
}

impl MotionPreference {
    pub const ALL: [Self; 3] = [Self::Full, Self::Reduced, Self::Off];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Reduced => "Reduced",
            Self::Off => "Off",
        }
    }
}

/// The motion policy in effect: the user's [`MotionPreference`] plus a time
/// scale for watching transitions in slow motion while debugging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionPolicy {
    pub preference: MotionPreference,
    /// Playback speed of transitions relative to their nominal durations:
    /// `0.25` plays them at quarter speed. Values are kept at or above
    /// [`MotionPolicy::MIN_TIME_SCALE`].
    pub time_scale: f32,
}

impl MotionPolicy {
    pub const MIN_TIME_SCALE: f32 = 0.01;
    pub const FULL: Self = Self::new(MotionPreference::Full);

    pub const fn new(preference: MotionPreference) -> Self {
        Self {
            preference,
            time_scale: 1.0,
        }
    }

    pub const fn with_time_scale(mut self, time_scale: f32) -> Self {
        self.time_scale = time_scale;
        self
    }

    /// Whether transitions animate at all.
    pub fn allows_motion(self) -> bool {
        self.preference != MotionPreference::Off
    }

    /// Whether transitions may move content across the screen.
    pub fn allows_movement(self) -> bool {
        self.preference == MotionPreference::Full
    }

    /// The effective time scale, kept positive.
    pub fn time_scale(self) -> f32 {
        if self.time_scale.is_finite() {
            self.time_scale.max(Self::MIN_TIME_SCALE)
        } else {
            1.0
        }
    }

    /// How long a transition that lasts `nominal` seconds at full motion
    /// should take: zero when motion is off, stretched by the time scale
    /// otherwise.
    pub fn duration(self, nominal: f64) -> f64 {
        if self.allows_motion() {
            nominal.max(0.0) / f64::from(self.time_scale())
        } else {
            0.0
        }
    }

    /// Like [`MotionPolicy::duration`], for a transition that moves content:
    /// zero unless the policy allows movement.
    pub fn movement_duration(self, nominal: f64) -> f64 {
        if self.allows_movement() {
            self.duration(nominal)
        } else {
            0.0
        }
    }

    /// Scale a frame delta for animation driven by elapsed time, such as a
    /// timeline player or a simulation, so it follows the time scale.
    pub fn scale_delta(self, delta: f64) -> f64 {
        delta * f64::from(self.time_scale())
    }

    /// The share of an entrance offset (such as a popover sliding into
    /// place) to apply at `progress`, from 1 when hidden to 0 when shown.
    /// Without movement, surfaces fade in place.
    pub fn entrance_offset(self, progress: f32) -> f32 {
        if self.allows_movement() {
            1.0 - progress
        } else {
            0.0
        }
    }
}

impl Default for MotionPolicy {
    fn default() -> Self {
        Self::FULL
    }
}

#[cfg(test)]
mod tests {
    use super::{MotionPolicy, MotionPreference};

    #[test]
    fn full_motion_keeps_nominal_durations() {
        let policy = MotionPolicy::FULL;

        assert_eq!(policy.duration(0.14), 0.14);
        assert_eq!(policy.movement_duration(0.14), 0.14);
        assert_eq!(policy.entrance_offset(0.25), 0.75);
    }

    #[test]
    fn time_scale_stretches_durations_and_slows_deltas() {
        let policy = MotionPolicy::FULL.with_time_scale(0.25);

        assert_eq!(policy.duration(0.1), 0.4);
        assert_eq!(policy.scale_delta(0.1), 0.025);
        assert_eq!(
            MotionPolicy::FULL.with_time_scale(0.0).time_scale(),
            MotionPolicy::MIN_TIME_SCALE
        );
        assert_eq!(
            MotionPolicy::FULL.with_time_scale(f32::NAN).time_scale(),
            1.0
        );
    }

    #[test]
    fn reduced_motion_keeps_fades_but_drops_movement() {
        let policy = MotionPolicy::new(MotionPreference::Reduced);

        assert!(policy.allows_motion());
        assert!(!policy.allows_movement());
        assert_eq!(policy.duration(0.14), 0.14);
        assert_eq!(policy.movement_duration(0.14), 0.0);
        assert_eq!(policy.entrance_offset(0.25), 0.0);
    }

    #[test]
    fn motion_off_finishes_every_transition_immediately() {
        let policy = MotionPolicy::new(MotionPreference::Off);

        assert!(!policy.allows_motion());
        assert_eq!(policy.duration(0.34), 0.0);
        assert_eq!(policy.movement_duration(0.34), 0.0);
    }
}
