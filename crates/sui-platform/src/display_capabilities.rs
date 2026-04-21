use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

use sui_core::WindowId;
use sui_render_wgpu::{DisplayCapabilities, DisplayColorPrimaries, OutputStrategy};
use sui_runtime::{
    WindowColorManagementMode, WindowDynamicRangeMode, WindowOutputColorPrimaries,
    WindowToneMappingMode,
};
use winit::window::Window;

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct WebCapabilityHints {
    wide_gamut: bool,
    hdr: bool,
    display_p3: bool,
    float16_canvas: bool,
    extended_tone_mapping: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowOutputDiagnostics {
    pub display_capabilities: DisplayCapabilities,
    pub requested_color_management_mode: WindowColorManagementMode,
    pub requested_output_primaries: WindowOutputColorPrimaries,
    pub requested_dynamic_range_mode: WindowDynamicRangeMode,
    pub requested_tone_mapping_mode: WindowToneMappingMode,
    pub requested_sdr_content_brightness_nits: f32,
    pub active_output_strategy: OutputStrategy,
}

fn diagnostics_store() -> &'static RwLock<HashMap<WindowId, WindowOutputDiagnostics>> {
    static STORE: OnceLock<RwLock<HashMap<WindowId, WindowOutputDiagnostics>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn publish_window_output_diagnostics(
    window_id: WindowId,
    diagnostics: WindowOutputDiagnostics,
) {
    let mut store = diagnostics_store()
        .write()
        .expect("window output diagnostics store lock should not be poisoned");
    store.insert(window_id, diagnostics);
}

pub fn window_output_diagnostics(window_id: WindowId) -> Option<WindowOutputDiagnostics> {
    let store = diagnostics_store()
        .read()
        .expect("window output diagnostics store lock should not be poisoned");
    store.get(&window_id).cloned()
}

pub fn clear_window_output_diagnostics(window_id: WindowId) {
    let mut store = diagnostics_store()
        .write()
        .expect("window output diagnostics store lock should not be poisoned");
    store.remove(&window_id);
}

pub fn clear_window_output_diagnostics_all() {
    let mut store = diagnostics_store()
        .write()
        .expect("window output diagnostics store lock should not be poisoned");
    store.clear();
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn parse_web_capability_hints(query: &str) -> WebCapabilityHints {
    let mut hints = WebCapabilityHints::default();
    for pair in query.trim_start_matches('?').split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        match key {
            "canvas-format" if matches!(value, "rgba16float" | "float16" | "hdr") => {
                hints.float16_canvas = true;
                hints.hdr = true;
            }
            "canvas-color-space" if matches!(value, "display-p3" | "p3") => {
                hints.display_p3 = true;
                hints.wide_gamut = true;
            }
            "canvas-tone-mapping" if matches!(value, "extended" | "hdr") => {
                hints.extended_tone_mapping = true;
                hints.hdr = true;
            }
            "color-management" if matches!(value, "prefer-wide-gamut" | "prefer-hdr") => {
                hints.wide_gamut = true;
            }
            "color-management" if value == "prefer-hdr" => {
                hints.hdr = true;
            }
            "output-primaries" if matches!(value, "display-p3" | "p3") => {
                hints.display_p3 = true;
                hints.wide_gamut = true;
            }
            "dynamic-range" if matches!(value, "hdr" | "high") => {
                hints.hdr = true;
            }
            _ => {}
        }
    }
    hints
}

#[cfg(any(target_os = "windows", test))]
const WINDOWS_SDR_REFERENCE_WHITE_NITS: f32 = 80.0;
#[cfg(any(target_os = "windows", test))]
const SRGB_PRIMARIES: [[f32; 2]; 3] = [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]];
#[cfg(any(target_os = "windows", test))]
const DISPLAY_P3_PRIMARIES: [[f32; 2]; 3] = [[0.68, 0.32], [0.265, 0.69], [0.15, 0.06]];

#[cfg(any(target_os = "windows", test))]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum WindowsColorSpace {
    #[default]
    Srgb,
    ScRgb,
    Hdr10P2020,
    Rgb2020,
    Unknown,
}

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, PartialEq)]
struct WindowsAdvancedColorInfo {
    monitor_name: String,
    device_name: Option<String>,
    bits_per_color: u32,
    color_space: WindowsColorSpace,
    red_primary: Option<[f32; 2]>,
    green_primary: Option<[f32; 2]>,
    blue_primary: Option<[f32; 2]>,
    white_point: Option<[f32; 2]>,
    min_luminance_nits: Option<f32>,
    max_luminance_nits: Option<f32>,
    max_full_frame_luminance_nits: Option<f32>,
}

