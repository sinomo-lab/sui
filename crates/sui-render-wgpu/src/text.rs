use crate::geometry::hash_normalized_analytic_path;
use crate::geometry::hash_path;
use crate::text_policy::StemDarkening;
use crate::text_policy::TextHinting;
use crate::text_policy::TextRenderMode;
use std::hash::Hash;
use sui_core::Path as ScenePath;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;
use sui_scene::StrokeStyle;
pub use sui_scene::TextSubpixelOrder;
use sui_text::ResolvedTextFace;
use sui_text::TextLayoutCacheSnapshot;

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
pub(crate) struct TextFrameStats {
    pub(crate) glyph_instances: usize,
    pub(crate) glyph_upload_bytes: u64,
    pub(crate) atlas_miss_count: usize,
    pub(crate) atlas_miss_time_us: u64,
}

pub(crate) const TEXT_ATLAS_WIDTH: usize = 2048;
pub(crate) const TEXT_ATLAS_HEIGHT: usize = 2048;
pub(crate) const TEXT_ATLAS_PADDING: usize = 2;
/// Maximum number of atlas pages (texture-array layers) before whole-page LRU eviction kicks in.
/// Each page is TEXT_ATLAS_WIDTH x TEXT_ATLAS_HEIGHT x 4 bytes (~16 MB); 4 pages -> ~64 MB.
pub(crate) const TEXT_ATLAS_MAX_PAGES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlyphFaceCacheKey {
    pub(crate) data_ptr: usize,
    pub(crate) data_len: usize,
    pub(crate) face_index: u32,
}

