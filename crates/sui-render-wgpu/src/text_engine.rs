use crate::draw::SceneRasterState;
use crate::gpu::TextAtlasInstance;
#[cfg(test)]
use crate::gpu::Vertex;
use crate::output::shader_color;
use crate::primitives::to_ndc;
use crate::submission::TEXT_ATLAS_INSTANCE_SIZE;
use crate::text::CachedGlyphAtlas;
use crate::text::GLYPH_SUBPIXEL_VARIANTS_X;
use crate::text::GLYPH_SUBPIXEL_VARIANTS_Y;
use crate::text::GlyphCacheKey;
use crate::text::GlyphCacheSnapshot;
use crate::text::GlyphFaceCacheKey;
use crate::text::GlyphSubpixelOffsetKey;
use crate::text::RendererTextCacheSnapshot;
use crate::text::TEXT_ATLAS_HEIGHT;
use crate::text::TEXT_ATLAS_MAX_PAGES;
use crate::text::TEXT_ATLAS_WIDTH;
use crate::text::TextAtlasColorMode;
use crate::text::TextAtlasInsertError;
use crate::text::TextAtlasPages;
use crate::text::TextAtlasUpload;
use crate::text::TextFrameStats;
use crate::text::glyph_scale_bucket;
use crate::text::glyph_scale_from_bucket;
use crate::text_policy::StemDarkening;
use crate::text_policy::TextCoveragePolicy;
use crate::text_policy::TextHinting;
use crate::text_policy::TextRenderMode;
use std::collections::HashMap;
use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use sui_core::Color;
use sui_core::Error;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Result;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;
use sui_scene::TextRenderCoveragePolicy;
use sui_scene::TextRenderHinting;
use sui_scene::TextRenderPolicy;
use sui_scene::TextRenderStemDarkening;
pub use sui_scene::TextSubpixelOrder;
use sui_text::FontRegistry;
use sui_text::ResolvedTextFace;
use sui_text::ShapedGlyph as SceneShapedGlyph;
use sui_text::ShapedText;
use sui_text::TextLayout;
use sui_text::TextRun;
use sui_text::TextSystem;
use swash::FontRef as SwashFontRef;
use swash::scale::Render as SwashRender;
use swash::scale::ScaleContext as SwashScaleContext;
use swash::scale::Source as SwashSource;
use swash::scale::StrikeWith as SwashStrikeWith;
use swash::scale::image::Content as SwashImageContent;
use swash::zeno::Format as SwashFormat;
use web_time::Instant;

pub(crate) struct TextEngine {
    pub(crate) system: TextSystem,
    pub(crate) glyph_cache: HashMap<GlyphCacheKey, CachedGlyphAtlas>,
    pub(crate) atlas: TextAtlasPages,
    pub(crate) swash_scale_context: SwashScaleContext,
    pub(crate) text_render_mode: TextRenderMode,
    pub(crate) text_subpixel_order: TextSubpixelOrder,
    pub(crate) text_hinting: TextHinting,
    pub(crate) stem_darkening: StemDarkening,
    pub(crate) coverage_policy: TextCoveragePolicy,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) glyph_cache_hits: usize,
    pub(crate) glyph_cache_misses: usize,
    /// Monotonic per-frame counter used to stamp atlas pages for LRU eviction.
    pub(crate) frame_counter: u64,
    #[cfg(test)]
    pub(crate) swash_face_parse_count: usize,
    pub(crate) frame_stats: TextFrameStats,
}

#[derive(Clone, Copy)]
pub(crate) struct SwashFaceState<'a> {
    pub(crate) font_ref: SwashFontRef<'a>,
    pub(crate) font_id: [u64; 2],
    pub(crate) units_per_em: f32,
}

impl<'a> SwashFaceState<'a> {
    pub(crate) fn new(face: &'a ResolvedTextFace, face_key: GlyphFaceCacheKey) -> Result<Self> {
        let face_index = usize::try_from(face.face_index())
            .map_err(|_| Error::new("text face index does not fit into usize"))?;
        let font_ref = SwashFontRef::from_index(face.bytes(), face_index)
            .ok_or_else(|| Error::new("failed to parse shaped text face data for swash"))?;
        let units_per_em = f32::from(font_ref.metrics(&[]).units_per_em.max(1));
        Ok(Self {
            font_ref,
            font_id: swash_font_id(face_key),
            units_per_em,
        })
    }

    pub(crate) fn ppem_for_scale(self, glyph_scale: f32) -> f32 {
        (glyph_scale * self.units_per_em).max(f32::EPSILON)
    }
}

pub(crate) fn swash_font_id(face_key: GlyphFaceCacheKey) -> [u64; 2] {
    let mut hasher = DefaultHasher::new();
    face_key.hash(&mut hasher);
    let primary = hasher.finish();
    let secondary = (face_key.data_ptr as u64).rotate_left(17)
        ^ (face_key.data_len as u64).rotate_left(7)
        ^ u64::from(face_key.face_index).rotate_left(31);
    [primary, secondary]
}