#[cfg(any(target_os = "windows", test))]
fn finite_positive(value: Option<f32>) -> Option<f32> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

#[cfg(any(target_os = "windows", test))]
fn primary_triplet(info: &WindowsAdvancedColorInfo) -> Option<[[f32; 2]; 3]> {
    Some([info.red_primary?, info.green_primary?, info.blue_primary?])
}

#[cfg(any(target_os = "windows", test))]
fn gamut_distance(primaries: [[f32; 2]; 3], reference: [[f32; 2]; 3]) -> f32 {
    primaries
        .into_iter()
        .zip(reference)
        .map(|(observed, expected)| {
            let dx = observed[0] - expected[0];
            let dy = observed[1] - expected[1];
            dx * dx + dy * dy
        })
        .sum()
}

#[cfg(any(target_os = "windows", test))]
fn looks_like_display_p3(info: &WindowsAdvancedColorInfo) -> bool {
    let Some(primaries) = primary_triplet(info) else {
        return false;
    };
    if primaries.iter().flatten().any(|value| !value.is_finite()) {
        return false;
    }
    gamut_distance(primaries, DISPLAY_P3_PRIMARIES) < gamut_distance(primaries, SRGB_PRIMARIES)
}

#[cfg(any(target_os = "windows", test))]
fn display_capabilities_from_windows_advanced_color_info(
    info: &WindowsAdvancedColorInfo,
) -> DisplayCapabilities {
    let hdr_active = matches!(
        info.color_space,
        WindowsColorSpace::ScRgb | WindowsColorSpace::Hdr10P2020
    );
    let display_p3_like = looks_like_display_p3(info);
    let supports_wide_gamut = hdr_active
        || display_p3_like
        || matches!(info.color_space, WindowsColorSpace::Rgb2020)
        || info.bits_per_color > 8;
    let max_luminance_nits = finite_positive(info.max_luminance_nits);
    let max_full_frame_luminance_nits = finite_positive(info.max_full_frame_luminance_nits);
    let min_luminance_nits = finite_positive(info.min_luminance_nits);
    let native_hdr_presentation_supported = hdr_active && info.bits_per_color >= 10;
    let preferred_primaries = if hdr_active {
        DisplayColorPrimaries::Srgb
    } else if supports_wide_gamut {
        DisplayColorPrimaries::DisplayP3
    } else {
        DisplayColorPrimaries::Srgb
    };
    let preferred_dynamic_range = if hdr_active {
        sui_render_wgpu::DynamicRangeMode::HighDynamicRange
    } else {
        sui_render_wgpu::DynamicRangeMode::StandardDynamicRange
    };
    let sdr_white_nits = hdr_active.then_some(WINDOWS_SDR_REFERENCE_WHITE_NITS);
    let max_content_headroom = if hdr_active {
        max_luminance_nits.map(|nits| nits / WINDOWS_SDR_REFERENCE_WHITE_NITS)
    } else {
        None
    };
    let mode_summary = match info.color_space {
        WindowsColorSpace::Hdr10P2020 => {
            "Advanced Color enabled (PQ/P2020 monitor mode -> scRGB presentation path)"
        }
        WindowsColorSpace::ScRgb => "Advanced Color enabled (linear scRGB monitor mode)",
        WindowsColorSpace::Rgb2020 => {
            "Advanced Color disabled; monitor reports wide-gamut SDR primaries"
        }
        WindowsColorSpace::Srgb => {
            if supports_wide_gamut {
                "Advanced Color disabled; monitor looks like wide-gamut SDR"
            } else {
                "Advanced Color disabled; monitor reports SDR/sRGB"
            }
        }
        WindowsColorSpace::Unknown => {
            "Advanced Color state unknown; using conservative Windows capability mapping"
        }
    };
    let gamut_summary = match preferred_primaries {
        DisplayColorPrimaries::Srgb if hdr_active => {
            "native HDR surface uses scRGB / P709 primaries"
        }
        DisplayColorPrimaries::DisplayP3 => {
            "wide-gamut SDR path prefers Display-P3-style primaries"
        }
        DisplayColorPrimaries::Srgb => "SDR path prefers sRGB primaries",
    };

    DisplayCapabilities {
        supports_wide_gamut,
        supports_hdr: hdr_active,
        preferred_primaries,
        preferred_dynamic_range,
        max_luminance_nits,
        sdr_white_nits,
        max_content_headroom,
        native_hdr_presentation_supported,
        notes: format!(
            "Windows monitor {}{}: {}; {}; bits_per_color={}; min_luminance_nits={:?}; max_full_frame_luminance_nits={:?}; white_point={:?}",
            info.monitor_name,
            info.device_name
                .as_ref()
                .map(|name| format!(" ({name})"))
                .unwrap_or_default(),
            mode_summary,
            gamut_summary,
            info.bits_per_color,
            min_luminance_nits,
            max_full_frame_luminance_nits,
            info.white_point,
        ),
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_monitor_capabilities(
    window: &Window,
    monitor_name: &str,
) -> Option<DisplayCapabilities> {
    use sui_platform_windows::{
        WindowsAdvancedColorProbe, WindowsAdvancedColorSpace, probe_monitor_for_hwnd,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    fn map_probe_color_space(color_space: WindowsAdvancedColorSpace) -> WindowsColorSpace {
        match color_space {
            WindowsAdvancedColorSpace::Srgb => WindowsColorSpace::Srgb,
            WindowsAdvancedColorSpace::ScRgb => WindowsColorSpace::ScRgb,
            WindowsAdvancedColorSpace::Hdr10P2020 => WindowsColorSpace::Hdr10P2020,
            WindowsAdvancedColorSpace::Rgb2020 => WindowsColorSpace::Rgb2020,
            WindowsAdvancedColorSpace::Unknown => WindowsColorSpace::Unknown,
        }
    }

    let hwnd = match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => handle.hwnd.get() as isize,
        _ => return None,
    };
    let probe: WindowsAdvancedColorProbe = probe_monitor_for_hwnd(hwnd)?;
    let info = WindowsAdvancedColorInfo {
        monitor_name: monitor_name.to_string(),
        device_name: probe.device_name,
        bits_per_color: probe.bits_per_color,
        color_space: map_probe_color_space(probe.color_space),
        red_primary: probe.red_primary,
        green_primary: probe.green_primary,
        blue_primary: probe.blue_primary,
        white_point: probe.white_point,
        min_luminance_nits: probe.min_luminance_nits,
        max_luminance_nits: probe.max_luminance_nits,
        max_full_frame_luminance_nits: probe.max_full_frame_luminance_nits,
    };
    Some(display_capabilities_from_windows_advanced_color_info(&info))
}

pub fn detect_window_display_capabilities(window: &Window) -> DisplayCapabilities {
    let monitor_name = window
        .current_monitor()
        .and_then(|monitor| monitor.name())
        .unwrap_or_else(|| "unknown monitor".to_string());

    #[cfg(target_os = "windows")]
    {
        return detect_windows_monitor_capabilities(window, &monitor_name).unwrap_or_else(|| {
            DisplayCapabilities {
                supports_wide_gamut: false,
                supports_hdr: false,
                preferred_primaries: DisplayColorPrimaries::Srgb,
                notes: format!(
                    "Windows monitor {monitor_name}: DXGI Advanced Color probe failed; falling back to SDR/sRGB defaults"
                ),
                ..DisplayCapabilities::default()
            }
        });
    }

    #[cfg(target_os = "macos")]
    {
        return DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: false,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            notes: format!(
                "macOS monitor {monitor_name}: conservative phase-2 heuristic assumes Display-P3 SDR; EDR headroom detection is not wired yet"
            ),
            ..DisplayCapabilities::default()
        };
    }

    #[cfg(target_arch = "wasm32")]
    {
        let query = web_sys::window()
            .and_then(|window| window.location().search().ok())
            .unwrap_or_default();
        let hints = parse_web_capability_hints(&query);
        return DisplayCapabilities {
            supports_wide_gamut: hints.wide_gamut || hints.display_p3,
            supports_hdr: hints.hdr,
            preferred_primaries: if hints.display_p3 {
                DisplayColorPrimaries::DisplayP3
            } else {
                DisplayColorPrimaries::Srgb
            },
            preferred_dynamic_range: if hints.hdr {
                sui_render_wgpu::DynamicRangeMode::HighDynamicRange
            } else {
                sui_render_wgpu::DynamicRangeMode::StandardDynamicRange
            },
            native_hdr_presentation_supported: hints.float16_canvas && hints.extended_tone_mapping,
            notes: format!(
                "Web output on {monitor_name}: query hints -> float16_canvas={} display_p3={} extended_tone_mapping={} hdr={}.",
                hints.float16_canvas, hints.display_p3, hints.extended_tone_mapping, hints.hdr,
            ),
            ..DisplayCapabilities::default()
        };
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_arch = "wasm32")))]
    {
        DisplayCapabilities {
            supports_wide_gamut: false,
            supports_hdr: false,
            preferred_primaries: DisplayColorPrimaries::Srgb,
            notes: format!(
                "Monitor {monitor_name}: no native phase-2 capability probe for this platform yet; using SDR/sRGB defaults"
            ),
            ..DisplayCapabilities::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DisplayColorPrimaries, WindowsAdvancedColorInfo, WindowsColorSpace,
        display_capabilities_from_windows_advanced_color_info, parse_web_capability_hints,
    };
    use sui_render_wgpu::DynamicRangeMode;

    #[test]
    fn parse_web_capability_hints_detects_phase4_query_preferences() {
        let hints = parse_web_capability_hints(
            "?canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-hdr&dynamic-range=hdr",
        );

        assert!(hints.float16_canvas);
        assert!(hints.display_p3);
        assert!(hints.extended_tone_mapping);
        assert!(hints.wide_gamut);
        assert!(hints.hdr);
    }

    #[test]
    fn windows_hdr_advanced_color_maps_to_scrgb_capabilities() {
        let capabilities =
            display_capabilities_from_windows_advanced_color_info(&WindowsAdvancedColorInfo {
                monitor_name: "HDR Panel".to_string(),
                device_name: Some("\\\\.\\DISPLAY1".to_string()),
                bits_per_color: 10,
                color_space: WindowsColorSpace::Hdr10P2020,
                red_primary: Some([0.68, 0.32]),
                green_primary: Some([0.265, 0.69]),
                blue_primary: Some([0.15, 0.06]),
                white_point: Some([0.3127, 0.3290]),
                min_luminance_nits: Some(0.05),
                max_luminance_nits: Some(1000.0),
                max_full_frame_luminance_nits: Some(600.0),
            });

        assert!(capabilities.supports_hdr);
        assert!(capabilities.native_hdr_presentation_supported);
        assert!(capabilities.supports_wide_gamut);
        assert_eq!(
            capabilities.preferred_primaries,
            DisplayColorPrimaries::Srgb
        );
        assert_eq!(
            capabilities.preferred_dynamic_range,
            DynamicRangeMode::HighDynamicRange
        );
        assert_eq!(capabilities.sdr_white_nits, Some(80.0));
        assert_eq!(capabilities.max_content_headroom, Some(12.5));
        assert!(capabilities.notes.contains("Advanced Color"));
        assert!(capabilities.notes.contains("scRGB"));
    }

    #[test]
    fn windows_sdr_display_p3_monitor_maps_to_wide_gamut_sdr_capabilities() {
        let capabilities =
            display_capabilities_from_windows_advanced_color_info(&WindowsAdvancedColorInfo {
                monitor_name: "Wide Gamut SDR".to_string(),
                device_name: Some("\\\\.\\DISPLAY2".to_string()),
                bits_per_color: 8,
                color_space: WindowsColorSpace::Srgb,
                red_primary: Some([0.68, 0.32]),
                green_primary: Some([0.265, 0.69]),
                blue_primary: Some([0.15, 0.06]),
                white_point: Some([0.3127, 0.3290]),
                min_luminance_nits: None,
                max_luminance_nits: Some(300.0),
                max_full_frame_luminance_nits: Some(280.0),
            });

        assert!(capabilities.supports_wide_gamut);
        assert!(!capabilities.supports_hdr);
        assert!(!capabilities.native_hdr_presentation_supported);
        assert_eq!(
            capabilities.preferred_primaries,
            DisplayColorPrimaries::DisplayP3
        );
        assert_eq!(
            capabilities.preferred_dynamic_range,
            DynamicRangeMode::StandardDynamicRange
        );
        assert!(capabilities.notes.contains("wide-gamut SDR"));
    }
}
