#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowsAdvancedColorSpace {
    #[default]
    Srgb,
    ScRgb,
    Hdr10P2020,
    Rgb2020,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowsAdvancedColorProbe {
    pub device_name: Option<String>,
    pub bits_per_color: u32,
    pub color_space: WindowsAdvancedColorSpace,
    pub red_primary: Option<[f32; 2]>,
    pub green_primary: Option<[f32; 2]>,
    pub blue_primary: Option<[f32; 2]>,
    pub white_point: Option<[f32; 2]>,
    pub min_luminance_nits: Option<f32>,
    pub max_luminance_nits: Option<f32>,
    pub max_full_frame_luminance_nits: Option<f32>,
    pub sdr_white_nits: Option<f32>,
}

#[cfg(target_os = "windows")]
mod dxgi;

#[cfg(target_os = "windows")]
pub use dxgi::probe_monitor_for_hwnd;

#[cfg(not(target_os = "windows"))]
pub fn probe_monitor_for_hwnd(_hwnd: isize) -> Option<WindowsAdvancedColorProbe> {
    None
}
