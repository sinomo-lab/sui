#![forbid(unsafe_code)]

mod desktop;
mod headless;

pub use desktop::DesktopPlatform;
pub use headless::{HeadlessPlatform, PlatformWindow};
