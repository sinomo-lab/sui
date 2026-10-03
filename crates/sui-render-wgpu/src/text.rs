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
        hinting_target: GlyphHintingTarget,
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
            hinting_target,
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
pub(crate) const LCD_SUBPIXEL_OFFSET: f32 = 1.0 / 3.0;

pub(crate) fn lcd_bgra_format() -> swash::zeno::Format {
    swash::zeno::Format::CustomSubpixel([-LCD_SUBPIXEL_OFFSET, 0.0, LCD_SUBPIXEL_OFFSET])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GlyphHintingTarget {
    None,
    /// Vertical grid fitting with preserved horizontal outlines and advances.
    Slight,
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
    /// Which atlas page (texture-array layer) this glyph was rasterized into, or `None` for a
    /// glyph without ink, which occupies no atlas space and must not keep any page alive.
    pub(crate) page_index: Option<usize>,
}

impl CachedGlyphAtlas {
    /// Which atlas holds this glyph. Color glyphs and LCD masks need RGB; everything else is
    /// single-channel coverage.
    pub(crate) fn atlas_kind(&self) -> TextAtlasKind {
        if self.is_color || self.color_mode == TextAtlasColorMode::LcdSubpixel {
            TextAtlasKind::Color
        } else {
            TextAtlasKind::Mask
        }
    }
}

/// The two glyph atlases. Grayscale coverage, the common case, is stored at one byte per pixel;
/// color glyphs and LCD subpixel masks need four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextAtlasKind {
    Mask,
    Color,
}

impl TextAtlasKind {
    pub(crate) const ALL: [Self; 2] = [Self::Mask, Self::Color];

    pub(crate) const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Mask => 1,
            Self::Color => 4,
        }
    }

    /// Value carried in `TextAtlasInstance::coverage_flags[3]` to select the texture.
    pub(crate) const fn shader_index(self) -> u8 {
        match self {
            Self::Mask => 0,
            Self::Color => 1,
        }
    }

    /// Bit for `page` of this atlas in a mask covering both atlases, as kept by retained
    /// packets. Both atlases together have `2 * TEXT_ATLAS_MAX_PAGES` page slots.
    pub(crate) const fn page_slot(self, page: usize) -> usize {
        self.shader_index() as usize * TEXT_ATLAS_MAX_PAGES + page
    }
}

/// Number of page slots across both atlases (see [`TextAtlasKind::page_slot`]).
pub(crate) const TEXT_ATLAS_PAGE_SLOTS: usize = 2 * TEXT_ATLAS_MAX_PAGES;

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

/// One rectangle to copy into an atlas page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextAtlasWrite {
    pub(crate) offset: (u32, u32),
    pub(crate) extent: (u32, u32),
    pub(crate) pixels: Vec<u8>,
}

/// Pending CPU->GPU work for one atlas page: an optional clear of the whole layer, then the
/// rectangles holding glyphs placed since the last upload. The destination page
/// (texture-array layer) is supplied alongside by `take_uploads`.
#[derive(Debug, Clone, Default)]
pub(crate) struct TextAtlasUpload {
    pub(crate) clear_texture: bool,
    pub(crate) writes: Vec<TextAtlasWrite>,
}

impl TextAtlasUpload {
    pub(crate) fn byte_len(&self) -> usize {
        self.writes.iter().map(|write| write.pixels.len()).sum()
    }
}

#[derive(Debug, Clone)]
struct PendingGlyph {
    placement: TextAtlasPlacement,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

/// A single atlas page: a shelf-packing cursor plus the glyphs placed since the last upload.
/// Pixels live only on the GPU; the multi-page atlas ([`TextAtlasPages`]) holds a collection
/// of these.
#[derive(Debug, Clone)]
pub(crate) struct TextAtlas {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) bytes_per_pixel: usize,
    pending: Vec<PendingGlyph>,
    pub(crate) clear_texture: bool,
    pub(crate) generation: u64,
    pub(crate) cursor: (usize, usize),
    pub(crate) row_height: usize,
    /// Frame index of the most recent insert/touch, used for whole-page LRU eviction.
    pub(crate) last_used_frame: u64,
}