impl Default for TextEngine {
    fn default() -> Self {
        Self {
            system: TextSystem::new(),
            glyph_cache: HashMap::new(),
            atlas: TextAtlasPages::new(TEXT_ATLAS_WIDTH, TEXT_ATLAS_HEIGHT, TEXT_ATLAS_MAX_PAGES),
            swash_scale_context: SwashScaleContext::new(),
            text_render_mode: TextRenderMode::default(),
            text_subpixel_order: TextSubpixelOrder::default(),
            text_hinting: TextHinting::default(),
            stem_darkening: StemDarkening::default(),
            coverage_policy: TextCoveragePolicy::default(),
            diagnostics_enabled: true,
            glyph_cache_hits: 0,
            glyph_cache_misses: 0,
            frame_counter: 0,
            #[cfg(test)]
            swash_face_parse_count: 0,
            frame_stats: TextFrameStats::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedTextRenderPolicy {
    pub(crate) render_mode: TextRenderMode,
    pub(crate) subpixel_order: TextSubpixelOrder,
    pub(crate) hinting: TextHinting,
    pub(crate) stem_darkening: StemDarkening,
    pub(crate) coverage_policy: TextCoveragePolicy,
}

pub(crate) fn map_scene_text_render_mode(mode: sui_scene::TextRenderMode) -> TextRenderMode {
    match mode {
        sui_scene::TextRenderMode::Grayscale => TextRenderMode::Grayscale,
        sui_scene::TextRenderMode::LcdSubpixel => TextRenderMode::LcdSubpixel,
    }
}

pub(crate) fn map_scene_text_subpixel_order(
    order: sui_scene::TextSubpixelOrder,
) -> TextSubpixelOrder {
    match order {
        sui_scene::TextSubpixelOrder::None => TextSubpixelOrder::None,
        sui_scene::TextSubpixelOrder::Rgb => TextSubpixelOrder::Rgb,
        sui_scene::TextSubpixelOrder::Bgr => TextSubpixelOrder::Bgr,
    }
}

pub(crate) fn map_text_render_hinting(hinting: TextRenderHinting) -> TextHinting {
    match hinting.normalized() {
        TextRenderHinting::None => TextHinting::None,
        TextRenderHinting::Slight { max_ppem } => TextHinting::Slight { max_ppem },
    }
}

pub(crate) fn map_text_render_stem_darkening(darkening: TextRenderStemDarkening) -> StemDarkening {
    match darkening.normalized() {
        TextRenderStemDarkening::None => StemDarkening::None,
        TextRenderStemDarkening::Enabled { max_ppem, amount } => {
            StemDarkening::Enabled { max_ppem, amount }
        }
    }
}

pub(crate) fn map_text_render_coverage_policy(
    policy: TextRenderCoveragePolicy,
) -> TextCoveragePolicy {
    match policy.normalized() {
        TextRenderCoveragePolicy::Perceptual => TextCoveragePolicy::Perceptual,
        TextRenderCoveragePolicy::Linear => TextCoveragePolicy::Linear,
        TextRenderCoveragePolicy::Gamma(gamma) => TextCoveragePolicy::Gamma(gamma),
        TextRenderCoveragePolicy::CoverageBoost(amount) => {
            TextCoveragePolicy::CoverageBoost(amount)
        }
        TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq => {
            TextCoveragePolicy::TwoCoverageMinusCoverageSq
        }
    }
}

impl TextEngine {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self::default())
    }

    pub(crate) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
        if !enabled {
            self.frame_stats = TextFrameStats::default();
        }
    }

    pub(crate) fn set_text_render_mode(&mut self, mode: TextRenderMode) {
        self.text_render_mode = mode;
    }

    pub(crate) fn set_text_subpixel_order(&mut self, order: TextSubpixelOrder) {
        self.text_subpixel_order = order;
    }

    pub(crate) fn set_text_hinting(&mut self, hinting: TextHinting) {
        self.text_hinting = hinting.normalized();
    }

    pub(crate) fn set_stem_darkening(&mut self, darkening: StemDarkening) {
        self.stem_darkening = darkening.normalized();
    }

    pub(crate) fn set_text_coverage_policy(&mut self, policy: TextCoveragePolicy) {
        self.coverage_policy = policy.normalized();
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_stats = TextFrameStats::default();
        // New frame: advance the clock used to age atlas pages for LRU eviction.
        self.frame_counter = self.frame_counter.wrapping_add(1);
    }

    pub(crate) fn frame_stats(&self) -> TextFrameStats {
        self.frame_stats
    }

    pub(crate) fn resolved_text_render_policy(
        &self,
        override_policy: Option<TextRenderPolicy>,
    ) -> ResolvedTextRenderPolicy {
        let Some(policy) = override_policy.map(TextRenderPolicy::normalized) else {
            return ResolvedTextRenderPolicy {
                render_mode: self.text_render_mode,
                subpixel_order: self.text_subpixel_order,
                hinting: self.text_hinting,
                stem_darkening: self.stem_darkening,
                coverage_policy: self.coverage_policy,
            };
        };

        ResolvedTextRenderPolicy {
            render_mode: policy
                .render_mode
                .map(map_scene_text_render_mode)
                .unwrap_or(self.text_render_mode),
            subpixel_order: policy
                .subpixel_order
                .map(map_scene_text_subpixel_order)
                .unwrap_or(self.text_subpixel_order),
            hinting: policy
                .hinting
                .map(map_text_render_hinting)
                .unwrap_or(self.text_hinting),
            stem_darkening: policy
                .stem_darkening
                .map(map_text_render_stem_darkening)
                .unwrap_or(self.stem_darkening),
            coverage_policy: policy
                .coverage_policy
                .map(map_text_render_coverage_policy)
                .unwrap_or(self.coverage_policy),
        }
    }

    pub(crate) fn append_text_run(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        text: &TextRun,
        font_registry: &FontRegistry,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if text.rect.is_empty() || text.text.is_empty() || viewport.is_empty() {
            return Ok(());
        }

        let layout = self.shape_text_run(text, font_registry)?;
        self.append_text_layout(
            atlas_instances,
            state,
            Point::new(text.rect.x(), text.rect.y()),
            &layout,
            None,
            viewport,
            raster_scale_factor,
        )
    }

    pub(crate) fn append_shaped_text(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        text: &ShapedText,
        text_layout_registry: &sui_text::TextLayoutRegistry,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if viewport.is_empty() {
            return Ok(());
        }

        let layout = text.resolve(text_layout_registry).ok_or_else(|| {
            Error::new(format!(
                "text layout handle {} version {} is not available in the frame registry",
                text.layout_handle.get(),
                text.layout_version.get(),
            ))
        })?;

        self.append_text_layout(
            atlas_instances,
            state,
            text.origin,
            layout,
            text.color_override,
            viewport,
            raster_scale_factor,
        )
    }

    pub(crate) fn append_shaped_text_window(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        text: &sui_text::ShapedTextWindow,
        text_layout_registry: &sui_text::TextLayoutRegistry,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if viewport.is_empty() {
            return Ok(());
        }

        let layout = text.resolve(text_layout_registry).ok_or_else(|| {
            Error::new(format!(
                "text layout handle {} version {} is not available in the frame registry",
                text.layout_handle.get(),
                text.layout_version.get(),
            ))
        })?;

        self.append_text_layout_window(
            atlas_instances,
            state,
            text.origin,
            layout,
            text.line_range.clone(),
            text.color_override,
            viewport,
            raster_scale_factor,
        )
    }

    pub(crate) fn append_text_layout(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        origin: Point,
        layout: &TextLayout,
        color_override: Option<Color>,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if layout.measurement().width <= 0.0 || layout.measurement().height <= 0.0 {
            return Ok(());
        }

        let translated_bounds = layout.measurement().bounds.translate(origin.to_vector());
        if state.visible_rect(translated_bounds).is_none() {
            return Ok(());
        }

        self.append_layout_glyphs(
            atlas_instances,
            state,
            origin,
            layout.glyph_instances(),
            color_override,
            viewport,
            raster_scale_factor,
        )
    }

