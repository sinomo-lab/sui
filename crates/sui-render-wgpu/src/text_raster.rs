//! LCD outline rasterization with bounds covering all physical channel samples.
use swash::scale::{
    Scaler, Source,
    image::{Content, Image},
    outline::Outline,
};
use swash::zeno::{Command, Fill, Mask, Origin, PathData, Placement, Scratch, Vector};

const LCD_X_SAMPLES: u32 = 6;
const MAX_LCD_SAMPLES: u32 = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum LcdSampling {
    Asymmetric,
    Symmetric,
}

impl LcdSampling {
    fn vertical_samples(self) -> u32 {
        match self {
            Self::Asymmetric => 1,
            Self::Symmetric => 5,
        }
    }
}

/// Reusable scratch for discrete LCD coverage. Font hinting and layout happen
/// before this stage; the sample grid never changes advances or glyph origins.
#[derive(Default)]
pub(crate) struct LcdMaskRasterizer {
    builder: tiny_skia::PathBuilder,
    samples: Option<tiny_skia::Mask>,
    column_sums: Vec<u16>,
    scratch: Scratch,
}

impl LcdMaskRasterizer {
    pub(crate) fn render(
        &mut self,
        path: impl PathData,
        offset: Vector,
        sampling: LcdSampling,
    ) -> Image {
        let bounds = self.scratch.bounds(&path, Fill::NonZero, None);
        let Some(placement) = lcd_placement(bounds, offset) else {
            return empty_lcd_image();
        };
        let ny = sampling.vertical_samples();
        let dimensions = placement
            .width
            .checked_mul(LCD_X_SAMPLES)
            .zip(placement.height.checked_mul(ny));
        let Some((width, height)) = dimensions.filter(|(w, h)| {
            // Include the u16 column sums and two zero columns on either side.
            h.checked_add(2)
                .and_then(|h| w.checked_mul(h))
                .and_then(|n| n.checked_add(8))
                .is_some_and(|n| n <= MAX_LCD_SAMPLES)
        }) else {
            // Keep scratch bounded for large or unusually wide glyphs.
            return render_lcd_mask(path, offset, &mut self.scratch);
        };

        self.builder.clear();
        for command in path.commands() {
            match command {
                Command::MoveTo(p) => self.builder.move_to(p.x, p.y),
                Command::LineTo(p) => self.builder.line_to(p.x, p.y),
                Command::QuadTo(c, p) => self.builder.quad_to(c.x, c.y, p.x, p.y),
                Command::CurveTo(a, b, p) => self.builder.cubic_to(a.x, a.y, b.x, b.y, p.x, p.y),
                Command::Close => self.builder.close(),
            }
        }
        let Some(outline) = std::mem::take(&mut self.builder).finish() else {
            return render_lcd_mask(path, offset, &mut self.scratch);
        };
        if self
            .samples
            .as_ref()
            .is_none_or(|mask| mask.width() != width || mask.height() != height)
        {
            self.samples = None;
            let columns = width as usize + 4;
            let sample_bytes = width as usize * height as usize;
            if self.column_sums.capacity() < columns
                || self.column_sums.capacity() * std::mem::size_of::<u16>() + sample_bytes
                    > MAX_LCD_SAMPLES as usize
            {
                // A wide glyph followed by a tall one must not retain column
                // capacity that pushes the combined scratch over the budget.
                self.column_sums = vec![0; columns];
            } else {
                self.column_sums.resize(columns, 0);
            }
            self.samples = tiny_skia::Mask::new(width, height);
        }
        let Some(mask) = self.samples.as_mut() else {
            self.builder = outline.clear();
            return render_lcd_mask(path, offset, &mut self.scratch);
        };
        mask.clear();
        mask.fill_path(
            &outline,
            tiny_skia::FillRule::Winding,
            false,
            tiny_skia::Transform::from_row(
                LCD_X_SAMPLES as f32,
                0.0,
                0.0,
                -(ny as f32),
                (offset.x - placement.left as f32) * LCD_X_SAMPLES as f32,
                (placement.top as f32 - offset.y) * ny as f32,
            ),
        );
        self.builder = outline.clear();

        let mut image = empty_lcd_image();
        image.placement = placement;
        image
            .data
            .resize((placement.width * placement.height * 4) as usize, 0);
        let denominator = LCD_X_SAMPLES * ny;
        for y in 0..placement.height {
            self.column_sums.fill(0);
            for sy in 0..ny {
                let start = ((y * ny + sy) * width) as usize;
                for (sum, sample) in self.column_sums[2..width as usize + 2]
                    .iter_mut()
                    .zip(&mask.data()[start..start + width as usize])
                {
                    *sum += u16::from(*sample);
                }
            }
            // A sliding six-sample window averages a pixel's coverage. Moving
            // two samples at a time visits R/G/B without summing overlapping
            // windows repeatedly. The zero columns retain the fringe bounds.
            let mut sum: u32 = self.column_sums[..6].iter().map(|v| u32::from(*v)).sum();
            let count = placement.width as usize * 3;
            for sample in 0..count {
                let x = sample / 3;
                let channel = sample % 3;
                // Swash subpixel masks are BGRA; the atlas converter handles
                // the requested physical RGB/BGR display order.
                image.data[(y as usize * placement.width as usize + x) * 4 + 2 - channel] =
                    ((sum + denominator / 2) / denominator) as u8;
                if sample + 1 < count {
                    let start = sample * 2;
                    sum -=
                        u32::from(self.column_sums[start]) + u32::from(self.column_sums[start + 1]);
                    sum += u32::from(self.column_sums[start + 6])
                        + u32::from(self.column_sums[start + 7]);
                }
            }
        }
        if image.data.iter().all(|value| *value == 0) {
            // A feature smaller than the sampling grid can fall between samples.
            // Retain its area coverage rather than silently dropping the glyph.
            return render_lcd_mask(path, offset, &mut self.scratch);
        }
        image
    }
}

