//! Reads the operating system's reduced-motion setting and reports it to the
//! runtime, which widgets consult when they start a transition.

use sui_core::MotionPreference;

/// The operating system's motion preference, or full motion where the
/// platform does not expose one.
pub(crate) fn system_motion_preference() -> MotionPreference {
    platform::read().unwrap_or_default()
}

/// Report the current system motion preference to the runtime. Called at
/// startup and when a window gains focus, since the setting can change while
/// the app runs.
pub(crate) fn sync_system_motion_preference() {
    sui_runtime::set_system_motion_preference(system_motion_preference());
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod platform {
    use sui_core::MotionPreference;
    use windows::{
        Win32::UI::WindowsAndMessaging::{
            SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
        core::BOOL,
    };

    /// "Show animations in Windows" (Settings > Accessibility > Visual
    /// effects).
    pub(super) fn read() -> Option<MotionPreference> {
        let mut enabled = BOOL(1);
        // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through the
        // pointer, which points at a live, writable BOOL.
        unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&raw mut enabled).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .ok()?;
        Some(if enabled.as_bool() {
            MotionPreference::Full
        } else {
            MotionPreference::Reduced
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use sui_core::MotionPreference;

    /// The `prefers-reduced-motion` media query.
    pub(super) fn read() -> Option<MotionPreference> {
        let query = web_sys::window()?
            .match_media("(prefers-reduced-motion: reduce)")
            .ok()
            .flatten()?;
        Some(if query.matches() {
            MotionPreference::Reduced
        } else {
            MotionPreference::Full
        })
    }
}

#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
mod platform {
    use sui_core::MotionPreference;

    pub(super) fn read() -> Option<MotionPreference> {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reading_the_system_preference_never_fails() {
        // The value depends on the machine; reading it must simply work.
        let _ = super::system_motion_preference();
    }
}
