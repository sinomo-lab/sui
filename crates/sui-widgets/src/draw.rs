//! Small helpers widgets share to lay out and paint: rect geometry, color
//! mixing, and text measurement.

use crate::Interpolate;
use sui_core::{Color, Point, Rect};
use sui_layout::Padding as Insets;
use sui_runtime::MeasureCtx;
use sui_text::{TextMeasurement, TextStyle};

/// `rect` moved in by `insets` on each side, never below zero size.
pub(crate) fn inset_rect(rect: Rect, insets: Insets) -> Rect {
    Rect::new(
        rect.x() + insets.left,
        rect.y() + insets.top,
        (rect.width() - insets.left - insets.right).max(0.0),
        (rect.height() - insets.top - insets.bottom).max(0.0),
    )
}

/// `rect` moved in by `amount` on every side, but no further than its
/// middle, so a small rect shrinks to a line or a point instead of turning
/// inside out.
pub(crate) fn inset_rect_evenly(rect: Rect, amount: f32) -> Rect {
    let horizontal = amount.min(rect.width() * 0.5);
    let vertical = amount.min(rect.height() * 0.5);
    Rect::new(
        rect.x() + horizontal,
        rect.y() + vertical,
        (rect.width() - horizontal * 2.0).max(0.0),
        (rect.height() - vertical * 2.0).max(0.0),
    )
}

/// The point in the middle of `rect`.
pub(crate) fn rect_center(rect: Rect) -> Point {
    Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    )
}

/// `from` blended toward `to` by `amount`, in OKLab as theme transitions
/// blend. Colors brighter than SDR white stay bright; clamp the result where
/// that is unwanted.
pub(crate) fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    Color::interpolate(from, to, amount)
}

/// How `text` lays out in `style`, or an empty line of `style`'s height when
/// the text system cannot measure it.
pub(crate) fn measure_text(ctx: &mut MeasureCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.layout()
        .measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: 0.0,
            height: style.line_height,
            bounds: Rect::new(0.0, 0.0, 0.0, style.line_height),
            ascent: style.font_size,
            descent: 0.0,
            cap_height: Some(style.font_size),
        })
}
