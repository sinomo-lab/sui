//! Font-directed hint targets unavailable through Swash's Boolean hint switch.
//! Keep Swash's color-glyph handling and Zeno rasterizer; only override outline
//! hinting when a version-1 gasp range asks for non-symmetric smoothing.
use std::num::NonZeroUsize;

use lru::LruCache;
use skrifa::{
    MetadataProvider,
    instance::Size,
    outline::{HintingInstance, OutlinePen, SmoothMode, Target},
};
use swash::scale::image::{Content, Image};
use swash::zeno::{Command, Format, Mask, Origin, PathBuilder, Scratch, Vector};

use crate::text::GlyphFaceCacheKey;
use crate::text::GlyphSubpixelOffsetKey;
use crate::text_engine::SwashFaceState;
use crate::text_policy::TextRenderMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct HintKey {
    font: [u64; 2],
    ppem: u32,
    weight: u16,
    lcd: bool,
}

pub(crate) struct FontAwareHinter {
    gasp_tables: LruCache<GlyphFaceCacheKey, Option<Vec<u8>>>,
    instances: LruCache<HintKey, HintingInstance>,
    path: Pen,
    scratch: Scratch,
}

impl Default for FontAwareHinter {
    fn default() -> Self {
        Self {
            gasp_tables: LruCache::new(NonZeroUsize::new(32).unwrap()),
            instances: LruCache::new(NonZeroUsize::new(16).unwrap()),
            path: Pen::default(),
            scratch: Scratch::new(),
        }
    }
}

/// Gasp version 0 has no symmetric-smoothing bit. Unknown/truncated/unsorted
/// tables must retain the established renderer behavior rather than guess.
pub(crate) fn symmetric_smoothing(gasp: &[u8], ppem: f32) -> Option<bool> {
    if !ppem.is_finite() || ppem <= 0.0 {
        return None;
    }
    let u16_at = |offset| {
        gasp.get(offset..offset + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
    };
    if u16_at(0)? != 1 {
        return None;
    }
    let count = usize::from(u16_at(2)?);
    if count == 0 || count > 1024 || gasp.len() < 4 + count * 4 {
        return None;
    }
    let size = ppem.round();
    let mut previous = None;
    let mut selected = None;
    for i in 0..count {
        let max = u16_at(4 + i * 4)?;
        if previous.is_some_and(|previous| max <= previous) {
            return None;
        }
        previous = Some(max);
        if selected.is_none() && size <= f32::from(max) {
            selected = Some(u16_at(6 + i * 4)? & 8 != 0);
        }
    }
    selected
}

impl FontAwareHinter {
    pub(crate) fn uses_asymmetric_smoothing(
        &mut self,
        face: &sui_text::ResolvedTextFace,
        requested_ppem: f32,
    ) -> bool {
        let key = GlyphFaceCacheKey::new(face);
        if !self.gasp_tables.contains(&key) {
            let table = ttf_parser::RawFace::parse(face.bytes(), face.face_index())
                .ok()
                .and_then(|font| font.table(ttf_parser::Tag::from_bytes(b"gasp")))
                .filter(|table| table.len() <= 4100)
                .map(<[u8]>::to_vec);
            self.gasp_tables.put(key, table);
        }
        self.gasp_tables
            .get(&key)
            .and_then(|table| table.as_deref())
            .and_then(|table| symmetric_smoothing(table, requested_ppem))
            == Some(false)
    }

    pub(crate) fn render_asymmetric(
        &mut self,
        face: &SwashFaceState<'_>,
        glyph: u16,
        ppem: f32,
        offset: GlyphSubpixelOffsetKey,
        weight: u16,
        mode: TextRenderMode,
    ) -> Option<Image> {
        let font = skrifa::FontRef::from_index(face.font_ref.data, face.face_index).ok()?;
        let outlines = font.outline_glyphs();
        let outline = outlines.get(skrifa::GlyphId::new(u32::from(glyph)))?;
        let lcd = mode == TextRenderMode::LcdSubpixel;
        let key = HintKey {
            font: face.font_id,
            ppem: ppem.to_bits(),
            weight,
            lcd,
        };
        if !self.instances.contains(&key) {
            let location = font.axes().location([("wght", f32::from(weight))]);
            let target = Target::Smooth {
                mode: if lcd {
                    SmoothMode::Lcd
                } else {
                    SmoothMode::Normal
                },
                symmetric_rendering: false,
                preserve_linear_metrics: true,
            };
            let instance =
                HintingInstance::new(&outlines, Size::new(ppem), &location, target).ok()?;
            self.instances.put(key, instance);
        }
        self.path.0.clear();
        outline
            .draw(self.instances.get(&key)?, &mut self.path)
            .ok()?;
        let offset: Vector = offset.as_swash_offset();
        if lcd {
            return Some(crate::text_raster::render_lcd_mask(
                self.path.0.as_slice(),
                offset,
                &mut self.scratch,
            ));
        }
        let mut image = Image {
            source: swash::scale::Source::Outline,
            content: Content::Mask,
            ..Image::default()
        };
        image.placement = Mask::with_scratch(self.path.0.as_slice(), &mut self.scratch)
            .format(Format::Alpha)
            .origin(Origin::BottomLeft)
            .offset(offset)
            .render_offset(offset)
            .inspect(|fmt, w, h| image.data.resize(fmt.buffer_size(w, h), 0))
            .render_into(&mut image.data, None);
        Some(image)
    }
}

#[derive(Default)]
struct Pen(Vec<Command>);
impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x, y));
    }
    fn quad_to(&mut self, x: f32, y: f32, x1: f32, y1: f32) {
        self.0.quad_to((x, y), (x1, y1));
    }
    fn curve_to(&mut self, x: f32, y: f32, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.0.curve_to((x, y), (x1, y1), (x2, y2));
    }
    fn close(&mut self) {
        self.0.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gasp_selects_font_ranges_at_physical_ppem_and_rejects_malformed_tables() {
        // The installed Segoe UI ranges: <=8 symmetric, 9..19 natural, >=20 symmetric.
        let table = [0, 1, 0, 3, 0, 8, 0, 10, 0, 19, 0, 7, 255, 255, 0, 15];
        for (size, expected) in [
            (8.0, true),
            (9.0, false),
            (15.0, false),
            (19.0, false),
            (19.49, false),
            (19.5, true),
            (22.5, true),
        ] {
            assert_eq!(symmetric_smoothing(&table, size), Some(expected));
        }
        assert_eq!(symmetric_smoothing(&table[..12], 15.0), None);
        assert_eq!(
            symmetric_smoothing(&[0, 0, 0, 1, 255, 255, 0, 3], 15.0),
            None
        );
        assert_eq!(
            symmetric_smoothing(&[0, 1, 0, 2, 0, 19, 0, 7, 0, 8, 0, 10], 15.0),
            None
        );
        assert_eq!(symmetric_smoothing(&table, f32::NAN), None);
    }
}
