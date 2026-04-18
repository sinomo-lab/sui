#![forbid(unsafe_code)]

mod feathering;
mod gpu;
mod retained;
mod scene;
mod text;

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::Instant,
};

use bytemuck::{Pod, Zeroable};
use lyon_path::{Path as LyonPath, builder::PathBuilder as LyonPathBuilder, math::point};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeVertex,
    StrokeVertexConstructor, VertexBuffers,
};
use sui_core::{
    Color, Error, ImageHandle, Path as ScenePath, PathElement, Point, Rect, Result, Size,
    Transform, Vector, WindowId,
};
use sui_scene::{
    Brush, RegisteredImage, RegisteredImageFormat, Scene, SceneCommand, SceneFrame, SceneLayer,
    SceneLayerId, SceneLayerUpdateKind, StrokeStyle,
};
use sui_text::{
    FontRegistry, ResolvedTextFace, ShapedGlyph as SceneShapedGlyph, ShapedText, TextLayout,
    TextLayoutCacheSnapshot, TextRun, TextStyle, TextSystem,
};
use swash::{
    FontRef as SwashFontRef,
    scale::{
        ScaleContext as SwashScaleContext, Source as SwashSource,
        StrikeWith as SwashStrikeWith,
        image::Content as SwashImageContent, Render as SwashRender,
    },
    zeno::Format as SwashFormat,
};
#[cfg(test)]
use tiny_skia::PathBuilder as TinySkiaPathBuilder;
use wgpu::util::DeviceExt;
use winit::window::Window;

use gpu::*;
use retained::*;
use scene::*;
use text::*;

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
pub struct ColorManagementMode {
    pub mode: RequestedColorManagementMode,
    pub output_primaries: RequestedOutputColorPrimaries,
    pub dynamic_range: RequestedDynamicRangeMode,
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
pub struct RendererInterop {
    pub raw_wgpu_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatheringOptions {
    pub enabled: bool,
    pub width: f32,
}

impl FeatheringOptions {
    pub const fn new(enabled: bool, width: f32) -> Self {
        Self { enabled, width }
    }

    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            width: self.width.max(0.0),
        }
    }

