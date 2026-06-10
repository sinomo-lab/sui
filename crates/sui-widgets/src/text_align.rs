use sui_core::{Rect, Size};
use sui_runtime::{PaintCtx, window_render_options};
use sui_text::{TextLayout, TextMeasurement, TextStyle};

pub(crate) fn aligned_text_rect_for_text(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
) -> Rect {
    let shaped_measurement = ctx
        .shape_text(
            text.to_string(),
            Size::new(rect.width().max(1.0), f32::INFINITY),
            style.clone(),
        )
        .ok()
        .map(|layout| layout.measurement());
    let fallback_measurement = || TextMeasurement {
        width: text.chars().count() as f32 * style.font_size * 0.56,
        height: style.line_height,
        bounds: Rect::ZERO,
        ascent: style.font_size,
        descent: 0.0,
        cap_height: Some(style.font_size),
    };
    let measurement = shaped_measurement.unwrap_or_else(|| {
        ctx.measure_text(text.to_string(), style.clone())
            .ok()
            .unwrap_or_else(fallback_measurement)
    });
    let width = measurement.width.min(rect.width()).max(0.0);
    let height = line_height.max(measurement.height).min(rect.height());
    let x = rect.x() + ((rect.width() - width).max(0.0) * horizontal_alignment.clamp(0.0, 1.0));
    let y = ctx
        .shape_text(
            text.to_string(),
            Size::new(width.max(1.0), height.max(1.0)),
            style.clone(),
        )
        .ok()
        .and_then(|layout| {
            let line = layout.lines().first()?;
            Some(
                rect.y() + (rect.height() * 0.5)
                    - line.baseline
                    - visual_center(ctx, layout.measurement()),
            )
        })
        .unwrap_or_else(|| vertically_centered_text_rect_y(ctx, rect, measurement, height));

    Rect::new(x, y, width, height)
}

pub(crate) fn aligned_text_rect_for_layout(
    ctx: &PaintCtx,
    rect: Rect,
    layout: &TextLayout,
    line_height: f32,
    horizontal_alignment: f32,
) -> Rect {
    let measurement = layout.measurement();
    let width = measurement.width.min(rect.width()).max(0.0);
    let height = line_height.max(measurement.height).min(rect.height());
    let x = rect.x() + ((rect.width() - width).max(0.0) * horizontal_alignment.clamp(0.0, 1.0));
    let y = layout
        .lines()
        .first()
        .map(|line| {
            rect.y() + (rect.height() * 0.5) - line.baseline - visual_center(ctx, measurement)
        })
        .unwrap_or_else(|| vertically_centered_text_rect_y(ctx, rect, measurement, height));

    Rect::new(x, y, width, height)
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
