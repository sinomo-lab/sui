use crate::WgpuRenderer;
use crate::diagnostics::RendererFrameStats;
use crate::resources::OffscreenTarget;
use bytemuck::Pod;
use bytemuck::Zeroable;
use sui_core::Color;
use sui_core::Error;
use sui_core::Result;
use sui_core::WindowId;
use sui_scene::SceneFrame;
use web_time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCapabilities {
    pub supports_color_management: bool,
    pub supports_offscreen_surfaces: bool,
}

impl Default for RendererCapabilities {
    fn default() -> Self {
        Self {
            supports_color_management: true,
            supports_offscreen_surfaces: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayColorPrimaries {
    #[default]
    Srgb,
    DisplayP3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicRangeMode {
    #[default]
    StandardDynamicRange,
    HighDynamicRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayTransferFunction {
    #[default]
    Srgb,
    LinearExtended,
    Pq,
    Hlg,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayCapabilities {
    pub supports_wide_gamut: bool,
    pub supports_hdr: bool,
    pub preferred_primaries: DisplayColorPrimaries,
    pub preferred_dynamic_range: DynamicRangeMode,
    pub max_luminance_nits: Option<f32>,
    pub sdr_white_nits: Option<f32>,
    pub max_content_headroom: Option<f32>,
    pub native_hdr_presentation_supported: bool,
    pub notes: String,
}

impl Default for DisplayCapabilities {
    fn default() -> Self {
        Self {
            supports_wide_gamut: false,
            supports_hdr: false,
            preferred_primaries: DisplayColorPrimaries::Srgb,
            preferred_dynamic_range: DynamicRangeMode::StandardDynamicRange,
            max_luminance_nits: None,
            sdr_white_nits: None,
            max_content_headroom: None,
            native_hdr_presentation_supported: false,
            notes: "Default SDR capability profile".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestedOutputColorPrimaries {
    #[default]
    Automatic,
    Srgb,
    DisplayP3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestedDynamicRangeMode {
    #[default]
    Automatic,
    StandardDynamicRange,
    HighDynamicRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestedColorManagementMode {
    #[default]
    Automatic,
    ForceSdr,
    PreferWideGamut,
    PreferHdr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestedToneMappingMode {
    #[default]
    Automatic,
    Clamp,
    Reinhard,
}

pub const DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS: f32 = 203.0;
pub(crate) const SCRGB_REFERENCE_WHITE_NITS: f32 = 80.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorManagementMode {
    pub mode: RequestedColorManagementMode,
    pub output_primaries: RequestedOutputColorPrimaries,
    pub dynamic_range: RequestedDynamicRangeMode,
    pub tone_mapping: RequestedToneMappingMode,
    pub sdr_content_brightness_nits: f32,
}

impl Default for ColorManagementMode {
    fn default() -> Self {
        Self {
            mode: RequestedColorManagementMode::Automatic,
            output_primaries: RequestedOutputColorPrimaries::Automatic,
            dynamic_range: RequestedDynamicRangeMode::Automatic,
            tone_mapping: RequestedToneMappingMode::Automatic,
            sdr_content_brightness_nits: DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStrategy {
    SdrSurface {
        format: wgpu::TextureFormat,
    },
    WideGamutSurface {
        format: wgpu::TextureFormat,
        primaries: DisplayColorPrimaries,
    },
    HdrNativeSurface {
        format: wgpu::TextureFormat,
        primaries: DisplayColorPrimaries,
        transfer: DisplayTransferFunction,
    },
    /// Debug/offscreen-only path for inspecting HDR scene values on SDR outputs.
    ///
    /// Normal presentation should choose native HDR when the environment can present
    /// HDR end-to-end; otherwise it should render SDR instead of tone mapping HDR.
    HdrIntermediateThenToneMap {
        intermediate_format: wgpu::TextureFormat,
        surface_format: wgpu::TextureFormat,
        primaries: DisplayColorPrimaries,
    },
}

impl OutputStrategy {
    pub const fn surface_format(self) -> wgpu::TextureFormat {
        match self {
            Self::SdrSurface { format }
            | Self::WideGamutSurface { format, .. }
            | Self::HdrNativeSurface { format, .. } => format,
            Self::HdrIntermediateThenToneMap { surface_format, .. } => surface_format,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugCaptureStage {
    HdrIntermediate,
    #[default]
    FinalComposed,
}

impl DebugCaptureStage {
    pub const fn is_hdr_capable(self) -> bool {
        matches!(self, Self::HdrIntermediate)
    }

    pub const fn uses_hdr_intermediate(self) -> bool {
        matches!(self, Self::HdrIntermediate)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugSdrVisualization {
    #[default]
    ToneMappedColor,
    LuminanceHeatmap,
    HeadroomHeatmap,
    ClipMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugCaptureEncoding {
    Exr,
    #[default]
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DebugCaptureRequest {
    pub stage: DebugCaptureStage,
    pub encoding: DebugCaptureEncoding,
    pub sdr_visualization: DebugSdrVisualization,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct OutputTransformUniform {
    pub(crate) tone_mapping_mode: u32,
    pub(crate) encode_srgb: u32,
    pub(crate) output_primaries: u32,
    pub(crate) _padding0: u32,
    pub(crate) sdr_content_scale: f32,
    pub(crate) _padding1: [u32; 3],
}

pub(crate) struct CachedOutputTransform {
    pub(crate) source_view: wgpu::TextureView,
    pub(crate) uniform: OutputTransformUniform,
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl OutputTransformUniform {
    pub(crate) const fn new(
        tone_mapping_mode: u32,
        encode_srgb: bool,
        output_primaries: DisplayColorPrimaries,
        sdr_content_scale: f32,
    ) -> Self {
        Self {
            tone_mapping_mode,
            encode_srgb: encode_srgb as u32,
            output_primaries: match output_primaries {
                DisplayColorPrimaries::Srgb => 0,
                DisplayColorPrimaries::DisplayP3 => 1,
            },
            _padding0: 0,
            sdr_content_scale,
            _padding1: [0; 3],
        }
    }
}
impl WgpuRenderer {
    pub(crate) fn render_debug_capture_stage(
        &mut self,
        frame: &SceneFrame,
        size: (u32, u32),
        request: DebugCaptureRequest,
    ) -> Result<RendererFrameStats> {
        match request.stage {
            DebugCaptureStage::HdrIntermediate => self.render_offscreen(frame, size),
            DebugCaptureStage::FinalComposed
                if request.encoding == DebugCaptureEncoding::Png
                    && request.sdr_visualization == DebugSdrVisualization::ToneMappedColor =>
            {
                self.render_offscreen(frame, size)
            }
            DebugCaptureStage::FinalComposed => self.render_final_composed_offscreen(frame, size),
        }
    }

    pub(crate) fn render_final_composed_offscreen(
        &mut self,
        frame: &SceneFrame,
        size: (u32, u32),
    ) -> Result<RendererFrameStats> {
        self.ensure_shared(None)?;

        let Some(surface_state) = self.surfaces.get(&frame.window_id) else {
            return self.render_offscreen(frame, size);
        };
        let strategy = surface_state.output_strategy;
        let requested_tone_mapping = surface_state.color_management.tone_mapping;
        let sdr_content_brightness_nits =
            surface_state.color_management.sdr_content_brightness_nits;
        let display_sdr_white_nits = surface_state.display_capabilities.sdr_white_nits;
        let final_format = strategy.surface_format();
        let final_view = self.ensure_offscreen_target(frame.window_id, size, final_format)?;
        let prepared = self.prepare_scene_submission(frame)?;

        if output_transform_requires_intermediate(strategy) {
            let intermediate_view = self.ensure_intermediate_target(frame.window_id, size)?;
            let intermediate_format = self
                .intermediate_targets
                .get(&frame.window_id)
                .map(|target| target.format)
                .ok_or_else(|| Error::new("missing HDR intermediate target after allocation"))?;
            let mut frame_stats =
                self.submit_prepared_scene(prepared, intermediate_format, &intermediate_view)?;
            self.submit_output_transform_pass(
                frame.window_id,
                &intermediate_view,
                &final_view,
                final_format,
                strategy,
                requested_tone_mapping,
                sdr_content_brightness_nits,
                display_sdr_white_nits,
                &mut frame_stats,
            )?;
            Ok(frame_stats)
        } else {
            self.submit_prepared_scene(prepared, final_format, &final_view)
        }
    }

    pub(crate) fn render_offscreen(
        &mut self,
        frame: &SceneFrame,
        size: (u32, u32),
    ) -> Result<RendererFrameStats> {
        self.ensure_shared(None)?;

        let final_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let final_view = self.ensure_offscreen_target(frame.window_id, size, final_format)?;
        let prepared = self.prepare_scene_submission(frame)?;
        let intermediate_view = self.ensure_intermediate_target(frame.window_id, size)?;
        let intermediate_format = self
            .intermediate_targets
            .get(&frame.window_id)
            .map(|target| target.format)
            .ok_or_else(|| Error::new("missing HDR intermediate target after allocation"))?;
        let mut frame_stats =
            self.submit_prepared_scene(prepared, intermediate_format, &intermediate_view)?;
        self.submit_output_transform_pass(
            frame.window_id,
            &intermediate_view,
            &final_view,
            final_format,
            OutputStrategy::SdrSurface {
                format: final_format,
            },
            RequestedToneMappingMode::Clamp,
            ColorManagementMode::default().sdr_content_brightness_nits,
            None,
            &mut frame_stats,
        )?;
        Ok(frame_stats)
    }

    pub(crate) fn ensure_offscreen_target(
        &mut self,
        window_id: WindowId,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Result<wgpu::TextureView> {
        let recreate = self
            .offscreen_targets
            .get(&window_id)
            .is_none_or(|target| target.size != size || target.format != format);
        if recreate {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("SUI offscreen frame"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            self.offscreen_targets.insert(
                window_id,
                OffscreenTarget {
                    view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    texture,
                    format,
                    size,
                },
            );
        }
        self.offscreen_targets
            .get(&window_id)
            .map(|target| target.view.clone())
            .ok_or_else(|| Error::new(format!("missing target for window {}", window_id.get())))
    }

    pub(crate) fn ensure_intermediate_target(
        &mut self,
        window_id: WindowId,
        size: (u32, u32),
    ) -> Result<wgpu::TextureView> {
        let format = wgpu::TextureFormat::Rgba16Float;
        let recreate = self
            .intermediate_targets
            .get(&window_id)
            .is_none_or(|target| target.size != size || target.format != format);
        if recreate {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("SUI HDR intermediate frame"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            self.intermediate_targets.insert(
                window_id,
                OffscreenTarget {
                    view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    texture,
                    format,
                    size,
                },
            );
        }
        self.intermediate_targets
            .get(&window_id)
            .map(|target| target.view.clone())
            .ok_or_else(|| Error::new(format!("missing target for window {}", window_id.get())))
    }

    pub(crate) fn submit_output_transform_pass(
        &mut self,
        window_id: WindowId,
        source_view: &wgpu::TextureView,
        destination_view: &wgpu::TextureView,
        destination_format: wgpu::TextureFormat,
        strategy: OutputStrategy,
        requested_tone_mapping: RequestedToneMappingMode,
        sdr_content_brightness_nits: f32,
        display_sdr_white_nits: Option<f32>,
        frame_stats: &mut RendererFrameStats,
    ) -> Result<()> {
        let prepare_started = self.runtime_diagnostics_enabled.then(Instant::now);
        let resolved_tone_mapping = match strategy {
            OutputStrategy::HdrNativeSurface { .. } => 0,
            _ => match requested_tone_mapping {
                RequestedToneMappingMode::Automatic => 1,
                RequestedToneMappingMode::Clamp => 1,
                RequestedToneMappingMode::Reinhard => 2,
            },
        };
        let shared = self
            .shared
            .as_mut()
            .expect("renderer shared state initialized");
        let sdr_content_scale = crate::output::output_sdr_content_scale(
            strategy,
            sdr_content_brightness_nits,
            display_sdr_white_nits,
        );
        let encode_srgb = matches!(
            destination_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
        ) || matches!(
            strategy,
            OutputStrategy::HdrNativeSurface {
                transfer: DisplayTransferFunction::Srgb,
                ..
            }
        );
        let uniform = OutputTransformUniform::new(
            resolved_tone_mapping,
            encode_srgb,
            crate::output::output_primaries(strategy),
            sdr_content_scale,
        );
        let cache = &mut self.frame_resources.output_transforms;
        let recreate = cache
            .get(&window_id)
            .is_none_or(|cached| cached.source_view != *source_view);
        if recreate {
            let uniform_buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SUI output transform uniform"),
                size: std::mem::size_of::<OutputTransformUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SUI output transform bind group"),
                layout: &shared.output_transform_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(source_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });
            cache.insert(
                window_id,
                CachedOutputTransform {
                    source_view: source_view.clone(),
                    uniform,
                    buffer: uniform_buffer,
                    bind_group,
                },
            );
        }
        let cached = cache
            .get_mut(&window_id)
            .expect("output transform resources allocated");
        if recreate || cached.uniform != uniform {
            self.frame_resources.uploads.write_buffer(
                &shared.device,
                &cached.buffer,
                0,
                bytemuck::bytes_of(&uniform),
            );
            cached.uniform = uniform;
        }
        let bind_group = cached.bind_group.clone();
        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SUI output transform encoder"),
            });
        if let Some(started) = prepare_started {
            frame_stats.gpu_upload_time_us += started.elapsed().as_micros() as u64;
        }
        let encode_started = self.runtime_diagnostics_enabled.then(Instant::now);
        {
            let pipeline = shared.output_transform_pipeline(destination_format);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SUI output transform pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: destination_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(started) = encode_started {
            frame_stats.pass_encode_time_us += started.elapsed().as_micros() as u64;
        }
        let submit_started = self.runtime_diagnostics_enabled.then(Instant::now);
        let uploads = self.frame_resources.uploads.finish();
        shared
            .queue
            .submit(uploads.into_iter().chain(std::iter::once(encoder.finish())));
        if let Some(started) = submit_started {
            frame_stats.queue_submit_time_us += started.elapsed().as_micros() as u64;
        }
        frame_stats.pass_count += 1;
        Ok(())
    }
}

pub(crate) fn shader_color(color: Color) -> [f32; 4] {
    let linear = color.to_linear_srgb();
    [
        linear.red,
        linear.green,
        linear.blue,
        linear.alpha.clamp(0.0, 1.0),
    ]
}

pub(crate) fn preferred_surface_format(
    formats: &[wgpu::TextureFormat],
) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
}

pub(crate) fn preferred_hdr_surface_format(
    formats: &[wgpu::TextureFormat],
) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(|format| matches!(format, wgpu::TextureFormat::Rgba16Float))
}

pub(crate) fn output_transform_requires_intermediate(strategy: OutputStrategy) -> bool {
    match strategy {
        OutputStrategy::HdrNativeSurface { .. }
        | OutputStrategy::HdrIntermediateThenToneMap { .. } => true,
        OutputStrategy::WideGamutSurface { format, primaries } => {
            !format.is_srgb() || !matches!(primaries, DisplayColorPrimaries::Srgb)
        }
        OutputStrategy::SdrSurface { format } => !format.is_srgb(),
    }
}

pub(crate) fn output_primaries(strategy: OutputStrategy) -> DisplayColorPrimaries {
    match strategy {
        OutputStrategy::SdrSurface { .. } => DisplayColorPrimaries::Srgb,
        OutputStrategy::WideGamutSurface { primaries, .. }
        | OutputStrategy::HdrNativeSurface { primaries, .. }
        | OutputStrategy::HdrIntermediateThenToneMap { primaries, .. } => primaries,
    }
}

pub(crate) fn output_sdr_content_scale(
    strategy: OutputStrategy,
    brightness_nits: f32,
    _display_sdr_white_nits: Option<f32>,
) -> f32 {
    let sanitized = if brightness_nits.is_finite() && brightness_nits > 0.0 {
        brightness_nits
    } else {
        DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS
    };

    match strategy {
        // Native HDR is presented as linear scRGB: 1.0 is the 80 nit scRGB
        // reference, so SDR reference white must be lifted to the requested
        // SDR-content brightness before presentation.
        OutputStrategy::HdrNativeSurface { .. } => sanitized / SCRGB_REFERENCE_WHITE_NITS,
        OutputStrategy::SdrSurface { .. }
        | OutputStrategy::WideGamutSurface { .. }
        | OutputStrategy::HdrIntermediateThenToneMap { .. } => 1.0,
    }
}

#[cfg(test)]
pub(crate) fn apply_output_transform_for_testing(
    color: [f32; 4],
    strategy: OutputStrategy,
    mode: RequestedToneMappingMode,
    sdr_content_brightness_nits: f32,
    display_sdr_white_nits: Option<f32>,
) -> [f32; 4] {
    let scale = output_sdr_content_scale(
        strategy,
        sdr_content_brightness_nits,
        display_sdr_white_nits,
    );
    let scaled = [
        color[0] * scale,
        color[1] * scale,
        color[2] * scale,
        color[3],
    ];

    let transformed = match strategy {
        OutputStrategy::HdrNativeSurface { .. } => [scaled[0], scaled[1], scaled[2], scaled[3]],
        _ => match mode {
            RequestedToneMappingMode::Automatic => match strategy {
                OutputStrategy::SdrSurface { .. }
                | OutputStrategy::WideGamutSurface { .. }
                | OutputStrategy::HdrIntermediateThenToneMap { .. } => {
                    tone_map_linear_color(scaled, RequestedToneMappingMode::Clamp)
                }
                OutputStrategy::HdrNativeSurface { .. } => unreachable!(),
            },
            RequestedToneMappingMode::Clamp => {
                tone_map_linear_color(scaled, RequestedToneMappingMode::Clamp)
            }
            RequestedToneMappingMode::Reinhard => {
                tone_map_linear_color(scaled, RequestedToneMappingMode::Reinhard)
            }
        },
    };

    let [red, green, blue] =
        linear_srgb_to_output_primaries([transformed[0], transformed[1], transformed[2]], strategy);
    [red, green, blue, transformed[3]]
}

#[cfg(test)]
pub(crate) fn linear_srgb_to_output_primaries(
    color: [f32; 3],
    strategy: OutputStrategy,
) -> [f32; 3] {
    match output_primaries(strategy) {
        DisplayColorPrimaries::Srgb => color,
        DisplayColorPrimaries::DisplayP3 => [
            (0.822_461_96 * color[0]) + (0.177_538_02 * color[1]),
            (0.033_194_2 * color[0]) + (0.966_805_76 * color[1]),
            (0.017_082_63 * color[0]) + (0.072_397_43 * color[1]) + (0.910_519_96 * color[2]),
        ],
    }
}

#[cfg(test)]
pub(crate) fn tone_map_linear_color(color: [f32; 4], mode: RequestedToneMappingMode) -> [f32; 4] {
    let transform = |channel: f32| match mode {
        RequestedToneMappingMode::Automatic | RequestedToneMappingMode::Clamp => {
            channel.clamp(0.0, 1.0)
        }
        RequestedToneMappingMode::Reinhard => {
            let channel = channel.max(0.0);
            channel / (1.0 + channel)
        }
    };

    [
        transform(color[0]),
        transform(color[1]),
        transform(color[2]),
        color[3].clamp(0.0, 1.0),
    ]
}

pub(crate) fn requested_output_primaries(
    capabilities: DisplayCapabilities,
    requested: ColorManagementMode,
) -> DisplayColorPrimaries {
    match requested.output_primaries {
        RequestedOutputColorPrimaries::Automatic => {
            if capabilities.supports_wide_gamut {
                capabilities.preferred_primaries
            } else {
                DisplayColorPrimaries::Srgb
            }
        }
        RequestedOutputColorPrimaries::Srgb => DisplayColorPrimaries::Srgb,
        RequestedOutputColorPrimaries::DisplayP3 => {
            if capabilities.supports_wide_gamut {
                DisplayColorPrimaries::DisplayP3
            } else {
                DisplayColorPrimaries::Srgb
            }
        }
    }
}

pub(crate) fn select_output_strategy(
    formats: &[wgpu::TextureFormat],
    capabilities: DisplayCapabilities,
    requested: ColorManagementMode,
) -> OutputStrategy {
    let sdr_format =
        preferred_surface_format(formats).unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
    let primaries = requested_output_primaries(capabilities.clone(), requested);
    let native_hdr_format = preferred_hdr_surface_format(formats);
    let native_hdr_available =
        capabilities.native_hdr_presentation_supported && native_hdr_format.is_some();

    if matches!(requested.mode, RequestedColorManagementMode::ForceSdr) {
        return OutputStrategy::SdrSurface { format: sdr_format };
    }

    let wants_hdr = match requested.dynamic_range {
        RequestedDynamicRangeMode::HighDynamicRange => true,
        RequestedDynamicRangeMode::StandardDynamicRange => false,
        RequestedDynamicRangeMode::Automatic => match requested.mode {
            RequestedColorManagementMode::PreferHdr => true,
            RequestedColorManagementMode::Automatic => {
                native_hdr_available
                    || capabilities.supports_hdr
                    || matches!(
                        capabilities.preferred_dynamic_range,
                        DynamicRangeMode::HighDynamicRange
                    )
            }
            RequestedColorManagementMode::ForceSdr
            | RequestedColorManagementMode::PreferWideGamut => false,
        },
    };
    let wants_wide_gamut = match requested.output_primaries {
        RequestedOutputColorPrimaries::DisplayP3 => true,
        RequestedOutputColorPrimaries::Srgb => false,
        RequestedOutputColorPrimaries::Automatic => match requested.mode {
            RequestedColorManagementMode::PreferWideGamut
            | RequestedColorManagementMode::PreferHdr => true,
            RequestedColorManagementMode::Automatic => capabilities.supports_wide_gamut,
            RequestedColorManagementMode::ForceSdr => false,
        },
    };

    if wants_hdr {
        if native_hdr_available && let Some(format) = native_hdr_format {
            let uses_linear_sc_rgb = capabilities.native_hdr_presentation_supported
                && matches!(
                    capabilities.preferred_primaries,
                    DisplayColorPrimaries::Srgb
                );
            let transfer = if uses_linear_sc_rgb {
                DisplayTransferFunction::LinearExtended
            } else {
                DisplayTransferFunction::Srgb
            };
            return OutputStrategy::HdrNativeSurface {
                format,
                primaries: DisplayColorPrimaries::Srgb,
                transfer,
            };
        }

        // If HDR cannot be presented end-to-end, normal presentation stays SDR.
        // HdrIntermediateThenToneMap is reserved for debug captures and explicit
        // diagnostics where seeing HDR scene values on an SDR display is useful.
        return OutputStrategy::SdrSurface { format: sdr_format };
    }

    if wants_wide_gamut && capabilities.supports_wide_gamut {
        return OutputStrategy::WideGamutSurface {
            format: sdr_format,
            primaries,
        };
    }

    OutputStrategy::SdrSurface { format: sdr_format }
}
