use std::{collections::HashMap, sync::{OnceLock, RwLock}};

use sui_core::WindowId;
use sui_render_wgpu::{DisplayCapabilities, DisplayColorPrimaries, OutputStrategy};
use sui_runtime::{
    WindowColorManagementMode, WindowDynamicRangeMode, WindowOutputColorPrimaries,
    WindowToneMappingMode,
};
use winit::window::Window;

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
        return DisplayCapabilities {
            supports_wide_gamut: true,
            supports_hdr: false,
            preferred_primaries: DisplayColorPrimaries::DisplayP3,
            notes: format!(
                "Web output on {monitor_name}: phase-2 scaffold assumes Display-P3 may be available, but canvas HDR capability probing is not wired yet"
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
