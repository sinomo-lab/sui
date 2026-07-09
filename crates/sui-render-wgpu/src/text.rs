use super::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct GlyphSubpixelOffsetKey {
    pub(crate) x: u8,
    pub(crate) y: u8,
}

impl GlyphSubpixelOffsetKey {
    pub(crate) fn new(x: u8, y: u8) -> Self {
        Self {
            x: x % GLYPH_SUBPIXEL_VARIANTS_X,
            y: y % GLYPH_SUBPIXEL_VARIANTS_Y,
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
pub(crate) struct CachedGlyphVertex {
    pub(crate) position: Point,
    pub(crate) coverage: f32,
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

#[derive(Debug, Default, Clone)]
pub(crate) struct CachedGlyphMesh {
    pub(crate) vertices: Vec<CachedGlyphVertex>,
    pub(crate) indices: Vec<u32>,
}

impl CachedGlyphMesh {
    pub(crate) fn push_vertex(&mut self, position: Point, coverage: f32) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(CachedGlyphVertex { position, coverage });
        index
    }

    #[cfg(test)]
    pub(crate) fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtlasRectU {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl AtlasRectU {
    const NOTHING: Self = Self {
        min_x: usize::MAX,
        min_y: usize::MAX,
        max_x: 0,
        max_y: 0,
    };

    const EVERYTHING: Self = Self {
        min_x: 0,
        min_y: 0,
        max_x: usize::MAX,
        max_y: usize::MAX,
    };

    fn include_rect(&mut self, x: usize, y: usize, width: usize, height: usize) {
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
}

/// A single atlas page: a CPU-side pixel buffer with a shelf-packing cursor and a dirty
/// rectangle for incremental GPU uploads. The multi-page atlas ([`TextAtlasPages`]) holds a
/// collection of these.
#[derive(Debug, Clone)]
pub(crate) struct TextAtlas {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
    dirty: AtlasRectU,
    pub(crate) full_upload: bool,
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
            dirty: AtlasRectU::EVERYTHING,
            full_upload: true,
            cursor: (TEXT_ATLAS_PADDING, TEXT_ATLAS_PADDING),
            row_height: 0,
            last_used_frame: 0,
        }
    }

    /// Reset this page so its space can be recycled (used when a page is evicted). Zeroes the
    /// pixels, rewinds the packing cursor, and forces a full re-upload of the cleared contents.
    pub(crate) fn clear_for_reuse(&mut self) {
        self.pixels.iter_mut().for_each(|byte| *byte = 0);
        self.cursor = (TEXT_ATLAS_PADDING, TEXT_ATLAS_PADDING);
        self.row_height = 0;
        self.dirty = AtlasRectU::EVERYTHING;
        self.full_upload = true;
        self.last_used_frame = 0;
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    fn allocate(
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

    fn write_rgba(
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
        if dirty == AtlasRectU::NOTHING {
            return None;
        }

        if self.full_upload || dirty == AtlasRectU::EVERYTHING {
            self.full_upload = false;
            return Some(TextAtlasUpload {
                offset: (0, 0),
                extent: self.size(),
                pixels: self.pixels.clone(),
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
    fn placed(page_index: usize, placement: TextAtlasPlacement) -> Self {
        Self {
            page_index,
            placement,
            evicted_page: None,
        }
    }

    fn evicted(page_index: usize, placement: TextAtlasPlacement) -> Self {
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
    pages: Vec<TextAtlas>,
    page_width: usize,
    page_height: usize,
    max_pages: usize,
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
mod page_tests {
    use super::*;

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
enum PathCacheKind {
    Fill,
    Stroke { line_width_bits: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PathCacheKey {
    signature: u64,
    kind: PathCacheKind,
    feather_width_bits: u32,
}

impl PathCacheKey {
    fn fill(path: &ScenePath, transform: Transform, feather_width: f32) -> Self {
        Self {
            signature: hash_path(path, transform),
            kind: PathCacheKind::Fill,
            feather_width_bits: feather_width.to_bits(),
        }
    }

    fn stroke(path: &ScenePath, transform: Transform, line_width: f32, feather_width: f32) -> Self {
        Self {
            signature: hash_path(path, transform),
            kind: PathCacheKind::Stroke {
                line_width_bits: line_width.to_bits(),
            },
            feather_width_bits: feather_width.to_bits(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PathMeshCache {
    meshes: HashMap<PathCacheKey, CachedGlyphMesh>,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) hits: usize,
    pub(crate) misses: usize,
}

impl Default for PathMeshCache {
    fn default() -> Self {
        Self {
            meshes: HashMap::new(),
            diagnostics_enabled: true,
            hits: 0,
            misses: 0,
        }
    }
}

impl PathMeshCache {
    pub(crate) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
    }

    pub(crate) fn cached_fill_mesh(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        feather_width: f32,
    ) -> Result<&CachedGlyphMesh> {
        let key = PathCacheKey::fill(path, transform, feather_width);
        match self.meshes.entry(key) {
            Entry::Occupied(entry) => {
                if self.diagnostics_enabled {
                    self.hits += 1;
                }
                Ok(entry.into_mut())
            }
            Entry::Vacant(entry) => {
                if self.diagnostics_enabled {
                    self.misses += 1;
                }
                let lyon_path = build_lyon_path(path, transform);
                let mesh = feathering::build_local_fill_mesh(&lyon_path, feather_width)?;
                Ok(entry.insert(mesh))
            }
        }
    }

    pub(crate) fn cached_stroke_mesh(
        &mut self,
        path: &ScenePath,
        transform: Transform,
        line_width: f32,
        feather_width: f32,
    ) -> Result<&CachedGlyphMesh> {
        let key = PathCacheKey::stroke(path, transform, line_width, feather_width);
        match self.meshes.entry(key) {
            Entry::Occupied(entry) => {
                if self.diagnostics_enabled {
                    self.hits += 1;
                }
                Ok(entry.into_mut())
            }
            Entry::Vacant(entry) => {
                if self.diagnostics_enabled {
                    self.misses += 1;
                }
                let lyon_path = build_lyon_path(path, transform);
                let mesh =
                    feathering::build_local_stroke_mesh(&lyon_path, line_width, feather_width)?;
                Ok(entry.insert(mesh))
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> (usize, usize, usize) {
        (self.meshes.len(), self.hits, self.misses)
    }

    pub(crate) fn snapshot(&self) -> GlyphCacheSnapshot {
        GlyphCacheSnapshot {
            entries: self.meshes.len(),
            hits: self.hits,
            misses: self.misses,
        }
    }
}
