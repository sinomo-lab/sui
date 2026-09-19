#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
#[path = "surface/windows.rs"]
mod windows_surface;

use crate::WgpuRenderer;
use crate::diagnostics::RendererFrameStats;
use crate::output::ColorManagementMode;
use crate::output::DisplayCapabilities;
use crate::output::OutputStrategy;
use crate::output::output_transform_requires_intermediate;
use crate::output::select_output_strategy;
use std::sync::Arc;
use sui_core::Error;
use sui_core::Result;
use sui_core::Size;
use sui_core::WindowId;
use sui_scene::SceneFrame;
use web_time::Instant;
use winit::window::Window;

impl WgpuRenderer {
    /// Begin device setup using this native surface before the first CPU layout.
    /// Call `register_window` afterwards to finish configuration. Preparation is
    /// optional, idempotent, and cancelled when the window/renderer is removed.
    pub fn prepare_window(&mut self, window_id: WindowId, window: Arc<Window>) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        if self.shared.is_none() && !self.prepared_surfaces.contains_key(&window_id) {
            let surface = Arc::new(
                self.instance
                    .create_surface(Arc::clone(&window))
                    .map_err(|error| {
                        Error::new(format!("failed to create wgpu surface: {error}"))
                    })?,
            );
            self.start_device_preparation(Some(window_id), Some(surface.clone()));
            self.prepared_surfaces
                .insert(window_id, PreparedSurface { window, surface });
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let _ = (window_id, window);
        Ok(())
    }

    pub fn register_window(&mut self, window_id: WindowId, window: Arc<Window>) -> Result<()> {
        let physical_size = window.inner_size();
        let size = normalize_surface_size(physical_size.width, physical_size.height);
        let state = match self.prepared_surfaces.remove(&window_id) {
            Some(prepared) if Arc::ptr_eq(&prepared.window, &window) => {
                self.configure_new_surface(window, prepared.surface, size)?
            }
            _ => self.create_surface_state(window, size)?,
        };

        self.surfaces.insert(window_id, state);
        self.offscreen_targets.remove(&window_id);
        self.intermediate_targets.remove(&window_id);
        Ok(())
    }

    pub(crate) fn render_surface(
        &mut self,
        frame: &SceneFrame,
        size: (u32, u32),
    ) -> Result<RendererFrameStats> {
        self.ensure_shared(None)?;
        self.resize_surface(frame.window_id, size)?;

        let prepared = self.prepare_scene_submission(frame)?;

        let Some((frame_texture, suboptimal, surface_acquire_time_us)) =
            self.acquire_surface_texture(frame.window_id, size)?
        else {
            return Ok(RendererFrameStats::default());
        };

        let (format, strategy, tone_mapping, sdr_content_brightness_nits, display_sdr_white_nits) = {
            let surface = self.surfaces.get(&frame.window_id).ok_or_else(|| {
                Error::new(format!(
                    "missing surface for window {}",
                    frame.window_id.get()
                ))
            })?;
            (
                surface.config.format,
                surface.output_strategy,
                surface.color_management.tone_mapping,
                surface.color_management.sdr_content_brightness_nits,
                surface.display_capabilities.sdr_white_nits,
            )
        };
        let view = frame_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut frame_stats = if output_transform_requires_intermediate(strategy) {
            let intermediate_view = self.ensure_intermediate_target(frame.window_id, size)?;
            let intermediate_format = self
                .intermediate_targets
                .get(&frame.window_id)
                .map(|target| target.format)
                .ok_or_else(|| Error::new("missing HDR intermediate target after allocation"))?;
            let mut encoder = self.frame_encoder();
            let mut frame_stats = self.encode_prepared_scene(
                prepared,
                intermediate_format,
                &intermediate_view,
                &mut encoder,
            )?;
            self.encode_output_transform_pass(
                frame.window_id,
                &intermediate_view,
                &view,
                format,
                strategy,
                tone_mapping,
                sdr_content_brightness_nits,
                display_sdr_white_nits,
                &mut frame_stats,
                &mut encoder,
            )?;
            self.submit_frame_encoder(encoder, &mut frame_stats);
            frame_stats
        } else {
            self.submit_prepared_scene(prepared, format, &view)?
        };
        frame_stats.surface_acquire_time_us = surface_acquire_time_us;
        let surface_present_started = self.runtime_diagnostics_enabled.then(Instant::now);
        self.shared
            .as_ref()
            .expect("renderer shared state initialized")
            .queue
            .present(frame_texture);
        frame_stats.surface_present_time_us = surface_present_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        if suboptimal {
            self.configure_surface(frame.window_id, size)?;
        }

        Ok(frame_stats)
    }

