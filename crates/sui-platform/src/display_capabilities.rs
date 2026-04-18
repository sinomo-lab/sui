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
    pub active_output_strategy: OutputStrategy,
}

fn diagnostics_store() -> &'static RwLock<HashMap<WindowId, WindowOutputDiagnostics>> {
    static STORE: OnceLock<RwLock<HashMap<WindowId, WindowOutputDiagnostics>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn publish_window_output_diagnostics(window_id: WindowId, diagnostics: WindowOutputDiagnostics) {
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

pub fn detect_window_display_capabilities(window: &Window) -> DisplayCapabilities {
    let monitor_name = window
        .current_monitor()
        .and_then(|monitor| monitor.name())
        .unwrap_or_else(|| "unknown monitor".to_string());

    #[cfg(target_os = "windows")]
    {
        return DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: false,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            notes: format!(
                "Windows monitor {monitor_name}: conservative phase-2 heuristic assumes wide-gamut SDR may be available; native HDR detection is not wired yet"
            ),
            ..DisplayCapabilities::default()
        };
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
                hints.float16_canvas,
                hints.display_p3,
                hints.extended_tone_mapping,
                hints.hdr,
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
    use super::parse_web_capability_hints;

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
}