    pub fn effective_width(self) -> f32 {
        if self.enabled {
            self.width.max(0.0)
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextRenderMode {
    #[default]
    Grayscale,
    LcdSubpixel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextHinting {
    None,
    Slight { max_ppem: f32 },
}

impl Default for TextHinting {
    fn default() -> Self {
        Self::None
    }
}

impl TextHinting {
    pub fn normalized(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Slight { max_ppem } if max_ppem.is_finite() && max_ppem > 0.0 => {
                Self::Slight { max_ppem }
            }
            Self::Slight { .. } => Self::None,
        }
    }

    pub fn should_hint(self, ppem: f32) -> bool {
        match self.normalized() {
            Self::None => false,
            Self::Slight { max_ppem } => ppem.is_finite() && ppem <= max_ppem,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StemDarkening {
    None,
    Enabled { max_ppem: f32, amount: f32 },
}

impl Default for StemDarkening {
    fn default() -> Self {
        Self::None
    }
}

impl StemDarkening {
    pub fn normalized(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Enabled { max_ppem, amount }
                if max_ppem.is_finite() && max_ppem > 0.0 && amount.is_finite() && amount > 0.0 =>
            {
                Self::Enabled {
                    max_ppem,
                    amount: amount.clamp(0.0, 1.0),
                }
            }
            Self::Enabled { .. } => Self::None,
        }
    }

    pub fn effective_amount(self, ppem: f32) -> f32 {
        match self.normalized() {
            Self::None => 0.0,
            Self::Enabled { max_ppem, amount } if ppem.is_finite() && ppem <= max_ppem => amount,
            Self::Enabled { .. } => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextCoveragePolicy {
    AutomaticByTextLuminance,
    Linear,
    Gamma(f32),
    TwoCoverageMinusCoverageSq,
}

impl Default for TextCoveragePolicy {
    fn default() -> Self {
        Self::AutomaticByTextLuminance
    }
}

impl TextCoveragePolicy {
    pub fn normalized(self) -> Self {
        match self {
            Self::AutomaticByTextLuminance => Self::AutomaticByTextLuminance,
            Self::Linear => Self::Linear,
            Self::Gamma(gamma) if gamma.is_finite() && gamma > 0.0 => Self::Gamma(gamma),
            Self::Gamma(_) => Self::Linear,
            Self::TwoCoverageMinusCoverageSq => Self::TwoCoverageMinusCoverageSq,
        }
    }

    pub fn resolved_for_text_color(self, color: Color) -> Self {
        match self.normalized() {
            Self::AutomaticByTextLuminance => {
                let rgba = shader_color(color);
                let luminance =
                    ((rgba[0] * 0.2126) + (rgba[1] * 0.7152) + (rgba[2] * 0.0722)).clamp(0.0, 1.0);
                if luminance >= 0.5 {
                    Self::TwoCoverageMinusCoverageSq
                } else {
                    Self::Linear
                }
            }
            policy => policy,
        }
    }

    pub fn apply(self, coverage: f32) -> f32 {
        let coverage = coverage.clamp(0.0, 1.0);
        match self.normalized() {
            Self::AutomaticByTextLuminance => coverage,
            Self::Linear => coverage,
            Self::Gamma(gamma) => coverage.powf(gamma),
            Self::TwoCoverageMinusCoverageSq => (2.0 * coverage) - (coverage * coverage),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlyphCacheSnapshot {
    pub entries: usize,
    pub hits: usize,
    pub misses: usize,
}

impl GlyphCacheSnapshot {
    pub const fn requests(self) -> usize {
        self.hits + self.misses
    }

    pub fn hit_rate(self) -> f64 {
        let requests = self.requests();
        if requests == 0 {
            0.0
        } else {
            self.hits as f64 / requests as f64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererTextCacheSnapshot {
    pub layout: TextLayoutCacheSnapshot,
    pub glyph: GlyphCacheSnapshot,
    pub path: GlyphCacheSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct TextFrameStats {
    glyph_instances: usize,
    glyph_upload_bytes: u64,
    atlas_miss_count: usize,
    atlas_miss_time_us: u64,
}

const TEXT_ATLAS_WIDTH: usize = 2048;
const TEXT_ATLAS_HEIGHT: usize = 2048;
const TEXT_ATLAS_PADDING: usize = 1;
const TEXT_ATLAS_TEXTURE_RING_LEN: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RendererFrameStats {
    pub pass_count: usize,
    pub draw_count: usize,
    pub uploaded_vertex_bytes: u64,
    pub text_glyph_instance_count: usize,
    pub text_vertex_bytes: u64,
    pub visible_layer_count: usize,
    pub direct_packet_count: usize,
    pub retained_state_update_time_us: u64,
    pub composition_time_us: u64,
    pub retained_scene_traversal_time_us: u64,
    pub retained_packet_build_time_us: u64,
    pub retained_packet_build_count: usize,
    pub retained_packet_rebuild_new_count: usize,
    pub retained_packet_rebuild_coordinate_space_count: usize,
    pub retained_packet_rebuild_signature_count: usize,
    pub retained_packet_rebuild_scene_count: usize,
    pub retained_packet_rebuild_state_count: usize,
    pub retained_packet_normalize_time_us: u64,
    pub retained_packet_signature_time_us: u64,
    pub retained_packet_raster_state_init_time_us: u64,
    pub retained_packet_scene_build_time_us: u64,
    pub retained_packet_command_count: usize,
    pub retained_packet_text_command_count: usize,
    pub retained_packet_path_command_count: usize,
    pub retained_packet_clip_path_command_count: usize,
    pub retained_packet_image_command_count: usize,
    pub retained_packet_rect_command_count: usize,
    pub retained_packet_text_command_time_us: u64,
    pub retained_packet_path_command_time_us: u64,
    pub retained_packet_clip_path_command_time_us: u64,
    pub retained_packet_image_command_time_us: u64,
    pub retained_packet_rect_command_time_us: u64,
    pub text_atlas_miss_count: usize,
    pub text_atlas_miss_time_us: u64,
    pub surface_acquire_time_us: u64,
    pub resource_collection_time_us: u64,
    pub bind_group_prepare_time_us: u64,
    pub image_bind_group_time_us: u64,
    pub analytic_path_bind_group_time_us: u64,
    pub analytic_path_bind_group_miss_count: usize,
    pub analytic_path_bind_group_upload_bytes: u64,
    pub text_atlas_bind_group_time_us: u64,
    pub text_atlas_upload_copy_time_us: u64,
    pub text_atlas_upload_write_time_us: u64,
    pub text_atlas_upload_bytes: u64,
    pub batch_prepare_time_us: u64,
    pub gpu_upload_time_us: u64,
    pub pass_encode_time_us: u64,
    pub queue_submit_time_us: u64,
    pub surface_present_time_us: u64,
    pub retained_packet_hotspot: Option<RendererPacketHotspot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererPacketHotspot {
    pub container_layer_id: Option<u64>,
    pub owner_widget_id: Option<u64>,
    pub segment_index: u32,
    pub total_time_us: u64,
    pub scene_build_time_us: u64,
    pub command_count: usize,
    pub text_command_count: usize,
    pub path_command_count: usize,
    pub rect_command_count: usize,
    pub text_command_time_us: u64,
    pub path_command_time_us: u64,
    pub rect_command_time_us: u64,
    pub text_sample: Option<String>,
}

impl RendererFrameStats {
    #[cfg(test)]
    fn from_prepared_frame(prepared: &PreparedFrameBatches) -> Self {
        Self::from_prepared_counts(
            prepared.passes.len().max(1),
            prepared
                .passes
                .iter()
                .map(|pass| pass.clip_paths.len() + pass.draws.len())
                .sum(),
            (prepared.scene_vertices.len() as u64 + prepared.clip_vertices.len() as u64)
                * VERTEX_SIZE
                + prepared.text_instances.len() as u64 * TEXT_ATLAS_INSTANCE_SIZE,
        )
    }

    fn from_prepared_counts(
        pass_count: usize,
        draw_count: usize,
        uploaded_vertex_bytes: u64,
    ) -> Self {
        Self {
            pass_count,
            draw_count,
            uploaded_vertex_bytes,
            text_glyph_instance_count: 0,
            text_vertex_bytes: 0,
            visible_layer_count: 0,
            direct_packet_count: 0,
            retained_state_update_time_us: 0,
            composition_time_us: 0,
            retained_scene_traversal_time_us: 0,
            retained_packet_build_time_us: 0,
            retained_packet_build_count: 0,
            retained_packet_rebuild_new_count: 0,
            retained_packet_rebuild_coordinate_space_count: 0,
            retained_packet_rebuild_signature_count: 0,
            retained_packet_rebuild_scene_count: 0,
            retained_packet_rebuild_state_count: 0,
            retained_packet_normalize_time_us: 0,
            retained_packet_signature_time_us: 0,
            retained_packet_raster_state_init_time_us: 0,
            retained_packet_scene_build_time_us: 0,
            retained_packet_command_count: 0,
            retained_packet_text_command_count: 0,
            retained_packet_path_command_count: 0,
            retained_packet_clip_path_command_count: 0,
            retained_packet_image_command_count: 0,
            retained_packet_rect_command_count: 0,
            retained_packet_text_command_time_us: 0,
            retained_packet_path_command_time_us: 0,
            retained_packet_clip_path_command_time_us: 0,
            retained_packet_image_command_time_us: 0,
            retained_packet_rect_command_time_us: 0,
            text_atlas_miss_count: 0,
            text_atlas_miss_time_us: 0,
            surface_acquire_time_us: 0,
            resource_collection_time_us: 0,
            bind_group_prepare_time_us: 0,
            image_bind_group_time_us: 0,
            analytic_path_bind_group_time_us: 0,
            analytic_path_bind_group_miss_count: 0,
            analytic_path_bind_group_upload_bytes: 0,
            text_atlas_bind_group_time_us: 0,
            text_atlas_upload_copy_time_us: 0,
            text_atlas_upload_write_time_us: 0,
            text_atlas_upload_bytes: 0,
            batch_prepare_time_us: 0,
            gpu_upload_time_us: 0,
            pass_encode_time_us: 0,
            queue_submit_time_us: 0,
            surface_present_time_us: 0,
            retained_packet_hotspot: None,
        }
    }

    fn with_compositor_stats(mut self, stats: RetainedCompositorFrameStats) -> Self {
        self.visible_layer_count = stats.visible_layers;
        self.direct_packet_count = stats.direct_packets;
        self.retained_state_update_time_us =
            (stats.state_update_time_ms * 1000.0).round() as u64;
        self.composition_time_us = (stats.composition_time_ms * 1000.0).round() as u64;
        self.retained_scene_traversal_time_us =
            (stats.scene_traversal_time_ms * 1000.0).round() as u64;
        self.retained_packet_build_time_us = (stats.packet_build_time_ms * 1000.0).round() as u64;
        self.retained_packet_build_count = stats.packet_build_count;
        self.retained_packet_rebuild_new_count = stats.packet_rebuild_new_count;
        self.retained_packet_rebuild_coordinate_space_count =
            stats.packet_rebuild_coordinate_space_count;
        self.retained_packet_rebuild_signature_count = stats.packet_rebuild_signature_count;
        self.retained_packet_rebuild_scene_count = stats.packet_rebuild_scene_count;
        self.retained_packet_rebuild_state_count = stats.packet_rebuild_state_count;
        self.retained_packet_normalize_time_us =
            (stats.packet_normalize_time_ms * 1000.0).round() as u64;
        self.retained_packet_signature_time_us =
            (stats.packet_signature_time_ms * 1000.0).round() as u64;
        self.retained_packet_raster_state_init_time_us =
            (stats.packet_raster_state_init_time_ms * 1000.0).round() as u64;
        self.retained_packet_scene_build_time_us =
            (stats.packet_scene_build_time_ms * 1000.0).round() as u64;
        self.retained_packet_command_count = stats.packet_command_count;
        self.retained_packet_text_command_count = stats.packet_text_command_count;
        self.retained_packet_path_command_count = stats.packet_path_command_count;
        self.retained_packet_clip_path_command_count = stats.packet_clip_path_command_count;
        self.retained_packet_image_command_count = stats.packet_image_command_count;
        self.retained_packet_rect_command_count = stats.packet_rect_command_count;
        self.retained_packet_text_command_time_us =
            (stats.packet_text_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_path_command_time_us =
            (stats.packet_path_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_clip_path_command_time_us =
            (stats.packet_clip_path_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_image_command_time_us =
            (stats.packet_image_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_rect_command_time_us =
            (stats.packet_rect_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_hotspot = stats.slowest_packet_build.map(|hotspot| RendererPacketHotspot {
            container_layer_id: hotspot.container_layer_id,
            owner_widget_id: hotspot.owner_widget_id,
            segment_index: hotspot.segment_index,
            total_time_us: (hotspot.total_time_ms * 1000.0).round() as u64,
            scene_build_time_us: (hotspot.scene_build_time_ms * 1000.0).round() as u64,
            command_count: hotspot.command_count,
            text_command_count: hotspot.text_command_count,
            path_command_count: hotspot.path_command_count,
            rect_command_count: hotspot.rect_command_count,
            text_command_time_us: (hotspot.text_command_time_ms * 1000.0).round() as u64,
            path_command_time_us: (hotspot.path_command_time_ms * 1000.0).round() as u64,
            rect_command_time_us: (hotspot.rect_command_time_ms * 1000.0).round() as u64,
            text_sample: hotspot.text_sample,
        });
        self
    }

    fn with_text_stats(mut self, stats: TextFrameStats) -> Self {
        self.text_glyph_instance_count = stats.glyph_instances;
        self.text_vertex_bytes = stats.glyph_upload_bytes;
        self.text_atlas_miss_count = stats.atlas_miss_count;
        self.text_atlas_miss_time_us = stats.atlas_miss_time_us;
        self
    }
}

pub struct WgpuRenderer {
    instance: wgpu::Instance,
    feathering_enabled: bool,
    feather_width: f32,
    text_render_mode: TextRenderMode,
    text_hinting: TextHinting,
    stem_darkening: StemDarkening,
    text_coverage_policy: TextCoveragePolicy,
    glyph_pixel_alignment_enabled: bool,
    vsync_enabled: bool,
    runtime_feathering_override: Option<FeatheringOptions>,
    runtime_text_hinting_override: Option<TextHinting>,
    runtime_stem_darkening_override: Option<StemDarkening>,
    runtime_text_coverage_policy_override: Option<TextCoveragePolicy>,
    runtime_glyph_pixel_alignment_override: Option<bool>,
    runtime_diagnostics_enabled: bool,
    frames_rendered: usize,
    capabilities: RendererCapabilities,
    last_frames: HashMap<WindowId, SceneFrame>,
    last_frame_stats: HashMap<WindowId, RendererFrameStats>,
    shared: Option<SharedRenderer>,
    text_engine: Option<TextEngine>,
    image_cache: HashMap<ImageHandle, CachedImageTexture>,
    text_atlas_textures: Vec<CachedTextAtlasTexture>,
    active_text_atlas_texture_index: usize,
    analytic_path_cache: HashMap<u64, CachedAnalyticPathGpu>,
    compositors: HashMap<WindowId, RetainedCompositorState>,
    surfaces: HashMap<WindowId, SurfaceState>,
    offscreen_targets: HashMap<WindowId, OffscreenTarget>,
    frame_resources: FrameResources,
}

#[derive(Default)]
struct FrameResources {
    stencil: Option<StencilTarget>,
    analytic_path_arena: AnalyticPathArena,
}

#[derive(Default)]
struct AnalyticPathArena {
    bind_group: Option<wgpu::BindGroup>,
    meta_buffer: Option<wgpu::Buffer>,
    contour_buffer: Option<wgpu::Buffer>,
    point_buffer: Option<wgpu::Buffer>,
    meta_capacity: usize,
    contour_capacity: usize,
    point_capacity: usize,
    used_slots: usize,
    used_contours: usize,
    used_points: usize,
}

struct StencilTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
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

const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const DEFAULT_FEATHER_WIDTH: f32 = 1.0;
const TEXT_ATLAS_RETRY_ERROR_MESSAGE: &str = "text atlas capacity exceeded; retry with reset";

fn text_atlas_retry_error() -> Error {
    Error::new(TEXT_ATLAS_RETRY_ERROR_MESSAGE)
}

fn is_text_atlas_retry_error(error: &Error) -> bool {
    error.message() == TEXT_ATLAS_RETRY_ERROR_MESSAGE
}

impl WgpuRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_feathering(mut self, feathering: FeatheringOptions) -> Self {
        self.set_feathering(feathering);
        self
    }

    pub fn with_feathering_enabled(mut self, enabled: bool) -> Self {
        self.set_feathering_enabled(enabled);
        self
    }

    pub fn with_text_coverage_policy(mut self, policy: TextCoveragePolicy) -> Self {
        self.set_text_coverage_policy(policy);
        self
    }

    pub fn with_text_render_mode(mut self, mode: TextRenderMode) -> Self {
        self.set_text_render_mode(mode);
        self
    }

    pub fn with_text_hinting(mut self, hinting: TextHinting) -> Self {
        self.set_text_hinting(hinting);
        self
    }

    pub fn with_stem_darkening(mut self, darkening: StemDarkening) -> Self {
        self.set_stem_darkening(darkening);
        self
    }

    pub fn with_glyph_pixel_alignment_enabled(mut self, enabled: bool) -> Self {
        self.set_glyph_pixel_alignment_enabled(enabled);
        self
    }

    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.set_feather_width(feather_width);
        self
    }

    pub fn with_vsync_enabled(mut self, enabled: bool) -> Self {
        self.set_vsync_enabled(enabled);
        self
    }

    pub fn feathering(&self) -> FeatheringOptions {
        FeatheringOptions::new(self.feathering_enabled, self.feather_width)
    }

    pub fn feathering_enabled(&self) -> bool {
        self.feathering_enabled
    }

    pub fn feather_width(&self) -> f32 {
        self.feather_width
    }

    pub fn vsync_enabled(&self) -> bool {
        self.vsync_enabled
    }

    pub fn text_coverage_policy(&self) -> TextCoveragePolicy {
        self.text_coverage_policy
    }

    pub fn text_render_mode(&self) -> TextRenderMode {
        self.text_render_mode
    }

    pub fn text_hinting(&self) -> TextHinting {
        self.text_hinting
    }

    pub fn stem_darkening(&self) -> StemDarkening {
        self.stem_darkening
    }

    pub fn glyph_pixel_alignment_enabled(&self) -> bool {
        self.glyph_pixel_alignment_enabled
    }

    pub fn set_feathering(&mut self, feathering: FeatheringOptions) {
        let feathering = feathering.clamped();
        self.feathering_enabled = feathering.enabled;
        self.feather_width = feathering.width;
    }

    pub fn set_feathering_enabled(&mut self, enabled: bool) {
        self.feathering_enabled = enabled;
    }

    pub fn set_text_coverage_policy(&mut self, policy: TextCoveragePolicy) {
        let policy = policy.normalized();
        if self.text_coverage_policy == policy {
            return;
        }

        self.text_coverage_policy = policy;
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_text_coverage_policy(policy);
        }
        self.invalidate_text_render_state();
    }

    pub fn set_text_render_mode(&mut self, mode: TextRenderMode) {
        if self.text_render_mode == mode {
            return;
        }

        self.text_render_mode = mode;
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_text_render_mode(mode);
        }
        self.invalidate_text_render_state();
    }

    pub fn set_text_hinting(&mut self, hinting: TextHinting) {
        let hinting = hinting.normalized();
        if self.text_hinting == hinting {
            return;
        }

        self.text_hinting = hinting;
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_text_hinting(hinting);
        }
    }

    pub fn set_stem_darkening(&mut self, darkening: StemDarkening) {
        let darkening = darkening.normalized();
        if self.stem_darkening == darkening {
            return;
        }

        self.stem_darkening = darkening;
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_stem_darkening(darkening);
        }
    }

    pub fn set_glyph_pixel_alignment_enabled(&mut self, enabled: bool) {
        self.glyph_pixel_alignment_enabled = enabled;
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_glyph_pixel_alignment_enabled(enabled);
        }
    }

    pub fn set_feather_width(&mut self, feather_width: f32) {
        self.feather_width = feather_width.max(0.0);
    }

    pub fn set_vsync_enabled(&mut self, enabled: bool) {
        self.vsync_enabled = enabled;
    }

    pub fn set_window_display_capabilities(
        &mut self,
        window_id: WindowId,
        capabilities: DisplayCapabilities,
    ) -> Result<()> {
        if let Some(surface) = self.surfaces.get_mut(&window_id) {
            surface.display_capabilities = capabilities;
        }
        self.configure_existing_surface(window_id)
    }

    pub fn window_display_capabilities(&self, window_id: WindowId) -> Option<DisplayCapabilities> {
        self.surfaces
            .get(&window_id)
            .map(|surface| surface.display_capabilities.clone())
    }

    pub fn set_window_color_management(
        &mut self,
        window_id: WindowId,
        color_management: ColorManagementMode,
    ) -> Result<()> {
        if let Some(surface) = self.surfaces.get_mut(&window_id) {
            surface.color_management = color_management;
        }
        self.configure_existing_surface(window_id)
    }

    pub fn window_output_strategy(&self, window_id: WindowId) -> Option<OutputStrategy> {
        self.surfaces.get(&window_id).map(|surface| surface.output_strategy)
    }

    pub fn set_runtime_feathering_override(&mut self, feathering: Option<FeatheringOptions>) {
        self.runtime_feathering_override = feathering.map(FeatheringOptions::clamped);
    }

    pub fn set_runtime_text_hinting_override(&mut self, hinting: Option<TextHinting>) {
        self.runtime_text_hinting_override = hinting.map(TextHinting::normalized);
    }

    pub fn set_runtime_stem_darkening_override(&mut self, darkening: Option<StemDarkening>) {
        self.runtime_stem_darkening_override = darkening.map(StemDarkening::normalized);
    }

    pub fn set_runtime_text_coverage_policy_override(
        &mut self,
        policy: Option<TextCoveragePolicy>,
    ) {
        self.runtime_text_coverage_policy_override = policy.map(TextCoveragePolicy::normalized);
    }

    pub fn set_runtime_glyph_pixel_alignment_override(&mut self, enabled: Option<bool>) {
        self.runtime_glyph_pixel_alignment_override = enabled;
    }

    pub fn set_runtime_diagnostics_enabled(&mut self, enabled: bool) {
        self.runtime_diagnostics_enabled = enabled;
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_diagnostics_enabled(enabled);
        }
        for compositor in self.compositors.values_mut() {
            compositor.set_diagnostics_enabled(enabled);
        }
    }

    fn active_feather_width(&self) -> f32 {
        self.runtime_feathering_override
            .unwrap_or_else(|| self.feathering())
            .effective_width()
    }

    fn active_text_hinting(&self) -> TextHinting {
        self.runtime_text_hinting_override
            .unwrap_or(self.text_hinting)
            .normalized()
    }

    fn active_stem_darkening(&self) -> StemDarkening {
        self.runtime_stem_darkening_override
            .unwrap_or(self.stem_darkening)
            .normalized()
    }

    fn active_text_coverage_policy(&self) -> TextCoveragePolicy {
        self.runtime_text_coverage_policy_override
            .unwrap_or(self.text_coverage_policy)
            .normalized()
    }

    fn active_glyph_pixel_alignment_enabled(&self) -> bool {
        self.runtime_glyph_pixel_alignment_override
            .unwrap_or(self.glyph_pixel_alignment_enabled)
    }

    fn invalidate_text_render_state(&mut self) {
        self.text_engine = None;
        self.text_atlas_textures.clear();
        self.active_text_atlas_texture_index = 0;
        self.compositors.clear();
        self.last_frames.clear();
        self.last_frame_stats.clear();
    }

    pub fn register_window(&mut self, window_id: WindowId, window: Arc<Window>) -> Result<()> {
        let physical_size = window.inner_size();
        let size = normalize_surface_size(physical_size.width, physical_size.height);
        let state = self.create_surface_state(window, size)?;

        self.surfaces.insert(window_id, state);
        self.offscreen_targets.remove(&window_id);
        Ok(())
    }

    pub fn remove_window(&mut self, window_id: WindowId) {
        self.surfaces.remove(&window_id);
        self.offscreen_targets.remove(&window_id);
        self.last_frames.remove(&window_id);
        self.last_frame_stats.remove(&window_id);
        self.compositors.remove(&window_id);
    }

    pub fn render(&mut self, frame: &SceneFrame) -> Result<()> {
        let viewport = normalize_framebuffer_size(frame.surface_size);
        let mut frame_stats = RendererFrameStats::default();

        if let Some(size) = viewport {
            if self.surfaces.contains_key(&frame.window_id) {
                frame_stats = self.render_surface(frame, size)?;
            } else {
                frame_stats = self.render_offscreen(frame, size)?;
            }
        }

        self.frames_rendered += 1;
        self.last_frames.insert(frame.window_id, frame.clone());
        self.last_frame_stats.insert(frame.window_id, frame_stats);
        self.analytic_path_cache
            .retain(|_, entry| self.frames_rendered.saturating_sub(entry.last_used_frame) <= 120);
        Ok(())
    }

    pub fn capabilities(&self) -> RendererCapabilities {
        self.capabilities
    }

    pub fn frames_rendered(&self) -> usize {
        self.frames_rendered
    }

    pub fn last_frame(&self, window_id: WindowId) -> Option<&SceneFrame> {
        self.last_frames.get(&window_id)
    }

    pub fn last_frame_stats(&self, window_id: WindowId) -> Option<RendererFrameStats> {
        self.last_frame_stats.get(&window_id).cloned()
    }

    pub fn text_cache_snapshot(&self, window_id: WindowId) -> RendererTextCacheSnapshot {
        let mut snapshot = self
            .text_engine
            .as_ref()
            .map(TextEngine::cache_snapshot)
            .unwrap_or_default();
        snapshot.path = self
            .compositors
            .get(&window_id)
            .map(|compositor| compositor.path_cache.snapshot())
            .unwrap_or_default();
        snapshot
    }

    pub fn capture_last_frame_rgba(&mut self, window_id: WindowId) -> Result<RgbaImage> {
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

        self.render_offscreen(&frame, size)?;
        self.capture_rgba(window_id)
    }

    pub fn capture_rgba(&self, window_id: WindowId) -> Result<RgbaImage> {
        let shared = self
            .shared
            .as_ref()
            .ok_or_else(|| Error::new("renderer has not initialized a wgpu device yet"))?;
        let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an offscreen render target available for screenshot capture",
                window_id.get()
            ))
        })?;

        let bytes_per_row = target.size.0 * 4;
        let padded_bytes_per_row = bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer_size = padded_bytes_per_row as u64 * target.size.1 as u64;
        let buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI screenshot readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SUI screenshot readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(target.size.1),
                },
            },
            wgpu::Extent3d {
                width: target.size.0,
                height: target.size.1,
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

        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((target.size.0 * target.size.1 * 4) as usize);
        for row in 0..target.size.1 as usize {
            let start = row * padded_bytes_per_row as usize;
            let row_slice = &mapped[start..start + bytes_per_row as usize];
            for chunk in row_slice.chunks_exact(4) {
                pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
            }
        }
        drop(mapped);
        buffer.unmap();

        RgbaImage::new(target.size.0, target.size.1, pixels)
    }

    fn ensure_shared(&mut self, compatible_surface: Option<&wgpu::Surface<'_>>) -> Result<()> {
        if self.shared.is_some() {
            return Ok(());
        }

        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface,
            }))
            .map_err(|error| Error::new(format!("failed to acquire wgpu adapter: {error}")))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SUI renderer device"),
            ..Default::default()
        }))
        .map_err(|error| Error::new(format!("failed to create wgpu device: {error}")))?;

        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI image bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let text_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI text atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let text_quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SUI text atlas quad"),
            contents: bytemuck::cast_slice(&TextAtlasQuadVertex::unit_quad()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let analytic_path_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI analytic path bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        self.shared = Some(SharedRenderer {
            adapter,
            device,
            queue,
            pipelines: HashMap::new(),
            image_bind_group_layout,
            analytic_path_bind_group_layout,
            image_sampler,
            text_atlas_sampler,
            text_quad_buffer,
        });

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn initialize_async(
        &mut self,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<()> {
        if self.shared.is_some() {
            return Ok(());
        }

        let adapter = self
            .instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface,
            })
            .await
            .map_err(|error| Error::new(format!("failed to acquire wgpu adapter: {error}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("SUI renderer device"),
                ..Default::default()
            })
            .await
            .map_err(|error| Error::new(format!("failed to create wgpu device: {error}")))?;

        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI image bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let text_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI text atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let text_quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SUI text atlas quad"),
            contents: bytemuck::cast_slice(&TextAtlasQuadVertex::unit_quad()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let analytic_path_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI analytic path bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        self.shared = Some(SharedRenderer {
            adapter,
            device,
            queue,
            pipelines: HashMap::new(),
            image_bind_group_layout,
            analytic_path_bind_group_layout,
            image_sampler,
            text_atlas_sampler,
            text_quad_buffer,
        });

        Ok(())
    }

    fn render_surface(
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

        let format = {
            let surface = self.surfaces.get(&frame.window_id).ok_or_else(|| {
                Error::new(format!(
                    "missing surface for window {}",
                    frame.window_id.get()
                ))
            })?;
            surface.config.format
        };
        let view = frame_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut frame_stats = self.submit_prepared_scene(prepared, format, &view)?;
        frame_stats.surface_acquire_time_us = surface_acquire_time_us;
        let surface_present_started = self.runtime_diagnostics_enabled.then(|| Instant::now());
        frame_texture.present();
        frame_stats.surface_present_time_us = surface_present_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        if suboptimal {
            self.configure_surface(frame.window_id, size)?;
        }

        Ok(frame_stats)
    }

    fn acquire_surface_texture(
        &mut self,
        window_id: WindowId,
        size: (u32, u32),
    ) -> Result<Option<(wgpu::SurfaceTexture, bool, u64)>> {
        let surface_acquire_started = self.runtime_diagnostics_enabled.then(|| Instant::now());
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

    fn render_offscreen(
        &mut self,
        frame: &SceneFrame,
        size: (u32, u32),
    ) -> Result<RendererFrameStats> {
        self.ensure_shared(None)?;

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let recreate = self
            .offscreen_targets
            .get(&frame.window_id)
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
                frame.window_id,
                OffscreenTarget {
                    texture,
                    format,
                    size,
                },
            );
        }

        let prepared = self.prepare_scene_submission(frame)?;
        let (format, view) = {
            let target = self
                .offscreen_targets
                .get(&frame.window_id)
                .ok_or_else(|| {
                    Error::new(format!(
                        "missing offscreen target for window {}",
                        frame.window_id.get()
                    ))
                })?;
            (
                target.format,
                target
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default()),
            )
        };
        self.submit_prepared_scene(prepared, format, &view)
    }

    fn resize_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let surface = self
            .surfaces
            .get(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

        if surface.config.width == size.0 && surface.config.height == size.1 {
            return Ok(());
        }

        self.configure_surface(window_id, size)
    }

    fn configure_existing_surface(&mut self, window_id: WindowId) -> Result<()> {
        let size = {
            let surface = self
                .surfaces
                .get(&window_id)
                .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;
            (surface.config.width.max(1), surface.config.height.max(1))
        };
        self.configure_surface(window_id, size)
    }

    fn configure_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let surface = self
            .surfaces
            .get_mut(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

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
        Ok(())
    }

    fn recreate_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
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

    fn prepare_scene_submission(&mut self, frame: &SceneFrame) -> Result<PreparedSceneSubmission> {
        let diagnostics_enabled = self.runtime_diagnostics_enabled;
        let feather_width = self.active_feather_width();
        let text_render_mode = self.text_render_mode();
        let text_hinting = self.active_text_hinting();
        let stem_darkening = self.active_stem_darkening();
        let text_coverage_policy = self.active_text_coverage_policy();
        let glyph_pixel_alignment_enabled = self.active_glyph_pixel_alignment_enabled();
        let mut atlas_retry_attempted = false;
        let (submission, compositor_stats, text_frame_stats) = loop {
            if self.text_engine.is_none() {
                self.text_engine = Some(TextEngine::new()?);
            }
            let attempt = {
                let text_engine = self
                    .text_engine
                    .as_mut()
                    .expect("text engine initialized before draw-op construction");
                text_engine.set_text_render_mode(text_render_mode);
                text_engine.set_text_hinting(text_hinting);
                text_engine.set_stem_darkening(stem_darkening);
                text_engine.set_text_coverage_policy(text_coverage_policy);
                text_engine.set_glyph_pixel_alignment_enabled(glyph_pixel_alignment_enabled);
                text_engine.set_diagnostics_enabled(diagnostics_enabled);
                text_engine.begin_frame();
                let compositor = self.compositors.entry(frame.window_id).or_default();
                compositor.set_diagnostics_enabled(diagnostics_enabled);
                compositor
                    .prepare_frame_submission(frame, text_engine, feather_width)
                    .map(|submission| {
                        (
                            submission,
                            compositor.last_frame_stats.clone(),
                            if diagnostics_enabled {
                                text_engine.frame_stats()
                            } else {
                                TextFrameStats::default()
                            },
                        )
                    })
            };

            match attempt {
                Ok(result) => break result,
                Err(error) if !atlas_retry_attempted && is_text_atlas_retry_error(&error) => {
                    atlas_retry_attempted = true;
                    self.invalidate_text_render_state();
                }
                Err(error) => return Err(error),
            }
        };
        let framebuffer_size = normalize_framebuffer_size(frame.surface_size).unwrap_or((1, 1));
        let mut analytic_paths = HashMap::new();
        let mut image_handles = HashSet::new();
        let mut uses_text_atlas = false;
        let resource_collection_started = diagnostics_enabled.then(|| Instant::now());
        for fragment in &submission.fragments {
            let RetainedFrameFragment::Transient(draw_ops) = fragment;
            uses_text_atlas |= collect_draw_op_resources(
                draw_ops,
                &mut analytic_paths,
                &mut image_handles,
            );
        }
        let resource_collection_time_us = resource_collection_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let bind_group_prepare_started = diagnostics_enabled.then(|| Instant::now());
        let (analytic_path_resources, analytic_path_stats) =
            self.prepare_analytic_path_resources(analytic_paths, diagnostics_enabled)?;
        let analytic_path_bind_group_time_us = analytic_path_stats.total_time_us;
        let analytic_path_bind_group_miss_count = analytic_path_stats.miss_count;
        let analytic_path_bind_group_upload_bytes = analytic_path_stats.upload_bytes;

        let image_bind_group_started = diagnostics_enabled.then(|| Instant::now());
        let mut image_bind_groups = HashMap::new();
        for handle in image_handles {
            let image = frame.image_registry.get(handle).ok_or_else(|| {
                Error::new(format!("image handle {} is not registered", handle.get()))
            })?;
            image_bind_groups.insert(handle, self.ensure_image_bind_group(handle, image)?);
        }
        let image_bind_group_time_us = image_bind_group_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        let mut text_atlas_bind_group_time_us = 0u64;
        let mut text_atlas_upload_copy_time_us = 0u64;
        let mut text_atlas_upload_write_time_us = 0u64;
        let mut text_atlas_upload_bytes = 0u64;
        let text_atlas_bind_group = if uses_text_atlas {
            let mut text_engine = self
                .text_engine
                .take()
                .expect("text engine initialized before text atlas upload");
            let (bind_group, stats) =
                self.ensure_text_atlas_bind_group(&mut text_engine, diagnostics_enabled)?;
            self.text_engine = Some(text_engine);
            text_atlas_bind_group_time_us = stats.total_time_us;
            text_atlas_upload_copy_time_us = stats.upload_copy_time_us;
            text_atlas_upload_write_time_us = stats.upload_write_time_us;
            text_atlas_upload_bytes = stats.upload_bytes;
            Some(bind_group)
        } else {
            None
        };
        let bind_group_prepare_time_us = bind_group_prepare_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        let mut prepared_fragments = Vec::new();
        let mut draw_count = 0usize;
        let mut uploaded_vertex_bytes = 0u64;
        let mut needs_stencil = false;
        let mut batch_prepare_time_us = 0u64;
        let mut gpu_upload_time_us = 0u64;

        for fragment in submission.fragments {
            let RetainedFrameFragment::Transient(draw_ops) = fragment;
            let batch_prepare_started = diagnostics_enabled.then(|| Instant::now());
            let prepared = prepare_frame_batches(draw_ops, frame.viewport, framebuffer_size);
            if let Some(started) = batch_prepare_started {
                batch_prepare_time_us += started.elapsed().as_micros() as u64;
            }
            if diagnostics_enabled {
                let (_, fragment_draw_count) = prepared_batch_counts(&prepared.passes);
                draw_count += fragment_draw_count;
                uploaded_vertex_bytes += (prepared.scene_vertices.len() as u64
                    + prepared.clip_vertices.len() as u64)
                    * VERTEX_SIZE
                    + prepared.text_instances.len() as u64 * TEXT_ATLAS_INSTANCE_SIZE;
            }

            if prepared.passes.is_empty() {
                continue;
            }

            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            needs_stencil |= prepared.passes.iter().any(|pass| !pass.clip_paths.is_empty());
            let gpu_upload_started = diagnostics_enabled.then(|| Instant::now());
            prepared_fragments.push(PreparedFragmentSubmission {
                passes: prepared.passes,
                scene_buffer: create_static_vertex_buffer(
                    &shared.device,
                    "SUI transient fragment scene",
                    &prepared.scene_vertices,
                ),
                clip_buffer: create_static_vertex_buffer(
                    &shared.device,
                    "SUI transient fragment clip",
                    &prepared.clip_vertices,
                ),
                text_instance_buffer: create_static_text_instance_buffer(
                    &shared.device,
                    "SUI transient fragment text instances",
                    &prepared.text_instances,
                ),
                translation: Vector::ZERO,
            });
            if let Some(started) = gpu_upload_started {
                gpu_upload_time_us += started.elapsed().as_micros() as u64;
            }
        }

        if needs_stencil {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            let gpu_upload_started = diagnostics_enabled.then(|| Instant::now());
            self.frame_resources
                .ensure_stencil(&shared.device, framebuffer_size);
            if let Some(started) = gpu_upload_started {
                gpu_upload_time_us += started.elapsed().as_micros() as u64;
            }
        }

        let batch_prepare_started = diagnostics_enabled.then(|| Instant::now());
        let encodable_passes = flatten_fragment_passes(&prepared_fragments);
        if let Some(started) = batch_prepare_started {
            batch_prepare_time_us += started.elapsed().as_micros() as u64;
        }
        let mut frame_stats = if diagnostics_enabled {
            RendererFrameStats::from_prepared_counts(0, draw_count, uploaded_vertex_bytes)
                .with_text_stats(text_frame_stats)
                .with_compositor_stats(compositor_stats)
        } else {
            RendererFrameStats::default()
        };
        frame_stats.resource_collection_time_us = resource_collection_time_us;
        frame_stats.bind_group_prepare_time_us = bind_group_prepare_time_us;
        frame_stats.image_bind_group_time_us = image_bind_group_time_us;
        frame_stats.analytic_path_bind_group_time_us = analytic_path_bind_group_time_us;
        frame_stats.analytic_path_bind_group_miss_count = analytic_path_bind_group_miss_count;
        frame_stats.analytic_path_bind_group_upload_bytes = analytic_path_bind_group_upload_bytes;
        frame_stats.text_atlas_bind_group_time_us = text_atlas_bind_group_time_us;
        frame_stats.text_atlas_upload_copy_time_us = text_atlas_upload_copy_time_us;
        frame_stats.text_atlas_upload_write_time_us = text_atlas_upload_write_time_us;
        frame_stats.text_atlas_upload_bytes = text_atlas_upload_bytes;
        frame_stats.batch_prepare_time_us = batch_prepare_time_us;
        frame_stats.gpu_upload_time_us = gpu_upload_time_us;
        Ok(PreparedSceneSubmission {
            viewport: frame.viewport,
            framebuffer_size,
            encodable_passes,
            image_bind_groups,
            text_atlas_bind_group,
            analytic_path_resources,
            frame_stats,
        })
    }

    fn submit_prepared_scene(
        &mut self,
        prepared: PreparedSceneSubmission,
        target_format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
    ) -> Result<RendererFrameStats> {
        let mut encoder = {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            shared
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SUI scene encoder"),
                })
        };

        let pass_encode_started = self.runtime_diagnostics_enabled.then(|| Instant::now());
        let pass_count = if prepared.encodable_passes.is_empty() {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SUI scene clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            1
        } else {
            let shared = self
                .shared
                .as_mut()
                .expect("renderer shared state initialized");
            let stencil_view = self.frame_resources.stencil.as_ref().map(|target| {
                let _ = &target.texture;
                &target.view
            });
            encode_fragment_passes(
                shared,
                &mut encoder,
                view,
                target_format,
                prepared.viewport,
                prepared.framebuffer_size,
                &prepared.encodable_passes,
                stencil_view,
                &prepared.image_bind_groups,
                prepared.text_atlas_bind_group.as_ref(),
                prepared.analytic_path_resources.as_ref(),
            )?
        };
        let pass_encode_time_us = pass_encode_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let queue_submit_started = self.runtime_diagnostics_enabled.then(|| Instant::now());
        self.shared
            .as_ref()
            .expect("renderer shared state initialized")
            .queue
            .submit([encoder.finish()]);
        let queue_submit_time_us = queue_submit_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let mut frame_stats = prepared.frame_stats;
        frame_stats.pass_encode_time_us = pass_encode_time_us;
        frame_stats.queue_submit_time_us = queue_submit_time_us;
        frame_stats.pass_count = pass_count.max(1);
        Ok(frame_stats)
    }

    fn ensure_image_bind_group(
        &mut self,
        handle: ImageHandle,
        image: &RegisteredImage,
    ) -> Result<wgpu::BindGroup> {
        if let Some(cached) = self.image_cache.get(&handle) {
            return Ok(cached.bind_group.clone());
        }

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let texture_format = match image.format() {
            RegisteredImageFormat::Rgba8 => wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI image texture"),
            size: wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        shared.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image.bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width() * 4),
                rows_per_image: Some(image.height()),
            },
            wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI image bind group"),
            layout: &shared.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&shared.image_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });

        self.image_cache.insert(
            handle,
            CachedImageTexture {
                _texture: texture,
                _view: view,
                bind_group: bind_group.clone(),
            },
        );

        Ok(bind_group)
    }

    fn ensure_text_atlas_bind_group(
        &mut self,
        text_engine: &mut TextEngine,
        collect_stats: bool,
    ) -> Result<(wgpu::BindGroup, TextAtlasBindGroupStats)> {
        let total_started = collect_stats.then(|| Instant::now());
        let upload_copy_started = collect_stats.then(|| Instant::now());
        let upload = text_engine.take_atlas_upload();
        let mut stats = TextAtlasBindGroupStats {
            upload_copy_time_us: upload_copy_started
                .map(|started| started.elapsed().as_micros() as u64)
                .unwrap_or(0),
            upload_bytes: if collect_stats {
                upload
                    .as_ref()
                    .map_or(0, |upload| upload.pixels.len() as u64)
            } else {
                0
            },
            ..TextAtlasBindGroupStats::default()
        };

        if let Some(upload) = upload {
            let target_index = if self.text_atlas_textures.is_empty() {
                0
            } else {
                (self.active_text_atlas_texture_index + 1) % TEXT_ATLAS_TEXTURE_RING_LEN
            };
            self.ensure_text_atlas_texture_slot(target_index, upload.size)?;
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            if !upload.full_texture && !self.text_atlas_textures.is_empty() {
                let source = self
                    .text_atlas_textures
                    .get(self.active_text_atlas_texture_index)
                    .expect("active text atlas texture exists before partial ring upload");
                let target = self
                    .text_atlas_textures
                    .get(target_index)
                    .expect("target text atlas texture exists before partial ring upload");
                let mut encoder =
                    shared
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("SUI text atlas ring copy"),
                        });
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &source.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &target.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: upload.size.0,
                        height: upload.size.1,
                        depth_or_array_layers: 1,
                    },
                );
                shared.queue.submit([encoder.finish()]);
            }
            let cached = self
                .text_atlas_textures
                .get(target_index)
                .expect("text atlas texture cached after creation");
            let upload_write_started = collect_stats.then(|| Instant::now());
            shared.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &cached.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: upload.offset.0,
                        y: upload.offset.1,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &upload.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(upload.extent.0 * 4),
                    rows_per_image: Some(upload.extent.1),
                },
                wgpu::Extent3d {
                    width: upload.extent.0,
                    height: upload.extent.1,
                    depth_or_array_layers: 1,
                },
            );
            stats.upload_write_time_us = upload_write_started
                .map(|started| started.elapsed().as_micros() as u64)
                .unwrap_or(0);
            self.active_text_atlas_texture_index = target_index;
        }

        let bind_group = self
            .text_atlas_textures
            .get(self.active_text_atlas_texture_index)
            .map(|cached| cached.bind_group.clone())
            .ok_or_else(|| Error::new("text atlas bind group requested before any atlas upload"))?;
        stats.total_time_us = total_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        Ok((bind_group, stats))
    }

    fn ensure_text_atlas_texture_slot(&mut self, index: usize, size: (u32, u32)) -> Result<()> {
        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized before text atlas texture setup");

        if self
            .text_atlas_textures
            .get(index)
            .is_some_and(|cached| cached.size == size)
        {
            return Ok(());
        }

        let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI text atlas texture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI text atlas bind group"),
            layout: &shared.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&shared.text_atlas_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        let cached = CachedTextAtlasTexture {
            texture,
            _view: view,
            bind_group,
            size,
        };

        if index < self.text_atlas_textures.len() {
            self.text_atlas_textures[index] = cached;
        } else {
            self.text_atlas_textures.push(cached);
        }

        Ok(())
    }

    fn prepare_analytic_path_resources(
        &mut self,
        analytic_paths: HashMap<u64, Arc<AnalyticPathCpuData>>,
        collect_stats: bool,
    ) -> Result<(
        Option<PreparedAnalyticPathResources>,
        AnalyticPathBindGroupStats,
    )> {
        if analytic_paths.is_empty() {
            return Ok((None, AnalyticPathBindGroupStats::default()));
        }

        let total_started = collect_stats.then(|| Instant::now());
        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let mut slots = HashMap::with_capacity(analytic_paths.len());
        let mut pending = Vec::new();
        let mut visible_signatures = Vec::with_capacity(analytic_paths.len());
        let mut sorted_paths: Vec<_> = analytic_paths.into_iter().collect();
        sorted_paths.sort_unstable_by_key(|(signature, _)| *signature);

        for (signature, path) in sorted_paths {
            visible_signatures.push(signature);
            if let Some(cached) = self.analytic_path_cache.get_mut(&signature) {
                cached.last_used_frame = self.frames_rendered;
                slots.insert(signature, cached.slot);
            } else {
                pending.push((signature, path));
            }
        }

        let mut stats = AnalyticPathBindGroupStats {
            miss_count: if collect_stats { pending.len() } else { 0 },
            ..AnalyticPathBindGroupStats::default()
        };
        let needs_rebuild = if self
            .frame_resources
            .analytic_path_arena
            .bind_group
            .is_none()
        {
            true
        } else if pending.is_empty() {
            false
        } else {
            let required_slots =
                self.frame_resources.analytic_path_arena.used_slots + pending.len();
            let required_contours = self.frame_resources.analytic_path_arena.used_contours
                + pending
                    .iter()
                    .map(|(_, path)| path.contours.len())
                    .sum::<usize>();
            let required_points = self.frame_resources.analytic_path_arena.used_points
                + pending
                    .iter()
                    .map(|(_, path)| path.points.len())
                    .sum::<usize>();
            !self.frame_resources.analytic_path_arena.has_capacity(
                required_slots,
                required_contours,
                required_points,
            )
        };

        if needs_rebuild {
            let mut cached_entries: Vec<_> = self
                .analytic_path_cache
                .iter()
                .map(|(signature, entry)| {
                    (
                        *signature,
                        entry.slot,
                        entry.last_used_frame,
                        entry.data.clone(),
                    )
                })
                .collect();
            cached_entries.sort_unstable_by_key(|(_, slot, _, _)| *slot);

            let total_slots = cached_entries.len() + pending.len();
            let total_contours = cached_entries
                .iter()
                .map(|(_, _, _, data)| data.contours.len())
                .sum::<usize>()
                + pending
                    .iter()
                    .map(|(_, data)| data.contours.len())
                    .sum::<usize>();
            let total_points = cached_entries
                .iter()
                .map(|(_, _, _, data)| data.points.len())
                .sum::<usize>()
                + pending
                    .iter()
                    .map(|(_, data)| data.points.len())
                    .sum::<usize>();

            self.frame_resources.analytic_path_arena.ensure_capacity(
                &shared.device,
                &shared.analytic_path_bind_group_layout,
                total_slots,
                total_contours,
                total_points,
            );

            let mut meta_data = Vec::with_capacity(total_slots);
            let mut contour_data = Vec::with_capacity(total_contours);
            let mut point_data = Vec::with_capacity(total_points);
            let mut rebuilt_cache = HashMap::with_capacity(total_slots);

            for (signature, _, last_used_frame, data) in cached_entries {
                let slot = meta_data.len() as u32;
                let contour_start = contour_data.len() as u32;
                let point_start = point_data.len() as u32;
                meta_data.push(data.meta(contour_start, point_start));
                contour_data.extend_from_slice(&data.contours);
                point_data.extend_from_slice(&data.points);
                rebuilt_cache.insert(
                    signature,
                    CachedAnalyticPathGpu {
                        data,
                        slot,
                        last_used_frame,
                    },
                );
            }

            for (signature, data) in pending {
                let slot = meta_data.len() as u32;
                let contour_start = contour_data.len() as u32;
                let point_start = point_data.len() as u32;
                meta_data.push(data.meta(contour_start, point_start));
                contour_data.extend_from_slice(&data.contours);
                point_data.extend_from_slice(&data.points);
                rebuilt_cache.insert(
                    signature,
                    CachedAnalyticPathGpu {
                        data,
                        slot,
                        last_used_frame: self.frames_rendered,
                    },
                );
            }

            let meta_buffer = self
                .frame_resources
                .analytic_path_arena
                .meta_buffer
                .as_ref()
                .expect("analytic path arena metadata buffer initialized");
            let contour_buffer = self
                .frame_resources
                .analytic_path_arena
                .contour_buffer
                .as_ref()
                .expect("analytic path arena contour buffer initialized");
            let point_buffer = self
                .frame_resources
                .analytic_path_arena
                .point_buffer
                .as_ref()
                .expect("analytic path arena point buffer initialized");
            if !meta_data.is_empty() {
                shared
                    .queue
                    .write_buffer(meta_buffer, 0, bytemuck::cast_slice(&meta_data));
            }
            if !contour_data.is_empty() {
                shared
                    .queue
                    .write_buffer(contour_buffer, 0, bytemuck::cast_slice(&contour_data));
            }
            if !point_data.is_empty() {
                shared
                    .queue
                    .write_buffer(point_buffer, 0, bytemuck::cast_slice(&point_data));
            }

            if collect_stats {
                stats.upload_bytes = (meta_data.len() * std::mem::size_of::<AnalyticPathMetaGpu>()
                    + contour_data.len() * std::mem::size_of::<AnalyticContourGpu>()
                    + point_data.len() * std::mem::size_of::<AnalyticPointGpu>())
                    as u64;
            }

            self.analytic_path_cache = rebuilt_cache;
            self.frame_resources.analytic_path_arena.used_slots = meta_data.len();
            self.frame_resources.analytic_path_arena.used_contours = contour_data.len();
            self.frame_resources.analytic_path_arena.used_points = point_data.len();

            for signature in visible_signatures {
                let slot = self
                    .analytic_path_cache
                    .get(&signature)
                    .expect("visible analytic path cached after arena rebuild")
                    .slot;
                slots.insert(signature, slot);
            }
        } else if !pending.is_empty() {
            let meta_buffer = self
                .frame_resources
                .analytic_path_arena
                .meta_buffer
                .as_ref()
                .expect("analytic path arena metadata buffer initialized");
            let contour_buffer = self
                .frame_resources
                .analytic_path_arena
                .contour_buffer
                .as_ref()
                .expect("analytic path arena contour buffer initialized");
            let point_buffer = self
                .frame_resources
                .analytic_path_arena
                .point_buffer
                .as_ref()
                .expect("analytic path arena point buffer initialized");
            let base_slot = self.frame_resources.analytic_path_arena.used_slots as u32;
            let base_contour = self.frame_resources.analytic_path_arena.used_contours as u32;
            let base_point = self.frame_resources.analytic_path_arena.used_points as u32;
            let total_contours = pending
                .iter()
                .map(|(_, data)| data.contours.len())
                .sum::<usize>();
            let total_points = pending
                .iter()
                .map(|(_, data)| data.points.len())
                .sum::<usize>();
            let mut meta_data = Vec::with_capacity(pending.len());
            let mut contour_data = Vec::with_capacity(total_contours);
            let mut point_data = Vec::with_capacity(total_points);

            for (signature, data) in pending {
                let slot = base_slot + meta_data.len() as u32;
                let contour_start = base_contour + contour_data.len() as u32;
                let point_start = base_point + point_data.len() as u32;
                meta_data.push(data.meta(contour_start, point_start));
                contour_data.extend_from_slice(&data.contours);
                point_data.extend_from_slice(&data.points);
                if collect_stats {
                    stats.upload_bytes += data.byte_size() as u64;
                }
                slots.insert(signature, slot);
                self.analytic_path_cache.insert(
                    signature,
                    CachedAnalyticPathGpu {
                        data,
                        slot,
                        last_used_frame: self.frames_rendered,
                    },
                );
            }

            if !meta_data.is_empty() {
                let meta_offset =
                    base_slot as u64 * std::mem::size_of::<AnalyticPathMetaGpu>() as u64;
                shared.queue.write_buffer(
                    meta_buffer,
                    meta_offset,
                    bytemuck::cast_slice(&meta_data),
                );
            }
            if !contour_data.is_empty() {
                let contour_offset =
                    base_contour as u64 * std::mem::size_of::<AnalyticContourGpu>() as u64;
                shared.queue.write_buffer(
                    contour_buffer,
                    contour_offset,
                    bytemuck::cast_slice(&contour_data),
                );
            }
            if !point_data.is_empty() {
                let point_offset =
                    base_point as u64 * std::mem::size_of::<AnalyticPointGpu>() as u64;
                shared.queue.write_buffer(
                    point_buffer,
                    point_offset,
                    bytemuck::cast_slice(&point_data),
                );
            }

            self.frame_resources.analytic_path_arena.used_slots += meta_data.len();
            self.frame_resources.analytic_path_arena.used_contours += contour_data.len();
            self.frame_resources.analytic_path_arena.used_points += point_data.len();
        }

        let bind_group = self
            .frame_resources
            .analytic_path_arena
            .bind_group
            .as_ref()
            .expect("analytic path arena bind group initialized")
            .clone();
        stats.total_time_us = total_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        Ok((
            Some(PreparedAnalyticPathResources { bind_group, slots }),
            stats,
        ))
    }

    fn create_surface_state(
        &mut self,
        window: Arc<Window>,
        size: (u32, u32),
    ) -> Result<SurfaceState> {
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| Error::new(format!("failed to create wgpu surface: {error}")))?;
        self.ensure_shared(Some(&surface))?;

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let default_capabilities = DisplayCapabilities::default();
        let default_color_management = ColorManagementMode::default();
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
        })
    }
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self {
            instance: wgpu::Instance::default(),
            feathering_enabled: true,
            feather_width: DEFAULT_FEATHER_WIDTH,
            text_render_mode: TextRenderMode::default(),
            text_hinting: TextHinting::default(),
            stem_darkening: StemDarkening::default(),
            text_coverage_policy: TextCoveragePolicy::default(),
            glyph_pixel_alignment_enabled: true,
            vsync_enabled: true,
            runtime_feathering_override: None,
            runtime_text_hinting_override: None,
            runtime_stem_darkening_override: None,
            runtime_text_coverage_policy_override: None,
            runtime_glyph_pixel_alignment_override: None,
            runtime_diagnostics_enabled: true,
            frames_rendered: 0,
            capabilities: RendererCapabilities::default(),
            last_frames: HashMap::new(),
            last_frame_stats: HashMap::new(),
            shared: None,
            text_engine: None,
            image_cache: HashMap::new(),
            text_atlas_textures: Vec::new(),
            active_text_atlas_texture_index: 0,
            analytic_path_cache: HashMap::new(),
            compositors: HashMap::new(),
            surfaces: HashMap::new(),
            offscreen_targets: HashMap::new(),
            frame_resources: FrameResources::default(),
        }
    }
}