    pub(crate) fn acquire_surface_texture(
        &mut self,
        window_id: WindowId,
        size: (u32, u32),
    ) -> Result<Option<(wgpu::SurfaceTexture, bool, u64)>> {
        let surface_acquire_started = self.runtime_diagnostics_enabled.then(Instant::now);
        let (frame_texture, suboptimal) = loop {
            let result = {
                let surface = self.surfaces.get(&window_id).ok_or_else(|| {
                    Error::new(format!("missing surface for window {}", window_id.get()))
                })?;
                surface.surface.get_current_texture()
            };

            match result {
                wgpu::CurrentSurfaceTexture::Success(texture) => break (texture, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => break (texture, true),
                wgpu::CurrentSurfaceTexture::Outdated => {
                    self.configure_surface(window_id, size)?;
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    self.recreate_surface(window_id, size)?;
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(None);
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(Error::new(
                        "wgpu surface acquisition triggered a validation error",
                    ));
                }
            }
        };
        let surface_acquire_time_us = surface_acquire_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        Ok(Some((frame_texture, suboptimal, surface_acquire_time_us)))
    }

    pub(crate) fn resize_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let surface = self
            .surfaces
            .get(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

        if surface.config.width == size.0 && surface.config.height == size.1 {
            return Ok(());
        }

        self.configure_surface(window_id, size)
    }

    pub(crate) fn configure_existing_surface(&mut self, window_id: WindowId) -> Result<()> {
        let size = {
            let surface = self.surfaces.get(&window_id).ok_or_else(|| {
                Error::new(format!("missing surface for window {}", window_id.get()))
            })?;
            (surface.config.width.max(1), surface.config.height.max(1))
        };
        self.configure_surface(window_id, size)
    }

    pub(crate) fn configure_surface(
        &mut self,
        window_id: WindowId,
        size: (u32, u32),
    ) -> Result<()> {
        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let surface = self
            .surfaces
            .get_mut(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

        let available_surface_formats = surface.surface.get_capabilities(&shared.adapter).formats;
        let (config, output_strategy) = configure_surface(
            &surface.surface,
            &shared.adapter,
            &shared.device,
            size,
            self.vsync_enabled,
            surface.display_capabilities.clone(),
            surface.color_management,
        )?;
        surface.config = config;
        surface.output_strategy = output_strategy;
        surface.available_surface_formats = available_surface_formats;
        Ok(())
    }

    pub(crate) fn recreate_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let window = self
            .surfaces
            .get(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?
            .window
            .clone();
        let state = self.create_surface_state(window, size)?;
        self.surfaces.insert(window_id, state);
        Ok(())
    }

    pub(crate) fn create_surface_state(
        &mut self,
        window: Arc<Window>,
        size: (u32, u32),
    ) -> Result<SurfaceState> {
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| Error::new(format!("failed to create wgpu surface: {error}")))?;
        self.configure_new_surface(window, Arc::new(surface), size)
    }

    fn configure_new_surface(
        &mut self,
        window: Arc<Window>,
        surface: Arc<wgpu::Surface<'static>>,
        size: (u32, u32),
    ) -> Result<SurfaceState> {
        self.ensure_shared(Some(&surface))?;

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let default_capabilities = DisplayCapabilities::default();
        let default_color_management = ColorManagementMode::default();
        let available_surface_formats = surface.get_capabilities(&shared.adapter).formats;
        let (config, output_strategy) = configure_surface(
            &surface,
            &shared.adapter,
            &shared.device,
            size,
            self.vsync_enabled,
            default_capabilities.clone(),
            default_color_management,
        )?;

        Ok(SurfaceState {
            window,
            surface,
            config,
            display_capabilities: default_capabilities,
            color_management: default_color_management,
            output_strategy,
            available_surface_formats,
        })
    }
}

pub(crate) fn normalize_framebuffer_size(size: Size) -> Option<(u32, u32)> {
    if size.is_empty() {
        None
    } else {
        Some(normalize_surface_size(
            size.width.round() as u32,
            size.height.round() as u32,
        ))
    }
}

pub(crate) fn normalize_surface_size(width: u32, height: u32) -> (u32, u32) {
    (width.max(1), height.max(1))
}

pub(crate) fn configure_surface_for_strategy(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    size: (u32, u32),
    vsync_enabled: bool,
    strategy: OutputStrategy,
) -> Result<wgpu::SurfaceConfiguration> {
    let mut config = surface
        .get_default_config(adapter, size.0, size.1)
        .ok_or_else(|| Error::new("wgpu adapter does not support presenting to this surface"))?;
    config.format = strategy.surface_format();
    config.present_mode = if vsync_enabled {
        wgpu::PresentMode::AutoVsync
    } else {
        wgpu::PresentMode::AutoNoVsync
    };
    let capabilities = surface.get_capabilities(adapter);
    if capabilities
        .alpha_modes
        .contains(&wgpu::CompositeAlphaMode::Opaque)
    {
        config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
    }
    surface.configure(device, &config);
    configure_native_hdr_surface_color_space(surface, strategy)?;
    Ok(config)
}

pub(crate) fn configure_native_hdr_surface_color_space(
    surface: &wgpu::Surface<'static>,
    strategy: OutputStrategy,
) -> Result<()> {
    if !matches!(strategy, OutputStrategy::HdrNativeSurface { .. }) {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        windows_surface::set_native_hdr_surface_color_space(surface)
            .map_err(|error| Error::new(format!("failed to configure native HDR surface: {error}")))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = surface;
        Ok(())
    }
}

pub(crate) fn configure_surface(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    size: (u32, u32),
    vsync_enabled: bool,
    display_capabilities: DisplayCapabilities,
    color_management: ColorManagementMode,
) -> Result<(wgpu::SurfaceConfiguration, OutputStrategy)> {
    let surface_capabilities = surface.get_capabilities(adapter);
    let strategy = select_output_strategy(
        &surface_capabilities.formats,
        display_capabilities,
        color_management,
    );
    let config =
        configure_surface_for_strategy(surface, adapter, device, size, vsync_enabled, strategy)?;
    Ok((config, strategy))
}

pub(crate) struct PreparedSurface {
    window: Arc<Window>,
    surface: Arc<wgpu::Surface<'static>>,
}

pub(crate) struct SurfaceState {
    pub(crate) window: Arc<Window>,
    pub(crate) surface: Arc<wgpu::Surface<'static>>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) display_capabilities: DisplayCapabilities,
    pub(crate) color_management: ColorManagementMode,
    pub(crate) output_strategy: OutputStrategy,
    pub(crate) available_surface_formats: Vec<wgpu::TextureFormat>,
}