#[derive(Default)]
pub(crate) struct LcdRasterizer {
    outline: Outline,
    mask: LcdMaskRasterizer,
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
        Some(
            self.mask
                .render(self.outline.path(), offset, LcdSampling::Symmetric),
        )
    }
}

pub(crate) fn render_lcd_mask(path: impl PathData, offset: Vector, scratch: &mut Scratch) -> Image {
    let mut image = empty_lcd_image();
    let bounds = scratch.bounds(&path, Fill::NonZero, None);
    let Some(placement) = lcd_placement(bounds, offset) else {
        return image;
    };
    image.placement = placement;
    let format = crate::text::lcd_bgra_format();
    image.data.resize(
        format.buffer_size(image.placement.width, image.placement.height),
        0,
    );
    Mask::with_scratch(path, scratch)
        .format(format)
        .origin(Origin::BottomLeft)
        .size(image.placement.width, image.placement.height)
        .offset(Vector::new(
            offset.x - placement.left as f32,
            offset.y - (placement.top as f32 - placement.height as f32),
        ))
        .render_into(&mut image.data, None);
    image
}

fn empty_lcd_image() -> Image {
    Image {
        source: Source::Outline,
        content: Content::SubpixelMask,
        ..Image::default()
    }
}

fn lcd_placement(bounds: swash::zeno::Bounds, offset: Vector) -> Option<Placement> {
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return None;
    }
    // Zeno's automatic bounds omit CustomSubpixel offsets. Include their
    // horizontal footprint before rasterizing; empty atlas padding is too late.
    let left = (bounds.min.x + offset.x - crate::text::LCD_SUBPIXEL_OFFSET).floor();
    let right = (bounds.max.x + offset.x + crate::text::LCD_SUBPIXEL_OFFSET).ceil();
    let bottom = (bounds.min.y + offset.y).floor();
    let top = (bounds.max.y + offset.y).ceil();
    Some(Placement {
        left: left as i32,
        top: top as i32,
        width: (right - left) as u32,
        height: (top - bottom) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use swash::zeno::{Command, Format, PathBuilder};

    fn rectangle(left: f32, bottom: f32, right: f32, top: f32) -> Vec<Command> {
        let mut path = Vec::new();
        path.move_to((left, bottom))
            .line_to((right, bottom))
            .line_to((right, top))
            .line_to((left, top))
            .close();
        path
    }

    fn rgb_at(image: &Image, x: i32, y: i32) -> [u8; 3] {
        let p = image.placement;
        let column = x - p.left;
        let row = p.top - y - 1;
        if column < 0 || column >= p.width as i32 || row < 0 || row >= p.height as i32 {
            return [0; 3];
        }
        let i = ((row as u32 * p.width + column as u32) * 4) as usize;
        [image.data[i + 2], image.data[i + 1], image.data[i]]
    }

    fn placement(image: &Image) -> (i32, i32, u32, u32) {
        let p = image.placement;
        (p.left, p.top, p.width, p.height)
    }

    #[test]
    fn discrete_lcd_sampling_preserves_physical_channel_order_and_fringe_bounds() {
        let mut rasterizer = LcdMaskRasterizer::default();
        for mode in [LcdSampling::Asymmetric, LcdSampling::Symmetric] {
            let stem = rectangle(0.5, 0.0, 1.5, 2.0);
            let image = rasterizer.render(stem.as_slice(), Vector::ZERO, mode);
            assert_eq!(rgb_at(&image, 0, 0), [43, 128, 213]);
            assert_eq!(rgb_at(&image, 1, 0), [213, 128, 43]);
            let stem = rectangle(0.0, 0.0, 1.0, 2.0);
            let image = rasterizer.render(stem.as_slice(), Vector::ZERO, mode);
            assert_eq!(rgb_at(&image, -1, 0), [0, 0, 85]);
            assert_eq!(rgb_at(&image, 1, 0), [85, 0, 0]);
        }
    }

    #[test]
    fn font_sampling_modes_control_vertical_antialiasing_without_moving_the_glyph() {
        let mut rasterizer = LcdMaskRasterizer::default();
        let bar = rectangle(0.0, 0.2, 4.0, 0.6);
        let natural = rasterizer.render(bar.as_slice(), Vector::ZERO, LcdSampling::Asymmetric);
        let symmetric = rasterizer.render(bar.as_slice(), Vector::ZERO, LcdSampling::Symmetric);
        assert_eq!(placement(&natural), placement(&symmetric));
        assert_eq!(rgb_at(&natural, 1, 0), [255; 3]);
        assert_eq!(rgb_at(&symmetric, 1, 0), [102; 3]);
    }

    #[test]
    fn discrete_lcd_sampling_keeps_counters_empty_and_is_translation_invariant() {
        let mut path = rectangle(0.0, 0.0, 3.0, 3.0);
        path.move_to((1.0, 1.0))
            .line_to((1.0, 2.0))
            .line_to((2.0, 2.0))
            .line_to((2.0, 1.0))
            .close();
        let mut rasterizer = LcdMaskRasterizer::default();
        for mode in [LcdSampling::Asymmetric, LcdSampling::Symmetric] {
            for phase in [0.0, 0.25, 0.5, 0.75] {
                let a = rasterizer.render(path.as_slice(), Vector::new(phase, 0.0), mode);
                let b = rasterizer.render(path.as_slice(), Vector::new(phase - 3.0, 2.0), mode);
                assert_eq!(a.data, b.data);
                assert_eq!(a.placement.left - 3, b.placement.left);
                assert_eq!(a.placement.top + 2, b.placement.top);
                if phase == 0.0 {
                    assert_eq!(rgb_at(&a, 1, 1)[1], 0);
                }
            }
        }
    }

    #[test]
    fn oversized_and_subsample_features_use_bounded_analytic_fallback() {
        let mut rasterizer = LcdMaskRasterizer::default();
        for path in [
            rectangle(0.0, 0.0, 512.0, 300.0),
            rectangle(0.01, 0.01, 0.075, 0.075),
        ] {
            let reference = render_lcd_mask(path.as_slice(), Vector::ZERO, &mut Scratch::new());
            let actual = rasterizer.render(path.as_slice(), Vector::ZERO, LcdSampling::Symmetric);
            assert_eq!(placement(&actual), placement(&reference));
            assert_eq!(actual.data, reference.data);
            assert!(reference.data.iter().any(|value| *value > 0));
        }
        let storage = rasterizer
            .samples
            .as_ref()
            .map_or(0, |mask| mask.data().len())
            + rasterizer.column_sums.capacity() * std::mem::size_of::<u16>();
        assert!(storage <= MAX_LCD_SAMPLES as usize);
    }

    #[test]
    fn sampled_scratch_stays_bounded_when_switching_from_wide_to_tall_glyphs() {
        let mut rasterizer = LcdMaskRasterizer::default();
        for path in [
            rectangle(0.0, 0.0, 8192.0, 1.0),
            rectangle(0.0, 0.0, 512.0, 270.0),
        ] {
            let image = rasterizer.render(path.as_slice(), Vector::ZERO, LcdSampling::Symmetric);
            assert!(image.data.iter().any(|value| *value != 0));
            let storage = rasterizer.samples.as_ref().unwrap().data().len()
                + rasterizer.column_sums.capacity() * std::mem::size_of::<u16>();
            assert!(storage <= MAX_LCD_SAMPLES as usize);
        }
    }

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
        let mut scratch = Scratch::new();
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
                    let outline = scaler.scale_outline(id).unwrap();
                    let lcd = render_lcd_mask(outline.path(), offset, &mut scratch);
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