impl TextAtlas {
    pub(crate) fn new(width: usize, height: usize, bytes_per_pixel: usize) -> Self {
        Self {
            width,
            height,
            bytes_per_pixel,
            pending: Vec::new(),
            clear_texture: false,
            generation: 1,
            cursor: (TEXT_ATLAS_PADDING, TEXT_ATLAS_PADDING),
            row_height: 0,
            last_used_frame: 0,
        }
    }

    /// Reset this page so its space can be recycled (used when a page is evicted). Flags the
    /// GPU layer for a clear and drops glyphs not uploaded yet, so only new glyphs are written.
    pub(crate) fn clear_for_reuse(&mut self) {
        self.pending.clear();
        self.cursor = (TEXT_ATLAS_PADDING, TEXT_ATLAS_PADDING);
        self.row_height = 0;
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

    /// Queue a glyph's pixels (`bytes_per_pixel` each) for upload at an allocated placement.
    fn write(
        &mut self,
        placement: TextAtlasPlacement,
        width: usize,
        height: usize,
        pixels: Vec<u8>,
    ) {
        debug_assert_eq!(pixels.len(), width * height * self.bytes_per_pixel);
        self.pending.push(PendingGlyph {
            placement,
            width,
            height,
            pixels,
        });
    }

    #[cfg(test)]
    pub(crate) fn insert(
        &mut self,
        width: usize,
        height: usize,
        pixels: Vec<u8>,
    ) -> std::result::Result<TextAtlasPlacement, TextAtlasInsertError> {
        let placement = self.allocate(width, height)?;
        self.write(placement, width, height, pixels);
        Ok(placement)
    }

    /// Drain the glyphs placed since the last call as GPU writes. Glyphs placed side by side on
    /// one shelf are merged into a single strip, with the padding between them and the space
    /// below shorter glyphs written as zero. That space can be written over safely: the shelf
    /// packer never places anything else in a row's span behind its cursor, and glyphs placed
    /// in earlier frames lie outside the strip.
    pub(crate) fn take_upload(&mut self) -> Option<TextAtlasUpload> {
        let clear_texture = std::mem::take(&mut self.clear_texture);
        if self.pending.is_empty() {
            return clear_texture.then(|| TextAtlasUpload {
                clear_texture,
                writes: Vec::new(),
            });
        }

        let bpp = self.bytes_per_pixel;
        let pending = std::mem::take(&mut self.pending);
        let mut writes = Vec::new();
        let mut start = 0;
        while start < pending.len() {
            let row_y = pending[start].placement.y;
            let mut end = start + 1;
            while end < pending.len() && pending[end].placement.y == row_y {
                end += 1;
            }
            let strip = &pending[start..end];
            let min_x = strip[0].placement.x;
            let last = &strip[strip.len() - 1];
            let width = last.placement.x + last.width - min_x;
            let height = strip.iter().map(|glyph| glyph.height).max().unwrap_or(0);
            let pixels = if let [glyph] = strip {
                // A lone glyph is already the exact strip.
                glyph.pixels.clone()
            } else {
                let mut pixels = vec![0; width * height * bpp];
                for glyph in strip {
                    let row_bytes = glyph.width * bpp;
                    let x = (glyph.placement.x - min_x) * bpp;
                    for row in 0..glyph.height {
                        let dst = row * width * bpp + x;
                        pixels[dst..dst + row_bytes]
                            .copy_from_slice(&glyph.pixels[row * row_bytes..(row + 1) * row_bytes]);
                    }
                }
                pixels
            };
            writes.push(TextAtlasWrite {
                offset: (min_x as u32, row_y as u32),
                extent: (width as u32, height as u32),
                pixels,
            });
            start = end;
        }

        Some(TextAtlasUpload {
            clear_texture,
            writes,
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
/// layer of the GPU texture array. An atlas may have no pages until its first glyph arrives.
#[derive(Debug, Clone)]
pub(crate) struct TextAtlasPages {
    pub(crate) pages: Vec<TextAtlas>,
    pub(crate) page_width: usize,
    pub(crate) page_height: usize,
    pub(crate) bytes_per_pixel: usize,
    pub(crate) max_pages: usize,
}

impl TextAtlasPages {
    /// An atlas with no pages yet; the first insert allocates one.
    pub(crate) fn new(
        page_width: usize,
        page_height: usize,
        bytes_per_pixel: usize,
        max_pages: usize,
    ) -> Self {
        Self {
            pages: Vec::new(),
            page_width,
            page_height,
            bytes_per_pixel,
            max_pages: max_pages.max(1),
        }
    }

    pub(crate) fn for_kind(kind: TextAtlasKind) -> Self {
        Self::new(
            TEXT_ATLAS_WIDTH,
            TEXT_ATLAS_HEIGHT,
            kind.bytes_per_pixel(),
            TEXT_ATLAS_MAX_PAGES,
        )
    }

    /// Number of allocated atlas pages (drives how many texture-array layers are allocated).
    pub(crate) fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Uniform dimensions of every page, in pixels.
    pub(crate) fn page_size(&self) -> (u32, u32) {
        (self.page_width as u32, self.page_height as u32)
    }

    fn new_page(&self) -> TextAtlas {
        TextAtlas::new(self.page_width, self.page_height, self.bytes_per_pixel)
    }

    /// Mark a page as used in `frame` so LRU eviction won't reclaim it prematurely. Called on a
    /// glyph-cache hit (the insert path stamps the page itself).
    pub(crate) fn touch_page(&mut self, index: usize, frame: u64) {
        if let Some(page) = self.pages.get_mut(index) {
            page.last_used_frame = frame;
        }
    }

    /// Insert a rasterized glyph, returning the page it landed on plus its placement within that
    /// page. Tries existing pages in order, then grows a new page if under budget. At budget it
    /// recycles the least-recently-used page not touched in `frame`, or returns `Full` if every
    /// page is in use this frame. `frame` stamps the chosen page for that eviction policy.
    pub(crate) fn insert(
        &mut self,
        width: usize,
        height: usize,
        pixels: Vec<u8>,
        frame: u64,
    ) -> std::result::Result<TextAtlasInsertion, TextAtlasInsertError> {
        // A glyph larger than a page can never fit on any page.
        if width == 0 || height == 0 || width > self.page_width || height > self.page_height {
            return Err(TextAtlasInsertError::TooLarge);
        }

        // Allocate first so the pixels move into whichever page has room.
        let mut target = None;
        for index in 0..self.pages.len() {
            match self.pages[index].allocate(width, height) {
                Ok(placement) => {
                    target = Some(TextAtlasInsertion::placed(index, placement));
                    break;
                }
                Err(TextAtlasInsertError::TooLarge) => return Err(TextAtlasInsertError::TooLarge),
                Err(TextAtlasInsertError::Full) => {}
            }
        }

        let insertion = match target {
            Some(insertion) => insertion,
            None if self.pages.len() < self.max_pages => {
                let mut page = self.new_page();
                let placement = page.allocate(width, height)?;
                self.pages.push(page);
                TextAtlasInsertion::placed(self.pages.len() - 1, placement)
            }
            None => {
                // At the page budget: evict the least-recently-used page that was NOT touched
                // this frame. Pages used earlier this frame are off-limits -- glyphs already
                // emitted this frame point into them, so clearing one would make those draws
                // sample garbage. This guard is the load-bearing invariant of the eviction scheme.
                let evict_index = self
                    .pages
                    .iter()
                    .enumerate()
                    .filter(|(_, page)| page.last_used_frame != frame)
                    .min_by_key(|(_, page)| page.last_used_frame)
                    .map(|(index, _)| index);
                let Some(evict_index) = evict_index else {
                    // Every page is hot this frame; signal Full so the caller drops this glyph.
                    return Err(TextAtlasInsertError::Full);
                };
                self.pages[evict_index].clear_for_reuse();
                let placement = self.pages[evict_index].allocate(width, height)?;
                TextAtlasInsertion::evicted(evict_index, placement)
            }
        };

        let page = &mut self.pages[insertion.page_index];
        page.write(insertion.placement, width, height, pixels);
        page.last_used_frame = frame;
        Ok(insertion)
    }

    /// Drain the pending upload from each page that has one, tagged with its page index
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

/// The mask and color atlases, addressed by kind or by page slot.
#[derive(Debug, Clone)]
pub(crate) struct TextAtlases {
    pub(crate) mask: TextAtlasPages,
    pub(crate) color: TextAtlasPages,
}

impl Default for TextAtlases {
    fn default() -> Self {
        Self {
            mask: TextAtlasPages::for_kind(TextAtlasKind::Mask),
            color: TextAtlasPages::for_kind(TextAtlasKind::Color),
        }
    }
}

impl TextAtlases {
    pub(crate) fn get(&self, kind: TextAtlasKind) -> &TextAtlasPages {
        match kind {
            TextAtlasKind::Mask => &self.mask,
            TextAtlasKind::Color => &self.color,
        }
    }

    pub(crate) fn get_mut(&mut self, kind: TextAtlasKind) -> &mut TextAtlasPages {
        match kind {
            TextAtlasKind::Mask => &mut self.mask,
            TextAtlasKind::Color => &mut self.color,
        }
    }

    fn slot(slot: usize) -> (TextAtlasKind, usize) {
        let kind = if slot < TEXT_ATLAS_MAX_PAGES {
            TextAtlasKind::Mask
        } else {
            TextAtlasKind::Color
        };
        (kind, slot % TEXT_ATLAS_MAX_PAGES)
    }

    /// Generation of the page in `slot` (see [`TextAtlasKind::page_slot`]), if it exists.
    pub(crate) fn slot_generation(&self, slot: usize) -> Option<u64> {
        let (kind, page) = Self::slot(slot);
        self.get(kind).pages.get(page).map(|page| page.generation)
    }

    pub(crate) fn touch_slot(&mut self, slot: usize, frame: u64) {
        let (kind, page) = Self::slot(slot);
        self.get_mut(kind).touch_page(page, frame);
    }
}

#[cfg(test)]
pub(crate) mod page_tests {
    use super::*;

    fn opaque(width: usize, height: usize) -> Vec<u8> {
        vec![255u8; width * height * 4]
    }

    #[test]
    fn optimization_regression_fresh_atlas_uploads_only_populated_pixels() {
        let mut atlas = TextAtlas::new(2048, 2048, 4);
        atlas.insert(12, 18, opaque(12, 18)).unwrap();
        let upload = atlas.take_upload().unwrap();
        assert_eq!(
            upload.byte_len(),
            12 * 18 * 4,
            "a tiny glyph uploaded a whole page"
        );
        assert!(atlas.take_upload().is_none());
    }

    #[test]
    fn mask_pages_store_one_byte_per_pixel() {
        let mut pages = TextAtlasPages::new(64, 64, 1, 2);
        assert_eq!(
            pages.page_count(),
            0,
            "an atlas allocates no page before its first glyph"
        );
        pages.insert(5, 7, vec![200; 5 * 7], 1).unwrap();
        let uploads = pages.take_uploads();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].1.writes[0].extent, (5, 7));
        assert_eq!(uploads[0].1.byte_len(), 5 * 7);
    }

    #[test]
    fn glyphs_on_one_shelf_upload_as_one_strip() {
        let mut atlas = TextAtlas::new(64, 64, 1);
        let first = atlas.insert(3, 4, vec![1; 3 * 4]).unwrap();
        let second = atlas.insert(2, 6, vec![2; 2 * 6]).unwrap();
        assert_eq!(first.y, second.y);
        let upload = atlas.take_upload().unwrap();
        assert_eq!(upload.writes.len(), 1);
        let strip = &upload.writes[0];
        let width = second.x + 2 - first.x;
        assert_eq!(strip.offset, (first.x as u32, first.y as u32));
        assert_eq!(strip.extent, (width as u32, 6));
        let at = |x: usize, y: usize| strip.pixels[y * width + x];
        assert_eq!(at(0, 0), 1);
        assert_eq!(at(0, 4), 0, "below a shorter glyph is empty");
        assert_eq!(at(3, 0), 0, "padding between glyphs is empty");
        assert_eq!(at(second.x - first.x, 5), 2);
    }

    #[test]
    fn glyphs_on_different_shelves_upload_separately() {
        let mut atlas = TextAtlas::new(16, 64, 1);
        let first = atlas.insert(10, 4, vec![1; 10 * 4]).unwrap();
        let second = atlas.insert(10, 4, vec![2; 10 * 4]).unwrap();
        assert_ne!(first.y, second.y);
        let upload = atlas.take_upload().unwrap();
        assert_eq!(upload.writes.len(), 2);
        assert!(upload.writes.iter().all(|write| write.extent == (10, 4)));
    }

    #[test]
    fn recycled_page_upload_clears_old_glyphs() {
        let mut atlas = TextAtlas::new(64, 64, 4);
        atlas.insert(60, 60, opaque(60, 60)).unwrap();
        atlas.take_upload().unwrap();
        let generation = atlas.generation;
        atlas.clear_for_reuse();
        atlas.insert(4, 4, opaque(4, 4)).unwrap();
        let upload = atlas.take_upload().unwrap();
        assert!(upload.clear_texture);
        assert_ne!(atlas.generation, generation);
        assert_eq!(upload.byte_len(), 4 * 4 * 4);
        atlas.clear_for_reuse();
        let clear_only = atlas.take_upload().unwrap();
        assert!(clear_only.clear_texture);
        assert!(clear_only.writes.is_empty());
        assert!(atlas.take_upload().is_none());
    }

    #[test]
    fn recycling_drops_glyphs_not_uploaded_yet() {
        let mut atlas = TextAtlas::new(64, 64, 1);
        atlas.insert(8, 8, vec![1; 64]).unwrap();
        atlas.clear_for_reuse();
        let upload = atlas.take_upload().unwrap();
        assert!(upload.clear_texture);
        assert!(upload.writes.is_empty());
    }

    #[test]
    fn overflow_allocates_second_page() {
        let mut pages = TextAtlasPages::new(64, 64, 4, 2);

        // First 60x60 fits on page 0; a second can't share the 64-tall page, so it grows.
        assert_eq!(
            pages.insert(60, 60, opaque(60, 60), 1).unwrap().page_index,
            0
        );
        assert_eq!(
            pages.insert(60, 60, opaque(60, 60), 1).unwrap().page_index,
            1
        );
        assert_eq!(pages.page_count(), 2);

        // Both pages are full, we are at the 2-page budget, and every page was touched this same
        // frame -> nothing is eligible for eviction -> Full.
        assert_eq!(
            pages.insert(60, 60, opaque(60, 60), 1),
            Err(TextAtlasInsertError::Full)
        );
    }

    #[test]
    fn eviction_reuses_lru_page() {
        let mut pages = TextAtlasPages::new(64, 64, 4, 2);

        // Page 0 last used at frame 1, page 1 at frame 2.
        assert_eq!(
            pages.insert(60, 60, opaque(60, 60), 1).unwrap().page_index,
            0
        );
        assert_eq!(
            pages.insert(60, 60, opaque(60, 60), 2).unwrap().page_index,
            1
        );

        // At budget; inserting at frame 3 evicts the LRU page (page 0) and reuses it.
        let insertion = pages.insert(60, 60, opaque(60, 60), 3).unwrap();
        assert_eq!(insertion.page_index, 0);
        assert_eq!(insertion.evicted_page, Some(0));
        assert_eq!(pages.page_count(), 2);
    }

    #[test]
    fn current_frame_page_not_evicted() {
        let mut pages = TextAtlasPages::new(64, 64, 4, 2);

        // Both pages are used in frame 5.
        pages.insert(60, 60, opaque(60, 60), 5).unwrap();
        pages.insert(60, 60, opaque(60, 60), 5).unwrap();

        // A third glyph in frame 5 must NOT evict a page referenced earlier this frame.
        assert_eq!(
            pages.insert(60, 60, opaque(60, 60), 5),
            Err(TextAtlasInsertError::Full)
        );
    }

    #[test]
    fn too_large_glyph_is_rejected() {
        let mut pages = TextAtlasPages::new(64, 64, 4, 4);
        assert_eq!(
            pages.insert(70, 70, opaque(70, 70), 1),
            Err(TextAtlasInsertError::TooLarge)
        );
        assert_eq!(pages.page_count(), 0);
    }

    #[test]
    fn take_uploads_returns_per_page() {
        let mut pages = TextAtlasPages::new(64, 64, 4, 2);
        pages.insert(60, 60, opaque(60, 60), 1).unwrap();
        pages.insert(60, 60, opaque(60, 60), 1).unwrap();

        let uploads = pages.take_uploads();
        let indices: Vec<usize> = uploads.iter().map(|(index, _)| *index).collect();
        assert_eq!(uploads.len(), 2);
        assert!(indices.contains(&0) && indices.contains(&1));

        // Pending state is consumed: a second drain yields nothing.
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
