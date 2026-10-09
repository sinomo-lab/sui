use crate::WgpuRenderer;
use crate::output::DebugCaptureEncoding;
use crate::output::DebugCaptureRequest;
use crate::output::DebugCaptureStage;
use crate::output::DebugSdrVisualization;
use crate::output::DisplayColorPrimaries;
use crate::resources::OffscreenTarget;
use crate::surface::normalize_framebuffer_size;
use half::f16;
use std::sync::{Arc, Mutex};
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
    for chunk in raw.as_chunks::<2>().0 {
        pixels.push(f16::from_bits(u16::from_le_bytes([chunk[0], chunk[1]])).to_f32());
    }
    pixels
}

pub(crate) fn hdr_image_to_sdr_rgba(
    image: &HdrRgbaImage,
    visualization: DebugSdrVisualization,
    reference_white: f32,
    output_primaries: DisplayColorPrimaries,
    fit: crate::output::SdrFit,
) -> Result<RgbaImage> {
    let reference_white = if reference_white.is_finite() && reference_white > 0.0 {
        reference_white
    } else {
        1.0
    };
    let mut pixels = Vec::with_capacity((image.width() * image.height() * 4) as usize);
    for rgba in image.pixels().as_chunks::<4>().0 {
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
                // then fit highlights as the window's SDR output would.
                // SDR-authored sRGB colors round back to their original PNG bytes.
                let fitted = fit.apply(normalized.map(|channel| channel.max(0.0)));
                pixels.extend_from_slice(&[
                    linear_to_srgb_capture_u8(fitted[0]),
                    linear_to_srgb_capture_u8(fitted[1]),
                    linear_to_srgb_capture_u8(fitted[2]),
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
    fit: crate::output::SdrFit,
) -> Result<DebugCaptureArtifact> {
    match request.encoding {
        DebugCaptureEncoding::Exr => Ok(DebugCaptureArtifact::HdrLinearRgbaF32(image)),
        DebugCaptureEncoding::Png => hdr_image_to_sdr_rgba(
            &image,
            request.sdr_visualization,
            sdr_reference_white,
            output_primaries,
            fit,
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
/// Identifies a debug capture the renderer began, until it is collected with
/// [`WgpuRenderer::take_finished_debug_captures`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugCaptureId(u64);

impl DebugCaptureId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Pixels being copied back from a texture. The copy finishes on the GPU's
/// schedule; nothing here waits for it.
pub(crate) struct Readback {
    buffer: wgpu::Buffer,
    bytes_per_row: usize,
    padded_bytes_per_row: usize,
    rows: usize,
    /// Set by the map callback: whether the buffer can be read.
    mapped: Arc<Mutex<Option<std::result::Result<(), String>>>>,
}

impl Readback {
    /// Whether the copy has come back, successfully or not.
    fn is_back(&self) -> bool {
        self.mapped
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    /// The pixels, tightly packed, once the copy is back; `None` while it is
    /// still on its way.
    fn bytes(&self) -> Option<Result<Vec<u8>>> {
        let mapped = self
            .mapped
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()?;
        Some(mapped.map_err(Error::new).and_then(|()| {
            let range = self.buffer.slice(..).get_mapped_range().map_err(|error| {
                Error::new(format!(
                    "failed to access the mapped readback buffer: {error}"
                ))
            })?;
            Ok(strip_padded_readback_rows(
                &range,
                self.bytes_per_row,
                self.padded_bytes_per_row,
                self.rows,
            ))
        }))
    }
}

/// How a capture's pixels become its artifact.
#[derive(Debug, Clone, Copy)]
enum CaptureDecoding {
    /// 8-bit sRGB-encoded color, in BGRA order when `bgra`.
    Sdr { bgra: bool },
    /// RGBA16 float, encoded as the request asks.
    Hdr {
        sdr_reference_white: f32,
        primaries: DisplayColorPrimaries,
        fit: crate::output::SdrFit,
    },
}

/// A capture whose pixels are being copied back.
/// How long [`WgpuRenderer::wait_for_readbacks_until`] waits for a copy to be
/// reported before leaving the caller to say it did not finish.
#[cfg(not(target_arch = "wasm32"))]
const READBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

pub(crate) struct InFlightCapture {
    id: DebugCaptureId,
    pub(crate) window_id: WindowId,
    request: DebugCaptureRequest,
    size: (u32, u32),
    decoding: CaptureDecoding,
    readback: Readback,
}

impl InFlightCapture {
    /// The artifact, once the pixels are back.
    fn artifact(&self) -> Option<Result<DebugCaptureArtifact>> {
        let bytes = self.readback.bytes()?;
        Some(bytes.and_then(|bytes| {
            let (width, height) = self.size;
            match self.decoding {
                CaptureDecoding::Sdr { bgra } => {
                    let pixels = if bgra { bgra_to_rgba(&bytes) } else { bytes };
                    RgbaImage::new(width, height, pixels).map(DebugCaptureArtifact::SdrRgba8)
                }
                CaptureDecoding::Hdr {
                    sdr_reference_white,
                    primaries,
                    fit,
                } => HdrRgbaImage::new(width, height, decode_rgba16f_pixels(&bytes)).and_then(
                    |image| {
                        encode_hdr_debug_artifact(
                            image,
                            self.request,
                            sdr_reference_white,
                            primaries,
                            fit,
                        )
                    },
                ),
            }
        }))
    }
}

fn bgra_to_rgba(bytes: &[u8]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(bytes.len());
    for chunk in bytes.as_chunks::<4>().0 {
        pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
    }
    pixels
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

    /// Capture `window_id`'s last frame at the requested stage, waiting for
    /// the GPU to copy it back. Browsers cannot wait; begin captures there
    /// with [`begin_debug_capture`](Self::begin_debug_capture).
    pub fn capture_last_frame_debug(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        self.render_last_frame_for_capture(window_id, request)?;
        self.capture_debug_frame(window_id, request)
    }

    /// Start capturing `window_id`'s last frame at the requested stage: draw
    /// it again offscreen and begin copying it back. Nothing waits for the
    /// GPU, so this works in browsers too. Collect the artifact with
    /// [`take_finished_debug_captures`](Self::take_finished_debug_captures)
    /// once the copy is back.
    pub fn begin_debug_capture(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureId> {
        self.render_last_frame_for_capture(window_id, request)?;
        let (target, decoding) = self.capture_target(window_id, request)?;
        let size = target.size;
        let bytes_per_pixel = match decoding {
            CaptureDecoding::Sdr { .. } => 4,
            CaptureDecoding::Hdr { .. } => 8,
        };
        let readback = self.start_readback(
            &target.texture,
            size,
            bytes_per_pixel,
            "SUI debug capture readback",
        )?;
        let id = DebugCaptureId(self.next_debug_capture_id);
        self.next_debug_capture_id += 1;
        self.debug_captures_in_flight.push(InFlightCapture {
            id,
            window_id,
            request,
            size,
            decoding,
            readback,
        });
        Ok(id)
    }

    /// Whether captures of `window_id` are still being copied back.
    pub fn has_debug_captures_in_flight(&self, window_id: WindowId) -> bool {
        self.debug_captures_in_flight
            .iter()
            .any(|capture| capture.window_id == window_id)
    }

    /// The captures of `window_id` whose pixels are back, each handed out
    /// once. Never waits for the GPU.
    pub fn take_finished_debug_captures(
        &mut self,
        window_id: WindowId,
    ) -> Vec<(DebugCaptureId, Result<DebugCaptureArtifact>)> {
        // Native devices deliver map callbacks when polled; browsers deliver
        // them on their own.
        if let Some(shared) = &self.shared {
            let _ = shared.device.poll(wgpu::PollType::Poll);
        }
        let mut finished = Vec::new();
        self.debug_captures_in_flight.retain(|capture| {
            if capture.window_id != window_id {
                return true;
            }
            match capture.artifact() {
                Some(artifact) => {
                    finished.push((capture.id, artifact));
                    false
                }
                None => true,
            }
        });
        finished
    }

    /// Wait until the GPU has copied back every capture begun. Browsers
    /// cannot wait, so this is native-only.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait_for_debug_captures(&self) -> Result<()> {
        self.wait_for_readbacks_until(|| {
            self.debug_captures_in_flight
                .iter()
                .all(|capture| capture.readback.is_back())
        })
    }

    fn render_last_frame_for_capture(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<()> {
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

        self.render_debug_capture_stage(&frame, size, request)
            .map(|_| ())
    }

    /// The target a capture at `request`'s stage reads, and how its pixels
    /// become the artifact.
    fn capture_target(
        &self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<(&OffscreenTarget, CaptureDecoding)> {
        let fit = self.sdr_preview_fit(window_id);
        match request.stage {
            DebugCaptureStage::FinalComposed => {
                let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
                    Error::new(format!(
                        "window {} does not have an offscreen target available for final composed debug capture",
                        window_id.get()
                    ))
                })?;
                let decoding = match target.format {
                    // Unorm targets hold color the output shader encoded, the
                    // same bytes an sRGB target stores.
                    wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm => {
                        CaptureDecoding::Sdr { bgra: true }
                    }
                    wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm => {
                        CaptureDecoding::Sdr { bgra: false }
                    }
                    wgpu::TextureFormat::Rgba16Float => CaptureDecoding::Hdr {
                        sdr_reference_white: self.final_composed_sdr_reference_white(window_id),
                        primaries: self.final_composed_output_primaries(window_id),
                        fit,
                    },
                    other => {
                        return Err(Error::new(format!(
                            "window {} uses unsupported final composed debug capture format {other:?}",
                            window_id.get()
                        )));
                    }
                };
                Ok((target, decoding))
            }
            DebugCaptureStage::HdrIntermediate => {
                let target = self.intermediate_targets.get(&window_id).ok_or_else(|| {
                    Error::new(format!(
                        "window {} does not have an HDR intermediate target available for debug capture",
                        window_id.get()
                    ))
                })?;
                Ok((
                    target,
                    CaptureDecoding::Hdr {
                        sdr_reference_white: 1.0,
                        primaries: DisplayColorPrimaries::Srgb,
                        fit,
                    },
                ))
            }
        }
    }

    /// Capture the frame last drawn at `request`'s stage, waiting for the
    /// GPU to copy it back.
    pub fn capture_debug_frame(
        &self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        let (target, decoding) = self.capture_target(window_id, request)?;
        let size = target.size;
        let bytes_per_pixel = match decoding {
            CaptureDecoding::Sdr { .. } => 4,
            CaptureDecoding::Hdr { .. } => 8,
        };
        let readback = self.start_readback(
            &target.texture,
            size,
            bytes_per_pixel,
            "SUI debug capture readback",
        )?;
        self.wait_for_readbacks_until(|| readback.is_back())?;
        InFlightCapture {
            id: DebugCaptureId(0),
            window_id,
            request,
            size,
            decoding,
            readback,
        }
        .artifact()
        .unwrap_or_else(|| Err(Error::new("the debug capture readback did not finish")))
    }

    pub fn capture_rgba(&self, window_id: WindowId) -> Result<RgbaImage> {
        let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an offscreen render target available for screenshot capture",
                window_id.get()
            ))
        })?;
        let raw =
            self.readback_target_bytes(&target.texture, target.size, 4, "SUI screenshot readback")?;
        let pixels = match target.format {
            wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm => raw,
            _ => bgra_to_rgba(&raw),
        };
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
        )?;
        let pixels = decode_rgba16f_pixels(&raw);
        HdrRgbaImage::new(target.size.0, target.size.1, pixels)
    }

    #[cfg(test)]
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
        let raw =
            self.readback_target_bytes(&target.texture, target.size, 8, "SUI HDR final readback")?;
        let pixels = decode_rgba16f_pixels(&raw);
        HdrRgbaImage::new(target.size.0, target.size.1, pixels)
    }

    /// How SDR previews of HDR captures fit highlights: as the window's own
    /// SDR output would.
    pub(crate) fn sdr_preview_fit(&self, window_id: WindowId) -> crate::output::SdrFit {
        crate::output::SdrFit::for_tone_mapping(
            self.window_color_management(window_id).tone_mapping,
        )
    }

    pub(crate) fn final_composed_sdr_reference_white(&self, window_id: WindowId) -> f32 {
        self.window_output_strategy(window_id)
            .map(|strategy| {
                crate::output::output_sdr_content_scale(
                    strategy,
                    self.window_color_management(window_id)
                        .sdr_content_brightness_nits,
                    self.window_display_capabilities(window_id)
                        .and_then(|capabilities| capabilities.sdr_white_nits),
                )
            })
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(1.0)
    }

    pub(crate) fn final_composed_output_primaries(
        &self,
        window_id: WindowId,
    ) -> DisplayColorPrimaries {
        self.window_output_strategy(window_id)
            .map(crate::output::output_primaries)
            .unwrap_or(DisplayColorPrimaries::Srgb)
    }

    /// Copy `texture` into a buffer and ask for it to be mapped, without
    /// waiting for either.
    pub(crate) fn start_readback(
        &self,
        texture: &wgpu::Texture,
        size: (u32, u32),
        bytes_per_pixel: u32,
        label: &'static str,
    ) -> Result<Readback> {
        let shared = self
            .shared
            .as_ref()
            .ok_or_else(|| Error::new("renderer has not initialized a wgpu device yet"))?;
        let bytes_per_row = size.0 * bytes_per_pixel;
        let padded_bytes_per_row = bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: padded_bytes_per_row as u64 * size.1 as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
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

        let mapped = Arc::new(Mutex::new(None));
        let callback_mapped = Arc::clone(&mapped);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                *callback_mapped
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(
                    result.map_err(|error| format!("failed to map the readback buffer: {error}")),
                );
            });
        Ok(Readback {
            buffer,
            bytes_per_row: bytes_per_row as usize,
            padded_bytes_per_row: padded_bytes_per_row as usize,
            rows: size.1 as usize,
            mapped,
        })
    }

    /// Wait for the GPU to finish the readbacks it was given, until `back`
    /// says the ones the caller wants have been reported. Renderers without
    /// a window share their device across threads, and another thread's poll
    /// can collect a finished copy and still be reporting it when this poll
    /// returns, so this polls again until the report is in, for at most
    /// [`READBACK_TIMEOUT`]. Browsers return at once, before they finish.
    fn wait_for_readbacks_until(&self, back: impl Fn() -> bool) -> Result<()> {
        let shared = self
            .shared
            .as_ref()
            .ok_or_else(|| Error::new("renderer has not initialized a wgpu device yet"))?;
        let poll = || {
            shared
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .map(|_| ())
                .map_err(|error| Error::new(format!("failed to wait for a readback: {error}")))
        };
        poll()?;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let deadline = std::time::Instant::now() + READBACK_TIMEOUT;
            while !back() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
                poll()?;
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = back;
        Ok(())
    }

    /// Copy `texture` back, waiting for the GPU.
    pub(crate) fn readback_target_bytes(
        &self,
        texture: &wgpu::Texture,
        size: (u32, u32),
        bytes_per_pixel: u32,
        label: &'static str,
    ) -> Result<Vec<u8>> {
        let readback = self.start_readback(texture, size, bytes_per_pixel, label)?;
        self.wait_for_readbacks_until(|| readback.is_back())?;
        readback
            .bytes()
            .unwrap_or_else(|| Err(Error::new("the readback did not finish")))
    }
}
