use windows::Win32::Graphics::Dxgi::Common::DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709;

pub(super) fn set_native_hdr_surface_color_space(
    surface: &wgpu::Surface<'_>,
) -> Result<(), String> {
    // SAFETY: The callback only borrows the backend surface for this call. wgpu returns `None`
    // when the active backend is not DX12.
    let Some(hal_surface) = (unsafe { surface.as_hal::<wgpu::hal::api::Dx12>() }) else {
        return Ok(());
    };
    let Some(swap_chain) = hal_surface.swap_chain() else {
        return Ok(());
    };

    // SAFETY: The swap chain belongs to the borrowed live wgpu surface, and scRGB is compatible
    // with the float16 surface format selected before this call.
    unsafe { swap_chain.SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709) }
        .map_err(|error| format!("IDXGISwapChain3::SetColorSpace1(scRGB) failed: {error}"))
}
