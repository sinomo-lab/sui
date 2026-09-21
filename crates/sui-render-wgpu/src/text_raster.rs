//! LCD outline rasterization with bounds covering all physical channel samples.
use swash::scale::{
    Scaler, Source,
    image::{Content, Image},
    outline::Outline,
};
use swash::zeno::{Fill, Mask, Origin, PathData, Placement, Scratch, Vector};

#[derive(Default)]
pub(crate) struct LcdRasterizer {
    outline: Outline,
    scratch: Scratch,
}

impl LcdRasterizer {
    pub(crate) fn render(
        &mut self,
        scaler: &mut Scaler<'_>,
        glyph_id: u16,
        offset: Vector,
    ) -> Option<Image> {
        if !scaler.scale_outline_into(glyph_id, &mut self.outline) {
            return None;
        }
        Some(render_lcd_mask(
            self.outline.path(),
            offset,
            &mut self.scratch,
        ))
    }
}

pub(crate) fn render_lcd_mask(path: impl PathData, offset: Vector, scratch: &mut Scratch) -> Image {
    let mut image = Image {
        source: Source::Outline,
        content: Content::SubpixelMask,
        ..Image::default()
    };
    let bounds = scratch.bounds(&path, Fill::NonZero, None);
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return image;
    }
    // Zeno's automatic bounds omit CustomSubpixel offsets. Include their
    // horizontal footprint before rasterizing; empty atlas padding is too late.
    let left = (bounds.min.x + offset.x - crate::text::LCD_SUBPIXEL_OFFSET).floor();
    let right = (bounds.max.x + offset.x + crate::text::LCD_SUBPIXEL_OFFSET).ceil();
    let bottom = (bounds.min.y + offset.y).floor();
    let top = (bounds.max.y + offset.y).ceil();
    image.placement = Placement {
        left: left as i32,
        top: top as i32,
        width: (right - left) as u32,
        height: (top - bottom) as u32,
    };
    let format = crate::text::lcd_bgra_format();
    image.data.resize(
        format.buffer_size(image.placement.width, image.placement.height),
        0,
    );
    Mask::with_scratch(path, scratch)
        .format(format)
        .origin(Origin::BottomLeft)
        .size(image.placement.width, image.placement.height)
        .offset(Vector::new(offset.x - left, offset.y - bottom))
        .render_into(&mut image.data, None);
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use swash::zeno::{Command, Format, PathBuilder};

    #[test]
    fn lcd_bounds_preserve_fringe_coverage_against_an_oversized_reference() {
        let mut scratch = Scratch::new();
        let mut fringes = 0;
        for left in [-1.875, 0.0, 0.25] {
            let mut path = Vec::new();
            path.move_to((left, -0.25))
                .line_to((left + 0.75, -0.25))
                .line_to((left + 0.75, 2.0))
                .line_to((left, 2.0))
                .close();
            for phase in [0.0, 0.25, 0.5, 0.75] {
                let actual =
                    render_lcd_mask(path.as_slice(), Vector::new(phase, 0.0), &mut scratch);
                let (reference, _) = Mask::new(path.as_slice())
                    .format(crate::text::lcd_bgra_format())
                    .origin(Origin::BottomLeft)
                    .size(12, 12)
                    .offset(Vector::new(4.0 + phase, 4.0))
                    .render();
                let p = actual.placement;
                for y in 0..12_i32 {
                    for x in 0..12_i32 {
                        let gx = x - 4;
                        let ax = gx - p.left;
                        let ay = y - (8 - p.top);
                        for channel in 0..3 {
                            let expected = reference[((y * 12 + x) * 4) as usize + channel];
                            let pixel = if ax >= 0
                                && ax < p.width as i32
                                && ay >= 0
                                && ay < p.height as i32
                            {
                                actual.data
                                    [((ay as u32 * p.width + ax as u32) * 4) as usize + channel]
                            } else {
                                0
                            };
                            assert!(
                                pixel.abs_diff(expected) <= 1,
                                "lost coverage at ({gx}, {y}), phase {phase}, channel {channel}: {pixel} != {expected}"
                            );
                            if expected > 0
                                && (gx < (left + phase).floor() as i32
                                    || gx >= (left + 0.75 + phase).ceil() as i32)
                            {
                                fringes += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(
            fringes > 0,
            "reference must exercise coverage outside ordinary outline bounds"
        );
    }

    #[test]
    fn expanded_lcd_glyphs_preserve_swash_green_coverage_and_placement() {
        let font = swash::FontRef::from_index(sui_text::BUNDLED_NOTO_SANS_REGULAR_FONT, 0).unwrap();
        let mut context = swash::scale::ScaleContext::new();
        let mut rasterizer = LcdRasterizer::default();
        for size in [12.0, 15.0, 22.5] {
            let mut scaler = context.builder(font).size(size).hint(true).build();
            for ch in ['|', 'A', 'j'] {
                let id = font.charmap().map(ch);
                for phase in [0.0, 0.25, 0.5, 0.75] {
                    let offset = Vector::new(phase, 0.0);
                    let gray = swash::scale::Render::new(&[Source::Outline])
                        .format(Format::Alpha)
                        .offset(offset)
                        .render(&mut scaler, id)
                        .unwrap();
                    let lcd = rasterizer.render(&mut scaler, id, offset).unwrap();
                    assert_eq!(lcd.placement.top, gray.placement.top);
                    assert_eq!(lcd.placement.height, gray.placement.height);
                    assert!(lcd.placement.left <= gray.placement.left);
                    assert!(
                        lcd.placement.left + lcd.placement.width as i32
                            >= gray.placement.left + gray.placement.width as i32
                    );
                    for y in 0..lcd.placement.height {
                        for x in 0..lcd.placement.width {
                            let gx = lcd.placement.left + x as i32 - gray.placement.left;
                            let expected = if gx >= 0 && gx < gray.placement.width as i32 {
                                gray.data[(y * gray.placement.width + gx as u32) as usize]
                            } else {
                                0
                            };
                            let actual = lcd.data[((y * lcd.placement.width + x) * 4 + 1) as usize];
                            assert!(actual.abs_diff(expected) <= 1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn empty_lcd_outline_has_no_bitmap_or_allocation() {
        let image = render_lcd_mask(&[] as &[Command], Vector::ZERO, &mut Scratch::new());
        assert_eq!(image.placement.width, 0);
        assert_eq!(image.placement.height, 0);
        assert!(image.data.is_empty());
    }
}