    pub(crate) fn append_text_layout_window(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        origin: Point,
        layout: &TextLayout,
        line_range: std::ops::Range<usize>,
        color_override: Option<Color>,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        let line_window = layout.line_window(line_range);
        if line_window.line_range.is_empty() {
            return Ok(());
        }

        let translated_bounds = line_window.bounds().translate(origin.to_vector());
        if translated_bounds.width() <= 0.0 || translated_bounds.height() <= 0.0 {
            return Ok(());
        }

        if state.visible_rect(translated_bounds).is_none() {
            return Ok(());
        }

        self.append_layout_glyphs(
            atlas_instances,
            state,
            origin,
            line_window.glyph_instances(),
            color_override,
            viewport,
            raster_scale_factor,
        )
    }

    pub(crate) fn append_layout_glyphs<'a, I>(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        origin: Point,
        glyphs: I,
        color_override: Option<Color>,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()>
    where
        I: IntoIterator<Item = sui_text::TextGlyphInstance<'a>>,
    {
        let mut active_face_index = None;
        let mut swash_face = None;
        let text_policy = self.resolved_text_render_policy(state.active_text_render_policy());
        let raster_transform = state.current_transform.then(state.text_raster_transform);
        let glyph_raster_scale = raster_scale_factor * text_transform_scale(raster_transform);
        for glyph in glyphs {
            let face_index = glyph.glyph.face_index;
            if active_face_index != Some(face_index) {
                active_face_index = Some(face_index);
                swash_face = None;
            }

            let glyph_face = glyph.face;
            let face_key = GlyphFaceCacheKey::new(glyph_face);
            let glyph_style = glyph.style;
            let glyph_color = color_override.unwrap_or(glyph_style.color);
            // Bound raster allocation during extreme canvas zoom. Larger text
            // reuses a high-resolution mask instead of attempting an unbounded
            // CPU image that cannot fit in an atlas page.
            let actual_raster_scale = glyph_raster_scale.min(
                (crate::text::TEXT_ATLAS_HEIGHT as f32 * 0.5)
                    / glyph_style.font_size.max(f32::EPSILON),
            );
            let resolution_limited = actual_raster_scale < glyph_raster_scale;
            let render_mode = if matches!(text_policy.render_mode, TextRenderMode::LcdSubpixel)
                && (matches!(text_policy.subpixel_order, TextSubpixelOrder::None)
                    || !allows_lcd_text(raster_transform)
                    || resolution_limited)
            {
                TextRenderMode::Grayscale
            } else {
                text_policy.render_mode
            };
            let subpixel_order = match render_mode {
                TextRenderMode::Grayscale => TextSubpixelOrder::None,
                TextRenderMode::LcdSubpixel => text_policy.subpixel_order,
            };
            let mut translated_glyph = glyph.glyph.clone();
            translated_glyph.origin_x += origin.x;
            translated_glyph.origin_y += origin.y;
            if let Some(bounds) = translated_glyph.bounds {
                translated_glyph.bounds = Some(bounds.translate(origin.to_vector()));
            }
            let background = translated_glyph.bounds.and_then(|bounds| {
                state
                    .text_background
                    .color_under(state.current_transform.transform_rect_bbox(bounds))
            });
            let coverage_policy = text_policy
                .coverage_policy
                .resolved_for_text_background(glyph_color, background);

            if let Some(atlas) = self.cached_glyph_primitive(
                glyph_face,
                &mut swash_face,
                face_key,
                glyph.glyph.glyph_id,
                glyph.glyph.scale,
                actual_raster_scale,
                if resolution_limited {
                    GlyphSubpixelOffsetKey::default()
                } else {
                    glyph_subpixel_offset(
                        raster_transform,
                        state.pixel_snap_offset,
                        &translated_glyph,
                        raster_scale_factor,
                    )
                },
                render_mode,
                subpixel_order,
                text_policy.hinting,
                text_policy.stem_darkening,
                glyph_style.weight.value(),
            )? && let Some(instance) = build_text_atlas_instance(
                atlas,
                &translated_glyph,
                glyph_color,
                coverage_policy,
                state.current_transform,
                state.text_raster_transform,
                state.pixel_snap_offset,
                viewport,
                raster_scale_factor,
            ) {
                atlas_instances.push(instance);
                if self.diagnostics_enabled {
                    self.frame_stats.glyph_instances += 1;
                    self.frame_stats.glyph_upload_bytes += TEXT_ATLAS_INSTANCE_SIZE;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn shape_text_run(
        &self,
        text: &TextRun,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.system.shape_text_run(text, font_registry)
    }

    pub(crate) fn cached_glyph_primitive<'face>(
        &mut self,
        face: &'face ResolvedTextFace,
        swash_face: &mut Option<SwashFaceState<'face>>,
        face_key: GlyphFaceCacheKey,
        glyph_id: u16,
        glyph_scale: f32,
        raster_scale_factor: f32,
        subpixel_offset: GlyphSubpixelOffsetKey,
        text_render_mode: TextRenderMode,
        text_subpixel_order: TextSubpixelOrder,
        text_hinting: TextHinting,
        stem_darkening: StemDarkening,
        weight: u16,
    ) -> Result<Option<&CachedGlyphAtlas>> {
        let raster_scale_factor = raster_scale_factor.max(f32::EPSILON);
        let atlas_physical_scale = glyph_scale * raster_scale_factor;
        let scale_bucket = glyph_scale_bucket(atlas_physical_scale);
        let key = GlyphCacheKey::new(
            face_key,
            glyph_id,
            scale_bucket,
            subpixel_offset,
            text_render_mode,
            text_subpixel_order,
            text_hinting,
            stem_darkening,
            weight,
        );
        // Hit: stamp the glyph's page as used this frame (for LRU) and return the cached entry.
        if self.glyph_cache.contains_key(&key) {
            if self.diagnostics_enabled {
                self.glyph_cache_hits += 1;
            }
            let page_index = self.glyph_cache[&key].page_index;
            self.atlas.touch_page(page_index, self.frame_counter);
            return Ok(self.glyph_cache.get(&key));
        }

        // Miss: rasterize and insert into the atlas (which may evict an LRU page).
        if self.diagnostics_enabled {
            self.glyph_cache_misses += 1;
        }
        if swash_face.is_none() {
            #[cfg(test)]
            {
                self.swash_face_parse_count += 1;
            }
            *swash_face = Some(SwashFaceState::new(face, face_key)?);
        }
        let swash_face = swash_face
            .as_ref()
            .expect("swash text face should be cached after initialization");
        let atlas_miss_started = self.diagnostics_enabled.then(Instant::now);
        let bucketed_physical_scale = glyph_scale_from_bucket(scale_bucket);
        let bucketed_logical_scale = bucketed_physical_scale / raster_scale_factor;
        let built = build_cached_glyph_atlas(
            &mut self.atlas,
            &mut self.swash_scale_context,
            swash_face,
            glyph_id,
            swash_face.ppem_for_scale(bucketed_physical_scale),
            raster_scale_factor,
            bucketed_logical_scale,
            subpixel_offset,
            text_render_mode,
            text_subpixel_order,
            text_hinting,
            stem_darkening,
            weight,
            self.frame_counter,
        )?;
        if let Some(started) = atlas_miss_started {
            self.frame_stats.atlas_miss_count += 1;
            self.frame_stats.atlas_miss_time_us += started.elapsed().as_micros() as u64;
        }
        let Some((primitive, evicted_page)) = built else {
            return Ok(None);
        };
        // If a page was recycled, drop every glyph that pointed into it: its atlas region -- and
        // therefore the UVs cached here -- are no longer valid.
        if let Some(evicted) = evicted_page {
            self.glyph_cache
                .retain(|_, cached| cached.page_index != evicted);
        }
        self.glyph_cache.insert(key.clone(), primitive);
        Ok(self.glyph_cache.get(&key))
    }

    /// Drain pending per-page atlas uploads, each tagged with its texture-array layer index.
    pub(crate) fn take_atlas_uploads(&mut self) -> Vec<(usize, TextAtlasUpload)> {
        self.atlas.take_uploads()
    }

    #[cfg(test)]
    pub(crate) fn glyph_cache_stats(&self) -> (usize, usize, usize) {
        (
            self.glyph_cache.len(),
            self.glyph_cache_hits,
            self.glyph_cache_misses,
        )
    }

    #[cfg(test)]
    pub(crate) fn swash_face_parse_count(&self) -> usize {
        self.swash_face_parse_count
    }

    pub(crate) fn cache_snapshot(&self) -> RendererTextCacheSnapshot {
        RendererTextCacheSnapshot {
            layout: self.system.layout_cache_snapshot(),
            glyph: GlyphCacheSnapshot {
                entries: self.glyph_cache.len(),
                hits: self.glyph_cache_hits,
                misses: self.glyph_cache_misses,
            },
            path: GlyphCacheSnapshot::default(),
        }
    }
}

pub(crate) fn build_cached_glyph_atlas(
    pages: &mut TextAtlasPages,
    scale_context: &mut SwashScaleContext,
    face: &SwashFaceState<'_>,
    glyph_id: u16,
    font_size_physical: f32,
    raster_scale_factor: f32,
    glyph_scale_logical: f32,
    subpixel_offset: GlyphSubpixelOffsetKey,
    text_render_mode: TextRenderMode,
    text_subpixel_order: TextSubpixelOrder,
    text_hinting: TextHinting,
    stem_darkening: StemDarkening,
    weight: u16,
    frame: u64,
) -> Result<Option<(CachedGlyphAtlas, Option<usize>)>> {
    let sources = [
        SwashSource::ColorOutline(0),
        SwashSource::ColorBitmap(SwashStrikeWith::BestFit),
        SwashSource::Outline,
    ];
    let mut scaler = scale_context
        .builder_with_id(face.font_ref, face.font_id)
        .size(font_size_physical)
        // Rasterize the requested weight instance to match cosmic-text's shaped advances.
        // No-op on static fonts (no `wght` axis).
        .variations([("wght", f32::from(weight))])
        .hint(text_hinting.should_hint(font_size_physical))
        .build();
    let mut renderer = SwashRender::new(&sources);
    renderer.format(match text_render_mode {
        TextRenderMode::Grayscale => SwashFormat::Alpha,
        TextRenderMode::LcdSubpixel => SwashFormat::subpixel_bgra(),
    });
    renderer.offset(subpixel_offset.as_swash_offset());
    let Some(image) = renderer.render(&mut scaler, glyph_id) else {
        return Ok(None);
    };

    let logical_offset = glyph_raster_offset(&image.placement, raster_scale_factor);

    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let pixel_count = width.saturating_mul(height);

    // A grayscale outline that rendered to a pure binary mask (jaggy) is re-rendered with
    // oversampling for true anti-aliased coverage; everything else uses the direct render.
    let needs_oversample = matches!(text_render_mode, TextRenderMode::Grayscale)
        && matches!(image.content, SwashImageContent::Mask)
        && pixel_count > 0
        && image.data.len() >= pixel_count
        && coverage_needs_oversampling(&image.data[..pixel_count], font_size_physical);

    let rasterized = if needs_oversample {
        let placement = image.placement;
        let oversampled = oversampled_mask_coverage(
            scale_context,
            face,
            glyph_id,
            font_size_physical,
            subpixel_offset,
            weight,
            &placement,
        )
        .map(|coverage| SwashRasterizedGlyph {
            pixels: mask_coverage_to_rgba(
                &coverage,
                stem_darkening.effective_amount(font_size_physical),
            ),
            is_color: false,
        });
        match oversampled.or_else(|| {
            swash_image_to_rgba(
                &image,
                font_size_physical,
                text_render_mode,
                text_subpixel_order,
                stem_darkening,
            )
        }) {
            Some(rasterized) => rasterized,
            None => return Ok(None),
        }
    } else {
        match swash_image_to_rgba(
            &image,
            font_size_physical,
            text_render_mode,
            text_subpixel_order,
            stem_darkening,
        ) {
            Some(rasterized) => rasterized,
            None => return Ok(None),
        }
    };

    if width == 0 || height == 0 {
        return Ok(Some((
            CachedGlyphAtlas {
                scale: glyph_scale_logical,
                offset: logical_offset,
                size: Size::ZERO,
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                color_mode: TextAtlasColorMode::from(text_render_mode),
                is_color: rasterized.is_color,
                page_index: 0,
            },
            None,
        )));
    }

    let insertion = match pages.insert_rgba(width, height, &rasterized.pixels, frame) {
        Ok(insertion) => insertion,
        // Too large for any page, or every page is already hot this frame: drop the glyph for
        // this frame. With on-demand page growth + LRU eviction there is no atlas-full cliff.
        Err(TextAtlasInsertError::TooLarge) | Err(TextAtlasInsertError::Full) => return Ok(None),
    };
    let page_index = insertion.page_index;
    let placement = insertion.placement;

    let atlas_size = pages.page_size();
    let inv_width = 1.0 / atlas_size.0 as f32;
    let inv_height = 1.0 / atlas_size.1 as f32;
    let logical_uv_min_x = placement.x as f32;
    let logical_uv_min_y = placement.y as f32;
    let logical_uv_max_x = logical_uv_min_x + image.placement.width as f32;
    let logical_uv_max_y = logical_uv_min_y + image.placement.height as f32;
    Ok(Some((
        CachedGlyphAtlas {
            scale: glyph_scale_logical,
            offset: logical_offset,
            size: Size::new(
                image.placement.width as f32 / raster_scale_factor,
                image.placement.height as f32 / raster_scale_factor,
            ),
            uv_min: [logical_uv_min_x * inv_width, logical_uv_min_y * inv_height],
            uv_max: [logical_uv_max_x * inv_width, logical_uv_max_y * inv_height],
            color_mode: TextAtlasColorMode::from(text_render_mode),
            is_color: rasterized.is_color,
            page_index,
        },
        insertion.evicted_page,
    )))
}

pub(crate) fn glyph_raster_offset(
    placement: &swash::zeno::Placement,
    raster_scale_factor: f32,
) -> Vector {
    Vector::new(
        placement.left as f32 / raster_scale_factor,
        -(placement.top as f32) / raster_scale_factor,
    )
}

pub(crate) struct SwashRasterizedGlyph {
    pub(crate) pixels: Vec<u8>,
    pub(crate) is_color: bool,
}

pub(crate) fn swash_image_to_rgba(
    image: &swash::scale::image::Image,
    ppem: f32,
    text_render_mode: TextRenderMode,
    text_subpixel_order: TextSubpixelOrder,
    stem_darkening: StemDarkening,
) -> Option<SwashRasterizedGlyph> {
    let width = usize::try_from(image.placement.width).ok()?;
    let height = usize::try_from(image.placement.height).ok()?;
    let pixel_count = width.checked_mul(height)?;

    let stem_darkening_amount = stem_darkening.effective_amount(ppem);

    match image.content {
        SwashImageContent::Mask => {
            if image.data.len() < pixel_count {
                return None;
            }
            // Coverage is already anti-aliased here; the binary-mask case is intercepted earlier
            // (build_cached_glyph_atlas) and re-rendered with oversampling for true coverage.
            Some(SwashRasterizedGlyph {
                pixels: mask_coverage_to_rgba(&image.data[..pixel_count], stem_darkening_amount),
                is_color: false,
            })
        }
        SwashImageContent::SubpixelMask => {
            if image.data.len() < pixel_count.checked_mul(4)? {
                return None;
            }

            let mut pixels = vec![0; pixel_count.checked_mul(4)?];
            for (source, pixel) in image.data.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
                pixel.copy_from_slice(&convert_subpixel_texel_for_mode(
                    [source[0], source[1], source[2], source[3]],
                    text_render_mode,
                    text_subpixel_order,
                    stem_darkening_amount,
                ));
            }

            Some(SwashRasterizedGlyph {
                pixels,
                is_color: false,
            })
        }
        SwashImageContent::Color => {
            let bytes = pixel_count.checked_mul(4)?;
            if image.data.len() < bytes {
                return None;
            }
            // Store the glyph's sRGB color verbatim; the fragment shader linearizes at full float
            // precision, avoiding the dark-tone banding of 8-bit linear storage.
            Some(SwashRasterizedGlyph {
                pixels: image.data[..bytes].to_vec(),
                is_color: true,
            })
        }
    }
}

pub(crate) fn convert_subpixel_texel_for_mode(
    source: [u8; 4],
    text_render_mode: TextRenderMode,
    text_subpixel_order: TextSubpixelOrder,
    stem_darkening_amount: f32,
) -> [u8; 4] {
    match text_render_mode {
        TextRenderMode::Grayscale => {
            let coverage =
                ((u16::from(source[0]) + u16::from(source[1]) + u16::from(source[2])) / 3) as u8;
            let coverage = apply_stem_darkening_to_coverage(coverage, stem_darkening_amount);
            [255, 255, 255, coverage]
        }
        TextRenderMode::LcdSubpixel => {
            let [red_source, green_source, blue_source] = match text_subpixel_order {
                TextSubpixelOrder::None => {
                    let coverage =
                        ((u16::from(source[0]) + u16::from(source[1]) + u16::from(source[2])) / 3)
                            as u8;
                    let coverage =
                        apply_stem_darkening_to_coverage(coverage, stem_darkening_amount);
                    return [255, 255, 255, coverage];
                }
                TextSubpixelOrder::Rgb => [source[2], source[1], source[0]],
                TextSubpixelOrder::Bgr => [source[0], source[1], source[2]],
            };
            let red = apply_stem_darkening_to_coverage(red_source, stem_darkening_amount);
            let green = apply_stem_darkening_to_coverage(green_source, stem_darkening_amount);
            let blue = apply_stem_darkening_to_coverage(blue_source, stem_darkening_amount);
            let alpha = red.max(green).max(blue);
            [red, green, blue, alpha]
        }
    }
}

/// Boost the coverage of partially-covered pixels (thin stems, antialiased edges) so small text
/// reads heavier. The boost is gated by `coverage` itself, so a fully-transparent pixel
/// (coverage 0) stays fully transparent and a fully-covered pixel (coverage 1) stays solid;
/// only the partial-coverage range in between is lifted. Without the `coverage` factor the old
/// formula mapped 0 -> `amount`, flooding every glyph cell's transparent background with
/// `amount` opacity and painting a gray box behind each glyph.
pub(crate) fn apply_stem_darkening_to_coverage(coverage: u8, amount: f32) -> u8 {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= f32::EPSILON {
        return coverage;
    }

    let coverage = coverage as f32 / 255.0;
    let darkened = coverage + (coverage * (1.0 - coverage) * amount);
    (darkened.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn mask_coverage_to_rgba(coverage: &[u8], stem_darkening_amount: f32) -> Vec<u8> {
    let mut pixels = vec![0u8; coverage.len() * 4];
    for (value, pixel) in coverage.iter().zip(pixels.chunks_exact_mut(4)) {
        let value = apply_stem_darkening_to_coverage(*value, stem_darkening_amount);
        pixel[0] = 255;
        pixel[1] = 255;
        pixel[2] = 255;
        pixel[3] = value;
    }
    pixels
}

pub(crate) fn is_binary_coverage(data: &[u8]) -> bool {
    !data.is_empty() && data.iter().all(|&value| value == 0 || value == 255)
}

pub(crate) fn coverage_needs_oversampling(data: &[u8], ppem: f32) -> bool {
    if is_binary_coverage(data) {
        return true;
    }
    if data.is_empty() || !ppem.is_finite() || ppem > 18.0 {
        return false;
    }

    let mut partial_values = [false; 256];
    let mut partial_value_count = 0usize;
    let mut partial_pixel_count = 0usize;
    let mut covered_pixel_count = 0usize;
    for &value in data {
        if value > 0 {
            covered_pixel_count += 1;
        }
        if value > 0 && value < 255 {
            partial_pixel_count += 1;
            let slot = &mut partial_values[value as usize];
            if !*slot {
                *slot = true;
                partial_value_count += 1;
            }
        }
    }

    covered_pixel_count >= 4
        && partial_pixel_count > 0
        && partial_value_count <= 2
        && partial_pixel_count * 5 <= covered_pixel_count * 2
}

/// Area-average a `factor`x oversampled coverage mask (`src`) down onto the 1x pixel grid defined
/// by the target placement, preserving the target's exact origin and dimensions. Oversample
/// samples that fall outside `src` count as zero (transparent outside the glyph), so each output
/// pixel is the true fractional coverage over its `factor` x `factor` footprint.
pub(crate) fn downsample_to_target(
    src: &[u8],
    src_width: usize,
    src_height: usize,
    src_left: i32,
    src_top: i32,
    dst_width: usize,
    dst_height: usize,
    dst_left: i32,
    dst_top: i32,
    factor: i32,
) -> Vec<u8> {
    let mut out = vec![0u8; dst_width * dst_height];
    // The glyph sits at factor x the 1x position, so 1x pixel p maps to oversample pixels
    // [p*factor, (p+1)*factor); align that span into src-local coordinates.
    let delta_x = dst_left * factor - src_left;
    let delta_y = dst_top * factor - src_top;
    let samples_per_pixel = (factor * factor).max(1) as u32;
    for j in 0..dst_height {
        let base_y = delta_y + j as i32 * factor;
        for i in 0..dst_width {
            let base_x = delta_x + i as i32 * factor;
            let mut sum = 0u32;
            for sy in 0..factor {
                let oy = base_y + sy;
                if oy < 0 || oy as usize >= src_height {
                    continue;
                }
                let row = oy as usize * src_width;
                for sx in 0..factor {
                    let ox = base_x + sx;
                    if ox < 0 || ox as usize >= src_width {
                        continue;
                    }
                    sum += u32::from(src[row + ox as usize]);
                }
            }
            out[j * dst_width + i] = (sum / samples_per_pixel) as u8;
        }
    }
    out
}

/// Re-render a glyph's coverage at OVERSAMPLE x and area-average it back onto the 1x grid defined
/// by `target`. Recovers true anti-aliased coverage for glyphs whose 1x render came out as a pure
/// binary mask (jaggy small or heavily-hinted outlines). Returns coverage at `target`'s exact
/// dimensions so glyph placement is unchanged.
pub(crate) fn oversampled_mask_coverage(
    scale_context: &mut SwashScaleContext,
    face: &SwashFaceState<'_>,
    glyph_id: u16,
    font_size_physical: f32,
    subpixel_offset: GlyphSubpixelOffsetKey,
    weight: u16,
    target: &swash::zeno::Placement,
) -> Option<Vec<u8>> {
    const OVERSAMPLE: i32 = 4;
    let sources = [SwashSource::Outline];
    let mut scaler = scale_context
        .builder_with_id(face.font_ref, face.font_id)
        .size(font_size_physical * OVERSAMPLE as f32)
        .variations([("wght", f32::from(weight))])
        .hint(false)
        .build();
    let mut renderer = SwashRender::new(&sources);
    renderer.format(SwashFormat::Alpha);
    // Same fractional sub-pixel position as the 1x render, expressed in oversample pixels.
    let offset = subpixel_offset.as_swash_offset();
    renderer.offset(swash::zeno::Vector::new(
        offset.x * OVERSAMPLE as f32,
        offset.y * OVERSAMPLE as f32,
    ));
    let image = renderer.render(&mut scaler, glyph_id)?;
    if !matches!(image.content, SwashImageContent::Mask) {
        return None;
    }
    Some(downsample_to_target(
        &image.data,
        image.placement.width as usize,
        image.placement.height as usize,
        image.placement.left,
        image.placement.top,
        target.width as usize,
        target.height as usize,
        target.left,
        target.top,
        OVERSAMPLE,
    ))
}

#[cfg(test)]
pub(crate) mod coverage_tests {
    use super::*;

    #[test]
    fn downsample_all_covered_is_full() {
        // A 4x4 fully-covered oversample block becomes one fully-covered 1x pixel.
        let src = vec![255u8; 16];
        let out = downsample_to_target(&src, 4, 4, 0, 0, 1, 1, 0, 0, 4);
        assert_eq!(out, vec![255]);
    }

    #[test]
    fn downsample_half_covered_is_half() {
        // Top two oversample rows covered, bottom two empty -> ~50% coverage.
        let mut src = vec![0u8; 16];
        for value in src.iter_mut().take(8) {
            *value = 255;
        }
        let out = downsample_to_target(&src, 4, 4, 0, 0, 1, 1, 0, 0, 4);
        assert_eq!(out, vec![127]); // 8*255 / 16 = 127 (integer division)
    }

    #[test]
    fn downsample_preserves_target_dimensions() {
        // 8x4 oversample -> 2x1 target; only the left 4x4 block is covered.
        let mut src = vec![0u8; 32];
        for y in 0..4 {
            for x in 0..4 {
                src[y * 8 + x] = 255;
            }
        }
        let out = downsample_to_target(&src, 8, 4, 0, 0, 2, 1, 0, 0, 4);
        assert_eq!(out, vec![255, 0]);
    }

    #[test]
    fn downsample_aligns_via_placement_delta() {
        // Target pixel at 1x x=1 covers oversample [4,8); src begins at global x=4, so its local
        // [0,4) maps exactly onto that pixel. The delta term must absorb src_left vs dst_left*4.
        let src = vec![255u8; 16];
        let out = downsample_to_target(&src, 4, 4, 4, 0, 1, 1, 1, 0, 4);
        assert_eq!(out, vec![255]);
    }

    #[test]
    fn binary_coverage_detection() {
        assert!(is_binary_coverage(&[0, 255, 0, 255]));
        assert!(!is_binary_coverage(&[0, 128, 255]));
        assert!(!is_binary_coverage(&[]));
    }

    #[test]
    fn oversampling_detection_includes_sparse_small_ppem_partial_masks() {
        assert!(coverage_needs_oversampling(
            &[0, 255, 255, 255, 64, 255, 0, 0],
            12.0
        ));
    }

    #[test]
    fn oversampling_detection_keeps_rich_antialiasing_direct() {
        assert!(!coverage_needs_oversampling(
            &[0, 32, 64, 96, 128, 160, 192, 224, 255],
            12.0
        ));
    }

    #[test]
    fn oversampling_detection_keeps_large_ppem_partial_masks_direct() {
        assert!(!coverage_needs_oversampling(
            &[0, 255, 255, 255, 64, 255, 0, 0],
            24.0
        ));
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlyphRasterBounds {
    pub(crate) logical_min_x: f32,
    pub(crate) logical_min_y: f32,
    pub(crate) logical_width: f32,
    pub(crate) logical_height: f32,
    pub(crate) raster_min_x: f32,
    pub(crate) raster_min_y: f32,
    pub(crate) raster_width: usize,
    pub(crate) raster_height: usize,
}

#[cfg(test)]
pub(crate) fn glyph_raster_bounds(path: &tiny_skia::Path) -> Option<GlyphRasterBounds> {
    let bounds = path.bounds().to_non_zero_rect()?;
    let logical_min_x = bounds.x();
    let logical_min_y = bounds.y();
    let logical_width = bounds.width();
    let logical_height = bounds.height();
    let raster_min_x = logical_min_x.floor();
    let raster_min_y = logical_min_y.floor();
    let raster_max_x = (logical_min_x + logical_width).ceil();
    let raster_max_y = (logical_min_y + logical_height).ceil();
    let raster_width = (raster_max_x - raster_min_x).max(0.0) as usize;
    let raster_height = (raster_max_y - raster_min_y).max(0.0) as usize;
    Some(GlyphRasterBounds {
        logical_min_x,
        logical_min_y,
        logical_width,
        logical_height,
        raster_min_x,
        raster_min_y,
        raster_width,
        raster_height,
    })
}

#[cfg(test)]
pub(crate) fn append_cached_glyph_atlas(
    vertices: &mut Vec<Vertex>,
    atlas: &CachedGlyphAtlas,
    glyph: &SceneShapedGlyph,
    color: Color,
    transform: Transform,
    viewport: Size,
    raster_scale_factor: f32,
) {
    if let Some(instance) = build_text_atlas_instance(
        atlas,
        glyph,
        color,
        TextCoveragePolicy::Linear,
        transform,
        Transform::IDENTITY,
        Vector::ZERO,
        viewport,
        raster_scale_factor,
    ) {
        append_text_instance_vertices(vertices, std::slice::from_ref(&instance));
    }
}

pub(crate) fn build_text_atlas_instance(
    atlas: &CachedGlyphAtlas,
    glyph: &SceneShapedGlyph,
    color: Color,
    coverage_policy: TextCoveragePolicy,
    transform: Transform,
    inherited_transform: Transform,
    pixel_snap_offset: Vector,
    viewport: Size,
    raster_scale_factor: f32,
) -> Option<TextAtlasInstance> {
    if atlas.size.is_empty() || viewport.is_empty() {
        return None;
    }

    let rgba = if atlas.is_color {
        [1.0, 1.0, 1.0, -color.clamped().alpha]
    } else {
        shader_color(color)
    };
    let residual_scale = glyph.scale / atlas.scale.max(f32::EPSILON);
    let left = glyph.origin_x + (atlas.offset.x * residual_scale);
    let top = glyph.origin_y + (atlas.offset.y * residual_scale);
    let width = atlas.size.width * residual_scale;
    let height = atlas.size.height * residual_scale;
    let raster_transform = transform.then(inherited_transform);
    let (top_left, top_right, bottom_left, bottom_right) = snapped_glyph_quad(
        raster_transform,
        pixel_snap_offset,
        Point::new(glyph.origin_x, glyph.origin_y),
        Rect::new(left, top, width, height),
        raster_scale_factor,
    );

    let inverse = inherited_transform.inverse()?;
    let to_local_ndc = |point: Point| {
        let local = inverse.transform_point(point);
        to_ndc(local.x, local.y, viewport)
    };
    let top_left = to_local_ndc(top_left);
    let top_right = to_local_ndc(top_right);
    let bottom_left = to_local_ndc(bottom_left);
    let _bottom_right = to_local_ndc(bottom_right);

    let atlas_contains_lcd_subpixels = matches!(atlas.color_mode, TextAtlasColorMode::LcdSubpixel);
    let (coverage_policy_kind, coverage_policy_parameter) =
        coverage_policy_shader_metadata(coverage_policy);

    Some(TextAtlasInstance {
        top_left,
        x_axis: [top_right[0] - top_left[0], top_right[1] - top_left[1]],
        y_axis: [bottom_left[0] - top_left[0], bottom_left[1] - top_left[1]],
        uv_min: atlas.uv_min.map(pack_unorm16),
        uv_max: atlas.uv_max.map(pack_unorm16),
        color: rgba,
        coverage_flags: [
            (atlas_contains_lcd_subpixels && allows_lcd_text(raster_transform)) as u8,
            atlas_contains_lcd_subpixels as u8,
            coverage_policy_kind.round().clamp(0.0, u8::MAX as f32) as u8,
            0,
        ],
        coverage_parameter: coverage_policy_parameter,
        layer: atlas.page_index as u32,
    })
}

pub(crate) fn pack_unorm16(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * u16::MAX as f32).round() as u16
}

#[cfg(test)]
pub(crate) fn unpack_unorm16(value: u16) -> f32 {
    value as f32 / u16::MAX as f32
}

pub(crate) fn coverage_policy_shader_metadata(policy: TextCoveragePolicy) -> (f32, f32) {
    match policy.normalized() {
        TextCoveragePolicy::Perceptual => {
            coverage_policy_shader_metadata(TextCoveragePolicy::PerceptualLuminance {
                text: 0.0,
                background: 1.0,
            })
        }
        TextCoveragePolicy::PerceptualLuminance { text, background } => {
            // Two 12-bit luminances fit exactly in the f32 instance parameter.
            (
                4.0,
                (text * 4095.0).round() * 4096.0 + (background * 4095.0).round(),
            )
        }
        TextCoveragePolicy::Linear => (0.0, 0.0),
        TextCoveragePolicy::Gamma(gamma) => (1.0, gamma),
        TextCoveragePolicy::CoverageBoost(amount) => (2.0, amount),
        TextCoveragePolicy::TwoCoverageMinusCoverageSq => (3.0, 0.0),
    }
}

pub(crate) fn allows_lcd_text(transform: Transform) -> bool {
    transform_is_lcd_safe(transform)
}

/// Largest singular value of the linear transform: enough raster resolution
/// for either axis, including rotation, nonuniform scaling, and shear.
pub(crate) fn text_transform_scale(transform: Transform) -> f32 {
    let [xx, xy, yx, yy] = [transform.xx, transform.xy, transform.yx, transform.yy].map(f64::from);
    let a = xx * xx + yx * yx;
    let b = xy * xy + yy * yy;
    let cross = xx * xy + yx * yy;
    let scale = ((a + b + (a - b).hypot(2.0 * cross)) * 0.5).sqrt();
    if scale.is_finite() {
        (scale as f32).max(f32::EPSILON)
    } else {
        1.0
    }
}

fn text_subpixel_variants_x(transform: Transform) -> u8 {
    // A raster texel maps to one physical pixel only under positive uniform
    // scaling. Other transforms use whole-pixel origins without a phase that
    // would be stretched or mirrored along with the mask.
    if transform.xx > 0.0 && (transform.xx - transform.yy).abs() <= 1e-5 * transform.xx {
        GLYPH_SUBPIXEL_VARIANTS_X
    } else {
        1
    }
}

pub(crate) fn glyph_subpixel_offset(
    transform: Transform,
    pixel_snap_offset: Vector,
    glyph: &SceneShapedGlyph,
    raster_scale_factor: f32,
) -> GlyphSubpixelOffsetKey {
    if !transform_is_axis_aligned(transform) || raster_scale_factor <= 0.0 {
        return GlyphSubpixelOffsetKey::default();
    }

    let origin =
        transform.transform_point(Point::new(glyph.origin_x, glyph.origin_y)) + pixel_snap_offset;
    GlyphSubpixelOffsetKey::new(
        physical_pixel_phase(
            origin.x * raster_scale_factor,
            text_subpixel_variants_x(transform),
        )
        .variant,
        physical_pixel_phase(origin.y * raster_scale_factor, GLYPH_SUBPIXEL_VARIANTS_Y).variant,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PhysicalPixelPhase {
    pub(crate) integer: f32,
    pub(crate) variant: u8,
}

pub(crate) fn physical_pixel_phase(physical_position: f32, variants: u8) -> PhysicalPixelPhase {
    // Equivalent DPI/font/scene scale products can land a few ULPs on either
    // side of a phase tie. Keep those equivalent placements in the same bin.
    let round_phase =
        |value: f32| (value + value.signum() * (4.0 * f32::EPSILON * value.abs().max(1.0))).round();
    if variants <= 1 {
        return PhysicalPixelPhase {
            integer: round_phase(physical_position),
            variant: 0,
        };
    }

    let variants_i32 = i32::from(variants);
    let rounded = round_phase(physical_position * f32::from(variants)) as i32;
    let variant = rounded.rem_euclid(variants_i32) as u8;
    let integer = (rounded - i32::from(variant)) as f32 / f32::from(variants);

    PhysicalPixelPhase { integer, variant }
}

#[cfg(test)]
pub(crate) fn append_text_instance_vertices(
    vertices: &mut Vec<Vertex>,
    instances: &[TextAtlasInstance],
) {
    for instance in instances {
        let uv_min = instance.uv_min.map(unpack_unorm16);
        let uv_max = instance.uv_max.map(unpack_unorm16);
        let top_left = instance.top_left;
        let top_right = [
            instance.top_left[0] + instance.x_axis[0],
            instance.top_left[1] + instance.x_axis[1],
        ];
        let bottom_left = [
            instance.top_left[0] + instance.y_axis[0],
            instance.top_left[1] + instance.y_axis[1],
        ];
        let bottom_right = [
            top_right[0] + instance.y_axis[0],
            top_right[1] + instance.y_axis[1],
        ];
        vertices.extend_from_slice(&[
            Vertex::basic(top_left, instance.color, uv_min, [0.0; 4]),
            Vertex::basic(top_right, instance.color, [uv_max[0], uv_min[1]], [0.0; 4]),
            Vertex::basic(
                bottom_left,
                instance.color,
                [uv_min[0], uv_max[1]],
                [0.0; 4],
            ),
            Vertex::basic(
                bottom_left,
                instance.color,
                [uv_min[0], uv_max[1]],
                [0.0; 4],
            ),
            Vertex::basic(top_right, instance.color, [uv_max[0], uv_min[1]], [0.0; 4]),
            Vertex::basic(bottom_right, instance.color, uv_max, [0.0; 4]),
        ]);
    }
}

pub(crate) fn snapped_glyph_quad(
    transform: Transform,
    pixel_snap_offset: Vector,
    glyph_origin: Point,
    rect: Rect,
    raster_scale_factor: f32,
) -> (Point, Point, Point, Point) {
    let transformed_origin = transform.transform_point(glyph_origin);
    let top_left = transform.transform_point(rect.origin);
    let top_right = transform.transform_point(Point::new(rect.max_x(), rect.y()));
    let bottom_left = transform.transform_point(Point::new(rect.x(), rect.max_y()));
    let bottom_right = transform.transform_point(Point::new(rect.max_x(), rect.max_y()));

    if !transform_is_axis_aligned(transform) || raster_scale_factor <= 0.0 {
        return (top_left, top_right, bottom_left, bottom_right);
    }

    let snap_origin = transformed_origin + pixel_snap_offset;
    let snapped_origin_x = physical_pixel_phase(
        snap_origin.x * raster_scale_factor,
        text_subpixel_variants_x(transform),
    )
    .integer
        / raster_scale_factor
        - pixel_snap_offset.x;
    let snapped_origin_y = physical_pixel_phase(
        snap_origin.y * raster_scale_factor,
        GLYPH_SUBPIXEL_VARIANTS_Y,
    )
    .integer
        / raster_scale_factor
        - pixel_snap_offset.y;
    let snapped_left = snapped_origin_x + (top_left.x - transformed_origin.x);
    let snapped_top = snapped_origin_y + (top_left.y - transformed_origin.y);
    let width = top_right.x - top_left.x;
    let height = bottom_left.y - top_left.y;

    (
        Point::new(snapped_left, snapped_top),
        Point::new(snapped_left + width, snapped_top),
        Point::new(snapped_left, snapped_top + height),
        Point::new(snapped_left + width, snapped_top + height),
    )
}

pub(crate) fn transform_is_axis_aligned(transform: Transform) -> bool {
    transform.xy.abs() <= f32::EPSILON && transform.yx.abs() <= f32::EPSILON
}

pub(crate) fn transform_is_lcd_safe(transform: Transform) -> bool {
    transform_is_axis_aligned(transform) && transform.xx > 0.0 && transform.yy > 0.0
}