impl GlyphFaceCacheKey {
    pub(crate) fn new(face: &ResolvedTextFace) -> Self {
        Self {
            data_ptr: face.data_ptr(),
            data_len: face.data_len(),
            face_index: face.face_index(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextAtlasColorMode {
    Grayscale,
    LcdSubpixel,
}

impl From<TextRenderMode> for TextAtlasColorMode {
    fn from(value: TextRenderMode) -> Self {
        match value {
            TextRenderMode::Grayscale => Self::Grayscale,
            TextRenderMode::LcdSubpixel => Self::LcdSubpixel,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GlyphCacheKey {
    pub(crate) face: GlyphFaceCacheKey,
    pub(crate) glyph_id: u16,
    pub(crate) scale_bucket: u32,
    pub(crate) subpixel_offset: GlyphSubpixelOffsetKey,
    pub(crate) atlas_color_mode: TextAtlasColorMode,
    pub(crate) subpixel_order: TextSubpixelOrderCacheKey,
    pub(crate) text_hinting: TextHintingCacheKey,
    pub(crate) hinting_target: GlyphHintingTarget,
    pub(crate) stem_darkening: StemDarkeningCacheKey,
    /// Requested `wght` axis value — different weights of a variable font rasterize to distinct
    /// outlines and must cache separately. (Static fonts ignore the axis, so this is constant.)
    pub(crate) weight: u16,
}

impl GlyphCacheKey {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        face: GlyphFaceCacheKey,
        glyph_id: u16,
        scale_bucket: u32,
        subpixel_offset: GlyphSubpixelOffsetKey,
        text_render_mode: TextRenderMode,
        text_subpixel_order: TextSubpixelOrder,
        text_hinting: TextHinting,
        stem_darkening: StemDarkening,
        weight: u16,
    ) -> Self {
        Self {
            face,
            glyph_id,
            scale_bucket,
            subpixel_offset,
            atlas_color_mode: TextAtlasColorMode::from(text_render_mode),
            subpixel_order: TextSubpixelOrderCacheKey::from(text_subpixel_order),
            text_hinting: TextHintingCacheKey::from(text_hinting),
            hinting_target: match text_hinting.normalized() {
                TextHinting::None => GlyphHintingTarget::None,
                TextHinting::Slight { .. } => GlyphHintingTarget::Symmetric,
            },
            stem_darkening: StemDarkeningCacheKey::from(stem_darkening),
            weight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextSubpixelOrderCacheKey {
    None,
    Rgb,
    Bgr,
}

impl From<TextSubpixelOrder> for TextSubpixelOrderCacheKey {
    fn from(value: TextSubpixelOrder) -> Self {
        match value {
            TextSubpixelOrder::None => Self::None,
            TextSubpixelOrder::Rgb => Self::Rgb,
            TextSubpixelOrder::Bgr => Self::Bgr,
        }
    }
}

pub(crate) const GLYPH_SUBPIXEL_VARIANTS_X: u8 = 4;
pub(crate) const GLYPH_SUBPIXEL_VARIANTS_Y: u8 = 1;

/// Zeno translates outlines, whereas physical subpixel positions translate
/// sample locations. A BGRA mask therefore needs the opposite offsets: blue
/// samples to the right, red to the left. Do not infer this from format names.
pub(crate) fn lcd_bgra_format() -> swash::zeno::Format {
    swash::zeno::Format::CustomSubpixel([-1.0 / 3.0, 0.0, 1.0 / 3.0])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GlyphHintingTarget {
    None,
    Symmetric,
    Asymmetric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct GlyphSubpixelOffsetKey {
    pub(crate) x: u8,
    pub(crate) y: u8,
}

impl GlyphSubpixelOffsetKey {
    pub(crate) fn new(x: u8, _y: u8) -> Self {
        Self {
            x: x % GLYPH_SUBPIXEL_VARIANTS_X,
            y: 0,
        }
    }

    pub(crate) fn as_swash_offset(self) -> swash::zeno::Vector {
        swash::zeno::Vector::new(
            f32::from(self.x) / f32::from(GLYPH_SUBPIXEL_VARIANTS_X),
            f32::from(self.y) / f32::from(GLYPH_SUBPIXEL_VARIANTS_Y),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextHintingCacheKey {
    None,
    Slight { max_ppem_bits: u32 },
}

impl From<TextHinting> for TextHintingCacheKey {
    fn from(value: TextHinting) -> Self {
        match value.normalized() {
            TextHinting::None => Self::None,
            TextHinting::Slight { max_ppem } => Self::Slight {
                max_ppem_bits: max_ppem.to_bits(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StemDarkeningCacheKey {
    None,
    Enabled {
        max_ppem_bits: u32,
        amount_bits: u32,
    },
}

impl From<StemDarkening> for StemDarkeningCacheKey {
    fn from(value: StemDarkening) -> Self {
        match value.normalized() {
            StemDarkening::None => Self::None,
            StemDarkening::Enabled { max_ppem, amount } => Self::Enabled {
                max_ppem_bits: max_ppem.to_bits(),
                amount_bits: amount.to_bits(),
            },
        }
    }
}

pub(crate) const GLYPH_SCALE_BUCKETS_PER_UNIT: f32 = 16_384.0;

pub(crate) fn glyph_scale_bucket(scale: f32) -> u32 {
    ((scale.max(f32::EPSILON) * GLYPH_SCALE_BUCKETS_PER_UNIT)
        .round()
        .max(1.0)) as u32
}

pub(crate) fn glyph_scale_from_bucket(bucket: u32) -> f32 {
    (bucket.max(1) as f32) / GLYPH_SCALE_BUCKETS_PER_UNIT
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CachedGlyphAtlas {
    pub(crate) scale: f32,
    pub(crate) offset: Vector,
    pub(crate) size: Size,
    pub(crate) uv_min: [f32; 2],
    pub(crate) uv_max: [f32; 2],
    pub(crate) color_mode: TextAtlasColorMode,
    pub(crate) is_color: bool,
    /// Which atlas page (texture-array layer) this glyph was rasterized into.
    pub(crate) page_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AtlasRectU {
    pub(crate) min_x: usize,
    pub(crate) min_y: usize,
    pub(crate) max_x: usize,
    pub(crate) max_y: usize,
}

impl AtlasRectU {
    const NOTHING: Self = Self {
        min_x: usize::MAX,
        min_y: usize::MAX,
        max_x: 0,
        max_y: 0,
    };

    pub(crate) fn include_rect(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x + width);
        self.max_y = self.max_y.max(y + height);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextAtlasPlacement {
    pub(crate) x: usize,
    pub(crate) y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextAtlasInsertError {
    Full,
    TooLarge,
}

/// A pending CPU->GPU copy for one atlas page: the dirty rectangle (`offset`/`extent`) and its
/// pixels. The destination page (texture-array layer) is supplied alongside by `take_uploads`.
#[derive(Debug, Clone)]
pub(crate) struct TextAtlasUpload {
    pub(crate) offset: (u32, u32),
    pub(crate) extent: (u32, u32),
    pub(crate) pixels: Vec<u8>,
    pub(crate) clear_texture: bool,
}

/// A single atlas page: a CPU-side pixel buffer with a shelf-packing cursor and a dirty
/// rectangle for incremental GPU uploads. The multi-page atlas ([`TextAtlasPages`]) holds a
/// collection of these.
#[derive(Debug, Clone)]
pub(crate) struct TextAtlas {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
    pub(crate) dirty: AtlasRectU,
    pub(crate) clear_texture: bool,
    pub(crate) generation: u64,
    pub(crate) cursor: (usize, usize),
    pub(crate) row_height: usize,
    /// Frame index of the most recent insert/touch, used for whole-page LRU eviction.
    pub(crate) last_used_frame: u64,
}

impl Default for TextAtlas {
    fn default() -> Self {
        Self::new(TEXT_ATLAS_WIDTH, TEXT_ATLAS_HEIGHT)
    }
}

impl TextAtlas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
            dirty: AtlasRectU::NOTHING,
            clear_texture: false,
            generation: 1,
            cursor: (TEXT_ATLAS_PADDING, TEXT_ATLAS_PADDING),
            row_height: 0,
            last_used_frame: 0,
        }
    }

    /// Reset this page so its space can be recycled (used when a page is evicted). Zeroes the
    /// CPU pixels and GPU page are cleared separately; only new glyphs need uploading.
    pub(crate) fn clear_for_reuse(&mut self) {
        self.pixels.iter_mut().for_each(|byte| *byte = 0);
        self.cursor = (TEXT_ATLAS_PADDING, TEXT_ATLAS_PADDING);
        self.row_height = 0;
        self.dirty = AtlasRectU::NOTHING;
        self.clear_texture = true;
        self.generation = self.generation.wrapping_add(1);
        self.last_used_frame = 0;
    }

    pub(crate) fn allocate(
        &mut self,
        width: usize,
        height: usize,
    ) -> std::result::Result<TextAtlasPlacement, TextAtlasInsertError> {
        if width == 0 || height == 0 || width > self.width || height > self.height {
            return Err(TextAtlasInsertError::TooLarge);
        }

        if self.cursor.0 + width + TEXT_ATLAS_PADDING > self.width {
            self.cursor.0 = TEXT_ATLAS_PADDING;
            self.cursor.1 += self.row_height + TEXT_ATLAS_PADDING;
            self.row_height = 0;
        }

        if self.cursor.1 + height + TEXT_ATLAS_PADDING > self.height {
            return Err(TextAtlasInsertError::Full);
        }

        let placement = TextAtlasPlacement {
            x: self.cursor.0,
            y: self.cursor.1,
        };
        self.cursor.0 += width + TEXT_ATLAS_PADDING;
        self.row_height = self.row_height.max(height);
        Ok(placement)
    }

    pub(crate) fn write_rgba(
        &mut self,
        placement: TextAtlasPlacement,
        width: usize,
        height: usize,
        pixels: &[u8],
    ) {
        for row in 0..height {
            let src_start = row * width * 4;
            let src_end = src_start + (width * 4);
            let dst_start = ((placement.y + row) * self.width + placement.x) * 4;
            let dst_end = dst_start + (width * 4);
            self.pixels[dst_start..dst_end].copy_from_slice(&pixels[src_start..src_end]);
        }
        self.dirty
            .include_rect(placement.x, placement.y, width, height);
    }

    pub(crate) fn insert_rgba(
        &mut self,
        width: usize,
        height: usize,
        pixels: &[u8],
    ) -> std::result::Result<TextAtlasPlacement, TextAtlasInsertError> {
        let placement = self.allocate(width, height)?;
        self.write_rgba(placement, width, height, pixels);
        Ok(placement)
    }

    pub(crate) fn take_upload(&mut self) -> Option<TextAtlasUpload> {
        let dirty = std::mem::replace(&mut self.dirty, AtlasRectU::NOTHING);
        let clear_texture = std::mem::take(&mut self.clear_texture);
        if dirty == AtlasRectU::NOTHING {
            return clear_texture.then(|| TextAtlasUpload {
                offset: (0, 0),
                extent: (0, 0),
                pixels: Vec::new(),
                clear_texture,
            });
        }

        let width = dirty.max_x - dirty.min_x;
        let height = dirty.max_y - dirty.min_y;
        let mut pixels = vec![0; width * height * 4];
        for row in 0..height {
            let src_start = ((dirty.min_y + row) * self.width + dirty.min_x) * 4;
            let src_end = src_start + (width * 4);
            let dst_start = row * width * 4;
            let dst_end = dst_start + (width * 4);
            pixels[dst_start..dst_end].copy_from_slice(&self.pixels[src_start..src_end]);
        }

        Some(TextAtlasUpload {
            offset: (dirty.min_x as u32, dirty.min_y as u32),
            extent: (width as u32, height as u32),
            pixels,
            clear_texture,
        })
    }
}

/// Result of inserting a glyph into the multi-page atlas: where it landed, plus the page that was
/// cleared to make room (if any) so the caller can drop the glyph-cache entries that pointed into
/// the now-recycled page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextAtlasInsertion {
    pub(crate) page_index: usize,
    pub(crate) placement: TextAtlasPlacement,
    pub(crate) evicted_page: Option<usize>,
}

impl TextAtlasInsertion {
    pub(crate) fn placed(page_index: usize, placement: TextAtlasPlacement) -> Self {
        Self {
            page_index,
            placement,
            evicted_page: None,
        }
    }

    pub(crate) fn evicted(page_index: usize, placement: TextAtlasPlacement) -> Self {
        Self {
            page_index,
            placement,
            evicted_page: Some(page_index),
        }
    }
}

/// A multi-page glyph atlas: a collection of uniformly sized [`TextAtlas`] pages that grows on
/// demand up to `max_pages`, then recycles the least-recently-used page. Each page maps to one
/// layer of the GPU texture array.
#[derive(Debug, Clone)]
pub(crate) struct TextAtlasPages {
    pub(crate) pages: Vec<TextAtlas>,
    pub(crate) page_width: usize,
    pub(crate) page_height: usize,
    pub(crate) max_pages: usize,
}

impl TextAtlasPages {
    pub(crate) fn new(page_width: usize, page_height: usize, max_pages: usize) -> Self {
        Self {
            pages: vec![TextAtlas::new(page_width, page_height)],
            page_width,
            page_height,
            max_pages: max_pages.max(1),
        }
    }

    /// Number of allocated atlas pages (drives how many texture-array layers are allocated).
    pub(crate) fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Uniform dimensions of every page, in pixels.
    pub(crate) fn page_size(&self) -> (u32, u32) {
        (self.page_width as u32, self.page_height as u32)
    }

    /// Mark a page as used in `frame` so LRU eviction won't reclaim it prematurely. Called on a
    /// glyph-cache hit (the insert path stamps the page itself).
    pub(crate) fn touch_page(&mut self, index: usize, frame: u64) {
        if let Some(page) = self.pages.get_mut(index) {
            page.last_used_frame = frame;
        }
    }

    /// Insert a rasterized glyph, returning the page it landed on plus its placement within that
    /// page. Tries existing pages in order, then grows a new page if under budget. At budget this
    /// returns `Full` for now; whole-page LRU eviction is added in a later phase. `frame` stamps
    /// the chosen page for that future eviction policy.
    pub(crate) fn insert_rgba(
        &mut self,
        width: usize,
        height: usize,
        pixels: &[u8],
        frame: u64,
    ) -> std::result::Result<TextAtlasInsertion, TextAtlasInsertError> {
        // A glyph larger than a page can never fit on any page.
        if width == 0 || height == 0 || width > self.page_width || height > self.page_height {
            return Err(TextAtlasInsertError::TooLarge);
        }

        for index in 0..self.pages.len() {
            match self.pages[index].insert_rgba(width, height, pixels) {
                Ok(placement) => {
                    self.pages[index].last_used_frame = frame;
                    return Ok(TextAtlasInsertion::placed(index, placement));
                }
                Err(TextAtlasInsertError::TooLarge) => return Err(TextAtlasInsertError::TooLarge),
                Err(TextAtlasInsertError::Full) => {}
            }
        }

        if self.pages.len() < self.max_pages {
            let mut page = TextAtlas::new(self.page_width, self.page_height);
            let placement = page.insert_rgba(width, height, pixels)?;
            page.last_used_frame = frame;
            let index = self.pages.len();
            self.pages.push(page);
            return Ok(TextAtlasInsertion::placed(index, placement));
        }

        // At the page budget: evict the least-recently-used page that was NOT touched this frame.
        // Pages used earlier this frame are off-limits -- glyphs already emitted this frame point
        // into them, so clearing one would make those draws sample garbage. This guard is the
        // load-bearing invariant of the eviction scheme.
        let evict_index = self
            .pages
            .iter()
            .enumerate()
            .filter(|(_, page)| page.last_used_frame != frame)
            .min_by_key(|(_, page)| page.last_used_frame)
            .map(|(index, _)| index);
        let Some(evict_index) = evict_index else {
            // Every page is hot this frame; signal Full so the caller drops this glyph for now.
            return Err(TextAtlasInsertError::Full);
        };

        self.pages[evict_index].clear_for_reuse();
        let placement = self.pages[evict_index].insert_rgba(width, height, pixels)?;
        self.pages[evict_index].last_used_frame = frame;
        Ok(TextAtlasInsertion::evicted(evict_index, placement))
    }

    /// Drain the pending dirty-rect upload from each page that has one, tagged with its page index
    /// (the destination texture-array layer).
    pub(crate) fn take_uploads(&mut self) -> Vec<(usize, TextAtlasUpload)> {
        let mut uploads = Vec::new();
        for (index, page) in self.pages.iter_mut().enumerate() {
            if let Some(upload) = page.take_upload() {
                uploads.push((index, upload));
            }
        }
        uploads
    }
}

#[cfg(test)]
pub(crate) mod page_tests {
    use super::*;

    #[test]
    fn optimization_regression_fresh_atlas_uploads_only_populated_pixels() {
        let mut atlas = TextAtlas::new(2048, 2048);
        atlas.insert_rgba(12, 18, &vec![255; 12 * 18 * 4]).unwrap();
        let upload = atlas.take_upload().unwrap();
        assert!(
            upload.pixels.len() < 4096,
            "a tiny glyph uploaded a whole page"
        );
        assert!(atlas.take_upload().is_none());
    }

    #[test]
    fn recycled_page_upload_clears_old_glyphs_without_copying_a_full_page() {
        let mut atlas = TextAtlas::new(64, 64);
        atlas.insert_rgba(60, 60, &vec![255; 60 * 60 * 4]).unwrap();
        atlas.take_upload().unwrap();
        let generation = atlas.generation;
        atlas.clear_for_reuse();
        atlas.insert_rgba(4, 4, &[255; 4 * 4 * 4]).unwrap();
        let upload = atlas.take_upload().unwrap();
        assert!(upload.clear_texture);
        assert_ne!(atlas.generation, generation);
        assert_eq!(upload.pixels.len(), 4 * 4 * 4);
        assert_eq!(atlas.pixels[(32 * 64 + 32) * 4], 0);
        atlas.clear_for_reuse();
        let clear_only = atlas.take_upload().unwrap();
        assert!(clear_only.clear_texture);
        assert!(clear_only.pixels.is_empty());
        assert!(atlas.take_upload().is_none());
    }

    fn opaque(width: usize, height: usize) -> Vec<u8> {
        vec![255u8; width * height * 4]
    }

    #[test]
    fn overflow_allocates_second_page() {
        let mut pages = TextAtlasPages::new(64, 64, 2);
        let glyph = opaque(60, 60);

        // First 60x60 fits on page 0; a second can't share the 64-tall page, so it grows.
        assert_eq!(pages.insert_rgba(60, 60, &glyph, 1).unwrap().page_index, 0);
        assert_eq!(pages.insert_rgba(60, 60, &glyph, 1).unwrap().page_index, 1);
        assert_eq!(pages.page_count(), 2);

        // Both pages are full, we are at the 2-page budget, and every page was touched this same
        // frame -> nothing is eligible for eviction -> Full.
        assert_eq!(
            pages.insert_rgba(60, 60, &glyph, 1),
            Err(TextAtlasInsertError::Full)
        );
    }

    #[test]
    fn eviction_reuses_lru_page() {
        let mut pages = TextAtlasPages::new(64, 64, 2);
        let glyph = opaque(60, 60);

        // Page 0 last used at frame 1, page 1 at frame 2.
        assert_eq!(pages.insert_rgba(60, 60, &glyph, 1).unwrap().page_index, 0);
        assert_eq!(pages.insert_rgba(60, 60, &glyph, 2).unwrap().page_index, 1);

        // At budget; inserting at frame 3 evicts the LRU page (page 0) and reuses it.
        let insertion = pages.insert_rgba(60, 60, &glyph, 3).unwrap();
        assert_eq!(insertion.page_index, 0);
        assert_eq!(insertion.evicted_page, Some(0));
        assert_eq!(pages.page_count(), 2);
    }

    #[test]
    fn current_frame_page_not_evicted() {
        let mut pages = TextAtlasPages::new(64, 64, 2);
        let glyph = opaque(60, 60);

        // Both pages are used in frame 5.
        pages.insert_rgba(60, 60, &glyph, 5).unwrap();
        pages.insert_rgba(60, 60, &glyph, 5).unwrap();

        // A third glyph in frame 5 must NOT evict a page referenced earlier this frame.
        assert_eq!(
            pages.insert_rgba(60, 60, &glyph, 5),
            Err(TextAtlasInsertError::Full)
        );
    }

    #[test]
    fn too_large_glyph_is_rejected() {
        let mut pages = TextAtlasPages::new(64, 64, 4);
        assert_eq!(
            pages.insert_rgba(70, 70, &opaque(70, 70), 1),
            Err(TextAtlasInsertError::TooLarge)
        );
        assert_eq!(pages.page_count(), 1);
    }

    #[test]
    fn take_uploads_returns_per_page() {
        let mut pages = TextAtlasPages::new(64, 64, 2);
        let glyph = opaque(60, 60);
        pages.insert_rgba(60, 60, &glyph, 1).unwrap();
        pages.insert_rgba(60, 60, &glyph, 1).unwrap();

        let uploads = pages.take_uploads();
        let indices: Vec<usize> = uploads.iter().map(|(index, _)| *index).collect();
        assert_eq!(uploads.len(), 2);
        assert!(indices.contains(&0) && indices.contains(&1));

        // Dirty state is consumed: a second drain yields nothing.
        assert!(pages.take_uploads().is_empty());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PathCacheKind {
    Fill,
    Stroke {
        line_width_bits: u32,
        cap: sui_scene::StrokeCap,
        join: sui_scene::StrokeJoin,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PathCacheKey {
    pub(crate) signature: u64,
    pub(crate) kind: PathCacheKind,
    pub(crate) feather_width_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AnalyticPathCacheKey {
    pub(crate) signature: u64,
    pub(crate) kind: PathCacheKind,
    pub(crate) feather_width_bits: u32,
}

impl AnalyticPathCacheKey {
    pub(crate) fn fill(path: &ScenePath, transform: Transform, feather_width: f32) -> Self {
        Self {
            signature: hash_normalized_analytic_path(path, transform),
            kind: PathCacheKind::Fill,
            feather_width_bits: feather_width.to_bits(),
        }
    }

    pub(crate) fn stroke(
        path: &ScenePath,
        transform: Transform,
        stroke: StrokeStyle,
        feather_width: f32,
    ) -> Self {
        Self {
            signature: hash_normalized_analytic_path(path, transform),
            kind: PathCacheKind::Stroke {
                line_width_bits: stroke.width.to_bits(),
                cap: stroke.cap,
                join: stroke.join,
            },
            feather_width_bits: feather_width.to_bits(),
        }
    }
}

impl PathCacheKey {
    pub(crate) fn fill(path: &ScenePath, transform: Transform, feather_width: f32) -> Self {
        Self {
            signature: hash_path(path, transform),
            kind: PathCacheKind::Fill,
            feather_width_bits: feather_width.to_bits(),
        }
    }

    pub(crate) fn stroke(
        path: &ScenePath,
        transform: Transform,
        stroke: StrokeStyle,
        feather_width: f32,
    ) -> Self {
        Self {
            signature: hash_path(path, transform),
            kind: PathCacheKind::Stroke {
                line_width_bits: stroke.width.to_bits(),
                cap: stroke.cap,
                join: stroke.join,
            },
            feather_width_bits: feather_width.to_bits(),
        }
    }
}
