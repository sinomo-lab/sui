//! Hint targets unavailable through Swash's Boolean hint switch. Keep Swash's
//! color-glyph handling while supporting vertical-only slight hinting on
//! Linux/Android and font-directed non-symmetric smoothing on other platforms.
use std::num::NonZeroUsize;

use lru::LruCache;
use skrifa::{
    MetadataProvider,
    instance::Size,
    outline::{
        Engine, GlyphStyles, HintingInstance, HintingOptions, OutlineGlyphFormat, OutlinePen,
        SmoothMode, Target,
    },
};
use swash::scale::image::{Content, Image};
use swash::zeno::{Command, Format, Mask, Origin, PathBuilder, Scratch, Vector};

use crate::text::GlyphSubpixelOffsetKey;
use crate::text::{GlyphFaceCacheKey, GlyphHintingTarget};
use crate::text_engine::SwashFaceState;
use crate::text_policy::TextRenderMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct HintKey {
    font: [u64; 2],
    ppem: u32,
    weight: u16,
    lcd: bool,
    target: GlyphHintingTarget,
}

pub(crate) struct FontAwareHinter {
    gasp_tables: LruCache<GlyphFaceCacheKey, Option<Vec<u8>>>,
    instances: LruCache<HintKey, HintingInstance>,
    glyph_styles: LruCache<[u64; 2], Option<GlyphStyles>>,
    path: Pen,
    scratch: Scratch,
    lcd_mask: crate::text_raster::LcdMaskRasterizer,
}

impl Default for FontAwareHinter {
    fn default() -> Self {
        Self {
            gasp_tables: LruCache::new(NonZeroUsize::new(32).unwrap()),
            instances: LruCache::new(NonZeroUsize::new(16).unwrap()),
            glyph_styles: LruCache::new(NonZeroUsize::new(16).unwrap()),
            path: Pen::default(),
            scratch: Scratch::new(),
            lcd_mask: Default::default(),
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_outline(
        &mut self,
        face: &SwashFaceState<'_>,
        glyph: u16,
        ppem: f32,
        offset: GlyphSubpixelOffsetKey,
        weight: u16,
        mode: TextRenderMode,
        hinting_target: GlyphHintingTarget,
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
            target: hinting_target,
        };
        if !self.instances.contains(&key) {
            let location = font.axes().location([("wght", f32::from(weight))]);
            let target = Target::Smooth {
                mode: if hinting_target == GlyphHintingTarget::Slight {
                    SmoothMode::Light
                } else if lcd {
                    SmoothMode::Lcd
                } else {
                    SmoothMode::Normal
                },
                symmetric_rendering: hinting_target != GlyphHintingTarget::Asymmetric,
                preserve_linear_metrics: true,
            };
            // Slight TrueType hinting uses the auto-hinter so native bytecode
            // cannot snap stems horizontally. CFF keeps its native hinter.
            let engine = if hinting_target == GlyphHintingTarget::Slight {
                if !self.glyph_styles.contains(&face.font_id) {
                    let styles = (outlines.format() == Some(OutlineGlyphFormat::Glyf)
                        && !outlines.require_interpreter())
                    .then(|| GlyphStyles::new(&outlines));
                    self.glyph_styles.put(face.font_id, styles);
                }
                match self.glyph_styles.get(&face.font_id).cloned().flatten() {
                    Some(styles) => Engine::Auto(Some(styles)),
                    None => Engine::AutoFallback,
                }
            } else {
                Engine::AutoFallback
            };
            let instance = HintingInstance::new(
                &outlines,
                Size::new(ppem),
                &location,
                HintingOptions { engine, target },
            )
            .ok()?;
            self.instances.put(key, instance);
        }
        self.path.0.clear();
        outline
            .draw(self.instances.get(&key)?, &mut self.path)
            .ok()?;
        let offset: Vector = offset.as_swash_offset();
        if lcd {
            return Some(self.lcd_mask.render(
                self.path.0.as_slice(),
                offset,
                if hinting_target == GlyphHintingTarget::Asymmetric {
                    crate::text_raster::LcdSampling::Asymmetric
                } else {
                    crate::text_raster::LcdSampling::Symmetric
                },
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
    fn slight_hinting_preserves_horizontal_outlines_and_antialiases_edges() {
        let face = sui_text::ResolvedTextFace::from_bytes(
            std::sync::Arc::from(sui_text::BUNDLED_NOTO_SANS_REGULAR_FONT),
            0,
        );
        let swash_face = SwashFaceState::new(&face, GlyphFaceCacheKey::new(&face)).unwrap();
        let font = skrifa::FontRef::new(face.bytes()).unwrap();
        let outlines = font.outline_glyphs();
        let mut hinter = FontAwareHinter::default();
        let points = |path: &[Command]| {
            path.iter()
                .flat_map(|command| match *command {
                    Command::MoveTo(p) | Command::LineTo(p) => vec![p],
                    Command::QuadTo(a, b) => vec![a, b],
                    Command::CurveTo(a, b, c) => vec![a, b, c],
                    Command::Close => vec![],
                })
                .collect::<Vec<_>>()
        };
        let mut adjusted_vertically = false;
        for ppem in [11.0, 13.75, 16.0, 22.5] {
            for ch in ['H', 'm', 'o'] {
                let glyph = ttf_parser::Face::parse(face.bytes(), 0)
                    .unwrap()
                    .glyph_index(ch)
                    .unwrap()
                    .0;
                let mut unhinted = Pen::default();
                outlines
                    .get(skrifa::GlyphId::new(u32::from(glyph)))
                    .unwrap()
                    .draw(
                        skrifa::outline::DrawSettings::unhinted(Size::new(ppem), &[][..]),
                        &mut unhinted,
                    )
                    .unwrap();
                let image = hinter
                    .render_outline(
                        &swash_face,
                        glyph,
                        ppem,
                        GlyphSubpixelOffsetKey::new(1, 0),
                        400,
                        TextRenderMode::Grayscale,
                        GlyphHintingTarget::Slight,
                    )
                    .unwrap();
                assert_eq!(image.content, Content::Mask);
                assert!(image.data.iter().any(|&value| value > 0 && value < 255));
                let original = points(&unhinted.0);
                let hinted = points(&hinter.path.0);
                assert_eq!(original.len(), hinted.len());
                for (original, hinted) in original.iter().zip(&hinted) {
                    // The auto-hinter uses a 26.6 fixed-point grid internally.
                    assert!(
                        (original.x - hinted.x).abs() <= 1.0 / 64.0,
                        "{ch} at {ppem}: horizontal hinting moved {} to {}",
                        original.x,
                        hinted.x
                    );
                    adjusted_vertically |= (original.y - hinted.y).abs() > 1.0 / 64.0;
                }
            }
        }
        assert!(
            adjusted_vertically,
            "slight hinting must grid-fit the vertical axis"
        );
        assert_eq!(hinter.glyph_styles.len(), 1, "sizes share font analysis");
    }

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
