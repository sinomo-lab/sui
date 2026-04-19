use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709, DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P2020, DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020,
    DXGI_COLOR_SPACE_TYPE,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput6,
};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
use windows::core::Interface;

use crate::{WindowsAdvancedColorProbe, WindowsAdvancedColorSpace};

fn decode_wide_string(buffer: &[u16]) -> String {
    let end = buffer
        .iter()
        .position(|&ch| ch == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

fn map_color_space(color_space: DXGI_COLOR_SPACE_TYPE) -> WindowsAdvancedColorSpace {
    if color_space == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020 {
        WindowsAdvancedColorSpace::Hdr10P2020
    } else if color_space == DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709 {
        WindowsAdvancedColorSpace::ScRgb
    } else if color_space == DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P2020 {
        WindowsAdvancedColorSpace::Rgb2020
    } else if color_space == DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709 {
        WindowsAdvancedColorSpace::Srgb
    } else {
        WindowsAdvancedColorSpace::Unknown
    }
}

pub fn probe_monitor_for_hwnd(hwnd: isize) -> Option<WindowsAdvancedColorProbe> {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let target_monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if target_monitor.0.is_null() {
        return None;
    }

    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1::<IDXGIFactory1>().ok()? };
    let mut adapter_index = 0;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(adapter_index) } {
        let adapter: IDXGIAdapter1 = adapter;
        let mut output_index = 0;
        while let Ok(output) = unsafe { adapter.EnumOutputs(output_index) } {
            let Ok(output6) = output.cast::<IDXGIOutput6>() else {
                output_index += 1;
                continue;
            };
            let Ok(desc) = (unsafe { output6.GetDesc1() }) else {
                output_index += 1;
                continue;
            };
            if desc.Monitor == target_monitor {
                return Some(WindowsAdvancedColorProbe {
                    device_name: Some(decode_wide_string(&desc.DeviceName)),
                    bits_per_color: desc.BitsPerColor,
                    color_space: map_color_space(desc.ColorSpace),
                    red_primary: Some(desc.RedPrimary),
                    green_primary: Some(desc.GreenPrimary),
                    blue_primary: Some(desc.BluePrimary),
                    white_point: Some(desc.WhitePoint),
                    min_luminance_nits: Some(desc.MinLuminance),
                    max_luminance_nits: Some(desc.MaxLuminance),
                    max_full_frame_luminance_nits: Some(desc.MaxFullFrameLuminance),
                });
            }
            output_index += 1;
        }
        adapter_index += 1;
    }

    None
}
