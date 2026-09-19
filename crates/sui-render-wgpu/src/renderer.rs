use crate::WgpuRenderer;
use crate::device::default_wgpu_instance;
use crate::diagnostics::RendererFrameStats;
use crate::interop::WgpuExternalTextureRegistry;
use crate::output::ColorManagementMode;
use crate::output::DisplayCapabilities;
use crate::output::OutputStrategy;
use crate::output::RendererCapabilities;
use crate::resources::DEFAULT_FEATHER_WIDTH;
use crate::resources::FrameResources;
use crate::surface::normalize_framebuffer_size;
use crate::text::RendererTextCacheSnapshot;
use crate::text_engine::TextEngine;
use crate::text_policy::FeatheringOptions;
use crate::text_policy::StemDarkening;
use crate::text_policy::TextCoveragePolicy;
use crate::text_policy::TextHinting;
use crate::text_policy::TextRenderMode;
use std::collections::HashMap;
use std::fmt;
use sui_core::Result;
use sui_core::WindowId;
use sui_scene::SceneFrame;
pub use sui_scene::TextSubpixelOrder;

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

    pub fn with_text_subpixel_order(mut self, order: TextSubpixelOrder) -> Self {
        self.set_text_subpixel_order(order);
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

    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.set_feather_width(feather_width);
        self
    }

    pub fn with_vsync_enabled(mut self, enabled: bool) -> Self {
        self.set_vsync_enabled(enabled);
        self
    }

    pub fn with_external_texture_registry(mut self, registry: WgpuExternalTextureRegistry) -> Self {
        self.set_external_texture_registry(registry);
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

    pub fn external_texture_registry(&self) -> Option<&WgpuExternalTextureRegistry> {
        self.external_texture_registry.as_ref()
    }

    pub fn set_external_texture_registry(&mut self, registry: WgpuExternalTextureRegistry) {
        if let Some(shared) = &self.shared {
            registry.attach(
                shared.device.clone(),
                shared.queue.clone(),
                shared.adapter.get_info(),
            );
        }
        self.external_texture_registry = Some(registry);
        self.external_image_cache.clear();
    }

    pub fn text_coverage_policy(&self) -> TextCoveragePolicy {
        self.text_coverage_policy
    }

    pub fn text_render_mode(&self) -> TextRenderMode {
        self.text_render_mode
    }

    pub fn text_subpixel_order(&self) -> TextSubpixelOrder {
        self.text_subpixel_order
    }

    pub fn text_hinting(&self) -> TextHinting {
        self.text_hinting
    }

    pub fn stem_darkening(&self) -> StemDarkening {
        self.stem_darkening
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

    pub fn set_text_subpixel_order(&mut self, order: TextSubpixelOrder) {
        if self.text_subpixel_order == order {
            return;
        }

        self.text_subpixel_order = order;
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_text_subpixel_order(order);
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
            if surface.display_capabilities == capabilities {
                return Ok(());
            }
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
            if surface.color_management == color_management {
                return Ok(());
            }
            surface.color_management = color_management;
        }
        self.configure_existing_surface(window_id)
    }

    pub fn window_output_strategy(&self, window_id: WindowId) -> Option<OutputStrategy> {
        self.surfaces
            .get(&window_id)
            .map(|surface| surface.output_strategy)
    }

    pub fn window_surface_formats(&self, window_id: WindowId) -> Option<Vec<wgpu::TextureFormat>> {
        self.surfaces
            .get(&window_id)
            .map(|surface| surface.available_surface_formats.clone())
    }

    pub fn set_runtime_feathering_override(&mut self, feathering: Option<FeatheringOptions>) {
        self.runtime_feathering_override = feathering.map(FeatheringOptions::clamped);
    }

    pub fn set_runtime_text_subpixel_order_override(&mut self, order: Option<TextSubpixelOrder>) {
        self.runtime_text_subpixel_order_override = order;
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

    pub fn set_runtime_diagnostics_enabled(&mut self, enabled: bool) {
        self.runtime_diagnostics_enabled = enabled;
        if let Some(shared) = &mut self.shared {
            shared.collect_pipeline_timings = enabled;
        }
        if let Some(text_engine) = self.text_engine.as_mut() {
            text_engine.set_diagnostics_enabled(enabled);
        }
        for compositor in self.compositors.values_mut() {
            compositor.set_diagnostics_enabled(enabled);
        }
    }

    pub(crate) fn active_feather_width(&self) -> f32 {
        self.runtime_feathering_override
            .unwrap_or_else(|| self.feathering())
            .effective_width()
    }

    pub(crate) fn active_text_hinting(&self) -> TextHinting {
        self.runtime_text_hinting_override
            .unwrap_or(self.text_hinting)
            .normalized()
    }

    pub(crate) fn active_text_subpixel_order(&self) -> TextSubpixelOrder {
        self.runtime_text_subpixel_order_override
            .unwrap_or(self.text_subpixel_order)
    }

    pub(crate) fn active_stem_darkening(&self) -> StemDarkening {
        self.runtime_stem_darkening_override
            .unwrap_or(self.stem_darkening)
            .normalized()
    }

    pub(crate) fn active_text_coverage_policy(&self) -> TextCoveragePolicy {
        self.runtime_text_coverage_policy_override
            .unwrap_or(self.text_coverage_policy)
            .normalized()
    }

    pub(crate) fn invalidate_text_render_state(&mut self) {
        self.text_engine = None;
        self.text_atlas_array = None;
        self.compositors.clear();
        self.last_frames.clear();
        self.last_frame_stats.clear();
    }

    pub fn remove_window(&mut self, window_id: WindowId) {
        self.frame_resources.fragments.remove(&window_id);
        self.frame_resources.output_transforms.remove(&window_id);
        self.surfaces.remove(&window_id);
        self.offscreen_targets.remove(&window_id);
        self.intermediate_targets.remove(&window_id);
        self.last_frames.remove(&window_id);
        self.last_frame_stats.remove(&window_id);
        self.compositors.remove(&window_id);
    }

    pub fn render(&mut self, frame: &SceneFrame) -> Result<()> {
        let pipeline_before = self.shared.as_ref().map_or((0, 0), |shared| {
            (shared.pipeline_create_time_us, shared.pipeline_create_count)
        });
        let viewport = normalize_framebuffer_size(frame.surface_size);
        let mut frame_stats = RendererFrameStats::default();

        if let Some(size) = viewport {
            if self.surfaces.contains_key(&frame.window_id) {
                frame_stats = self.render_surface(frame, size)?;
            } else {
                frame_stats = self.render_offscreen(frame, size)?;
            }
        }

        frame_stats.device_prepare_time_us =
            std::mem::take(&mut self.pending_device_prepare_time_us);
        if let Some(shared) = &self.shared {
            frame_stats.pipeline_create_time_us =
                shared.pipeline_create_time_us - pipeline_before.0;
            frame_stats.pipeline_create_count = shared.pipeline_create_count - pipeline_before.1;
        }
        self.frames_rendered += 1;
        self.last_frames.insert(frame.window_id, frame.clone());
        self.last_frame_stats.insert(frame.window_id, frame_stats);
        self.analytic_path_cache
            .retain(|_, entry| self.frames_rendered.saturating_sub(entry.last_used_frame) <= 120);
        self.image_cache
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
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self {
            instance: default_wgpu_instance(),
            feathering_enabled: false,
            feather_width: DEFAULT_FEATHER_WIDTH,
            text_render_mode: TextRenderMode::default(),
            text_subpixel_order: TextSubpixelOrder::default(),
            text_hinting: TextHinting::default(),
            stem_darkening: StemDarkening::default(),
            text_coverage_policy: TextCoveragePolicy::default(),
            vsync_enabled: true,
            runtime_feathering_override: None,
            runtime_text_subpixel_order_override: None,
            runtime_text_hinting_override: None,
            runtime_stem_darkening_override: None,
            runtime_text_coverage_policy_override: None,
            runtime_diagnostics_enabled: true,
            pending_device_prepare_time_us: 0,
            frames_rendered: 0,
            capabilities: RendererCapabilities::default(),
            last_frames: HashMap::new(),
            last_frame_stats: HashMap::new(),
            shared: None,
            text_engine: None,
            image_cache: HashMap::new(),
            external_texture_registry: None,
            external_image_cache: HashMap::new(),
            text_atlas_array: None,
            analytic_path_cache: HashMap::new(),
            compositors: HashMap::new(),
            surfaces: HashMap::new(),
            offscreen_targets: HashMap::new(),
            intermediate_targets: HashMap::new(),
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
            .field("frames_rendered", &self.frames_rendered)
            .field("capabilities", &self.capabilities)
            .field("last_frame_count", &self.last_frames.len())
            .field("last_frame_stats_count", &self.last_frame_stats.len())
            .field("has_device", &self.shared.is_some())
            .field("surface_count", &self.surfaces.len())
            .finish()
    }
}
