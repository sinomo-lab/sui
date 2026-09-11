#![deny(unsafe_code)]
#![allow(clippy::too_many_arguments)]

mod capture;
mod device;
mod diagnostics;
mod draw;
mod feathering;
mod geometry;
mod gpu;
mod interop;
mod output;
mod path_cache;
mod paths;
mod primitives;
mod renderer;
mod resources;
mod retained;
mod scene;
mod shaders;
mod submission;
mod surface;
#[cfg(test)]
mod tests;
mod text;
mod text_engine;
mod text_policy;

pub use sui_scene::TextSubpixelOrder;

pub use capture::DebugCaptureArtifact;
pub use capture::HdrRgbaImage;
pub use capture::RgbaImage;
pub use diagnostics::RendererFrameStats;
pub use diagnostics::RendererPacketHotspot;
pub use diagnostics::RetainedPacketRebuildStats;
pub use interop::RendererInterop;
pub use interop::WgpuExternalTextureContext;
pub use interop::WgpuExternalTextureRegistry;
pub use output::ColorManagementMode;
pub use output::DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS;
pub use output::DebugCaptureEncoding;
pub use output::DebugCaptureRequest;
pub use output::DebugCaptureStage;
pub use output::DebugSdrVisualization;
pub use output::DisplayCapabilities;
pub use output::DisplayColorPrimaries;
pub use output::DisplayTransferFunction;
pub use output::DynamicRangeMode;
pub use output::OutputStrategy;
pub use output::RendererCapabilities;
pub use output::RequestedColorManagementMode;
pub use output::RequestedDynamicRangeMode;
pub use output::RequestedOutputColorPrimaries;
pub use output::RequestedToneMappingMode;
pub use text::GlyphCacheSnapshot;
pub use text::RendererTextCacheSnapshot;
pub use text_policy::FeatheringOptions;
pub use text_policy::StemDarkening;
pub use text_policy::TextCoveragePolicy;
pub use text_policy::TextHinting;
pub use text_policy::TextRenderMode;

use crate::gpu::SharedRenderer;
use crate::resources::CachedAnalyticPathGpu;
use crate::resources::CachedExternalTextureBindGroup;
use crate::resources::CachedImageTexture;
use crate::resources::CachedTextAtlasTexture;
use crate::resources::FrameResources;
use crate::resources::ImageTextureCacheKey;
use crate::resources::OffscreenTarget;
use crate::retained::RetainedCompositorState;
use crate::surface::SurfaceState;
use crate::text_engine::TextEngine;
use std::collections::HashMap;
use sui_core::ImageHandle;
use sui_core::WindowId;
use sui_scene::SceneFrame;

pub struct WgpuRenderer {
    instance: wgpu::Instance,
    feathering_enabled: bool,
    feather_width: f32,
    text_render_mode: TextRenderMode,
    text_subpixel_order: TextSubpixelOrder,
    text_hinting: TextHinting,
    stem_darkening: StemDarkening,
    text_coverage_policy: TextCoveragePolicy,
    vsync_enabled: bool,
    runtime_feathering_override: Option<FeatheringOptions>,
    runtime_text_subpixel_order_override: Option<TextSubpixelOrder>,
    runtime_text_hinting_override: Option<TextHinting>,
    runtime_stem_darkening_override: Option<StemDarkening>,
    runtime_text_coverage_policy_override: Option<TextCoveragePolicy>,
    runtime_diagnostics_enabled: bool,
    frames_rendered: usize,
    capabilities: RendererCapabilities,
    last_frames: HashMap<WindowId, SceneFrame>,
    last_frame_stats: HashMap<WindowId, RendererFrameStats>,
    shared: Option<SharedRenderer>,
    text_engine: Option<TextEngine>,
    image_cache: HashMap<ImageTextureCacheKey, CachedImageTexture>,
    external_texture_registry: Option<WgpuExternalTextureRegistry>,
    external_image_cache: HashMap<ImageHandle, CachedExternalTextureBindGroup>,
    text_atlas_array: Option<CachedTextAtlasTexture>,
    analytic_path_cache: HashMap<u64, CachedAnalyticPathGpu>,
    compositors: HashMap<WindowId, RetainedCompositorState>,
    surfaces: HashMap<WindowId, SurfaceState>,
    offscreen_targets: HashMap<WindowId, OffscreenTarget>,
    intermediate_targets: HashMap<WindowId, OffscreenTarget>,
    frame_resources: FrameResources,
}
