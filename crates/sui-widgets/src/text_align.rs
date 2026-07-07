use sui_core::{Color, Point, Rect, Size};
use sui_runtime::{PaintCtx, window_render_options};
use sui_text::{
    TextAlign, TextDocument, TextLayout, TextLayoutRequest, TextMeasurement, TextStyle, TextWrap,
};

pub(crate) struct AlignedTextLayout {
    pub(crate) rect: Rect,
    origin: Point,
    layout: TextLayout,
    color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HorizontalTextAlignmentMode {
    Advance,
    Optical,
}

#[derive(Debug, Clone, Copy)]
struct HorizontalPlacement {
    rect_x: f32,
    rect_width: f32,
    origin_x: f32,
}

pub(crate) fn aligned_text_rect_for_text(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
) -> Rect {
    aligned_text_rect_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        HorizontalTextAlignmentMode::Advance,
    )
}

pub(crate) fn aligned_text_rect_for_text_with_mode(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> Rect {
    if let Some(aligned) = aligned_text_layout_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    ) {
        return aligned.rect;
    }

    let fallback_measurement = || TextMeasurement {
        width: text.chars().count() as f32 * style.font_size * 0.56,
        height: style.line_height,
        bounds: Rect::ZERO,
        ascent: style.font_size,
        descent: 0.0,
        cap_height: Some(style.font_size),
    };
    let measurement = ctx
        .measure_text(text.to_string(), style.clone())
        .ok()
        .unwrap_or_else(fallback_measurement);
    let placement = horizontal_placement(rect, measurement, horizontal_alignment, horizontal_mode);
    let height = line_height.max(measurement.height).min(rect.height());
    let y = vertically_centered_text_rect_y(ctx, rect, measurement, height);

    Rect::new(placement.rect_x, y, placement.rect_width, height)
}

pub(crate) fn aligned_text_layout_for_text_with_mode(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> Option<AlignedTextLayout> {
    aligned_text_layout_for_text_with_mode_and_wrap(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
        TextWrap::Word,
    )
}

fn aligned_text_layout_for_text_with_mode_and_wrap(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
    wrap: TextWrap,
) -> Option<AlignedTextLayout> {
    let color = style.color;
    let mut layout_style = style.clone();
    layout_style.color = Color::WHITE;
    let mut document = TextDocument::from_plain_text(text.to_string(), layout_style);
    let paragraph_align = paragraph_alignment(horizontal_alignment, horizontal_mode);
    for paragraph in &mut document.paragraphs {
        paragraph.style.align = paragraph_align;
        paragraph.style.wrap = wrap;
    }
    let layout = ctx
        .layout_text_document(
            TextLayoutRequest::new(document)
                .with_box_size(Size::new(rect.width().max(1.0), rect.height().max(1.0))),
        )
        .ok()?;
    let measurement = layout.measurement();
    let placement = horizontal_placement(rect, measurement, horizontal_alignment, horizontal_mode);
    let aligned_rect = aligned_text_rect_for_layout_with_mode(
        ctx,
        rect,
        &layout,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    );
    Some(AlignedTextLayout {
        origin: Point::new(placement.origin_x, aligned_rect.y()),
        rect: aligned_rect,
        layout,
        color,
    })
}

pub(crate) fn paint_aligned_text(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
) {
    paint_aligned_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        HorizontalTextAlignmentMode::Optical,
    );
}

pub(crate) fn paint_aligned_text_with_mode(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) {
    if let Some(aligned) = aligned_text_layout_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    ) {
        ctx.draw_text_layout_with_color(aligned.origin, &aligned.layout, aligned.color);
        return;
    }

    let fallback_rect = aligned_text_rect_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    );
    ctx.draw_text(fallback_rect, text.to_string(), style.clone());
}

pub(crate) fn paint_single_line_aligned_text(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
) {
    let horizontal_mode = HorizontalTextAlignmentMode::Optical;
    if let Some(aligned) = aligned_text_layout_for_text_with_mode_and_wrap(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
        TextWrap::NoWrap,
    ) {
        ctx.draw_text_layout_with_color(aligned.origin, &aligned.layout, aligned.color);
        return;
    }

    let fallback_rect = aligned_text_rect_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    );
    ctx.draw_text(fallback_rect, text.to_string(), style.clone());
}