impl fmt::Debug for WgpuRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WgpuRenderer")
            .field("feathering_enabled", &self.feathering_enabled)
            .field("feather_width", &self.feather_width)
            .field("text_coverage_policy", &self.text_coverage_policy)
            .field(
                "glyph_pixel_alignment_enabled",
                &self.glyph_pixel_alignment_enabled,
            )
            .field("frames_rendered", &self.frames_rendered)
            .field("capabilities", &self.capabilities)
            .field("last_frame_count", &self.last_frames.len())
            .field("last_frame_stats_count", &self.last_frame_stats.len())
            .field("has_device", &self.shared.is_some())
            .field("surface_count", &self.surfaces.len())
            .finish()
    }
}

impl FrameResources {
    fn ensure_stencil(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let needs_recreate = self
            .stencil
            .as_ref()
            .is_none_or(|target| target.size != size);
        if !needs_recreate {
            return;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI scene stencil"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.stencil = Some(StencilTarget {
            texture,
            view,
            size,
        });
    }
}

impl AnalyticPathArena {
    fn has_capacity(&self, meta_count: usize, contour_count: usize, point_count: usize) -> bool {
        self.bind_group.is_some()
            && self.meta_capacity >= meta_count
            && self.contour_capacity >= contour_count
            && self.point_capacity >= point_count
    }

    fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        meta_count: usize,
        contour_count: usize,
        point_count: usize,
    ) {
        if self.has_capacity(meta_count, contour_count, point_count) {
            return;
        }

        let meta_capacity = grow_analytic_path_capacity(self.meta_capacity, meta_count);
        let contour_capacity = grow_analytic_path_capacity(self.contour_capacity, contour_count);
        let point_capacity = grow_analytic_path_capacity(self.point_capacity, point_count);

        let meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI analytic path metadata arena"),
            size: analytic_path_buffer_size::<AnalyticPathMetaGpu>(meta_capacity),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let contour_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI analytic path contour arena"),
            size: analytic_path_buffer_size::<AnalyticContourGpu>(contour_capacity),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI analytic path point arena"),
            size: analytic_path_buffer_size::<AnalyticPointGpu>(point_capacity),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI analytic path arena bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: contour_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: point_buffer.as_entire_binding(),
                },
            ],
        });

        self.bind_group = Some(bind_group);
        self.meta_buffer = Some(meta_buffer);
        self.contour_buffer = Some(contour_buffer);
        self.point_buffer = Some(point_buffer);
        self.meta_capacity = meta_capacity;
        self.contour_capacity = contour_capacity;
        self.point_capacity = point_capacity;
    }
}

const SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const TEXTURED_SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
};

@group(0) @binding(0)
var image_sampler: sampler;

@group(0) @binding(1)
var image_texture: texture_2d<f32>;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.tex_coords = tex_coords;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, in.tex_coords) * in.color;
}
"#;

const TEXT_ATLAS_SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) metadata: vec2<f32>,
};

@group(0) @binding(0)
var text_atlas_sampler: sampler;

@group(0) @binding(1)
var text_atlas_texture: texture_2d<f32>;

@vertex
fn vs_main(
    @location(0) local_pos: vec2<f32>,
    @location(1) top_left: vec2<f32>,
    @location(2) x_axis: vec2<f32>,
    @location(3) y_axis: vec2<f32>,
    @location(4) uv_min: vec2<f32>,
    @location(5) uv_max: vec2<f32>,
    @location(6) color: vec4<f32>,
    @location(7) metadata: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(top_left + local_pos.x * x_axis + local_pos.y * y_axis, 0.0, 1.0);
    out.color = color;
    out.tex_coords = uv_min + local_pos * (uv_max - uv_min);
    out.metadata = metadata;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(text_atlas_texture, text_atlas_sampler, in.tex_coords);
    if in.color.a < 0.0 {
        let opacity = -in.color.a;
        let alpha = sampled.a * opacity;
        return vec4<f32>(sampled.rgb * alpha, alpha);
    }

    if in.metadata.x > 0.5 {
        let coverage = sampled.rgb;
        let max_coverage = max(max(coverage.r, coverage.g), coverage.b);
        let premul = in.color.rgb * coverage * in.color.a;
        return vec4<f32>(premul, in.color.a * max_coverage);
    }

    if in.metadata.y > 0.5 {
        let coverage = (sampled.r + sampled.g + sampled.b) / 3.0;
        let alpha = in.color.a * coverage;
        return vec4<f32>(in.color.rgb * alpha, alpha);
    }

    let coverage = sampled.a;
    let alpha = in.color.a * coverage;
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
"#;

const ANALYTIC_PATH_SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) scene_position: vec2<f32>,
    @location(2) @interpolate(flat) path_index: u32,
};

struct AnalyticPathMeta {
    contour_start: u32,
    contour_count: u32,
    point_start: u32,
    mode: u32,
    feather_width: f32,
    stroke_width: f32,
    _pad0: vec2<f32>,
};

struct AnalyticContour {
    start: u32,
    len: u32,
    flags: u32,
    _pad0: u32,
};

const ANALYTIC_CONTOUR_FLAG_CLOSED: u32 = 1u;
const ANALYTIC_PATH_MODE_FILL: u32 = 0u;
const ANALYTIC_PATH_MODE_STROKE: u32 = 1u;

struct AnalyticPoint {
    position: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<storage, read> path_metas: array<AnalyticPathMeta>;

@group(0) @binding(1)
var<storage, read> contours: array<AnalyticContour>;

@group(0) @binding(2)
var<storage, read> points: array<AnalyticPoint>;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) scene_position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.scene_position = scene_position;
    out.path_index = instance_index;
    return out;
}

