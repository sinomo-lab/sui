//! The motion policy shared by every widget on a UI thread.
//!
//! The platform reports the operating system's motion preference, an app can
//! override it (for example from a settings page), and a time scale slows
//! every transition down for debugging. Widgets read the combined
//! [`MotionPolicy`] when they start a transition, so changes apply to the
//! next animation rather than cutting running ones short.

use std::cell::Cell;

use sui_core::{MotionPolicy, MotionPreference};

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
