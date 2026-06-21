use std::fmt;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(raw: u64) -> Self {
                Self(raw)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(WidgetId);
define_id!(
    /// Identifies a host render/input target.
    ///
    /// A `WindowId` may refer to a native platform window or to an embedded
    /// viewport/region owned by a host application. Platform events, runtime
    /// scheduling, and `SceneFrame` submission use this ID as the common target
    /// identity.
    WindowId
);
define_id!(SurfaceId);
define_id!(ImageHandle);
define_id!(FontHandle);
define_id!(TimerToken);
define_id!(AsyncWakeToken);