fn segment_distance(point: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let denom = max(dot(ab, ab), 1e-5);
    let t = clamp(dot(point - a, ab) / denom, 0.0, 1.0);
    return length(point - (a + (ab * t)));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let path_meta = path_metas[in.path_index];
    if path_meta.contour_count == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let point = in.scene_position;
    var inside = false;
    var min_distance = 1e9;

    for (var contour_index = 0u; contour_index < path_meta.contour_count; contour_index = contour_index + 1u) {
        let contour = contours[path_meta.contour_start + contour_index];
        if contour.len < 2u {
            continue;
        }

        let closed = (contour.flags & ANALYTIC_CONTOUR_FLAG_CLOSED) != 0u;
        let point_start = path_meta.point_start + contour.start;
        var previous = select(
            points[point_start].position,
            points[point_start + contour.len - 1u].position,
            closed,
        );
        var start_index = select(1u, 0u, closed);
        for (var point_index = start_index; point_index < contour.len; point_index = point_index + 1u) {
            let current = points[point_start + point_index].position;
                let denom = previous.y - current.y;
                let safe_denom = select(
                    denom,
                    select(-1e-5, 1e-5, denom >= 0.0),
                    abs(denom) < 1e-5,
                );
            let intersects = ((current.y > point.y) != (previous.y > point.y))
                && (point.x < (((previous.x - current.x) * (point.y - current.y))
                / safe_denom) + current.x);
            if intersects {
                inside = !inside;
            }

            min_distance = min(min_distance, segment_distance(point, previous, current));
            previous = current;
        }
    }

    let derivative_width = length(vec2<f32>(fwidth(point.x), fwidth(point.y)));
    let feather = max(path_meta.feather_width, derivative_width);
    var coverage = 0.0;

    if path_meta.mode == ANALYTIC_PATH_MODE_FILL {
        let signed_distance = select(min_distance, -min_distance, inside);
        coverage = clamp(0.5 - (signed_distance / max(feather, 1e-4)), 0.0, 1.0);
    } else {
        if path_meta.stroke_width <= feather {
            let opacity = clamp(path_meta.stroke_width / max(feather, 1e-4), 0.0, 1.0);
            coverage = opacity * clamp(1.0 - (min_distance / max(feather, 1e-4)), 0.0, 1.0);
        } else {
            let inner_radius = max(0.0, 0.5 * (path_meta.stroke_width - path_meta.feather_width));
            let outer_radius = 0.5 * (path_meta.stroke_width + path_meta.feather_width);
            coverage = select(
                clamp((outer_radius - min_distance) / max(feather, 1e-4), 0.0, 1.0),
                1.0,
                min_distance <= inner_radius,
            );
        }
    }

    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
"#;

#[cfg(test)]
mod tests {
    use super::{
        CachedGlyphAtlas, CachedGlyphMesh, ClipState, ColorManagementMode,
        CompositionContainerId, DEFAULT_FEATHER_WIDTH, DisplayCapabilities,
        DisplayColorPrimaries, DisplayTransferFunction, DrawOp, DrawOpArena, DrawOpKind,
        DynamicRangeMode, OutputStrategy, PreparedClipPath, PreparedDrawBatch,
        PreparedDrawKind, PreparedFrameBatches, PreparedPassBatch, PreparedVertices,
        RendererFrameStats, RetainedCompositorState, RetainedPacketId, ScissorRect,
        StemDarkening, TextCoveragePolicy, TextEngine, TextHinting, TextRenderMode,
        TEXT_ATLAS_HEIGHT, TEXT_ATLAS_WIDTH, TextAtlas, TextAtlasColorMode, VERTEX_SIZE,
        Vertex, WgpuRenderer, RequestedColorManagementMode, RequestedDynamicRangeMode,
        RequestedOutputColorPrimaries, append_cached_path_mesh, batch_draw_ops, build_vertices,
        prepare_frame_batches, SwashImageContent, SwashSource, SwashStrikeWith,
        scene::{
            CachedDrawBatch, CachedPassBatch, allows_lcd_text,
            append_cached_glyph_atlas, apply_stem_darkening_to_coverage,
            convert_subpixel_texel_for_mode, glyph_raster_offset, linearized_color_unorm,
            prepare_cached_passes, select_output_strategy, swash_image_to_rgba,
        },
        shader_color, to_ndc,
    };
    use std::sync::Arc;
    use sui_core::{
        Color, FontHandle, ImageHandle, Path, PathBuilder, Point, Rect, Size, Transform, Vector,
        WidgetId, WindowId,
    };
    use sui_scene::{
        ImageRegistry, ImageSource, LayerCompositionMode, RegisteredImage, Scene,
        SceneCommand, SceneFrame, SceneLayer, SceneLayerDescriptor, SceneLayerId, SceneLayerUpdate,
        SceneLayerUpdateKind, StrokeStyle,
    };
    use sui_text::{
        FontRegistry, RegisteredFont, ShapedGlyph, ShapedText, TextLayoutRegistry, TextRun,
        TextStyle, TextSystem,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum LayerCachePolicy {
        Auto,
        Direct,
        Cached,
    }

    trait TestSceneLayerDescriptorExt {
        fn with_cache_policy(self, _cache_policy: LayerCachePolicy) -> Self;
    }

    impl TestSceneLayerDescriptorExt for SceneLayerDescriptor {
        fn with_cache_policy(self, _cache_policy: LayerCachePolicy) -> Self {
            self
        }
    }

    fn load_test_font() -> RegisteredFont {
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();
        let families = [fontdb::Family::SansSerif];
        let font_id = font_db
            .query(&fontdb::Query {
                families: &families,
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })
            .or_else(|| font_db.faces().next().map(|face| face.id))
            .expect("system font available for renderer tests");

        font_db
            .with_face_data(font_id, |font_data, face_index| {
                RegisteredFont::from_bytes(font_data.to_vec()).with_face_index(face_index)
            })
            .expect("font data should be readable from system font database")
    }

    fn content_update(widget_id: WidgetId) -> SceneLayerUpdate {
        SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Content,
            SceneLayerDescriptor::new(SceneLayerId::from_widget(widget_id), widget_id, Rect::ZERO),
        )
        .with_damage(Rect::ZERO)
    }

    fn content_updates<const N: usize>(widget_ids: [WidgetId; N]) -> Vec<SceneLayerUpdate> {
        widget_ids.into_iter().map(content_update).collect()
    }

    fn prepare_with_compositor(
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        compositor: &mut RetainedCompositorState,
    ) -> sui_core::Result<DrawOpArena> {
        compositor.prepare_frame(frame, text_engine, DEFAULT_FEATHER_WIDTH)
    }

    fn packet_signature(
        compositor: &RetainedCompositorState,
        container: CompositionContainerId,
    ) -> u64 {
        compositor.packets[&RetainedPacketId {
            container,
            segment_index: 0,
        }]
            .signature
    }

    fn assert_rgba_images_match(left: &super::RgbaImage, right: &super::RgbaImage) {
        assert_eq!(left.width(), right.width(), "image widths differ");
        assert_eq!(left.height(), right.height(), "image heights differ");

        let mut diff_count = 0usize;
        let mut diff_bounds: Option<(u32, u32, u32, u32)> = None;
        let width = left.width();
        for (index, (left_px, right_px)) in left
            .pixels()
            .chunks_exact(4)
            .zip(right.pixels().chunks_exact(4))
            .enumerate()
        {
            if left_px != right_px {
                diff_count += 1;
                let x = (index as u32) % width;
                let y = (index as u32) / width;
                diff_bounds = Some(match diff_bounds {
                    Some((min_x, min_y, max_x, max_y)) => {
                        (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                    }
                    None => (x, y, x, y),
                });
            }
        }

        if diff_count != 0 {
            let (min_x, min_y, max_x, max_y) = diff_bounds.expect("diff bounds present");
            panic!(
                "images differ at {} pixels within bounds ({}, {})..({}, {})",
                diff_count, min_x, min_y, max_x, max_y
            );
        }
    }

    fn rgba_image_diff_count(left: &super::RgbaImage, right: &super::RgbaImage) -> usize {
        assert_eq!(left.width(), right.width(), "image widths differ");
        assert_eq!(left.height(), right.height(), "image heights differ");

        left.pixels()
            .chunks_exact(4)
            .zip(right.pixels().chunks_exact(4))
            .filter(|(left_px, right_px)| left_px != right_px)
            .count()
    }

    fn ink_pixel_count(image: &super::RgbaImage, rect: Rect) -> usize {
        let min_x = rect.x().floor().max(0.0) as u32;
        let min_y = rect.y().floor().max(0.0) as u32;
        let max_x = rect.max_x().ceil().min(image.width() as f32) as u32;
        let max_y = rect.max_y().ceil().min(image.height() as f32) as u32;
        let pixels = image.pixels();
        let width = image.width() as usize;

        let mut count = 0usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let index = ((y as usize * width) + x as usize) * 4;
                let red = pixels[index] as i32;
                let green = pixels[index + 1] as i32;
                let blue = pixels[index + 2] as i32;
                let alpha = pixels[index + 3] as i32;
                if alpha > 0 && (red + green + blue) < 680 {
                    count += 1;
                }
            }
        }
        count
    }

    fn rgba_pixel(image: &super::RgbaImage, x: u32, y: u32) -> [u8; 4] {
        let width = image.width() as usize;
        let index = ((y as usize * width) + x as usize) * 4;
        let pixels = image.pixels();
        [
            pixels[index],
            pixels[index + 1],
            pixels[index + 2],
            pixels[index + 3],
        ]
    }

    fn build_translucent_scroll_child_frame(
        window_id: WindowId,
        scroll_cache_policy: LayerCachePolicy,
        child_cache_policy: LayerCachePolicy,
        child_y: f32,
        update_kind: SceneLayerUpdateKind,
    ) -> SceneFrame {
        let shell_id = WidgetId::new(210);
        let scroll_id = WidgetId::new(211);
        let child_id = WidgetId::new(212);
        let scroll_bounds = Rect::new(24.0, 24.0, 382.0, 292.0);
        let child_bounds = Rect::new(42.0, child_y, 360.0, 220.0);
        let selected_row = Rect::new(42.0, child_y + 32.0, 360.0, 28.0);
        let thumb = Rect::new(396.0, child_y + 14.0, 4.0, 58.0);

        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 430.0, 360.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(scroll_id),
            scroll_id,
            scroll_bounds,
        )
        .with_content_bounds(scroll_bounds)
        .with_paint_bounds(scroll_bounds)
        .with_cache_policy(scroll_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let child_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(child_id),
            child_id,
            child_bounds,
        )
        .with_content_bounds(child_bounds)
        .with_paint_bounds(child_bounds)
        .with_cache_policy(child_cache_policy);

        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: child_bounds,
            brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
        });
        child_scene.push(SceneCommand::PushClip { rect: child_bounds });
        child_scene.push(SceneCommand::FillPath {
            path: Path::rounded_rect(selected_row, 6.0),
            brush: Color::rgba(0.09, 0.40, 0.92, 0.14).into(),
        });
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(58.0, child_y + 40.0, 172.0, 12.0),
            brush: Color::rgba(0.17, 0.21, 0.29, 1.0).into(),
        });
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(58.0, child_y + 64.0, 140.0, 10.0),
            brush: Color::rgba(0.45, 0.52, 0.61, 1.0).into(),
        });
        child_scene.push(SceneCommand::PopClip);
        child_scene.push(SceneCommand::FillPath {
            path: Path::rounded_rect(thumb, 2.0),
            brush: Color::rgba(0.54, 0.60, 0.68, 0.75).into(),
        });

        let mut scroll_scene = Scene::new();
        scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            child_descriptor.clone(),
            child_scene,
        )));

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 430.0, 360.0),
            brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
        });
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            scroll_descriptor.clone(),
            scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor,
            shell_scene,
        )));

        let update_damage = if update_kind == SceneLayerUpdateKind::Transform {
            Rect::new(42.0, 0.0, 360.0, 320.0)
        } else {
            child_bounds
        };

        SceneFrame {
            window_id,
            viewport: Size::new(430.0, 360.0),
            surface_size: Size::new(430.0, 360.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(update_kind, child_descriptor)
                    .with_damage(update_damage),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    }

    fn logical_x_from_ndc(ndc_x: f32, viewport: Size) -> f32 {
        ((ndc_x + 1.0) * 0.5) * viewport.width
    }

    fn is_physically_pixel_aligned(value: f32, scale_factor: f32) -> bool {
        let physical = value * scale_factor;
        (physical - physical.round()).abs() < 0.0001
    }

    #[test]
    fn build_vertices_applies_clip_and_transform_to_fill_rects() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::PushTransform {
            transform: Transform::translation(10.0, 5.0),
        });
        scene.push(SceneCommand::PushClip {
            rect: Rect::new(0.0, 0.0, 16.0, 12.0),
        });
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 3.0, 20.0, 10.0),
            brush: Color::WHITE.into(),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(1),
                viewport: Size::new(100.0, 100.0),
                surface_size: Size::new(100.0, 100.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
        )
        .unwrap();

        let expected_min = to_ndc(14.0, 8.0, Size::new(100.0, 100.0));
        let expected_max = to_ndc(26.0, 17.0, Size::new(100.0, 100.0));

        assert!(vertices.len() > 6);
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.position == expected_min)
        );
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.position == expected_max)
        );
        assert!(vertices.iter().any(|vertex| vertex.color[3] == 0.0));
    }

    #[test]
    fn build_vertices_supports_text_and_stroke_primitives() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 80.0, 24.0),
            text: "scene".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));
        scene.push(SceneCommand::StrokeRect {
            rect: Rect::new(2.0, 2.0, 20.0, 10.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
            stroke: StrokeStyle::new(2.0),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(2),
                viewport: Size::new(100.0, 80.0),
                surface_size: Size::new(100.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
        assert!(vertices.len() >= 30);
    }

    #[test]
    fn build_vertices_supports_non_rect_paths() {
        let mut triangle = Path::builder();
        triangle
            .move_to(Point::new(10.0, 10.0))
            .line_to(Point::new(40.0, 10.0))
            .line_to(Point::new(24.0, 36.0))
            .close();

        let mut curve = Path::builder();
        curve
            .move_to(Point::new(8.0, 44.0))
            .quad_to(Point::new(24.0, 24.0), Point::new(48.0, 44.0));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillPath {
            path: triangle.build(),
            brush: Color::rgba(0.2, 0.8, 0.4, 1.0).into(),
        });
        scene.push(SceneCommand::StrokePath {
            path: curve.build(),
            brush: Color::rgba(0.9, 0.4, 0.2, 1.0).into(),
            stroke: StrokeStyle::new(3.0),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(5),
                viewport: Size::new(80.0, 60.0),
                surface_size: Size::new(80.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
        assert!(vertices.len() >= 12);
    }

    #[test]
    fn shader_color_linearizes_srgb_inputs() {
        let rgba = shader_color(Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 1.0));

        assert!((rgba[0] - 0.05448).abs() < 0.0001);
        assert!((rgba[1] - 0.02315).abs() < 0.0001);
        assert!((rgba[2] - 0.66539).abs() < 0.0001);
        assert_eq!(rgba[3], 1.0);
    }

    #[test]
    fn shader_color_converts_display_p3_primaries_into_linear_srgb_working_space() {
        let rgba = shader_color(Color::display_p3(1.0, 0.0, 0.0, 1.0));

        assert!((rgba[0] - 1.22494).abs() < 0.0001);
        assert!((rgba[1] + 0.04205).abs() < 0.0001);
        assert!((rgba[2] + 0.01963).abs() < 0.0001);
        assert_eq!(rgba[3], 1.0);
    }

    #[test]
    fn select_output_strategy_prefers_wide_gamut_when_requested_and_supported() {
        let strategy = select_output_strategy(
            &[wgpu::TextureFormat::Bgra8UnormSrgb],
            DisplayCapabilities {
                supports_wide_gamut: true,
                preferred_primaries: DisplayColorPrimaries::DisplayP3,
                ..DisplayCapabilities::default()
            },
            ColorManagementMode {
                mode: RequestedColorManagementMode::PreferWideGamut,
                output_primaries: RequestedOutputColorPrimaries::DisplayP3,
                dynamic_range: RequestedDynamicRangeMode::Automatic,
            },
        );

        assert_eq!(
            strategy,
            OutputStrategy::WideGamutSurface {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                primaries: DisplayColorPrimaries::DisplayP3,
            }
        );
    }

    #[test]
    fn select_output_strategy_uses_hdr_intermediate_when_hdr_is_requested_without_native_support() {
        let strategy = select_output_strategy(
            &[wgpu::TextureFormat::Bgra8UnormSrgb],
            DisplayCapabilities {
                supports_wide_gamut: true,
                supports_hdr: true,
                preferred_primaries: DisplayColorPrimaries::DisplayP3,
                preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
                native_hdr_presentation_supported: false,
                ..DisplayCapabilities::default()
            },
            ColorManagementMode {
                mode: RequestedColorManagementMode::PreferHdr,
                output_primaries: RequestedOutputColorPrimaries::DisplayP3,
                dynamic_range: RequestedDynamicRangeMode::HighDynamicRange,
            },
        );

        assert_eq!(
            strategy,
            OutputStrategy::HdrIntermediateThenToneMap {
                intermediate_format: wgpu::TextureFormat::Rgba16Float,
                surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
                primaries: DisplayColorPrimaries::DisplayP3,
            }
        );
    }

    #[test]
    fn select_output_strategy_uses_native_hdr_surface_when_supported() {
        let strategy = select_output_strategy(
            &[
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureFormat::Bgra8UnormSrgb,
            ],
            DisplayCapabilities {
                supports_wide_gamut: true,
                supports_hdr: true,
                preferred_primaries: DisplayColorPrimaries::DisplayP3,
                preferred_dynamic_range: DynamicRangeMode::HighDynamicRange,
                native_hdr_presentation_supported: true,
                ..DisplayCapabilities::default()
            },
            ColorManagementMode {
                mode: RequestedColorManagementMode::PreferHdr,
                output_primaries: RequestedOutputColorPrimaries::DisplayP3,
                dynamic_range: RequestedDynamicRangeMode::HighDynamicRange,
            },
        );

        assert_eq!(
            strategy,
            OutputStrategy::HdrNativeSurface {
                format: wgpu::TextureFormat::Rgba16Float,
                primaries: DisplayColorPrimaries::DisplayP3,
                transfer: DisplayTransferFunction::LinearExtended,
            }
        );
    }

    #[test]
    fn shader_color_preserves_linear_display_p3_channels_before_gamut_conversion() {
        let encoded = shader_color(Color::display_p3(0.5, 0.25, 0.75, 1.0));
        let linear = shader_color(Color::linear_display_p3(
            0.21404114,
            0.05087609,
            0.52252156,
            1.0,
        ));

        for index in 0..3 {
            assert!((encoded[index] - linear[index]).abs() < 0.0001);
        }
        assert_eq!(linear[3], 1.0);
    }

    #[test]
    fn cached_path_mesh_linearizes_srgb_inputs() {
        let mut mesh = CachedGlyphMesh::default();
        let a = mesh.push_vertex(Point::new(0.0, 0.0), 1.0);
        let b = mesh.push_vertex(Point::new(10.0, 0.0), 1.0);
        let c = mesh.push_vertex(Point::new(0.0, 10.0), 1.0);
        mesh.add_triangle(a, b, c);

        let color = Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 1.0);
        let mut vertices = Vec::new();
        append_cached_path_mesh(&mut vertices, &mesh, color, Size::new(32.0, 32.0));

        assert_eq!(vertices.len(), 3);
        let expected = shader_color(color);
        for vertex in vertices {
            assert!((vertex.color[0] - expected[0]).abs() < 0.0001);
            assert!((vertex.color[1] - expected[1]).abs() < 0.0001);
            assert!((vertex.color[2] - expected[2]).abs() < 0.0001);
            assert_eq!(vertex.color[3], 1.0);
        }
    }

    #[test]
    fn cached_glyph_atlas_linearizes_srgb_inputs() {
        let atlas = CachedGlyphAtlas {
            scale: 12.0,
            offset: Vector::new(1.0, 2.0),
            size: Size::new(8.0, 10.0),
            uv_min: [0.25, 0.5],
            uv_max: [0.5, 0.75],
            color_mode: TextAtlasColorMode::Grayscale,
            is_color: false,
        };
        let glyph = ShapedGlyph {
            glyph_id: 42,
            cluster: 0,
            span_id: sui_text::TextSpanId {
                paragraph_index: 0,
                span_index: 0,
            },
            run_index: 0,
            line_index: 0,
            face_index: 0,
            origin_x: 12.0,
            origin_y: 20.0,
            advance: Vector::new(8.0, 0.0),
            scale: 12.0,
            bounds: Some(Rect::new(13.0, 22.0, 8.0, 10.0)),
        };

        let color = Color::srgba(66.0 / 255.0, 42.0 / 255.0, 213.0 / 255.0, 0.75);
        let mut vertices = Vec::new();
        append_cached_glyph_atlas(
            &mut vertices,
            &atlas,
            &glyph,
            color,
            Transform::IDENTITY,
            Size::new(64.0, 64.0),
            1.0,
            true,
        );

        assert_eq!(vertices.len(), 6);
        let expected = shader_color(color);
        for vertex in vertices {
            assert!((vertex.color[0] - expected[0]).abs() < 0.0001);
            assert!((vertex.color[1] - expected[1]).abs() < 0.0001);
            assert!((vertex.color[2] - expected[2]).abs() < 0.0001);
            assert!((vertex.color[3] - expected[3]).abs() < 0.0001);
        }
    }

    #[test]
    fn swash_placement_offsets_are_converted_to_screen_space() {
        let offset = glyph_raster_offset(
            &swash::zeno::Placement {
                left: 6,
                top: 10,
                width: 12,
                height: 14,
            },
            2.0,
        );

        assert_eq!(offset, Vector::new(3.0, -5.0));
    }

    #[test]
    fn cached_color_glyph_atlas_uses_opacity_sentinel() {
        let atlas = CachedGlyphAtlas {
            scale: 12.0,
            offset: Vector::new(1.0, 2.0),
            size: Size::new(8.0, 10.0),
            uv_min: [0.25, 0.5],
            uv_max: [0.5, 0.75],
            color_mode: TextAtlasColorMode::Grayscale,
            is_color: true,
        };
        let glyph = ShapedGlyph {
            glyph_id: 42,
            cluster: 0,
            span_id: sui_text::TextSpanId {
                paragraph_index: 0,
                span_index: 0,
            },
            run_index: 0,
            line_index: 0,
            face_index: 0,
            origin_x: 12.0,
            origin_y: 20.0,
            advance: Vector::new(8.0, 0.0),
            scale: 12.0,
            bounds: Some(Rect::new(13.0, 22.0, 8.0, 10.0)),
        };

        let mut vertices = Vec::new();
        append_cached_glyph_atlas(
            &mut vertices,
            &atlas,
            &glyph,
            Color::srgba(0.2, 0.4, 0.6, 0.75),
            Transform::IDENTITY,
            Size::new(64.0, 64.0),
            1.0,
            true,
        );

        assert_eq!(vertices.len(), 6);
        for vertex in vertices {
            assert_eq!(vertex.color[0], 1.0);
            assert_eq!(vertex.color[1], 1.0);
            assert_eq!(vertex.color[2], 1.0);
            assert_eq!(vertex.color[3], -0.75);
        }
    }

    #[test]
    fn swash_color_glyph_images_are_linearized_for_text_atlas() {
        let image = swash::scale::image::Image {
            source: SwashSource::ColorBitmap(SwashStrikeWith::BestFit),
            content: SwashImageContent::Color,
            placement: swash::zeno::Placement {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            },
            data: vec![66, 42, 213, 128],
        };

        let rasterized = swash_image_to_rgba(
            &image,
            14.0,
            TextRenderMode::Grayscale,
            StemDarkening::None,
            TextCoveragePolicy::Linear,
        )
            .expect("color glyph should convert into atlas pixels");

        assert!(rasterized.is_color);
        assert_eq!(rasterized.pixels.len(), 4);
        assert_eq!(rasterized.pixels[0], linearized_color_unorm(66));
        assert_eq!(rasterized.pixels[1], linearized_color_unorm(42));
        assert_eq!(rasterized.pixels[2], linearized_color_unorm(213));
        assert_eq!(rasterized.pixels[3], 128);
    }

    #[test]
    fn text_atlas_shader_outputs_premultiplied_alpha() {
        let color = shader_color(Color::srgba(
            66.0 / 255.0,
            42.0 / 255.0,
            213.0 / 255.0,
            0.75,
        ));
        let coverage = 0.5;
        let alpha = color[3] * coverage;
        let premultiplied = [color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha];

        assert!((premultiplied[0] - 0.02043).abs() < 0.0001);
        assert!((premultiplied[1] - 0.00868).abs() < 0.0001);
        assert!((premultiplied[2] - 0.24952).abs() < 0.0001);
        assert!((premultiplied[3] - 0.375).abs() < 0.0001);
    }

    #[test]
    fn color_text_atlas_shader_outputs_sampled_premultiplied_alpha() {
        let sampled = [
            linearized_color_unorm(66) as f32 / 255.0,
            linearized_color_unorm(42) as f32 / 255.0,
            linearized_color_unorm(213) as f32 / 255.0,
            0.5,
        ];
        let opacity = 0.75;
        let alpha = sampled[3] * opacity;
        let premultiplied = [sampled[0] * alpha, sampled[1] * alpha, sampled[2] * alpha, alpha];

        assert!((premultiplied[0] - sampled[0] * 0.375).abs() < 0.0001);
        assert!((premultiplied[1] - sampled[1] * 0.375).abs() < 0.0001);
        assert!((premultiplied[2] - sampled[2] * 0.375).abs() < 0.0001);
        assert!((premultiplied[3] - 0.375).abs() < 0.0001);
    }

    #[test]
    fn text_coverage_policy_matches_egui_reference_formulas() {
        assert!((TextCoveragePolicy::Linear.apply(0.5) - 0.5).abs() < 0.0001);
        assert!((TextCoveragePolicy::Gamma(2.0).apply(0.5) - 0.25).abs() < 0.0001);
        assert!((TextCoveragePolicy::TwoCoverageMinusCoverageSq.apply(0.5) - 0.75).abs() < 0.0001);
    }

    #[test]
    fn automatic_text_coverage_policy_resolves_by_text_luminance() {
        assert_eq!(
            TextCoveragePolicy::AutomaticByTextLuminance.resolved_for_text_color(Color::BLACK),
            TextCoveragePolicy::Linear
        );
        assert_eq!(
            TextCoveragePolicy::AutomaticByTextLuminance.resolved_for_text_color(Color::WHITE),
            TextCoveragePolicy::TwoCoverageMinusCoverageSq
        );
    }

    #[test]
    fn text_render_mode_defaults_to_grayscale() {
        assert_eq!(TextRenderMode::default(), TextRenderMode::Grayscale);
    }

    #[test]
    fn slight_hinting_enables_below_threshold() {
        let config = TextHinting::Slight { max_ppem: 18.0 };
        assert!(config.should_hint(14.0));
        assert!(!config.should_hint(24.0));
    }

    #[test]
    fn stem_darkening_applies_only_below_threshold() {
        let config = StemDarkening::Enabled {
            max_ppem: 18.0,
            amount: 0.08,
        };
        assert!(config.effective_amount(14.0) > 0.0);
        assert_eq!(config.effective_amount(24.0), 0.0);
    }

    #[test]
    fn stem_darkening_boosts_partial_coverage() {
        let darkened = apply_stem_darkening_to_coverage(128, 0.1);
        assert!(darkened > 128);
    }

    #[test]
    fn lcd_text_render_mode_has_distinct_cache_identity() {
        assert_ne!(
            TextAtlasColorMode::from(TextRenderMode::Grayscale),
            TextAtlasColorMode::from(TextRenderMode::LcdSubpixel),
        );
    }

    #[test]
    fn subpixel_mask_preserves_distinct_rgb_channels_in_lcd_mode() {
        let converted = convert_subpixel_texel_for_mode(
            [255, 128, 32, 255],
            TextRenderMode::LcdSubpixel,
            0.0,
            TextCoveragePolicy::Linear,
        );
        assert_eq!(converted, [255, 128, 32, 255]);
    }

    #[test]
    fn lcd_text_requires_axis_aligned_pixel_snapped_path() {
        assert!(allows_lcd_text(Transform::IDENTITY, true));
        assert!(!allows_lcd_text(Transform::rotation(std::f32::consts::FRAC_PI_4), true));
        assert!(!allows_lcd_text(Transform::scale(-1.0, 1.0), true));
        assert!(!allows_lcd_text(Transform::rotation(std::f32::consts::PI), true));
        assert!(!allows_lcd_text(Transform::IDENTITY, false));
    }

    #[test]
    fn atlas_text_snaps_repeated_stems_to_physical_pixels() {
        let handle = FontHandle::new(31);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let viewport = Size::new(260.0, 52.0);
        let frame = SceneFrame {
            window_id: WindowId::new(98),
            viewport,
            surface_size: Size::new(390.0, 78.0),
            scale_factor: 1.5,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene: {
                let mut scene = Scene::new();
                scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(8.0, 10.0, 220.0, 24.0),
                    text: "scroll".to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 14.0,
                        line_height: 18.0,
                        color: Color::rgba(0.12, 0.16, 0.22, 1.0),
                        ..TextStyle::default()
                    },
                }));
                scene
            },
            font_registry: Arc::new(fonts),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(&frame, &mut text_engine).unwrap();

        assert!(
            vertices.len() >= 12,
            "expected atlas vertices for repeated l glyphs"
        );

        let first_l_left = logical_x_from_ndc(vertices[24].position[0], viewport);
        let second_l_left = logical_x_from_ndc(vertices[30].position[0], viewport);

        assert!(
            is_physically_pixel_aligned(first_l_left, frame.scale_factor),
            "first l did not snap to the physical pixel grid: x={first_l_left}"
        );
        assert!(
            is_physically_pixel_aligned(second_l_left, frame.scale_factor),
            "second l did not snap to the physical pixel grid: x={second_l_left}"
        );
    }

    #[test]
    fn renderer_text_coverage_policy_keeps_separate_cache_entries_for_explicit_policies() {
        let handle = FontHandle::new(33);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let frame = SceneFrame {
            window_id: WindowId::new(201),
            viewport: Size::new(320.0, 84.0),
            surface_size: Size::new(320.0, 84.0),
            scale_factor: 1.25,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene: {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 320.0, 84.0),
                    brush: Color::BLACK.into(),
                });
                scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(12.0, 8.0, 296.0, 64.0),
                    text: "Reusable".to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 55.5,
                        line_height: 59.5,
                        color: Color::WHITE,
                        ..TextStyle::default()
                    },
                }));
                scene
            },
            font_registry: Arc::new(fonts),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        text_engine.set_text_coverage_policy(TextCoveragePolicy::Linear);
        let _ = build_vertices(&frame, &mut text_engine).unwrap();
        let linear_stats = text_engine.glyph_cache_stats();

        text_engine.set_text_coverage_policy(TextCoveragePolicy::TwoCoverageMinusCoverageSq);
        let _ = build_vertices(&frame, &mut text_engine).unwrap();
        let dark_stats = text_engine.glyph_cache_stats();

        assert!(linear_stats.0 > 0, "linear policy should populate the glyph cache");
        assert!(linear_stats.2 > 0, "first pass should record glyph cache misses");
        assert!(dark_stats.0 > linear_stats.0);
        assert!(dark_stats.2 > linear_stats.2, "switching policy should add distinct cache entries");
    }

    #[test]
    fn automatic_text_coverage_policy_keeps_separate_glyph_cache_entries_for_light_and_dark_text() {
        let handle = FontHandle::new(34);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let frame = SceneFrame {
            window_id: WindowId::new(202),
            viewport: Size::new(240.0, 96.0),
            surface_size: Size::new(240.0, 96.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene: {
                let mut scene = Scene::new();
                scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(8.0, 8.0, 100.0, 32.0),
                    text: "I".to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 24.0,
                        line_height: 28.0,
                        color: Color::BLACK,
                        ..TextStyle::default()
                    },
                }));
                scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(8.0, 48.0, 100.0, 32.0),
                    text: "I".to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 24.0,
                        line_height: 28.0,
                        color: Color::WHITE,
                        ..TextStyle::default()
                    },
                }));
                scene
            },
            font_registry: Arc::new(fonts),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        text_engine.set_text_coverage_policy(TextCoveragePolicy::AutomaticByTextLuminance);
        let _ = build_vertices(&frame, &mut text_engine).unwrap();

        assert_eq!(text_engine.glyph_cache_stats(), (2, 0, 2));
    }

    #[test]
    fn retained_compositor_carries_active_path_clips() {
        let mut clip = Path::builder();
        clip.move_to(Point::new(8.0, 8.0))
            .line_to(Point::new(32.0, 8.0))
            .line_to(Point::new(20.0, 28.0))
            .close();

        let mut scene = Scene::new();
        scene.push(SceneCommand::PushClipPath { path: clip.build() });
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            brush: Color::WHITE.into(),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let ops = prepare_with_compositor(
            &SceneFrame {
                window_id: WindowId::new(6),
                viewport: Size::new(64.0, 64.0),
                surface_size: Size::new(64.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
            &mut compositor,
        )
        .unwrap();

        assert_eq!(ops.draw_ops.len(), 1);
        let op = &ops.draw_ops[0];
        let clip_state = &ops.clip_states[op.clip_state_index];
        assert!(op.clip_state_index > 0);
        assert_eq!(clip_state.clip_paths.len(), 1);
        assert!(clip_state.clip_paths[0].len > 0);
        assert_eq!(op.clip_rect, Some(Rect::new(8.0, 8.0, 24.0, 20.0)));
    }

    #[test]
    fn batch_draw_ops_merges_consecutive_matching_state() {
        let passes = batch_draw_ops(
            &DrawOpArena {
                scene_vertices: vec![
                    Vertex {
                        position: [0.0, 0.0],
                        color: [1.0, 1.0, 1.0, 1.0],
                        tex_coords: [0.0, 0.0],
                    };
                    6
                ],
                clip_vertices: Vec::new(),
                text_instances: Vec::new(),
                clip_states: vec![ClipState {
                    clip_paths: Vec::new(),
                }],
                draw_ops: vec![
                    DrawOp {
                        kind: DrawOpKind::Solid,
                        vertices: PreparedVertices { start: 0, len: 3 },
                        clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                        clip_state_index: 0,
                    },
                    DrawOp {
                        kind: DrawOpKind::Solid,
                        vertices: PreparedVertices { start: 3, len: 3 },
                        clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                        clip_state_index: 0,
                    },
                ],
                analytic_paths: std::collections::HashMap::new(),
                next_analytic_path_id: 0,
            },
            Size::new(50.0, 40.0),
            (100, 80),
        );

        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].draws.len(), 1);
        assert_eq!(passes[0].draws[0].vertices.len, 6);
    }

    #[test]
    fn prepare_frame_batches_converts_clip_rects_to_scissors() {
        let prepared = prepare_frame_batches(
            DrawOpArena {
                scene_vertices: vec![
                    Vertex {
                        position: [0.0, 0.0],
                        color: [1.0, 1.0, 1.0, 1.0],
                        tex_coords: [0.0, 0.0],
                    };
                    6
                ],
                clip_vertices: Vec::new(),
                text_instances: Vec::new(),
                clip_states: vec![ClipState {
                    clip_paths: Vec::new(),
                }],
                draw_ops: vec![DrawOp {
                    kind: DrawOpKind::Solid,
                    clip_rect: Some(Rect::new(5.0, 8.0, 20.0, 10.0)),
                    vertices: PreparedVertices { start: 0, len: 6 },
                    clip_state_index: 0,
                }],
                analytic_paths: std::collections::HashMap::new(),
                next_analytic_path_id: 0,
            },
            Size::new(50.0, 40.0),
            (100, 80),
        );

        assert_eq!(prepared.passes.len(), 1);
        assert_eq!(
            prepared.passes[0].draws[0].clip_rect,
            Some(ScissorRect {
                x: 10,
                y: 16,
                width: 40,
                height: 20,
            })
        );
    }

    #[test]
    fn prepare_cached_passes_snap_translated_adjacent_clip_edges_without_overlap() {
        let passes = prepare_cached_passes(
            &[
                CachedPassBatch {
                    clip_paths: Vec::new(),
                    draws: vec![CachedDrawBatch {
                        kind: PreparedDrawKind::Solid,
                        clip_rect: Some(Rect::new(0.0, 0.0, 384.0, 128.0)),
                        vertices: PreparedVertices { start: 0, len: 6 },
                    }],
                },
                CachedPassBatch {
                    clip_paths: Vec::new(),
                    draws: vec![CachedDrawBatch {
                        kind: PreparedDrawKind::Solid,
                        clip_rect: Some(Rect::new(384.0, 0.0, 128.0, 128.0)),
                        vertices: PreparedVertices { start: 6, len: 6 },
                    }],
                },
            ],
            Size::new(512.0, 128.0),
            (768, 192),
            Vector::new(0.25, 0.0),
            None,
            0,
            0,
            0,
        );

        let first = passes[0].draws[0].clip_rect.expect("first scissor");
        let second = passes[1].draws[0].clip_rect.expect("second scissor");

        assert_eq!(first.x + first.width, second.x);
        assert!(first.x + first.width <= second.x);
    }

    #[test]
    fn prepare_cached_passes_uses_external_clip_for_unclipped_draws() {
        let passes = prepare_cached_passes(
            &[CachedPassBatch {
                clip_paths: Vec::new(),
                draws: vec![CachedDrawBatch {
                    kind: PreparedDrawKind::Image {
                        handle: ImageHandle::new(99),
                    },
                    clip_rect: None,
                    vertices: PreparedVertices { start: 0, len: 6 },
                }],
            }],
            Size::new(100.0, 100.0),
            (100, 100),
            Vector::ZERO,
            Some(Rect::new(20.0, 30.0, 40.0, 50.0)),
            0,
            0,
            0,
        );

        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].draws.len(), 1);
        assert_eq!(
            passes[0].draws[0].clip_rect,
            Some(ScissorRect {
                x: 20,
                y: 30,
                width: 40,
                height: 50,
            })
        );
    }

    #[test]
    fn prepare_cached_passes_drops_draws_fully_outside_external_clip() {
        let passes = prepare_cached_passes(
            &[CachedPassBatch {
                clip_paths: Vec::new(),
                draws: vec![CachedDrawBatch {
                    kind: PreparedDrawKind::Image {
                        handle: ImageHandle::new(100),
                    },
                    clip_rect: Some(Rect::new(70.0, 70.0, 20.0, 20.0)),
                    vertices: PreparedVertices { start: 0, len: 6 },
                }],
            }],
            Size::new(100.0, 100.0),
            (100, 100),
            Vector::ZERO,
            Some(Rect::new(0.0, 0.0, 20.0, 20.0)),
            0,
            0,
            0,
        );

        assert_eq!(passes.len(), 1);
        assert!(passes[0].draws.is_empty());
    }

    #[test]
    fn renderer_frame_stats_count_passes_draws_and_uploaded_vertices() {
        let vertex = Vertex {
            position: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            tex_coords: [0.0, 0.0],
        };
        let prepared = PreparedFrameBatches {
            scene_vertices: vec![vertex; 9],
            clip_vertices: vec![vertex; 6],
            text_instances: Vec::new(),
            passes: vec![
                PreparedPassBatch {
                    clip_paths: vec![PreparedClipPath {
                        vertices: PreparedVertices { start: 0, len: 6 },
                    }],
                    draws: vec![
                        PreparedDrawBatch {
                            kind: PreparedDrawKind::Solid,
                            clip_rect: None,
                            vertices: PreparedVertices { start: 0, len: 3 },
                        },
                        PreparedDrawBatch {
                            kind: PreparedDrawKind::Solid,
                            clip_rect: Some(ScissorRect {
                                x: 0,
                                y: 0,
                                width: 10,
                                height: 10,
                            }),
                            vertices: PreparedVertices { start: 3, len: 6 },
                        },
                    ],
                },
                PreparedPassBatch {
                    clip_paths: Vec::new(),
                    draws: vec![PreparedDrawBatch {
                        kind: PreparedDrawKind::Image {
                            handle: ImageHandle::new(1),
                        },
                        clip_rect: None,
                        vertices: PreparedVertices { start: 0, len: 3 },
                    }],
                },
            ],
        };

        let stats = RendererFrameStats::from_prepared_frame(&prepared);

        assert_eq!(stats.pass_count, 2);
        assert_eq!(stats.draw_count, 4);
        assert_eq!(stats.uploaded_vertex_bytes, 15 * VERTEX_SIZE);
        assert_eq!(stats.visible_layer_count, 0);
        assert_eq!(stats.retained_state_update_time_us, 0);
    }

    #[test]
    fn text_engine_shapes_text_with_font_metrics() {
        let text = TextRun {
            rect: Rect::new(8.0, 10.0, 160.0, 32.0),
            text: "office".to_string(),
            style: TextStyle::new(Color::WHITE),
        };

        let text_engine = TextEngine::new().unwrap();
        let layout = text_engine
            .shape_text_run(&text, &FontRegistry::new())
            .unwrap();

        assert!(!layout.glyphs().is_empty());
        assert!(layout.measurement().width > 0.0);
        assert!(layout.measurement().height >= text.style.font_size);
    }

    #[test]
    fn build_vertices_supports_pre_shaped_text() {
        let text_system = TextSystem::new();
        let layout = text_system
            .shape_text_persistent(
                None,
                "scene",
                Size::new(80.0, 24.0),
                TextStyle::new(Color::WHITE),
                &FontRegistry::new(),
            )
            .unwrap();

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawShapedText(ShapedText::new(
            Point::new(4.0, 6.0),
            &layout,
        )));

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(11),
                viewport: Size::new(100.0, 80.0),
                surface_size: Size::new(100.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: text_system.text_layout_registry(),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
    }

    #[test]
    fn text_engine_reuses_cached_glyph_atlas_entries_across_repeated_builds() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "abc".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));

        let frame = SceneFrame {
            window_id: WindowId::new(12),
            viewport: Size::new(160.0, 60.0),
            surface_size: Size::new(160.0, 60.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let first = build_vertices(&frame, &mut text_engine).unwrap();
        assert!(!first.is_empty());
        assert_eq!(first.len(), 18);
        assert!(first.iter().any(|vertex| vertex.tex_coords != [0.0, 0.0]));
        assert_eq!(text_engine.glyph_cache_stats(), (3, 0, 3));

        let second = build_vertices(&frame, &mut text_engine).unwrap();
        assert_eq!(first.len(), second.len());
        assert!(first.iter().zip(&second).all(|(left, right)| {
            left.position == right.position
                && left.color == right.color
                && left.tex_coords == right.tex_coords
        }));
        assert_eq!(text_engine.glyph_cache_stats(), (3, 3, 3));
    }

    #[test]
    fn text_engine_parses_swash_face_once_per_text_run_when_glyphs_miss() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "abc".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));

        let frame = SceneFrame {
            window_id: WindowId::new(15),
            viewport: Size::new(160.0, 60.0),
            surface_size: Size::new(160.0, 60.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let first = build_vertices(&frame, &mut text_engine).unwrap();
        assert!(!first.is_empty());
        assert_eq!(text_engine.glyph_cache_stats(), (3, 0, 3));
        assert_eq!(text_engine.swash_face_parse_count(), 1);

        let second = build_vertices(&frame, &mut text_engine).unwrap();
        assert_eq!(first.len(), second.len());
        assert_eq!(text_engine.glyph_cache_stats(), (3, 3, 3));
        assert_eq!(text_engine.swash_face_parse_count(), 1);
    }

    #[test]
    fn text_engine_buckets_cached_glyph_atlas_entries_by_scale() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "abc".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));

        let base_frame = SceneFrame {
            window_id: WindowId::new(13),
            viewport: Size::new(160.0, 60.0),
            surface_size: Size::new(160.0, 60.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut scaled_scene = Scene::new();
        scaled_scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 56.0),
            text: "abc".to_string(),
            style: TextStyle {
                font_size: 28.0,
                line_height: 32.0,
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let scaled_frame = SceneFrame {
            window_id: WindowId::new(14),
            viewport: Size::new(160.0, 80.0),
            surface_size: Size::new(160.0, 80.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene: scaled_scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let first = build_vertices(&base_frame, &mut text_engine).unwrap();
        assert!(!first.is_empty());
        assert_eq!(text_engine.glyph_cache_stats(), (3, 0, 3));

        let second = build_vertices(&scaled_frame, &mut text_engine).unwrap();
        assert!(!second.is_empty());
        assert_eq!(text_engine.glyph_cache_stats(), (6, 0, 6));
    }

    #[test]
    fn retained_compositor_reuses_cached_path_meshes_across_cached_tiles() {
        let layer_id = WidgetId::new(70);
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillPath {
            path: Path::rect(Rect::new(0.0, 0.0, 512.0, 128.0)),
            brush: Color::rgba(0.24, 0.48, 0.72, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        let frame = SceneFrame {
            window_id: WindowId::new(30),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let draw_ops = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(!draw_ops.draw_ops.is_empty());
        assert_eq!(compositor.path_cache.stats(), (0, 0, 0));
        assert_eq!(draw_ops.analytic_paths.len(), 2);
        assert!(
            draw_ops
                .draw_ops
                .iter()
                .any(|draw| matches!(draw.kind, DrawOpKind::AnalyticPath { .. }))
        );
    }

    #[test]
    fn retained_compositor_uses_analytic_stroke_paths_across_cached_tiles() {
        let layer_id = WidgetId::new(71);
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);

        let mut stroke_path = Path::builder();
        stroke_path
            .move_to(Point::new(8.0, 24.0))
            .line_to(Point::new(180.0, 92.0))
            .line_to(Point::new(340.0, 20.0))
            .line_to(Point::new(500.0, 92.0));

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::StrokePath {
            path: stroke_path.build(),
            brush: Color::rgba(0.92, 0.46, 0.18, 1.0).into(),
            stroke: StrokeStyle::new(12.0),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        let frame = SceneFrame {
            window_id: WindowId::new(31),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let draw_ops = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(!draw_ops.draw_ops.is_empty());
        assert_eq!(compositor.path_cache.stats(), (0, 0, 0));
        assert_eq!(draw_ops.analytic_paths.len(), 2);
        assert!(
            draw_ops
                .draw_ops
                .iter()
                .any(|draw| matches!(draw.kind, DrawOpKind::AnalyticPath { .. }))
        );
    }

    #[test]
    fn retained_compositor_reuses_layer_packets_until_content_changes() {
        let layer_id = WidgetId::new(41);
        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Clear(Color::BLACK));
        scene.push(SceneCommand::Layer(SceneLayer::new(
            layer_id,
            Rect::new(4.0, 6.0, 32.0, 24.0),
            child_scene,
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(21),
            viewport: Size::new(96.0, 64.0),
            surface_size: Size::new(96.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([layer_id]),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert!(compositor.last_frame_stats.packet_build_count > 0);
        let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
        let first_signature = packet_signature(&compositor, layer_container);
        let first_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        frame.layer_updates.clear();
        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
        assert_eq!(first.scene_vertices, second.scene_vertices);
        assert_eq!(
            first_signature,
            packet_signature(&compositor, layer_container)
        );

        frame.layer_updates = content_updates([layer_id]);
        let mut updated_child_scene = Scene::new();
        updated_child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
        });
        updated_child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(12.0, 10.0, 8.0, 8.0),
            brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
        });
        assert!(frame.scene.replace_layer(
            layer_id,
            SceneLayer::new(
                layer_id,
                Rect::new(4.0, 6.0, 32.0, 24.0),
                updated_child_scene
            ),
        ));
        let third = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert!(compositor.last_frame_stats.packet_build_count > 0);
        let third_signature = packet_signature(&compositor, layer_container);
        let third_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        assert!(third_content_version > first_content_version);
        assert_ne!(first_signature, third_signature);
        assert_ne!(first.scene_vertices, third.scene_vertices);
    }

    #[test]
    fn retained_compositor_skips_packet_rebuild_for_unchanged_content_updates() {
        let layer_id = WidgetId::new(411);
        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::new(
            layer_id,
            Rect::new(4.0, 6.0, 32.0, 24.0),
            child_scene,
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(211),
            viewport: Size::new(96.0, 64.0),
            surface_size: Size::new(96.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([layer_id]),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert!(compositor.last_frame_stats.packet_build_count > 0);
        let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
        let first_signature = packet_signature(&compositor, layer_container);

        frame.layer_updates = content_updates([layer_id]);
        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
        assert_eq!(first_signature, packet_signature(&compositor, layer_container));
        assert_eq!(first.scene_vertices, second.scene_vertices);
    }

    #[test]
    fn retained_compositor_reuses_parent_packets_when_only_child_content_changes() {
        let parent_id = WidgetId::new(51);
        let child_id = WidgetId::new(52);

        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(8.0, 8.0, 10.0, 10.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
        });

        let mut parent_scene = Scene::new();
        parent_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 4.0, 24.0, 24.0),
            brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
        });
        parent_scene.push(SceneCommand::Layer(SceneLayer::new(
            child_id,
            Rect::new(8.0, 8.0, 10.0, 10.0),
            child_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::new(
            parent_id,
            Rect::new(4.0, 4.0, 24.0, 24.0),
            parent_scene,
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(22),
            viewport: Size::new(64.0, 64.0),
            surface_size: Size::new(64.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([parent_id, child_id]),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let parent_container = CompositionContainerId::Layer(SceneLayerId::from_widget(parent_id));
        let child_container = CompositionContainerId::Layer(SceneLayerId::from_widget(child_id));
        let parent_signature = packet_signature(&compositor, parent_container);
        let child_signature = packet_signature(&compositor, child_container);

        frame.layer_updates.clear();
        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert_eq!(first.scene_vertices, second.scene_vertices);
        assert_eq!(
            parent_signature,
            packet_signature(&compositor, parent_container)
        );
        assert_eq!(
            child_signature,
            packet_signature(&compositor, child_container)
        );

        let mut updated_child_scene = Scene::new();
        updated_child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(8.0, 8.0, 10.0, 10.0),
            brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
        });

        let mut updated_parent_scene = Scene::new();
        updated_parent_scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 4.0, 24.0, 24.0),
            brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
        });
        updated_parent_scene.push(SceneCommand::Layer(SceneLayer::new(
            child_id,
            Rect::new(8.0, 8.0, 10.0, 10.0),
            updated_child_scene,
        )));
        assert!(frame.scene.replace_layer(
            parent_id,
            SceneLayer::new(
                parent_id,
                Rect::new(4.0, 4.0, 24.0, 24.0),
                updated_parent_scene
            ),
        ));

        frame.layer_updates = content_updates([child_id]);
        let third = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_eq!(
            parent_signature,
            packet_signature(&compositor, parent_container)
        );
        assert_ne!(
            child_signature,
            packet_signature(&compositor, child_container)
        );
        assert_ne!(first.scene_vertices, third.scene_vertices);
    }

    #[test]
    fn retained_compositor_reuses_direct_packets_across_layer_translation() {
        let layer_id = WidgetId::new(53);

        let build_layer = |x: f32| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(layer_id),
                layer_id,
                Rect::new(x, 10.0, 80.0, 36.0),
            )
            .with_content_bounds(Rect::new(x, 10.0, 80.0, 36.0))
            .with_paint_bounds(Rect::new(x, 10.0, 80.0, 36.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(x, 10.0, 80.0, 36.0),
                brush: Color::rgba(0.82, 0.36, 0.18, 1.0).into(),
            });

            SceneLayer::from_descriptor(descriptor, layer_scene)
        };

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(build_layer(8.0)));

        let mut frame = SceneFrame {
            window_id: WindowId::new(24),
            viewport: Size::new(160.0, 80.0),
            surface_size: Size::new(160.0, 80.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([layer_id]),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        assert!(compositor.last_frame_stats.packet_build_count > 0);
        let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
        let first_signature = packet_signature(&compositor, layer_container);
        let first_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        frame.scene = {
            let mut next = Scene::new();
            next.push(SceneCommand::Layer(build_layer(44.0)));
            next
        };
        frame.layer_updates = vec![SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Transform,
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(layer_id),
                layer_id,
                Rect::new(44.0, 10.0, 80.0, 36.0),
            )
            .with_content_bounds(Rect::new(44.0, 10.0, 80.0, 36.0))
            .with_paint_bounds(Rect::new(44.0, 10.0, 80.0, 36.0))
            .with_cache_policy(LayerCachePolicy::Direct),
        )];

        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let second_signature = packet_signature(&compositor, layer_container);
        let second_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        assert_eq!(first_signature, second_signature);
        assert_eq!(first_content_version, second_content_version);
        assert_eq!(compositor.last_frame_stats.direct_packets, 1);
        assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
        assert_ne!(first.scene_vertices, second.scene_vertices);
    }

    #[test]
    fn retained_compositor_reuses_direct_packets_across_clip_only_updates() {
        let layer_id = WidgetId::new(54);
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(8.0, 8.0, 96.0, 48.0),
        )
        .with_content_bounds(Rect::new(8.0, 8.0, 96.0, 48.0))
        .with_paint_bounds(Rect::new(8.0, 8.0, 96.0, 48.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let build_layer = || {
            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(8.0, 8.0, 96.0, 48.0),
                brush: Color::rgba(0.16, 0.52, 0.84, 1.0).into(),
            });
            SceneLayer::from_descriptor(descriptor.clone(), layer_scene)
        };

        let build_scene = |clip: Rect| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::PushClip { rect: clip });
            scene.push(SceneCommand::Layer(build_layer()));
            scene.push(SceneCommand::PopClip);
            scene
        };

        let mut frame = SceneFrame {
            window_id: WindowId::new(25),
            viewport: Size::new(160.0, 96.0),
            surface_size: Size::new(160.0, 96.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([layer_id]),
            scene: build_scene(Rect::new(0.0, 0.0, 160.0, 96.0)),
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
        let first_signature = packet_signature(&compositor, layer_container);
        let first_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;
        let first_clip_rects = first
            .draw_ops
            .iter()
            .map(|draw_op| draw_op.clip_rect)
            .collect::<Vec<_>>();

        frame.scene = build_scene(Rect::new(24.0, 8.0, 64.0, 48.0));
        frame.layer_updates = vec![SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Clip,
            descriptor.clone(),
        )];

        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let second_signature = packet_signature(&compositor, layer_container);
        let second_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;
        let second_clip_rects = second
            .draw_ops
            .iter()
            .map(|draw_op| draw_op.clip_rect)
            .collect::<Vec<_>>();

        assert_eq!(first_signature, second_signature);
        assert_eq!(first_content_version, second_content_version);
        assert_eq!(compositor.last_frame_stats.direct_packets, 1);
        assert_eq!(first.scene_vertices, second.scene_vertices);
        assert_ne!(first_clip_rects, second_clip_rects);
    }

    #[test]
    fn retained_compositor_regenerates_cached_tiles_across_ancestor_clip_updates() {
        let shell_id = WidgetId::new(55);
        let scroll_id = WidgetId::new(56);
        let content_id = WidgetId::new(57);

        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 220.0, 180.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 180.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 180.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(scroll_id),
            scroll_id,
            Rect::new(0.0, 0.0, 220.0, 360.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Cached)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let content_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(content_id),
            content_id,
            Rect::new(0.0, 0.0, 220.0, 360.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let build_scene = |clip: Rect| {
            let mut content_scene = Scene::new();
            content_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 220.0, 360.0),
                brush: Color::WHITE.into(),
            });
            content_scene.push(SceneCommand::FillRect {
                rect: Rect::new(16.0, 16.0, 188.0, 120.0),
                brush: Color::rgba(0.28, 0.20, 0.86, 1.0).into(),
            });
            content_scene.push(SceneCommand::FillRect {
                rect: Rect::new(16.0, 220.0, 188.0, 120.0),
                brush: Color::rgba(0.14, 0.55, 0.82, 1.0).into(),
            });

            let mut scroll_scene = Scene::new();
            scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                content_descriptor.clone(),
                content_scene,
            )));

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::PushClip { rect: clip });
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                scroll_descriptor.clone(),
                scroll_scene,
            )));
            shell_scene.push(SceneCommand::PopClip);

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));
            scene
        };

        let mut frame = SceneFrame {
            window_id: WindowId::new(26),
            viewport: Size::new(220.0, 180.0),
            surface_size: Size::new(220.0, 180.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                scroll_descriptor.clone(),
            )
            .with_damage(scroll_descriptor.paint_bounds)],
            scene: build_scene(Rect::new(0.0, 0.0, 220.0, 180.0)),
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let first_clip_rects = first
            .draw_ops
            .iter()
            .map(|draw_op| draw_op.clip_rect)
            .collect::<Vec<_>>();

        frame.scene = build_scene(Rect::new(0.0, 0.0, 220.0, 96.0));
        frame.layer_updates = vec![SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Clip,
            shell_descriptor.clone(),
        )
        .with_damage(shell_descriptor.paint_bounds)];

        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let second_clip_rects = second
            .draw_ops
            .iter()
            .map(|draw_op| draw_op.clip_rect)
            .collect::<Vec<_>>();

        assert_ne!(first_clip_rects, second_clip_rects);
    }

    #[test]
    fn retained_compositor_prunes_removed_layers_and_packets() {
        let removed_id = WidgetId::new(61);
        let replacement_id = WidgetId::new(62);

        let mut first_scene = Scene::new();
        first_scene.push(SceneCommand::Layer(SceneLayer::new(
            removed_id,
            Rect::new(0.0, 0.0, 24.0, 24.0),
            {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 24.0, 24.0),
                    brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
                });
                scene
            },
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(23),
            viewport: Size::new(64.0, 64.0),
            surface_size: Size::new(64.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: content_updates([removed_id]),
            scene: first_scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
        let removed_layer_id = SceneLayerId::from_widget(removed_id);
        let replacement_layer_id = SceneLayerId::from_widget(replacement_id);
        let removed_packet_id = RetainedPacketId {
            container: CompositionContainerId::Layer(removed_layer_id),
            segment_index: 0,
        };
        assert!(compositor.layers.contains_key(&removed_layer_id));
        assert!(compositor.packets.contains_key(&removed_packet_id));

        let mut second_scene = Scene::new();
        second_scene.push(SceneCommand::Layer(SceneLayer::new(
            replacement_id,
            Rect::new(8.0, 8.0, 24.0, 24.0),
            {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(8.0, 8.0, 24.0, 24.0),
                    brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
                });
                scene
            },
        )));
        frame.scene = second_scene;
        frame.layer_updates = content_updates([replacement_id]);

        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(!compositor.layers.contains_key(&removed_layer_id));
        assert!(!compositor.packets.contains_key(&removed_packet_id));
        assert!(compositor.layers.contains_key(&replacement_layer_id));
        assert!(compositor.packets.contains_key(&RetainedPacketId {
            container: CompositionContainerId::Layer(replacement_layer_id),
            segment_index: 0,
        }));
    }

    #[test]
    fn overlay_layers_paint_above_later_normal_siblings() {
        let normal_id = WidgetId::new(205);
        let overlay_id = WidgetId::new(206);

        let build_frame = || {
            let normal_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(normal_id),
                normal_id,
                Rect::new(32.0, 12.0, 72.0, 48.0),
            )
            .with_content_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
            .with_paint_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
            .with_cache_policy(LayerCachePolicy::Direct);
            let overlay_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(overlay_id),
                overlay_id,
                Rect::new(12.0, 12.0, 72.0, 48.0),
            )
            .with_content_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
            .with_paint_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Overlay);

            let mut overlay_scene = Scene::new();
            overlay_scene.push(SceneCommand::FillRect {
                rect: overlay_descriptor.bounds,
                brush: Color::rgba(0.90, 0.16, 0.16, 1.0).into(),
            });

            let mut normal_scene = Scene::new();
            normal_scene.push(SceneCommand::FillRect {
                rect: normal_descriptor.bounds,
                brush: Color::rgba(0.16, 0.72, 0.24, 1.0).into(),
            });

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                overlay_descriptor.clone(),
                overlay_scene,
            )));
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                normal_descriptor.clone(),
                normal_scene,
            )));

            SceneFrame {
                window_id: WindowId::new(143),
                viewport: Size::new(128.0, 80.0),
                surface_size: Size::new(128.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        overlay_descriptor,
                    )
                    .with_damage(Rect::new(12.0, 12.0, 72.0, 48.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        normal_descriptor,
                    )
                    .with_damage(Rect::new(32.0, 12.0, 72.0, 48.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let frame = build_frame();
        renderer.render(&frame).unwrap();
        let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
        let overlap = rgba_pixel(&pixels, 48, 24);

        assert!(
            overlap[0] > overlap[1],
            "expected overlay pixel to dominate overlap, got rgba={overlap:?}"
        );
    }

    #[test]
    fn nested_overlay_layers_paint_above_later_root_normal_siblings() {
        let shell_id = WidgetId::new(207);
        let nested_overlay_id = WidgetId::new(208);
        let blocker_id = WidgetId::new(209);

        let build_frame = || {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 128.0, 80.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 128.0, 80.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 128.0, 80.0))
            .with_cache_policy(LayerCachePolicy::Direct);
            let nested_overlay_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(nested_overlay_id),
                nested_overlay_id,
                Rect::new(12.0, 12.0, 72.0, 48.0),
            )
            .with_content_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
            .with_paint_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Overlay);
            let blocker_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(blocker_id),
                blocker_id,
                Rect::new(32.0, 12.0, 72.0, 48.0),
            )
            .with_content_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
            .with_paint_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut nested_overlay_scene = Scene::new();
            nested_overlay_scene.push(SceneCommand::FillRect {
                rect: nested_overlay_descriptor.bounds,
                brush: Color::rgba(0.90, 0.16, 0.16, 1.0).into(),
            });

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::FillRect {
                rect: shell_descriptor.bounds,
                brush: Color::rgba(0.97, 0.98, 1.0, 1.0).into(),
            });
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                nested_overlay_descriptor.clone(),
                nested_overlay_scene,
            )));

            let mut blocker_scene = Scene::new();
            blocker_scene.push(SceneCommand::FillRect {
                rect: blocker_descriptor.bounds,
                brush: Color::rgba(0.16, 0.72, 0.24, 1.0).into(),
            });

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                blocker_descriptor.clone(),
                blocker_scene,
            )));

            SceneFrame {
                window_id: WindowId::new(144),
                viewport: Size::new(128.0, 80.0),
                surface_size: Size::new(128.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        shell_descriptor,
                    )
                    .with_damage(Rect::new(0.0, 0.0, 128.0, 80.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        nested_overlay_descriptor,
                    )
                    .with_damage(Rect::new(12.0, 12.0, 72.0, 48.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        blocker_descriptor,
                    )
                    .with_damage(Rect::new(32.0, 12.0, 72.0, 48.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let frame = build_frame();
        renderer.render(&frame).unwrap();
        let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
        let overlap = rgba_pixel(&pixels, 48, 24);

        assert!(
            overlap[0] > overlap[1],
            "expected nested overlay pixel to dominate overlap, got rgba={overlap:?}"
        );
    }

    #[test]
    fn cached_ancestor_overlay_layers_paint_above_later_root_normal_siblings() {
        let shell_id = WidgetId::new(210);
        let nested_overlay_id = WidgetId::new(211);
        let blocker_id = WidgetId::new(212);

        let build_frame = || {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 512.0, 120.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 120.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 120.0))
            .with_cache_policy(LayerCachePolicy::Cached);
            let nested_overlay_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(nested_overlay_id),
                nested_overlay_id,
                Rect::new(12.0, 12.0, 180.0, 80.0),
            )
            .with_content_bounds(Rect::new(12.0, 12.0, 180.0, 80.0))
            .with_paint_bounds(Rect::new(12.0, 12.0, 180.0, 80.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Overlay);
            let blocker_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(blocker_id),
                blocker_id,
                Rect::new(96.0, 40.0, 220.0, 44.0),
            )
            .with_content_bounds(Rect::new(96.0, 40.0, 220.0, 44.0))
            .with_paint_bounds(Rect::new(96.0, 40.0, 220.0, 44.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut nested_overlay_scene = Scene::new();
            nested_overlay_scene.push(SceneCommand::FillRect {
                rect: nested_overlay_descriptor.bounds,
                brush: Color::rgba(0.90, 0.16, 0.16, 1.0).into(),
            });

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::FillRect {
                rect: shell_descriptor.bounds,
                brush: Color::rgba(0.97, 0.98, 1.0, 1.0).into(),
            });
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                nested_overlay_descriptor.clone(),
                nested_overlay_scene,
            )));

            let mut blocker_scene = Scene::new();
            blocker_scene.push(SceneCommand::FillRect {
                rect: blocker_descriptor.bounds,
                brush: Color::rgba(0.16, 0.72, 0.24, 1.0).into(),
            });

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                blocker_descriptor.clone(),
                blocker_scene,
            )));

            SceneFrame {
                window_id: WindowId::new(145),
                viewport: Size::new(512.0, 120.0),
                surface_size: Size::new(512.0, 120.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        shell_descriptor,
                    )
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 120.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        nested_overlay_descriptor,
                    )
                    .with_damage(Rect::new(12.0, 12.0, 180.0, 80.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        blocker_descriptor,
                    )
                    .with_damage(Rect::new(96.0, 40.0, 220.0, 44.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let frame = build_frame();
        renderer.render(&frame).unwrap();
        let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
        let overlap = rgba_pixel(&pixels, 120, 52);

        assert!(
            overlap[0] > overlap[1],
            "expected cached-ancestor overlay pixel to dominate overlap, got rgba={overlap:?}"
        );
    }

    #[test]
    fn retained_compositor_routes_descendant_damage_into_cached_parent_tiles() {
        let parent_id = WidgetId::new(81);
        let child_id = WidgetId::new(82);

        let parent_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(parent_id),
            parent_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);
        let child_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(child_id),
            child_id,
            Rect::new(300.0, 24.0, 48.0, 48.0),
        );

        let build_parent_scene = |child_brush: Color| {
            let mut child_scene = Scene::new();
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(300.0, 24.0, 48.0, 48.0),
                brush: child_brush.into(),
            });

            let mut parent_scene = Scene::new();
            parent_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 512.0, 128.0),
                brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
            });
            parent_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                child_descriptor.clone(),
                child_scene,
            )));
            parent_scene
        };

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            parent_descriptor.clone(),
            build_parent_scene(Color::rgba(1.0, 0.0, 0.0, 1.0)),
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(32),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    parent_descriptor.clone(),
                )
                .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    child_descriptor.clone(),
                )
                .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(frame.scene.replace_layer(
            parent_id,
            SceneLayer::from_descriptor(
                parent_descriptor.clone(),
                build_parent_scene(Color::rgba(0.0, 1.0, 0.0, 1.0)),
            ),
        ));
        frame.layer_updates = vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                child_descriptor.clone(),
            )
            .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
        ];

        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    }

    #[test]
    fn retained_compositor_routes_descendant_transform_into_cached_parent_tiles() {
        let parent_id = WidgetId::new(181);
        let child_id = WidgetId::new(182);

        let parent_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(parent_id),
            parent_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);

        let child_descriptor = |x: f32| {
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(child_id),
                child_id,
                Rect::new(x, 24.0, 48.0, 48.0),
            )
            .with_content_bounds(Rect::new(x, 24.0, 48.0, 48.0))
            .with_paint_bounds(Rect::new(x, 24.0, 48.0, 48.0))
            .with_cache_policy(LayerCachePolicy::Direct)
        };

        let build_parent_scene = |child_x: f32| {
            let mut child_scene = Scene::new();
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(child_x, 24.0, 48.0, 48.0),
                brush: Color::rgba(0.84, 0.32, 0.18, 1.0).into(),
            });

            let mut parent_scene = Scene::new();
            parent_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 512.0, 128.0),
                brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
            });
            parent_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                child_descriptor(child_x),
                child_scene,
            )));
            parent_scene
        };

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            parent_descriptor.clone(),
            build_parent_scene(300.0),
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(132),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    parent_descriptor.clone(),
                )
                .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    child_descriptor(300.0),
                )
                .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(frame.scene.replace_layer(
            parent_id,
            SceneLayer::from_descriptor(parent_descriptor.clone(), build_parent_scene(340.0)),
        ));
        frame.layer_updates = vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Transform,
                child_descriptor(340.0),
            )
            .with_damage(Rect::new(300.0, 24.0, 88.0, 48.0)),
        ];

        let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    }

    #[test]
    fn retained_compositor_updates_nested_cached_scroll_layer_after_child_transform() {
        let shell_id = WidgetId::new(191);
        let scroll_id = WidgetId::new(192);
        let content_id = WidgetId::new(193);
        let first_id = WidgetId::new(194);
        let second_id = WidgetId::new(195);
        let third_id = WidgetId::new(196);

        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 360.0, 220.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 360.0, 220.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 360.0, 220.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(scroll_id),
            scroll_id,
            Rect::new(0.0, 0.0, 240.0, 220.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 240.0, 220.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 240.0, 220.0))
        .with_cache_policy(LayerCachePolicy::Cached)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let content_descriptor = |y: f32| {
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(content_id),
                content_id,
                Rect::new(0.0, y, 360.0, 360.0),
            )
            .with_content_bounds(Rect::new(0.0, y, 220.0, 360.0))
            .with_paint_bounds(Rect::new(0.0, y, 220.0, 360.0))
            .with_cache_policy(LayerCachePolicy::Direct)
        };

        let child_layer = |id: WidgetId, y: f32, brush: Color| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, y, 220.0, 120.0),
                brush: brush.into(),
            });
            SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(id),
                    id,
                    Rect::new(0.0, y, 220.0, 120.0),
                )
                .with_content_bounds(Rect::new(0.0, y, 220.0, 120.0))
                .with_paint_bounds(Rect::new(0.0, y, 220.0, 120.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                scene,
            )
        };

        let build_content_scene = |y: f32| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(child_layer(
                first_id,
                y,
                Color::rgba(0.82, 0.36, 0.18, 1.0),
            )));
            scene.push(SceneCommand::Layer(child_layer(
                second_id,
                y + 120.0,
                Color::rgba(0.18, 0.54, 0.82, 1.0),
            )));
            scene.push(SceneCommand::Layer(child_layer(
                third_id,
                y + 240.0,
                Color::rgba(0.24, 0.72, 0.36, 1.0),
            )));
            scene
        };

        let build_shell_scene = |content_y: f32| {
            let mut scroll_scene = Scene::new();
            scroll_scene.push(SceneCommand::PushClip {
                rect: Rect::new(0.0, 0.0, 230.0, 220.0),
            });
            scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                content_descriptor(content_y),
                build_content_scene(content_y),
            )));
            scroll_scene.push(SceneCommand::PopClip);

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                scroll_descriptor.clone(),
                scroll_scene,
            )));
            shell_scene.push(SceneCommand::FillRect {
                rect: Rect::new(240.0, 0.0, 120.0, 220.0),
                brush: Color::rgba(0.94, 0.95, 0.97, 1.0).into(),
            });
            shell_scene
        };

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            build_shell_scene(0.0),
        )));

        let mut frame = SceneFrame {
            window_id: WindowId::new(142),
            viewport: Size::new(360.0, 220.0),
            surface_size: Size::new(360.0, 220.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    scroll_descriptor.clone(),
                )
                .with_damage(scroll_descriptor.paint_bounds),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    content_descriptor(0.0),
                )
                .with_damage(Rect::new(0.0, 0.0, 220.0, 360.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first_frame = frame.clone();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert!(
            frame
                .scene
                .translate_layer(content_id, Vector::new(0.0, -72.0))
        );
        frame.layer_updates = vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Transform,
                content_descriptor(-72.0),
            )
            .with_damage(Rect::new(0.0, -72.0, 220.0, 432.0)),
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Transform,
                scroll_descriptor.clone(),
            )
            .with_damage(Rect::new(0.0, 0.0, 220.0, 360.0)),
        ];

        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_ne!(first.scene_vertices, second.scene_vertices);
        let mut renderer = WgpuRenderer::default();
        renderer.render(&first_frame).unwrap();
        let before = renderer
            .capture_last_frame_rgba(first_frame.window_id)
            .unwrap();
        renderer.render(&frame).unwrap();
        let after = renderer.capture_last_frame_rgba(frame.window_id).unwrap();

        assert!(
            before
                .pixels()
                .iter()
                .zip(after.pixels().iter())
                .any(|(left, right)| left != right)
        );
    }

    #[test]
    fn retained_compositor_routes_nested_cached_descendant_damage_into_tile_owner() {
        let outer_id = WidgetId::new(83);
        let inner_id = WidgetId::new(84);
        let leaf_id = WidgetId::new(85);

        let outer_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(outer_id),
            outer_id,
            Rect::new(0.0, 0.0, 512.0, 128.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);
        let inner_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(inner_id),
            inner_id,
            Rect::new(256.0, 0.0, 256.0, 128.0),
        )
        .with_content_bounds(Rect::new(256.0, 0.0, 256.0, 128.0))
        .with_paint_bounds(Rect::new(256.0, 0.0, 256.0, 128.0))
        .with_cache_policy(LayerCachePolicy::Cached);
        let leaf_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(leaf_id),
            leaf_id,
            Rect::new(300.0, 24.0, 48.0, 48.0),
        )
        .with_content_bounds(Rect::new(300.0, 24.0, 48.0, 48.0))
        .with_paint_bounds(Rect::new(300.0, 24.0, 48.0, 48.0));

        let build_scene = |leaf_brush: Color| {
            let mut leaf_scene = Scene::new();
            leaf_scene.push(SceneCommand::FillRect {
                rect: leaf_descriptor.bounds,
                brush: leaf_brush.into(),
            });

            let mut inner_scene = Scene::new();
            inner_scene.push(SceneCommand::FillRect {
                rect: inner_descriptor.bounds,
                brush: Color::rgba(0.12, 0.12, 0.16, 1.0).into(),
            });
            inner_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                leaf_descriptor.clone(),
                leaf_scene,
            )));

            let mut outer_scene = Scene::new();
            outer_scene.push(SceneCommand::FillRect {
                rect: outer_descriptor.bounds,
                brush: Color::rgba(0.08, 0.08, 0.10, 1.0).into(),
            });
            outer_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                inner_descriptor.clone(),
                inner_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                outer_descriptor.clone(),
                outer_scene,
            )));
            scene
        };

        let mut frame = SceneFrame {
            window_id: WindowId::new(36),
            viewport: Size::new(512.0, 128.0),
            surface_size: Size::new(512.0, 128.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    outer_descriptor.clone(),
                )
                .with_damage(outer_descriptor.paint_bounds),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    inner_descriptor.clone(),
                )
                .with_damage(inner_descriptor.paint_bounds),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    leaf_descriptor.clone(),
                )
                .with_damage(leaf_descriptor.paint_bounds),
            ],
            scene: build_scene(Color::rgba(1.0, 0.0, 0.0, 1.0)),
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        frame.scene = build_scene(Color::rgba(0.0, 1.0, 0.0, 1.0));
        frame.layer_updates = vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                leaf_descriptor.clone(),
            )
            .with_damage(leaf_descriptor.paint_bounds),
        ];

        let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

        assert_ne!(first.scene_vertices, second.scene_vertices);
    }

    #[test]
    fn cached_tiles_match_direct_text_across_tile_boundaries() {
        let handle = FontHandle::new(27);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let layer_id = WidgetId::new(92);
        let build_frame = |cache_policy| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(layer_id),
                layer_id,
                Rect::new(0.0, 0.0, 512.0, 72.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 72.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 72.0))
            .with_cache_policy(cache_policy);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 512.0, 72.0),
                brush: Color::rgba(0.08, 0.09, 0.11, 1.0).into(),
            });
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(332.0, 18.0, 156.0, 28.0),
                text: "boundary glyph sample".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    ..TextStyle::new(Color::WHITE)
                },
            }));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor.clone(),
                layer_scene,
            )));

            SceneFrame {
                window_id: WindowId::new(92),
                viewport: Size::new(512.0, 72.0),
                surface_size: Size::new(512.0, 72.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                        .with_damage(Rect::new(0.0, 0.0, 512.0, 72.0)),
                ],
                scene,
                font_registry: Arc::new(fonts.clone()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_ancestors_match_direct_for_child_layer_text_across_tile_boundaries() {
        let handle = FontHandle::new(28);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let shell_id = WidgetId::new(93);
        let child_id = WidgetId::new(94);
        let build_frame = |shell_cache_policy| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 512.0, 84.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
            .with_cache_policy(shell_cache_policy);

            let child_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(child_id),
                child_id,
                Rect::new(0.0, 0.0, 512.0, 84.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut child_scene = Scene::new();
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 512.0, 84.0),
                brush: Color::rgba(0.08, 0.09, 0.11, 1.0).into(),
            });
            child_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(326.0, 22.0, 164.0, 28.0),
                text: "tab boundary sample".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    ..TextStyle::new(Color::WHITE)
                },
            }));

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                child_descriptor.clone(),
                child_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));

            SceneFrame {
                window_id: WindowId::new(93),
                viewport: Size::new(512.0, 84.0),
                surface_size: Size::new(512.0, 84.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        shell_descriptor,
                    )
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 84.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        child_descriptor,
                    )
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 84.0)),
                ],
                scene,
                font_registry: Arc::new(fonts.clone()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_tiles_match_direct_for_theme_preview_style_cards() {
        let handle = FontHandle::new(151);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let widget_id = WidgetId::new(152);
        let build_frame = |window_id, cache_policy| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(widget_id),
                widget_id,
                Rect::new(0.0, 0.0, 640.0, 220.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 640.0, 220.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 640.0, 220.0))
            .with_cache_policy(cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 640.0, 220.0),
                brush: Color::rgba(0.94, 0.95, 0.98, 1.0).into(),
            });

            let card_specs = [
                (
                    56.0,
                    Color::rgba(0.99, 0.99, 1.0, 1.0),
                    Color::rgba(0.19, 0.46, 0.91, 1.0),
                    Color::rgba(0.15, 0.73, 0.70, 1.0),
                    Color::rgba(0.10, 0.13, 0.19, 1.0),
                    Color::rgba(0.39, 0.45, 0.54, 1.0),
                    Color::rgba(0.82, 0.85, 0.91, 1.0),
                    "Light theme",
                ),
                (
                    344.0,
                    Color::rgba(0.14, 0.16, 0.21, 1.0),
                    Color::rgba(0.45, 0.60, 0.98, 1.0),
                    Color::rgba(0.96, 0.54, 0.31, 1.0),
                    Color::rgba(0.94, 0.95, 0.98, 1.0),
                    Color::rgba(0.68, 0.72, 0.80, 1.0),
                    Color::rgba(0.28, 0.31, 0.38, 1.0),
                    "Dark theme",
                ),
            ];

            for (
                card_x,
                surface,
                accent,
                secondary,
                text_color,
                subtle_text,
                border,
                title,
            ) in card_specs
            {
                let card_rect = Rect::new(card_x, 24.0, 240.0, 172.0);
                layer_scene.push(SceneCommand::FillPath {
                    path: Path::rounded_rect(card_rect, 18.0),
                    brush: surface.into(),
                });
                layer_scene.push(SceneCommand::StrokePath {
                    path: Path::rounded_rect(card_rect, 18.0),
                    brush: border.into(),
                    stroke: StrokeStyle::new(1.0),
                });
                layer_scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(card_x + 20.0, 44.0, 172.0, 24.0),
                    text: title.to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 18.0,
                        line_height: 22.0,
                        color: text_color,
                    },
                }));
                layer_scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(card_x + 20.0, 76.0, 188.0, 20.0),
                    text: "Cached tiles must match direct".to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 13.0,
                        line_height: 18.0,
                        color: subtle_text,
                    },
                }));

                let swatch_colors = [
                    Color::rgba(0.84, 0.87, 0.92, 1.0),
                    accent,
                    secondary,
                ];
                for (index, swatch_color) in swatch_colors.into_iter().enumerate() {
                    let swatch_rect = Rect::new(
                        card_x + 24.0 + (index as f32 * 72.0),
                        124.0,
                        60.0,
                        32.0,
                    );
                    layer_scene.push(SceneCommand::FillPath {
                        path: Path::rounded_rect(swatch_rect, 10.0),
                        brush: swatch_color.into(),
                    });
                    layer_scene.push(SceneCommand::StrokePath {
                        path: Path::rounded_rect(swatch_rect, 10.0),
                        brush: border.into(),
                        stroke: StrokeStyle::new(1.0),
                    });
                }
            }

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor.clone(),
                layer_scene,
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(640.0, 220.0),
                surface_size: Size::new(640.0, 220.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                        .with_damage(Rect::new(0.0, 0.0, 640.0, 220.0)),
                ],
                scene,
                font_registry: Arc::new(fonts.clone()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(WindowId::new(152), LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(WindowId::new(153), LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_layer_matches_direct_at_fractional_tile_boundaries() {
        let widget_id = WidgetId::new(95);
        let build_frame = |window_id, cache_policy| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(widget_id),
                widget_id,
                Rect::new(0.0, -150.5, 420.0, 700.0),
            )
            .with_content_bounds(Rect::new(0.0, -150.5, 420.0, 700.0))
            .with_paint_bounds(Rect::new(0.0, -150.5, 420.0, 700.0))
            .with_cache_policy(cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, -150.5, 420.0, 700.0),
                brush: Color::WHITE.into(),
            });
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(32.0, 372.0, 340.0, 18.0),
                brush: Color::rgba(0.18, 0.24, 0.34, 1.0).into(),
            });
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(350.0, 356.0, 44.0, 58.0),
                brush: Color::rgba(0.12, 0.35, 0.78, 1.0).into(),
            });
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(334.0, 392.0, 64.0, 16.0),
                brush: Color::rgba(0.88, 0.52, 0.18, 1.0).into(),
            });

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor.clone(),
                layer_scene,
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(420.0, 240.0),
                surface_size: Size::new(420.0, 240.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                        .with_damage(Rect::new(0.0, -150.5, 420.0, 700.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(WindowId::new(94), LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(WindowId::new(95), LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_layer_matches_direct_for_clipped_rows_across_tile_boundary() {
        let widget_id = WidgetId::new(96);
        let clip_rect = Rect::new(42.0, 628.0, 360.0, 220.0);
        let build_frame = |window_id, cache_policy| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(widget_id),
                widget_id,
                Rect::new(24.0, -478.0, 1232.0, 2046.0),
            )
            .with_content_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
            .with_paint_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
            .with_cache_policy(cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(24.0, -478.0, 1232.0, 2046.0),
                brush: Color::WHITE.into(),
            });
            layer_scene.push(SceneCommand::PushClip { rect: clip_rect });
            for (index, y) in [636.0, 668.0, 700.0, 732.0].into_iter().enumerate() {
                let brush = if index == 1 {
                    Color::rgba(0.79, 0.86, 0.98, 1.0)
                } else {
                    Color::rgba(0.90, 0.93, 0.97, 1.0)
                };
                layer_scene.push(SceneCommand::FillRect {
                    rect: Rect::new(42.0, y, 360.0, 28.0),
                    brush: brush.into(),
                });
                layer_scene.push(SceneCommand::FillRect {
                    rect: Rect::new(58.0, y + 8.0, 172.0, 12.0),
                    brush: Color::rgba(0.17, 0.21, 0.29, 1.0).into(),
                });
                layer_scene.push(SceneCommand::FillRect {
                    rect: Rect::new(248.0, y + 8.0, 96.0, 12.0),
                    brush: Color::rgba(0.41, 0.48, 0.58, 1.0).into(),
                });
            }
            layer_scene.push(SceneCommand::PopClip);

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor.clone(),
                layer_scene,
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(430.0, 900.0),
                surface_size: Size::new(430.0, 900.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                        .with_damage(Rect::new(24.0, -478.0, 1232.0, 2046.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(WindowId::new(96), LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(WindowId::new(97), LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_ancestor_matches_direct_for_clipped_child_layer_rows() {
        let shell_id = WidgetId::new(97);
        let child_id = WidgetId::new(98);
        let clip_rect = Rect::new(42.0, 628.0, 360.0, 220.0);
        let build_frame = |window_id, shell_cache_policy| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(24.0, -478.0, 1232.0, 2046.0),
            )
            .with_content_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
            .with_paint_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
            .with_cache_policy(shell_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let child_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(child_id),
                child_id,
                Rect::new(41.5, 627.5, 361.0, 221.0),
            )
            .with_content_bounds(Rect::new(41.5, 627.5, 361.0, 221.0))
            .with_paint_bounds(Rect::new(41.5, 627.5, 361.0, 221.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut child_scene = Scene::new();
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(41.5, 627.5, 361.0, 221.0),
                brush: Color::WHITE.into(),
            });
            child_scene.push(SceneCommand::PushClip { rect: clip_rect });
            for (index, y) in [636.0, 668.0, 700.0, 732.0].into_iter().enumerate() {
                let brush = if index == 1 {
                    Color::rgba(0.79, 0.86, 0.98, 1.0)
                } else {
                    Color::rgba(0.90, 0.93, 0.97, 1.0)
                };
                child_scene.push(SceneCommand::FillRect {
                    rect: Rect::new(42.0, y, 360.0, 28.0),
                    brush: brush.into(),
                });
                child_scene.push(SceneCommand::FillRect {
                    rect: Rect::new(58.0, y + 8.0, 172.0, 12.0),
                    brush: Color::rgba(0.17, 0.21, 0.29, 1.0).into(),
                });
                child_scene.push(SceneCommand::FillRect {
                    rect: Rect::new(248.0, y + 8.0, 96.0, 12.0),
                    brush: Color::rgba(0.41, 0.48, 0.58, 1.0).into(),
                });
            }
            child_scene.push(SceneCommand::PopClip);

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                child_descriptor.clone(),
                child_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(430.0, 900.0),
                surface_size: Size::new(430.0, 900.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        shell_descriptor,
                    )
                    .with_damage(Rect::new(24.0, -478.0, 1232.0, 2046.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        child_descriptor,
                    )
                    .with_damage(Rect::new(41.5, 627.5, 361.0, 221.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(WindowId::new(98), LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(WindowId::new(99), LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_ancestor_clips_fully_outside_child_layer() {
        let shell_id = WidgetId::new(100);
        let child_id = WidgetId::new(101);
        let clip_rect = Rect::new(42.0, 628.0, 360.0, 220.0);
        let build_frame = |window_id, shell_cache_policy| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(24.0, -478.0, 1232.0, 2046.0),
            )
            .with_content_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
            .with_paint_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
            .with_cache_policy(shell_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let child_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(child_id),
                child_id,
                Rect::new(96.0, 904.0, 180.0, 96.0),
            )
            .with_content_bounds(Rect::new(96.0, 904.0, 180.0, 96.0))
            .with_paint_bounds(Rect::new(96.0, 904.0, 180.0, 96.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut child_scene = Scene::new();
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(96.0, 904.0, 180.0, 96.0),
                brush: Color::rgba(0.82, 0.16, 0.18, 1.0).into(),
            });

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::FillRect {
                rect: Rect::new(24.0, -478.0, 1232.0, 2046.0),
                brush: Color::WHITE.into(),
            });
            shell_scene.push(SceneCommand::PushClip { rect: clip_rect });
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                child_descriptor.clone(),
                child_scene,
            )));
            shell_scene.push(SceneCommand::PopClip);

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(430.0, 900.0),
                surface_size: Size::new(430.0, 900.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        shell_descriptor,
                    )
                    .with_damage(Rect::new(24.0, -478.0, 1232.0, 2046.0)),
                    SceneLayerUpdate::from_descriptor(
                        SceneLayerUpdateKind::Content,
                        child_descriptor,
                    )
                    .with_damage(Rect::new(96.0, 904.0, 180.0, 96.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(WindowId::new(100), LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(WindowId::new(101), LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_ancestor_matches_direct_after_child_translation() {
        let shell_id = WidgetId::new(102);
        let scroll_id = WidgetId::new(103);
        let content_id = WidgetId::new(104);
        let first_id = WidgetId::new(105);
        let second_id = WidgetId::new(106);
        let third_id = WidgetId::new(107);
        let clip_rect = Rect::new(42.0, 60.0, 360.0, 220.0);

        let content_descriptor = |y: f32| {
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(content_id),
                content_id,
                Rect::new(42.0, y, 360.0, 360.0),
            )
            .with_content_bounds(Rect::new(42.0, y, 360.0, 360.0))
            .with_paint_bounds(Rect::new(42.0, y, 360.0, 360.0))
            .with_cache_policy(LayerCachePolicy::Direct)
        };

        let child_layer = |id: WidgetId, y: f32, brush: Color| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(42.0, y, 360.0, 96.0),
                brush: brush.into(),
            });
            SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(id),
                    id,
                    Rect::new(42.0, y, 360.0, 96.0),
                )
                .with_content_bounds(Rect::new(42.0, y, 360.0, 96.0))
                .with_paint_bounds(Rect::new(42.0, y, 360.0, 96.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                scene,
            )
        };

        let build_content_scene = |y: f32| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(child_layer(
                first_id,
                y,
                Color::rgba(0.82, 0.36, 0.18, 1.0),
            )));
            scene.push(SceneCommand::Layer(child_layer(
                second_id,
                y + 120.0,
                Color::rgba(0.18, 0.54, 0.82, 1.0),
            )));
            scene.push(SceneCommand::Layer(child_layer(
                third_id,
                y + 240.0,
                Color::rgba(0.24, 0.72, 0.36, 1.0),
            )));
            scene
        };

        let build_frame = |window_id, scroll_cache_policy, content_y: f32, update_kind| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 430.0, 360.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(scroll_id),
                scroll_id,
                Rect::new(24.0, 24.0, 382.0, 292.0),
            )
            .with_content_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
            .with_paint_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
            .with_cache_policy(scroll_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let mut scroll_scene = Scene::new();
            scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                content_descriptor(content_y),
                build_content_scene(content_y),
            )));

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 430.0, 360.0),
                brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
            });
            shell_scene.push(SceneCommand::PushClip { rect: clip_rect });
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                scroll_descriptor.clone(),
                scroll_scene,
            )));
            shell_scene.push(SceneCommand::PopClip);

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));

            let update = if update_kind == SceneLayerUpdateKind::Content {
                SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                    .with_damage(Rect::new(42.0, content_y, 360.0, 360.0))
            } else {
                SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                    .with_damage(Rect::new(42.0, 0.0, 360.0, 432.0))
            };

            SceneFrame {
                window_id,
                viewport: Size::new(430.0, 360.0),
                surface_size: Size::new(430.0, 360.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![update],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_frame(
            WindowId::new(110),
            LayerCachePolicy::Direct,
            0.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_frame(
            WindowId::new(110),
            LayerCachePolicy::Direct,
            72.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_frame(
            WindowId::new(111),
            LayerCachePolicy::Cached,
            0.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_frame(
            WindowId::new(111),
            LayerCachePolicy::Cached,
            72.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_internal_clip_matches_direct_after_child_translation() {
        let shell_id = WidgetId::new(108);
        let scroll_id = WidgetId::new(109);
        let content_id = WidgetId::new(110);
        let first_id = WidgetId::new(111);
        let second_id = WidgetId::new(112);
        let third_id = WidgetId::new(113);
        let clip_rect = Rect::new(42.0, 60.0, 360.0, 220.0);

        let content_descriptor = |y: f32| {
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(content_id),
                content_id,
                Rect::new(42.0, y, 360.0, 360.0),
            )
            .with_content_bounds(Rect::new(42.0, y, 360.0, 360.0))
            .with_paint_bounds(Rect::new(42.0, y, 360.0, 360.0))
            .with_cache_policy(LayerCachePolicy::Direct)
        };

        let child_layer = |id: WidgetId, y: f32, brush: Color| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(42.0, y, 360.0, 96.0),
                brush: brush.into(),
            });
            SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(id),
                    id,
                    Rect::new(42.0, y, 360.0, 96.0),
                )
                .with_content_bounds(Rect::new(42.0, y, 360.0, 96.0))
                .with_paint_bounds(Rect::new(42.0, y, 360.0, 96.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                scene,
            )
        };

        let build_content_scene = |y: f32| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(child_layer(
                first_id,
                y,
                Color::rgba(0.82, 0.36, 0.18, 1.0),
            )));
            scene.push(SceneCommand::Layer(child_layer(
                second_id,
                y + 120.0,
                Color::rgba(0.18, 0.54, 0.82, 1.0),
            )));
            scene.push(SceneCommand::Layer(child_layer(
                third_id,
                y + 240.0,
                Color::rgba(0.24, 0.72, 0.36, 1.0),
            )));
            scene
        };

        let build_frame = |window_id, scroll_cache_policy, content_y: f32, update_kind| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 430.0, 360.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(scroll_id),
                scroll_id,
                Rect::new(24.0, 24.0, 382.0, 292.0),
            )
            .with_content_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
            .with_paint_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
            .with_cache_policy(scroll_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let mut scroll_scene = Scene::new();
            scroll_scene.push(SceneCommand::PushClip { rect: clip_rect });
            scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                content_descriptor(content_y),
                build_content_scene(content_y),
            )));
            scroll_scene.push(SceneCommand::PopClip);

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 430.0, 360.0),
                brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
            });
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                scroll_descriptor.clone(),
                scroll_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));

            let update = if update_kind == SceneLayerUpdateKind::Content {
                SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                    .with_damage(Rect::new(42.0, content_y, 360.0, 360.0))
            } else {
                SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                    .with_damage(Rect::new(42.0, 0.0, 360.0, 432.0))
            };

            SceneFrame {
                window_id,
                viewport: Size::new(430.0, 360.0),
                surface_size: Size::new(430.0, 360.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![update],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_frame(
            WindowId::new(112),
            LayerCachePolicy::Direct,
            0.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_frame(
            WindowId::new(112),
            LayerCachePolicy::Direct,
            72.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_frame(
            WindowId::new(113),
            LayerCachePolicy::Cached,
            0.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_frame(
            WindowId::new(113),
            LayerCachePolicy::Cached,
            72.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_translucent_auto_child_matches_direct_after_translation() {
        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_translucent_scroll_child_frame(
            WindowId::new(214),
            LayerCachePolicy::Direct,
            LayerCachePolicy::Direct,
            60.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_translucent_scroll_child_frame(
            WindowId::new(214),
            LayerCachePolicy::Direct,
            LayerCachePolicy::Direct,
            132.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_translucent_scroll_child_frame(
            WindowId::new(215),
            LayerCachePolicy::Cached,
            LayerCachePolicy::Auto,
            60.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_translucent_scroll_child_frame(
            WindowId::new(215),
            LayerCachePolicy::Cached,
            LayerCachePolicy::Auto,
            132.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_translucent_direct_child_matches_direct_after_translation() {
        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_translucent_scroll_child_frame(
            WindowId::new(216),
            LayerCachePolicy::Direct,
            LayerCachePolicy::Direct,
            60.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_translucent_scroll_child_frame(
            WindowId::new(216),
            LayerCachePolicy::Direct,
            LayerCachePolicy::Direct,
            132.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_translucent_scroll_child_frame(
            WindowId::new(217),
            LayerCachePolicy::Cached,
            LayerCachePolicy::Direct,
            60.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_translucent_scroll_child_frame(
            WindowId::new(217),
            LayerCachePolicy::Cached,
            LayerCachePolicy::Direct,
            132.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_scroll_internal_clip_image_matches_direct_after_child_translation() {
        let shell_id = WidgetId::new(114);
        let scroll_id = WidgetId::new(115);
        let content_id = WidgetId::new(116);
        let image_layer_id = WidgetId::new(117);
        let filler_id = WidgetId::new(118);
        let image_handle = ImageHandle::new(41);
        let clip_rect = Rect::new(42.0, 60.0, 360.0, 220.0);

        let mut images = ImageRegistry::new();
        images.insert(
            image_handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64,
                    255,
                ],
            )
            .unwrap(),
        );
        let images = Arc::new(images);

        let content_descriptor = |y: f32| {
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(content_id),
                content_id,
                Rect::new(42.0, y, 360.0, 360.0),
            )
            .with_content_bounds(Rect::new(42.0, y, 360.0, 360.0))
            .with_paint_bounds(Rect::new(42.0, y, 360.0, 360.0))
            .with_cache_policy(LayerCachePolicy::Direct)
        };

        let image_layer = |y: f32| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::DrawImage {
                rect: Rect::new(42.0, y, 220.0, 220.0),
                source: ImageSource::new(image_handle),
            });
            SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(image_layer_id),
                    image_layer_id,
                    Rect::new(42.0, y, 220.0, 220.0),
                )
                .with_content_bounds(Rect::new(42.0, y, 220.0, 220.0))
                .with_paint_bounds(Rect::new(42.0, y, 220.0, 220.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                scene,
            )
        };

        let filler_layer = |y: f32| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(42.0, y, 360.0, 96.0),
                brush: Color::rgba(0.24, 0.72, 0.36, 1.0).into(),
            });
            SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(filler_id),
                    filler_id,
                    Rect::new(42.0, y, 360.0, 96.0),
                )
                .with_content_bounds(Rect::new(42.0, y, 360.0, 96.0))
                .with_paint_bounds(Rect::new(42.0, y, 360.0, 96.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                scene,
            )
        };

        let build_content_scene = |y: f32| {
            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(image_layer(y)));
            scene.push(SceneCommand::Layer(filler_layer(y + 240.0)));
            scene
        };

        let build_frame = |window_id, scroll_cache_policy, content_y: f32, update_kind| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 430.0, 360.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(scroll_id),
                scroll_id,
                Rect::new(24.0, 24.0, 382.0, 292.0),
            )
            .with_content_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
            .with_paint_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
            .with_cache_policy(scroll_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let mut scroll_scene = Scene::new();
            scroll_scene.push(SceneCommand::PushClip { rect: clip_rect });
            scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                content_descriptor(content_y),
                build_content_scene(content_y),
            )));
            scroll_scene.push(SceneCommand::PopClip);

            let mut shell_scene = Scene::new();
            shell_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 430.0, 360.0),
                brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
            });
            shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                scroll_descriptor.clone(),
                scroll_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor.clone(),
                shell_scene,
            )));

            let update = if update_kind == SceneLayerUpdateKind::Content {
                SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                    .with_damage(Rect::new(42.0, content_y, 360.0, 360.0))
            } else {
                SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                    .with_damage(Rect::new(42.0, 0.0, 360.0, 460.0))
            };

            SceneFrame {
                window_id,
                viewport: Size::new(430.0, 360.0),
                surface_size: Size::new(430.0, 360.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![update],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::clone(&images),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_frame(
            WindowId::new(114),
            LayerCachePolicy::Direct,
            0.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_frame(
            WindowId::new(114),
            LayerCachePolicy::Direct,
            72.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_frame(
            WindowId::new(115),
            LayerCachePolicy::Cached,
            0.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_frame(
            WindowId::new(115),
            LayerCachePolicy::Cached,
            72.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn nested_cached_scroll_image_outside_parent_bounds_matches_direct_render() {
        let shell_id = WidgetId::new(119);
        let outer_scroll_id = WidgetId::new(120);
        let inner_scroll_id = WidgetId::new(121);
        let image_layer_id = WidgetId::new(122);
        let image_handle = ImageHandle::new(42);

        let mut images = ImageRegistry::new();
        images.insert(
            image_handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64,
                    255,
                ],
            )
            .unwrap(),
        );
        let images = Arc::new(images);

        let build_frame = |window_id, inner_cache_policy| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 1280.0, 720.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let outer_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(outer_scroll_id),
                outer_scroll_id,
                Rect::new(320.0, 28.0, 428.0, 336.0),
            )
            .with_content_bounds(Rect::new(320.0, 28.0, 428.0, 336.0))
            .with_paint_bounds(Rect::new(320.0, 28.0, 428.0, 336.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let inner_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(inner_scroll_id),
                inner_scroll_id,
                Rect::new(321.0, 60.0, 426.0, 303.0),
            )
            .with_content_bounds(Rect::new(321.0, 60.0, 426.0, 303.0))
            .with_paint_bounds(Rect::new(321.0, 60.0, 426.0, 303.0))
            .with_cache_policy(inner_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll)
            .with_is_stack_surface(true);

            let mut image_scene = Scene::new();
            image_scene.push(SceneCommand::DrawImage {
                rect: Rect::new(363.0, 376.0, 220.0, 220.0),
                source: ImageSource::new(image_handle),
            });
            let image_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(image_layer_id),
                    image_layer_id,
                    Rect::new(363.0, 376.0, 220.0, 220.0),
                )
                .with_content_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
                .with_paint_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                image_scene,
            );

            let mut inner_scroll_scene = Scene::new();
            inner_scroll_scene.push(SceneCommand::FillRect {
                rect: Rect::new(321.0, 60.0, 426.0, 303.0),
                brush: Color::rgba(0.96, 0.97, 0.99, 1.0).into(),
            });
            inner_scroll_scene.push(SceneCommand::Layer(image_layer));

            let mut outer_scroll_scene = Scene::new();
            outer_scroll_scene.push(SceneCommand::FillRect {
                rect: Rect::new(320.0, 28.0, 428.0, 336.0),
                brush: Color::rgba(0.96, 0.97, 0.99, 1.0).into(),
            });
            outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                inner_scroll_descriptor,
                inner_scroll_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
                brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
            });
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor,
                {
                    let mut shell_scene = Scene::new();
                    shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                        outer_scroll_descriptor,
                        outer_scroll_scene,
                    )));
                    shell_scene
                },
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(1280.0, 720.0),
                surface_size: Size::new(1280.0, 720.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    SceneLayerDescriptor::new(
                        SceneLayerId::from_widget(image_layer_id),
                        image_layer_id,
                        Rect::new(363.0, 376.0, 220.0, 220.0),
                    )
                    .with_content_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
                    .with_paint_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
                    .with_cache_policy(LayerCachePolicy::Direct),
                )],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::clone(&images),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_frame = build_frame(WindowId::new(116), LayerCachePolicy::Direct);
        renderer.render(&direct_frame).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_frame.window_id)
            .unwrap();

        let cached_frame = build_frame(WindowId::new(117), LayerCachePolicy::Cached);
        renderer.render(&cached_frame).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_frame.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn nested_cached_scroll_large_paint_bounds_image_matches_direct_render() {
        let shell_id = WidgetId::new(123);
        let outer_scroll_id = WidgetId::new(124);
        let inner_scroll_id = WidgetId::new(125);
        let image_layer_id = WidgetId::new(126);
        let image_handle = ImageHandle::new(43);

        let mut images = ImageRegistry::new();
        images.insert(
            image_handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64,
                    255,
                ],
            )
            .unwrap(),
        );
        let images = Arc::new(images);

        let build_frame = |window_id, inner_cache_policy| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 1280.0, 720.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let outer_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(outer_scroll_id),
                outer_scroll_id,
                Rect::new(320.0, 28.0, 428.0, 336.0),
            )
            .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let inner_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(inner_scroll_id),
                inner_scroll_id,
                Rect::new(321.0, 60.0, 426.0, 303.0),
            )
            .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_cache_policy(inner_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll)
            .with_is_stack_surface(true);

            let mut image_scene = Scene::new();
            image_scene.push(SceneCommand::DrawImage {
                rect: Rect::new(363.0, 376.0, 220.0, 220.0),
                source: ImageSource::new(image_handle),
            });
            let image_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(image_layer_id),
                    image_layer_id,
                    Rect::new(363.0, 376.0, 220.0, 220.0),
                )
                .with_content_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
                .with_paint_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                image_scene,
            );

            let mut inner_scroll_scene = Scene::new();
            inner_scroll_scene.push(SceneCommand::Layer(image_layer));

            let mut outer_scroll_scene = Scene::new();
            outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                inner_scroll_descriptor,
                inner_scroll_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
                brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
            });
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor,
                {
                    let mut shell_scene = Scene::new();
                    shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                        outer_scroll_descriptor,
                        outer_scroll_scene,
                    )));
                    shell_scene
                },
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(1280.0, 720.0),
                surface_size: Size::new(1280.0, 720.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    SceneLayerDescriptor::new(
                        SceneLayerId::from_widget(image_layer_id),
                        image_layer_id,
                        Rect::new(363.0, 376.0, 220.0, 220.0),
                    )
                    .with_content_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
                    .with_paint_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
                    .with_cache_policy(LayerCachePolicy::Direct),
                )],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::clone(&images),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_frame = build_frame(WindowId::new(118), LayerCachePolicy::Direct);
        renderer.render(&direct_frame).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_frame.window_id)
            .unwrap();

        let cached_frame = build_frame(WindowId::new(119), LayerCachePolicy::Cached);
        renderer.render(&cached_frame).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_frame.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn nested_cached_scroll_internal_clip_matches_direct_after_image_scroll() {
        let shell_id = WidgetId::new(127);
        let outer_scroll_id = WidgetId::new(128);
        let inner_scroll_id = WidgetId::new(129);
        let image_layer_id = WidgetId::new(130);
        let image_handle = ImageHandle::new(44);

        let mut images = ImageRegistry::new();
        images.insert(
            image_handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64,
                    255,
                ],
            )
            .unwrap(),
        );
        let images = Arc::new(images);

        let build_frame = |window_id, inner_cache_policy, image_y, update_kind| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 1280.0, 720.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let outer_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(outer_scroll_id),
                outer_scroll_id,
                Rect::new(320.0, 28.0, 428.0, 336.0),
            )
            .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let inner_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(inner_scroll_id),
                inner_scroll_id,
                Rect::new(321.0, 60.0, 426.0, 303.0),
            )
            .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_cache_policy(inner_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll)
            .with_is_stack_surface(true);

            let image_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(image_layer_id),
                image_layer_id,
                Rect::new(363.0, image_y, 220.0, 220.0),
            )
            .with_content_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
            .with_paint_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let mut image_scene = Scene::new();
            image_scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
                brush: Color::rgba(0.965, 0.975, 0.99, 1.0).into(),
            });
            image_scene.push(SceneCommand::PushClip {
                rect: Rect::new(363.0, image_y, 220.0, 220.0),
            });
            image_scene.push(SceneCommand::DrawImage {
                rect: Rect::new(363.0, image_y, 220.0, 220.0),
                source: ImageSource::new(image_handle),
            });
            image_scene.push(SceneCommand::PopClip);
            image_scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
                brush: Color::rgba(0.8335978, 0.8335974, 0.835042, 1.0).into(),
                stroke: StrokeStyle { width: 1.0 },
            });
            let image_layer = SceneLayer::from_descriptor(image_descriptor.clone(), image_scene);

            let mut inner_scroll_scene = Scene::new();
            inner_scroll_scene.push(SceneCommand::PushClip {
                rect: Rect::new(321.0, 60.0, 426.0, 303.0),
            });
            inner_scroll_scene.push(SceneCommand::Layer(image_layer));
            inner_scroll_scene.push(SceneCommand::PopClip);

            let mut outer_scroll_scene = Scene::new();
            outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                inner_scroll_descriptor,
                inner_scroll_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
                brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
            });
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor,
                {
                    let mut shell_scene = Scene::new();
                    shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                        outer_scroll_descriptor,
                        outer_scroll_scene,
                    )));
                    shell_scene
                },
            )));

            let update = SceneLayerUpdate::from_descriptor(update_kind, image_descriptor.clone())
                .with_damage(Rect::new(362.5, 139.5, 221.0, 457.0));

            SceneFrame {
                window_id,
                viewport: Size::new(1280.0, 720.0),
                surface_size: Size::new(1280.0, 720.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![update],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::clone(&images),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_frame(
            WindowId::new(120),
            LayerCachePolicy::Direct,
            140.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_frame(
            WindowId::new(120),
            LayerCachePolicy::Direct,
            376.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_frame(
            WindowId::new(121),
            LayerCachePolicy::Cached,
            140.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_frame(
            WindowId::new(121),
            LayerCachePolicy::Cached,
            376.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn nested_cached_scroll_sibling_transforms_match_direct_after_image_scroll() {
        let shell_id = WidgetId::new(131);
        let outer_scroll_id = WidgetId::new(132);
        let inner_scroll_id = WidgetId::new(133);
        let top_section_id = WidgetId::new(134);
        let bottom_section_id = WidgetId::new(135);
        let image_layer_id = WidgetId::new(136);
        let image_handle = ImageHandle::new(45);

        let mut images = ImageRegistry::new();
        images.insert(
            image_handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64,
                    255,
                ],
            )
            .unwrap(),
        );
        let images = Arc::new(images);

        let build_frame = |window_id, inner_cache_policy, top_y, bottom_y, update_kind| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 1280.0, 720.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let outer_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(outer_scroll_id),
                outer_scroll_id,
                Rect::new(320.0, 28.0, 428.0, 336.0),
            )
            .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let inner_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(inner_scroll_id),
                inner_scroll_id,
                Rect::new(321.0, 60.0, 426.0, 303.0),
            )
            .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_cache_policy(inner_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll)
            .with_is_stack_surface(true);

            let mut top_scene = Scene::new();
            top_scene.push(SceneCommand::FillRect {
                rect: Rect::new(345.0, top_y, 378.0, 379.0),
                brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
            });
            top_scene.push(SceneCommand::FillRect {
                rect: Rect::new(363.0, top_y + 81.0, 440.2, 240.0),
                brush: Color::rgba(0.97, 0.981, 0.992, 1.0).into(),
            });
            let top_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(top_section_id),
                    top_section_id,
                    Rect::new(345.0, top_y, 378.0, 379.0),
                )
                .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                top_scene,
            );

            let image_y = bottom_y + 133.0;
            let mut image_scene = Scene::new();
            image_scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
                brush: Color::rgba(0.965, 0.975, 0.99, 1.0).into(),
            });
            image_scene.push(SceneCommand::PushClip {
                rect: Rect::new(363.0, image_y, 220.0, 220.0),
            });
            image_scene.push(SceneCommand::DrawImage {
                rect: Rect::new(363.0, image_y, 220.0, 220.0),
                source: ImageSource::new(image_handle),
            });
            image_scene.push(SceneCommand::PopClip);
            let image_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(image_layer_id),
                    image_layer_id,
                    Rect::new(363.0, image_y, 220.0, 220.0),
                )
                .with_content_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
                .with_paint_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                image_scene,
            );

            let mut bottom_scene = Scene::new();
            bottom_scene.push(SceneCommand::FillRect {
                rect: Rect::new(345.0, bottom_y, 378.0, 371.0),
                brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
            });
            bottom_scene.push(SceneCommand::Layer(image_layer));
            let bottom_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(bottom_section_id),
                    bottom_section_id,
                    Rect::new(345.0, bottom_y, 378.0, 371.0),
                )
                .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                .with_cache_policy(LayerCachePolicy::Direct),
                bottom_scene,
            );

            let mut inner_scroll_scene = Scene::new();
            inner_scroll_scene.push(SceneCommand::PushClip {
                rect: Rect::new(321.0, 60.0, 426.0, 303.0),
            });
            inner_scroll_scene.push(SceneCommand::Layer(top_layer));
            inner_scroll_scene.push(SceneCommand::Layer(bottom_layer));
            inner_scroll_scene.push(SceneCommand::PopClip);

            let mut outer_scroll_scene = Scene::new();
            outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                inner_scroll_descriptor,
                inner_scroll_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
                brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
            });
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor,
                {
                    let mut shell_scene = Scene::new();
                    shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                        outer_scroll_descriptor,
                        outer_scroll_scene,
                    )));
                    shell_scene
                },
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(1280.0, 720.0),
                surface_size: Size::new(1280.0, 720.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        update_kind,
                        SceneLayerDescriptor::new(
                            SceneLayerId::from_widget(top_section_id),
                            top_section_id,
                            Rect::new(345.0, top_y, 378.0, 379.0),
                        )
                        .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                        .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                        .with_cache_policy(LayerCachePolicy::Direct),
                    )
                    .with_damage(Rect::new(345.0, -178.0, 1172.2, 403.0)),
                    SceneLayerUpdate::from_descriptor(
                        update_kind,
                        SceneLayerDescriptor::new(
                            SceneLayerId::from_widget(bottom_section_id),
                            bottom_section_id,
                            Rect::new(345.0, bottom_y, 378.0, 371.0),
                        )
                        .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                        .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                        .with_cache_policy(LayerCachePolicy::Direct),
                    )
                    .with_damage(Rect::new(345.0, 219.0, 378.0, 395.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::clone(&images),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_frame(
            WindowId::new(122),
            LayerCachePolicy::Direct,
            -178.0,
            219.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_frame(
            WindowId::new(122),
            LayerCachePolicy::Direct,
            -154.0,
            243.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_frame(
            WindowId::new(123),
            LayerCachePolicy::Cached,
            -178.0,
            219.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_frame(
            WindowId::new(123),
            LayerCachePolicy::Cached,
            -154.0,
            243.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn nested_cached_scroll_auto_sibling_transforms_match_direct_after_image_scroll() {
        let shell_id = WidgetId::new(137);
        let outer_scroll_id = WidgetId::new(138);
        let inner_scroll_id = WidgetId::new(139);
        let top_section_id = WidgetId::new(140);
        let bottom_section_id = WidgetId::new(141);
        let image_layer_id = WidgetId::new(142);
        let image_handle = ImageHandle::new(46);

        let mut images = ImageRegistry::new();
        images.insert(
            image_handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64,
                    255,
                ],
            )
            .unwrap(),
        );
        let images = Arc::new(images);

        let build_frame = |window_id, inner_cache_policy, top_y, bottom_y, update_kind| {
            let shell_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(shell_id),
                shell_id,
                Rect::new(0.0, 0.0, 1280.0, 720.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
            .with_cache_policy(LayerCachePolicy::Direct);

            let outer_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(outer_scroll_id),
                outer_scroll_id,
                Rect::new(320.0, 28.0, 428.0, 336.0),
            )
            .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
            .with_cache_policy(LayerCachePolicy::Direct)
            .with_composition_mode(LayerCompositionMode::Scroll);

            let inner_scroll_descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(inner_scroll_id),
                inner_scroll_id,
                Rect::new(321.0, 60.0, 426.0, 303.0),
            )
            .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
            .with_cache_policy(inner_cache_policy)
            .with_composition_mode(LayerCompositionMode::Scroll)
            .with_is_stack_surface(true);

            let mut top_scene = Scene::new();
            top_scene.push(SceneCommand::FillRect {
                rect: Rect::new(345.0, top_y, 378.0, 379.0),
                brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
            });
            top_scene.push(SceneCommand::FillRect {
                rect: Rect::new(363.0, top_y + 81.0, 440.2, 240.0),
                brush: Color::rgba(0.97, 0.981, 0.992, 1.0).into(),
            });
            let top_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(top_section_id),
                    top_section_id,
                    Rect::new(345.0, top_y, 378.0, 379.0),
                )
                .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0)),
                top_scene,
            );

            let image_y = bottom_y + 133.0;
            let mut image_scene = Scene::new();
            image_scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
                brush: Color::rgba(0.965, 0.975, 0.99, 1.0).into(),
            });
            image_scene.push(SceneCommand::PushClip {
                rect: Rect::new(363.0, image_y, 220.0, 220.0),
            });
            image_scene.push(SceneCommand::DrawImage {
                rect: Rect::new(363.0, image_y, 220.0, 220.0),
                source: ImageSource::new(image_handle),
            });
            image_scene.push(SceneCommand::PopClip);
            let image_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(image_layer_id),
                    image_layer_id,
                    Rect::new(363.0, image_y, 220.0, 220.0),
                )
                .with_content_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
                .with_paint_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0)),
                image_scene,
            );

            let mut bottom_scene = Scene::new();
            bottom_scene.push(SceneCommand::FillRect {
                rect: Rect::new(345.0, bottom_y, 378.0, 371.0),
                brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
            });
            bottom_scene.push(SceneCommand::Layer(image_layer));
            let bottom_layer = SceneLayer::from_descriptor(
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(bottom_section_id),
                    bottom_section_id,
                    Rect::new(345.0, bottom_y, 378.0, 371.0),
                )
                .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0)),
                bottom_scene,
            );

            let mut inner_scroll_scene = Scene::new();
            inner_scroll_scene.push(SceneCommand::PushClip {
                rect: Rect::new(321.0, 60.0, 426.0, 303.0),
            });
            inner_scroll_scene.push(SceneCommand::Layer(top_layer));
            inner_scroll_scene.push(SceneCommand::Layer(bottom_layer));
            inner_scroll_scene.push(SceneCommand::PopClip);

            let mut outer_scroll_scene = Scene::new();
            outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                inner_scroll_descriptor,
                inner_scroll_scene,
            )));

            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
                brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
            });
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                shell_descriptor,
                {
                    let mut shell_scene = Scene::new();
                    shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                        outer_scroll_descriptor,
                        outer_scroll_scene,
                    )));
                    shell_scene
                },
            )));

            SceneFrame {
                window_id,
                viewport: Size::new(1280.0, 720.0),
                surface_size: Size::new(1280.0, 720.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(
                        update_kind,
                        SceneLayerDescriptor::new(
                            SceneLayerId::from_widget(top_section_id),
                            top_section_id,
                            Rect::new(345.0, top_y, 378.0, 379.0),
                        )
                        .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                        .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0)),
                    )
                    .with_damage(Rect::new(345.0, -178.0, 1172.2, 403.0)),
                    SceneLayerUpdate::from_descriptor(
                        update_kind,
                        SceneLayerDescriptor::new(
                            SceneLayerId::from_widget(bottom_section_id),
                            bottom_section_id,
                            Rect::new(345.0, bottom_y, 378.0, 371.0),
                        )
                        .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                        .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0)),
                    )
                    .with_damage(Rect::new(345.0, 219.0, 378.0, 395.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::clone(&images),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();

        let direct_initial = build_frame(
            WindowId::new(124),
            LayerCachePolicy::Direct,
            -178.0,
            219.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&direct_initial).unwrap();
        let direct_updated = build_frame(
            WindowId::new(124),
            LayerCachePolicy::Direct,
            -154.0,
            243.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&direct_updated).unwrap();
        let direct_pixels = renderer
            .capture_last_frame_rgba(direct_updated.window_id)
            .unwrap();

        let cached_initial = build_frame(
            WindowId::new(125),
            LayerCachePolicy::Cached,
            -178.0,
            219.0,
            SceneLayerUpdateKind::Content,
        );
        renderer.render(&cached_initial).unwrap();
        let cached_updated = build_frame(
            WindowId::new(125),
            LayerCachePolicy::Cached,
            -154.0,
            243.0,
            SceneLayerUpdateKind::Transform,
        );
        renderer.render(&cached_updated).unwrap();
        let cached_pixels = renderer
            .capture_last_frame_rgba(cached_updated.window_id)
            .unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn glyph_raster_bounds_expand_fractional_edges() {
        let mut builder = super::TinySkiaPathBuilder::new();
        builder.move_to(0.6, -0.2);
        builder.line_to(10.2, -0.2);
        builder.line_to(10.2, 4.4);
        builder.line_to(0.6, 4.4);
        builder.close();
        let path = builder.finish().expect("fractional rectangle path");

        let bounds = super::glyph_raster_bounds(&path).expect("bounds for fractional rectangle");

        assert!((bounds.logical_min_x - 0.6).abs() < 0.0001);
        assert!((bounds.logical_min_y + 0.2).abs() < 0.0001);
        assert!((bounds.logical_width - 9.6).abs() < 0.0001);
        assert!((bounds.logical_height - 4.6).abs() < 0.0001);
        assert_eq!(bounds.raster_min_x, 0.0);
        assert_eq!(bounds.raster_min_y, -1.0);
        assert_eq!(bounds.raster_width, 11);
        assert_eq!(bounds.raster_height, 6);
    }

    #[test]
    fn atlas_text_keeps_terminal_glyphs_at_fractional_scale() {
        let handle = FontHandle::new(30);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let build_frame = |text: &str| SceneFrame {
            window_id: WindowId::new(96),
            viewport: Size::new(260.0, 52.0),
            surface_size: Size::new(390.0, 78.0),
            scale_factor: 1.5,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene: {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 260.0, 52.0),
                    brush: Color::WHITE.into(),
                });
                scene.push(SceneCommand::DrawText(TextRun {
                    rect: Rect::new(8.0, 10.0, 220.0, 24.0),
                    text: text.to_string(),
                    style: TextStyle {
                        font: Some(handle),
                        font_size: 14.0,
                        line_height: 18.0,
                        color: Color::rgba(0.12, 0.16, 0.22, 1.0),
                        ..TextStyle::default()
                    },
                }));
                scene
            },
            font_registry: Arc::new(fonts.clone()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut renderer = WgpuRenderer::default();
        let without_terminal = build_frame("inspecto");
        renderer.render(&without_terminal).unwrap();
        let without_terminal_pixels = renderer
            .capture_last_frame_rgba(without_terminal.window_id)
            .unwrap();

        let with_terminal = build_frame("inspector");
        renderer.render(&with_terminal).unwrap();
        let with_terminal_pixels = renderer
            .capture_last_frame_rgba(with_terminal.window_id)
            .unwrap();

        let diff_count = rgba_image_diff_count(&without_terminal_pixels, &with_terminal_pixels);

        assert!(
            diff_count > 0,
            "terminal glyph vanished at fractional scale (diff_count={diff_count})"
        );
    }

    #[test]
    fn feathered_stroke_rect_renders_at_fractional_scale() {
        let frame = SceneFrame {
            window_id: WindowId::new(97),
            viewport: Size::new(220.0, 64.0),
            surface_size: Size::new(330.0, 96.0),
            scale_factor: 1.5,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene: {
                let mut scene = Scene::new();
                scene.push(SceneCommand::FillRect {
                    rect: Rect::new(0.0, 0.0, 220.0, 64.0),
                    brush: Color::WHITE.into(),
                });
                scene.push(SceneCommand::StrokeRect {
                    rect: Rect::new(12.0, 12.0, 180.0, 32.0),
                    brush: Color::rgba(0.18, 0.33, 0.85, 1.0).into(),
                    stroke: StrokeStyle::new(1.0),
                });
                scene
            },
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut renderer = WgpuRenderer::default();
        renderer.render(&frame).unwrap();
        let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();

        let changed_pixels = pixels
            .pixels()
            .chunks_exact(4)
            .filter(|pixel| *pixel != [255, 255, 255, 255])
            .count();

        assert!(
            changed_pixels > 500,
            "feathered stroke rect disappeared at fractional scale (changed_pixels={changed_pixels})"
        );
    }

    #[test]
    fn feathered_rounded_border_retains_most_ink_at_fractional_scale() {
        let build_frame = || {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 240.0, 72.0),
                brush: Color::WHITE.into(),
            });
            scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(Rect::new(12.0, 16.0, 196.0, 36.0), 8.0),
                brush: Color::rgba(1.0, 1.0, 1.0, 1.0).into(),
            });
            scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(Rect::new(12.0, 16.0, 196.0, 36.0), 8.0),
                brush: Color::rgba(0.18, 0.33, 0.85, 1.0).into(),
                stroke: StrokeStyle::new(1.0 / 1.5),
            });

            SceneFrame {
                window_id: WindowId::new(99),
                viewport: Size::new(240.0, 72.0),
                surface_size: Size::new(360.0, 108.0),
                scale_factor: 1.5,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let frame = build_frame();

        let mut feathered = WgpuRenderer::default();
        feathered.render(&frame).unwrap();
        let feathered_pixels = feathered.capture_last_frame_rgba(frame.window_id).unwrap();

        let mut hard = WgpuRenderer::default().with_feathering_enabled(false);
        hard.render(&frame).unwrap();
        let hard_pixels = hard.capture_last_frame_rgba(frame.window_id).unwrap();

        let crop = Rect::new(10.0, 14.0, 200.0, 40.0);
        let feathered_ink = ink_pixel_count(&feathered_pixels, crop);
        let hard_ink = ink_pixel_count(&hard_pixels, crop);

        assert!(
            feathered_ink * 5 >= hard_ink * 4,
            "feathered rounded border lost too much ink at fractional scale (feathered_ink={feathered_ink}, hard_ink={hard_ink})"
        );
    }

    #[test]
    fn feathered_control_border_and_chevrons_retain_visible_ink() {
        fn line_path(start: Point, end: Point) -> Path {
            let mut builder = PathBuilder::new();
            builder.move_to(start).line_to(end);
            builder.build()
        }

        fn chevron_path(bounds: Rect, direction: f32) -> Path {
            let center = Point::new(
                bounds.x() + (bounds.width() * 0.5),
                bounds.y() + (bounds.height() * 0.5),
            );
            let mut builder = PathBuilder::new();
            if direction.is_sign_positive() {
                builder
                    .move_to(Point::new(bounds.x(), bounds.y() + (bounds.height() * 0.3)))
                    .line_to(Point::new(
                        center.x,
                        bounds.max_y() - (bounds.height() * 0.3),
                    ))
                    .line_to(Point::new(
                        bounds.max_x(),
                        bounds.y() + (bounds.height() * 0.3),
                    ));
            } else {
                builder
                    .move_to(Point::new(
                        bounds.x(),
                        bounds.max_y() - (bounds.height() * 0.3),
                    ))
                    .line_to(Point::new(center.x, bounds.y() + (bounds.height() * 0.3)))
                    .line_to(Point::new(
                        bounds.max_x(),
                        bounds.max_y() - (bounds.height() * 0.3),
                    ));
            }
            builder.build()
        }

        let build_frame = || {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 220.0, 72.0),
                brush: Color::WHITE.into(),
            });
            scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(Rect::new(10.0, 16.0, 170.0, 40.0), 6.0),
                brush: Color::rgba(0.47, 0.49, 0.53, 1.0).into(),
                stroke: StrokeStyle::new(1.0),
            });
            scene.push(SceneCommand::StrokePath {
                path: line_path(Point::new(154.0, 22.0), Point::new(154.0, 50.0)),
                brush: Color::rgba(0.73, 0.73, 0.75, 1.0).into(),
                stroke: StrokeStyle::new(1.0),
            });
            scene.push(SceneCommand::StrokePath {
                path: chevron_path(Rect::new(156.0, 18.0, 16.0, 14.0), -1.0),
                brush: Color::rgba(0.12, 0.12, 0.12, 1.0).into(),
                stroke: StrokeStyle::new(1.8),
            });
            scene.push(SceneCommand::StrokePath {
                path: chevron_path(Rect::new(156.0, 40.0, 16.0, 14.0), 1.0),
                brush: Color::rgba(0.12, 0.12, 0.12, 1.0).into(),
                stroke: StrokeStyle::new(1.8),
            });
            scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(Rect::new(10.0, 16.0, 196.0, 40.0), 6.0),
                brush: Color::rgba(0.73, 0.73, 0.75, 1.0).into(),
                stroke: StrokeStyle::new(1.0),
            });
            scene.push(SceneCommand::StrokePath {
                path: chevron_path(Rect::new(178.0, 27.0, 18.0, 18.0), 1.0),
                brush: Color::rgba(0.12, 0.12, 0.12, 1.0).into(),
                stroke: StrokeStyle::new(1.8),
            });

            SceneFrame {
                window_id: WindowId::new(102),
                viewport: Size::new(220.0, 72.0),
                surface_size: Size::new(220.0, 72.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let frame = build_frame();

        let mut feathered = WgpuRenderer::default();
        feathered.render(&frame).unwrap();
        let feathered_pixels = feathered.capture_last_frame_rgba(frame.window_id).unwrap();

        let mut hard = WgpuRenderer::default().with_feathering_enabled(false);
        hard.render(&frame).unwrap();
        let hard_pixels = hard.capture_last_frame_rgba(frame.window_id).unwrap();

        let number_input_crop = Rect::new(8.0, 14.0, 172.0, 44.0);
        let select_crop = Rect::new(8.0, 14.0, 200.0, 44.0);
        let feathered_number_input_ink = ink_pixel_count(&feathered_pixels, number_input_crop);
        let hard_number_input_ink = ink_pixel_count(&hard_pixels, number_input_crop);
        let feathered_select_ink = ink_pixel_count(&feathered_pixels, select_crop);
        let hard_select_ink = ink_pixel_count(&hard_pixels, select_crop);

        assert!(
            feathered_number_input_ink * 3 >= hard_number_input_ink,
            "feathered number-input border or chevrons lost too much ink (feathered={feathered_number_input_ink}, hard={hard_number_input_ink})"
        );
        assert!(
            feathered_select_ink * 3 >= hard_select_ink,
            "feathered select border or chevron lost too much ink (feathered={feathered_select_ink}, hard={hard_select_ink})"
        );
    }

    #[test]
    fn cached_layer_matches_direct_for_tight_rounded_border_at_fractional_scale() {
        let widget_id = WidgetId::new(100);
        let build_frame = |cache_policy| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(widget_id),
                widget_id,
                Rect::new(0.0, 0.0, 220.0, 64.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 64.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 64.0))
            .with_cache_policy(cache_policy);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 220.0, 64.0),
                brush: Color::WHITE.into(),
            });
            layer_scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(Rect::new(0.0, 0.0, 220.0, 64.0), 10.0),
                brush: Color::rgba(0.18, 0.33, 0.85, 1.0).into(),
                stroke: StrokeStyle::new(1.0 / 1.5),
            });

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor.clone(),
                layer_scene,
            )));

            SceneFrame {
                window_id: WindowId::new(100),
                viewport: Size::new(220.0, 64.0),
                surface_size: Size::new(330.0, 96.0),
                scale_factor: 1.5,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                        .with_damage(Rect::new(0.0, 0.0, 220.0, 64.0)),
                ],
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn cached_layer_matches_direct_for_tight_tab_text_at_fractional_scale() {
        let handle = FontHandle::new(30);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let widget_id = WidgetId::new(101);
        let build_frame = |cache_policy| {
            let descriptor = SceneLayerDescriptor::new(
                SceneLayerId::from_widget(widget_id),
                widget_id,
                Rect::new(0.0, 0.0, 236.0, 44.0),
            )
            .with_content_bounds(Rect::new(0.0, 0.0, 236.0, 44.0))
            .with_paint_bounds(Rect::new(0.0, 0.0, 236.0, 44.0))
            .with_cache_policy(cache_policy);

            let mut layer_scene = Scene::new();
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 236.0, 44.0),
                brush: Color::rgba(0.96, 0.97, 0.99, 1.0).into(),
            });
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(10.0, 10.0, 216.0, 20.0),
                text: "Inspector".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 12.0,
                    line_height: 16.0,
                    color: Color::rgba(0.15, 0.19, 0.26, 1.0),
                    ..TextStyle::default()
                },
            }));

            let mut scene = Scene::new();
            scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor.clone(),
                layer_scene,
            )));

            SceneFrame {
                window_id: WindowId::new(101),
                viewport: Size::new(236.0, 44.0),
                surface_size: Size::new(354.0, 66.0),
                scale_factor: 1.5,
                dirty_regions: Vec::new(),
                layer_updates: vec![
                    SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                        .with_damage(Rect::new(0.0, 0.0, 236.0, 44.0)),
                ],
                scene,
                font_registry: Arc::new(fonts.clone()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            }
        };

        let mut renderer = WgpuRenderer::default();
        let direct = build_frame(LayerCachePolicy::Direct);
        renderer.render(&direct).unwrap();
        let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

        let cached = build_frame(LayerCachePolicy::Cached);
        renderer.render(&cached).unwrap();
        let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

        assert_rgba_images_match(&direct_pixels, &cached_pixels);
    }

    #[test]
    fn build_vertices_uses_registered_font_handle() {
        let handle = FontHandle::new(17);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "registered".to_string(),
            style: TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(3),
                viewport: Size::new(160.0, 60.0),
                surface_size: Size::new(160.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(fonts),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
    }

    #[test]
    fn build_vertices_errors_for_unregistered_font_handle() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "missing".to_string(),
            style: TextStyle {
                font: Some(FontHandle::new(404)),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let error = match build_vertices(
            &SceneFrame {
                window_id: WindowId::new(4),
                viewport: Size::new(160.0, 60.0),
                surface_size: Size::new(160.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
        ) {
            Ok(_) => panic!("expected missing font handle to fail during shaping"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("font handle 404 is not registered")
        );
    }

    #[test]
    fn renderer_resets_and_retries_when_text_atlas_fills_mid_frame() {
        let mut scene = Scene::new();
        let text: String = (33u8..=126).map(char::from).collect();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 1800.0, 32.0),
            text,
            style: TextStyle {
                font_size: 18.0,
                line_height: 22.0,
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let frame = SceneFrame {
            window_id: WindowId::new(16),
            viewport: Size::new(1800.0, 64.0),
            surface_size: Size::new(1800.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        };

        let mut renderer = WgpuRenderer::new();
        let mut text_engine = TextEngine::new().unwrap();
        text_engine.atlas = TextAtlas::new(64, 64);
        renderer.text_engine = Some(text_engine);

        renderer.render(&frame).unwrap();

        let active_text_engine = renderer
            .text_engine
            .as_ref()
            .expect("renderer should recreate the text engine after atlas retry");
        assert_eq!(
            active_text_engine.atlas.size(),
            (TEXT_ATLAS_WIDTH as u32, TEXT_ATLAS_HEIGHT as u32)
        );
        assert!(active_text_engine.glyph_cache_stats().0 > 32);

        let stats = renderer
            .last_frame_stats(frame.window_id)
            .expect("renderer should record frame stats after retry");
        assert!(stats.text_glyph_instance_count > 0);

        let image = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
        assert!(
            image.pixels().chunks_exact(4).any(|pixel| pixel[3] != 0),
            "retried frame should still render visible text"
        );
    }

    #[test]
    fn retained_compositor_uses_registered_image_handle() {
        let handle = ImageHandle::new(23);
        let mut images = ImageRegistry::new();
        images.insert(
            handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                ],
            )
            .unwrap(),
        );

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawImage {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            source: ImageSource::new(handle),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let ops = prepare_with_compositor(
            &SceneFrame {
                window_id: WindowId::new(7),
                viewport: Size::new(96.0, 64.0),
                surface_size: Size::new(96.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(images),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
            &mut compositor,
        )
        .unwrap();

        assert_eq!(ops.draw_ops.len(), 1);
        let op = &ops.draw_ops[0];
        assert!(matches!(op.kind, DrawOpKind::Image { handle: value } if value == handle));
        assert_eq!(op.vertices.len, 6);
    }

    #[test]
    fn retained_compositor_errors_for_unregistered_image_handle() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawImage {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            source: ImageSource::new(ImageHandle::new(88)),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let error = prepare_with_compositor(
            &SceneFrame {
                window_id: WindowId::new(8),
                viewport: Size::new(96.0, 64.0),
                surface_size: Size::new(96.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                layer_updates: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
                text_layout_registry: Arc::new(TextLayoutRegistry::default()),
            },
            &mut text_engine,
            &mut compositor,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("image handle 88 is not registered")
        );
    }

    #[test]
    fn renderer_feather_width_is_configurable() {
        let mut renderer = WgpuRenderer::new().with_feather_width(2.5);

        assert_eq!(renderer.feather_width(), 2.5);
        assert!(renderer.feathering_enabled());

        renderer.set_feather_width(-3.0);

        assert_eq!(renderer.feather_width(), 0.0);

        renderer.set_feathering_enabled(false);

        assert!(!renderer.feathering_enabled());
    }
}