/// Greedy word-wrap `text` to `max_width`, using `measure` for run widths.
///
/// Explicit newlines are preserved. Words longer than `max_width` are kept on
/// their own line rather than split, which matches common UI metadata wrapping.
pub fn wrap_text_lines(
    text: &str,
    max_width: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let space_width = measure(" ");

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        let mut current_width = 0.0_f32;
        for word in paragraph.split(' ') {
            let word_width = measure(word);
            if current.is_empty() {
                current.push_str(word);
                current_width = word_width;
            } else if current_width + space_width + word_width <= max_width {
                current.push(' ');
                current.push_str(word);
                current_width += space_width + word_width;
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
                current_width = word_width;
            }
        }
        lines.push(current);
    }

    lines
}

fn paragraph_alignment(
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> TextAlign {
    if horizontal_mode == HorizontalTextAlignmentMode::Optical {
        return TextAlign::Left;
    }

    let horizontal_alignment = horizontal_alignment.clamp(0.0, 1.0);
    if horizontal_alignment <= 0.0 {
        TextAlign::Left
    } else if horizontal_alignment >= 1.0 {
        TextAlign::Right
    } else {
        TextAlign::Center
    }
}

pub(crate) fn aligned_text_rect_for_layout(
    ctx: &PaintCtx,
    rect: Rect,
    layout: &TextLayout,
    line_height: f32,
    horizontal_alignment: f32,
) -> Rect {
    aligned_text_rect_for_layout_with_mode(
        ctx,
        rect,
        layout,
        line_height,
        horizontal_alignment,
        HorizontalTextAlignmentMode::Advance,
    )
}

pub(crate) fn aligned_text_rect_for_layout_with_mode(
    ctx: &PaintCtx,
    rect: Rect,
    layout: &TextLayout,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> Rect {
    let measurement = layout.measurement();
    let placement = horizontal_placement(rect, measurement, horizontal_alignment, horizontal_mode);
    let height = line_height.max(measurement.height).min(rect.height());
    let y = layout
        .lines()
        .first()
        .map(|line| {
            rect.y() + (rect.height() * 0.5) - line.baseline - visual_center(ctx, measurement)
        })
        .unwrap_or_else(|| vertically_centered_text_rect_y(ctx, rect, measurement, height));

    Rect::new(placement.rect_x, y, placement.rect_width, height)
}

fn horizontal_placement(
    rect: Rect,
    measurement: TextMeasurement,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> HorizontalPlacement {
    let horizontal_alignment = horizontal_alignment.clamp(0.0, 1.0);
    let (width, origin_shift) = match horizontal_mode {
        HorizontalTextAlignmentMode::Advance => (measurement.width, 0.0),
        HorizontalTextAlignmentMode::Optical => {
            let bounds = measurement.bounds;
            if bounds.width().is_finite() && bounds.width() > 0.0 {
                (bounds.width(), -bounds.x())
            } else {
                (measurement.width, 0.0)
            }
        }
    };
    let rect_width = width.min(rect.width()).max(0.0);
    let rect_x = rect.x() + ((rect.width() - rect_width).max(0.0) * horizontal_alignment);

    HorizontalPlacement {
        rect_x,
        rect_width,
        origin_x: rect_x + origin_shift,
    }
}

pub(crate) fn vertically_centered_text_rect_y(
    ctx: &PaintCtx,
    rect: Rect,
    measurement: TextMeasurement,
    height: f32,
) -> f32 {
    let visual_center = visual_center(ctx, measurement);
    let baseline = rect.y() + (rect.height() * 0.5) - visual_center;
    let leading_above = ((height - (measurement.ascent + measurement.descent)).max(0.0)) * 0.5;

    baseline - measurement.ascent - leading_above
}

fn visual_center(ctx: &PaintCtx, measurement: TextMeasurement) -> f32 {
    let optical_centering = window_render_options(ctx.window_id())
        .map(|options| options.optical_vertical_text_alignment_enabled)
        .unwrap_or(true);
    let top = if optical_centering {
        -measurement.cap_height.unwrap_or(measurement.ascent)
    } else {
        -measurement.ascent
    };
    let bottom = if optical_centering {
        measurement.descent * 0.5
    } else {
        measurement.descent
    };
    (top + bottom) * 0.5
}

#[cfg(test)]
mod tests {
    use super::wrap_text_lines;

    #[test]
    fn wrap_text_lines_preserves_newlines_and_keeps_long_words() {
        let lines = wrap_text_lines("alpha beta\nsuperlongword gamma", 10.0, |text| {
            text.chars().count() as f32
        });

        assert_eq!(
            lines,
            vec![
                "alpha beta".to_string(),
                "superlongword".to_string(),
                "gamma".to_string(),
            ]
        );
    }
}
