use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QDC_VIRTUAL_MODE_AWARE, QueryDisplayConfig,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HWND};
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

fn sdr_white_level_nits(raw_level: u32) -> Option<f32> {
    let nits = raw_level as f32 / 1000.0 * 80.0;
    (nits.is_finite() && nits > 0.0).then_some(nits)
}

fn query_active_display_paths() -> Option<Vec<DISPLAYCONFIG_PATH_INFO>> {
    let flags = QDC_ONLY_ACTIVE_PATHS | QDC_VIRTUAL_MODE_AWARE;

    loop {
        let mut path_count = 0;
        let mut mode_count = 0;
        let size_result =
            unsafe { GetDisplayConfigBufferSizes(flags, &mut path_count, &mut mode_count) };
        if size_result != ERROR_SUCCESS {
            return None;
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let query_result = unsafe {
            QueryDisplayConfig(
                flags,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            )
        };

        if query_result == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if query_result != ERROR_SUCCESS {
            return None;
        }

        paths.truncate(path_count as usize);
        return Some(paths);
    }
}

fn source_device_name_for_path(path: &DISPLAYCONFIG_PATH_INFO) -> Option<String> {
    let mut source_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            adapterId: path.sourceInfo.adapterId,
            id: path.sourceInfo.id,
        },
        ..DISPLAYCONFIG_SOURCE_DEVICE_NAME::default()
    };

    let result = unsafe { DisplayConfigGetDeviceInfo(&mut source_name.header) };
    (result == ERROR_SUCCESS.0 as i32)
        .then(|| decode_wide_string(&source_name.viewGdiDeviceName))
        .filter(|name| !name.is_empty())
}

fn sdr_white_nits_for_path(path: &DISPLAYCONFIG_PATH_INFO) -> Option<f32> {
    let mut sdr_white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
            size: std::mem::size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        ..DISPLAYCONFIG_SDR_WHITE_LEVEL::default()
    };

    let result = unsafe { DisplayConfigGetDeviceInfo(&mut sdr_white.header) };
    (result == ERROR_SUCCESS.0 as i32)
        .then_some(sdr_white.SDRWhiteLevel)
        .and_then(sdr_white_level_nits)
}

fn query_sdr_white_nits_for_gdi_device(device_name: &str) -> Option<f32> {
    query_active_display_paths()?
        .iter()
        .find(|path| {
            source_device_name_for_path(path)
                .is_some_and(|source_name| source_name.eq_ignore_ascii_case(device_name))
        })
        .and_then(sdr_white_nits_for_path)
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
                let device_name = decode_wide_string(&desc.DeviceName);
                let sdr_white_nits = query_sdr_white_nits_for_gdi_device(&device_name);
                return Some(WindowsAdvancedColorProbe {
                    device_name: Some(device_name),
                    bits_per_color: desc.BitsPerColor,
                    color_space: map_color_space(desc.ColorSpace),
                    red_primary: Some(desc.RedPrimary),
                    green_primary: Some(desc.GreenPrimary),
                    blue_primary: Some(desc.BluePrimary),
                    white_point: Some(desc.WhitePoint),
                    min_luminance_nits: Some(desc.MinLuminance),
                    max_luminance_nits: Some(desc.MaxLuminance),
                    max_full_frame_luminance_nits: Some(desc.MaxFullFrameLuminance),
                    sdr_white_nits,
                });
            }
            output_index += 1;
        }
        adapter_index += 1;
    }

    None
}

pub fn set_native_hdr_surface_color_space(surface: &wgpu::Surface<'_>) -> Result<(), String> {
    let Some(hal_surface) = (unsafe { surface.as_hal::<wgpu::hal::api::Dx12>() }) else {
        return Ok(());
    };
    let Some(swap_chain) = hal_surface.swap_chain() else {
        return Ok(());
    };

    unsafe { swap_chain.SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709) }
        .map_err(|error| format!("IDXGISwapChain3::SetColorSpace1(scRGB) failed: {error}"))
}
