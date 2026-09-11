use crate::WgpuRenderer;
use crate::output::DebugCaptureEncoding;
use crate::output::DebugCaptureRequest;
use crate::output::DebugCaptureStage;
use crate::output::DebugSdrVisualization;
use crate::output::DisplayColorPrimaries;
use crate::surface::normalize_framebuffer_size;
use half::f16;
use sui_core::Error;
use sui_core::Result;
use sui_core::WindowId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HdrRgbaImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DebugCaptureArtifact {
    SdrRgba8(RgbaImage),
    HdrLinearRgbaF32(HdrRgbaImage),
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        let expected_len = width as usize * height as usize * 4;
        if pixels.len() != expected_len {
            return Err(Error::new(format!(
                "RGBA image pixel buffer length {} does not match {}x{} image size",
                pixels.len(),
                width,
                height
            )));
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

impl HdrRgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<f32>) -> Result<Self> {
        let expected_len = width as usize * height as usize * 4;
        if pixels.len() != expected_len {
            return Err(Error::new(format!(
                "HDR RGBA image pixel buffer length {} does not match {}x{} image size",
                pixels.len(),
                width,
                height
            )));
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[f32] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<f32> {
        self.pixels
    }
}

pub(crate) fn strip_padded_readback_rows(
    mapped: &[u8],
    bytes_per_row: usize,
    padded_bytes_per_row: usize,
    rows: usize,
) -> Vec<u8> {
    let mut tightly_packed = Vec::with_capacity(bytes_per_row * rows);
    for row in 0..rows {
        let start = row * padded_bytes_per_row;
        tightly_packed.extend_from_slice(&mapped[start..start + bytes_per_row]);
    }
    tightly_packed
}

pub(crate) fn decode_rgba16f_pixels(raw: &[u8]) -> Vec<f32> {
    let mut pixels = Vec::with_capacity(raw.len() / 2);
    for chunk in raw.chunks_exact(2) {
        pixels.push(f16::from_bits(u16::from_le_bytes([chunk[0], chunk[1]])).to_f32());
    }
    pixels
}

pub(crate) fn hdr_image_to_sdr_rgba(
    image: &HdrRgbaImage,
    visualization: DebugSdrVisualization,
    reference_white: f32,
    output_primaries: DisplayColorPrimaries,
) -> Result<RgbaImage> {
    let reference_white = if reference_white.is_finite() && reference_white > 0.0 {
        reference_white
    } else {
        1.0
    };
    let mut pixels = Vec::with_capacity((image.width() * image.height() * 4) as usize);
    for rgba in image.pixels().chunks_exact(4) {
        let normalized = linear_output_primaries_to_srgb(
            [
                rgba[0] / reference_white,
                rgba[1] / reference_white,
                rgba[2] / reference_white,
            ],
            output_primaries,
        );
        match visualization {
            DebugSdrVisualization::ToneMappedColor => {
                // Native HDR final targets store SDR white above 1.0. Divide that
                // headroom back out, convert the output primaries to linear sRGB,
                // then clamp during sRGB encoding. SDR-authored sRGB colors round
                // back to their original PNG bytes; HDR values overflow to white.
                pixels.extend_from_slice(&[
                    linear_to_srgb_capture_u8(normalized[0]),
                    linear_to_srgb_capture_u8(normalized[1]),
                    linear_to_srgb_capture_u8(normalized[2]),
                    linear_alpha_to_capture_u8(rgba[3]),
                ]);
            }
            DebugSdrVisualization::LuminanceHeatmap => {
                let luminance =
                    (normalized[0] * 0.2126 + normalized[1] * 0.7152 + normalized[2] * 0.0722)
                        .max(0.0);
                let normalized = (luminance / (1.0 + luminance)).clamp(0.0, 1.0);
                let value = (normalized * 255.0).round() as u8;
                pixels.extend_from_slice(&[value, value, value, 255]);
            }
            DebugSdrVisualization::HeadroomHeatmap => {
                let headroom = normalized[0].max(normalized[1]).max(normalized[2]).max(0.0);
                let normalized = (headroom / (1.0 + headroom)).clamp(0.0, 1.0);
                let red = (normalized * 255.0).round() as u8;
                let blue = ((1.0 - normalized) * 96.0).round() as u8;
                pixels.extend_from_slice(&[red, 32, blue, 255]);
            }
            DebugSdrVisualization::ClipMask => {
                let clipped = normalized[0].max(normalized[1]).max(normalized[2]) > 1.0;
                if clipped {
                    pixels.extend_from_slice(&[255, 64, 64, 255]);
                } else {
                    pixels.extend_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
    }
    RgbaImage::new(image.width(), image.height(), pixels)
}

pub(crate) fn linear_output_primaries_to_srgb(
    color: [f32; 3],
    output_primaries: DisplayColorPrimaries,
) -> [f32; 3] {
    match output_primaries {
        DisplayColorPrimaries::Srgb => color,
        DisplayColorPrimaries::DisplayP3 => {
            let det = (0.822_461_96 * 0.966_805_76) - (0.177_538_02 * 0.033_194_2);
            let red = (0.966_805_76 * color[0] - 0.177_538_02 * color[1]) / det;
            let green = (-0.033_194_2 * color[0] + 0.822_461_96 * color[1]) / det;
            let blue = (color[2] - (0.017_082_63 * red) - (0.072_397_43 * green)) / 0.910_519_96;
            [red, green, blue]
        }
    }
}

pub(crate) fn encode_hdr_debug_artifact(
    image: HdrRgbaImage,
    request: DebugCaptureRequest,
    sdr_reference_white: f32,
    output_primaries: DisplayColorPrimaries,
) -> Result<DebugCaptureArtifact> {
    match request.encoding {
        DebugCaptureEncoding::Exr => Ok(DebugCaptureArtifact::HdrLinearRgbaF32(image)),
        DebugCaptureEncoding::Png => hdr_image_to_sdr_rgba(
            &image,
            request.sdr_visualization,
            sdr_reference_white,
            output_primaries,
        )
        .map(DebugCaptureArtifact::SdrRgba8),
    }
}

pub(crate) fn linear_to_srgb_capture_u8(channel: f32) -> u8 {
    let value = channel.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        (1.055 * value.powf(1.0 / 2.4)) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn linear_alpha_to_capture_u8(alpha: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
}
impl WgpuRenderer {
    pub fn capture_last_frame_rgba(&mut self, window_id: WindowId) -> Result<RgbaImage> {
        match self.capture_last_frame_debug(window_id, DebugCaptureRequest::default())? {
            DebugCaptureArtifact::SdrRgba8(image) => Ok(image),
            DebugCaptureArtifact::HdrLinearRgbaF32(_) => Err(Error::new(format!(
                "window {} returned an HDR debug artifact when SDR RGBA capture was requested",
                window_id.get()
            ))),
        }
    }

    pub fn capture_last_frame_debug(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        let frame = self.last_frames.get(&window_id).cloned().ok_or_else(|| {
            Error::new(format!(
                "window {} does not have a previously rendered frame available for capture",
                window_id.get()
            ))
        })?;

        let size = normalize_framebuffer_size(frame.surface_size).ok_or_else(|| {
            Error::new(format!(
                "window {} last rendered frame has an invalid framebuffer size",
                window_id.get()
            ))
        })?;

        self.render_debug_capture_stage(&frame, size, request)?;
        self.capture_debug_frame(window_id, request)
    }

    pub fn capture_debug_frame(
        &self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        match request.stage {
            DebugCaptureStage::FinalComposed => {
                self.capture_final_composed_debug_artifact(window_id, request)
            }
            DebugCaptureStage::HdrIntermediate => {
                let image = self.capture_hdr_intermediate_rgba_f32(window_id)?;
                encode_hdr_debug_artifact(image, request, 1.0, DisplayColorPrimaries::Srgb)
            }
        }
    }

    pub fn capture_rgba(&self, window_id: WindowId) -> Result<RgbaImage> {
        let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an offscreen render target available for screenshot capture",
                window_id.get()
            ))
        })?;
        let raw = self.readback_target_bytes(
            &target.texture,
            target.size,
            4,
            "SUI screenshot readback",
            "SUI screenshot readback encoder",
        )?;

        let mut pixels = Vec::with_capacity((target.size.0 * target.size.1 * 4) as usize);
        for chunk in raw.chunks_exact(4) {
            pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
        }

        RgbaImage::new(target.size.0, target.size.1, pixels)
    }

    pub fn capture_hdr_intermediate_rgba_f32(&self, window_id: WindowId) -> Result<HdrRgbaImage> {
        let target = self.intermediate_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an HDR intermediate target available for debug capture",
                window_id.get()
            ))
        })?;
        let raw = self.readback_target_bytes(
            &target.texture,
            target.size,
            8,
            "SUI HDR intermediate readback",
            "SUI HDR intermediate readback encoder",
        )?;
        let pixels = decode_rgba16f_pixels(&raw);
        HdrRgbaImage::new(target.size.0, target.size.1, pixels)
    }

    pub(crate) fn capture_final_composed_debug_artifact(
        &self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an offscreen target available for final composed debug capture",
                window_id.get()
            ))
        })?;

        match target.format {
            wgpu::TextureFormat::Bgra8UnormSrgb => self
                .capture_rgba(window_id)
                .map(DebugCaptureArtifact::SdrRgba8),
            wgpu::TextureFormat::Rgba16Float => {
                let image = self.capture_hdr_offscreen_rgba_f32(window_id)?;
                encode_hdr_debug_artifact(
                    image,
                    request,
                    self.final_composed_sdr_reference_white(window_id),
                    self.final_composed_output_primaries(window_id),
                )
            }
            other => Err(Error::new(format!(
                "window {} uses unsupported final composed debug capture format {other:?}",
                window_id.get()
            ))),
        }
    }

    pub(crate) fn capture_hdr_offscreen_rgba_f32(
        &self,
        window_id: WindowId,
    ) -> Result<HdrRgbaImage> {
        let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an offscreen HDR target available for debug capture",
                window_id.get()
            ))
        })?;
        let raw = self.readback_target_bytes(
            &target.texture,
            target.size,
            8,
            "SUI HDR final readback",
            "SUI HDR final readback encoder",
        )?;
        let pixels = decode_rgba16f_pixels(&raw);
        HdrRgbaImage::new(target.size.0, target.size.1, pixels)
    }

    pub(crate) fn final_composed_sdr_reference_white(&self, window_id: WindowId) -> f32 {
        self.surfaces
            .get(&window_id)
            .map(|surface| {
                crate::output::output_sdr_content_scale(
                    surface.output_strategy,
                    surface.color_management.sdr_content_brightness_nits,
                    surface.display_capabilities.sdr_white_nits,
                )
            })
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(1.0)
    }

    pub(crate) fn final_composed_output_primaries(
        &self,
        window_id: WindowId,
    ) -> DisplayColorPrimaries {
        self.surfaces
            .get(&window_id)
            .map(|surface| crate::output::output_primaries(surface.output_strategy))
            .unwrap_or(DisplayColorPrimaries::Srgb)
    }

    pub(crate) fn readback_target_bytes(
        &self,
        texture: &wgpu::Texture,
        size: (u32, u32),
        bytes_per_pixel: u32,
        buffer_label: &'static str,
        encoder_label: &'static str,
    ) -> Result<Vec<u8>> {
        let shared = self
            .shared
            .as_ref()
            .ok_or_else(|| Error::new("renderer has not initialized a wgpu device yet"))?;
        let bytes_per_row = size.0 * bytes_per_pixel;
        let padded_bytes_per_row = bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer_size = padded_bytes_per_row as u64 * size.1 as u64;
        let buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(buffer_label),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(encoder_label),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(size.1),
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        shared.queue.submit([encoder.finish()]);

        let (sender, receiver) = std::sync::mpsc::channel();
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        shared
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| {
                Error::new(format!(
                    "failed to poll device for screenshot capture: {error}"
                ))
            })?;
        receiver
            .recv()
            .map_err(|error| {
                Error::new(format!(
                    "failed to receive screenshot readback completion: {error}"
                ))
            })?
            .map_err(|error| {
                Error::new(format!("failed to map screenshot readback buffer: {error}"))
            })?;

        let mapped = slice.get_mapped_range().map_err(|error| {
            Error::new(format!(
                "failed to access mapped screenshot readback buffer: {error}"
            ))
        })?;
        let tightly_packed = strip_padded_readback_rows(
            &mapped,
            bytes_per_row as usize,
            padded_bytes_per_row as usize,
            size.1 as usize,
        );
        drop(mapped);
        buffer.unmap();
        Ok(tightly_packed)
    }
}
