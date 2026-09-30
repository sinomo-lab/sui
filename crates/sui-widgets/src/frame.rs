//! Bordered boxes drawn on the physical pixel grid. A border lies inside its
//! box, as in CSS, and the box's edges are moved to whole physical pixels,
//! so a one-pixel border covers one row of pixels instead of blending across
//! two at half strength.

use sui_core::{Color, Path, Rect};
use sui_runtime::PaintCtx;
use sui_scene::StrokeStyle;

/// `rect` with each edge moved to the nearest physical pixel. Under a
/// transform that scales or turns, pixels don't line up with the edges, so
/// `rect` is returned as it is.
pub(crate) fn snap_to_pixels(ctx: &PaintCtx, rect: Rect) -> Rect {
    let transform = ctx.presentation_transform();
    if transform.xx != 1.0 || transform.yx != 0.0 || transform.xy != 0.0 || transform.yy != 1.0 {
        return rect;
    }
    let scale = ctx.dpi().pixels_per_point();
    let snap = |value: f32, offset: f32| ((value + offset) * scale).round() / scale - offset;
    let left = snap(rect.x(), transform.dx);
    let top = snap(rect.y(), transform.dy);
    let right = snap(rect.max_x(), transform.dx);
    let bottom = snap(rect.max_y(), transform.dy);
    Rect::new(left, top, right - left, bottom - top)
}

/// `width` rounded to whole physical pixels, and to at least one unless it
/// is zero.
pub(crate) fn snap_width_to_pixels(ctx: &PaintCtx, width: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    let scale = ctx.dpi().pixels_per_point();
    (width * scale).round().max(1.0) / scale
}

/// Draws a border `width` wide along the inside of the rounded `rect`. Snap
/// both to pixels first for a crisp border.
pub(crate) fn stroke_border(ctx: &mut PaintCtx, rect: Rect, radius: f32, width: f32, color: Color) {
    if width <= 0.0 {
        return;
    }
    let half = width * 0.5;
    ctx.stroke(
        rounded_rect(rect.inflate(-half, -half), radius - half),
        color,
        StrokeStyle::new(width),
    );
}

/// Fills a rounded box and draws its border inside it, both on whole physical
/// pixels. `border_width` is in logical pixels and is rounded to whole
/// physical ones.
pub(crate) fn draw_control_shape(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    border_width: f32,
    background: Color,
    border: Color,
) {
    let bounds = snap_to_pixels(ctx, bounds);
    ctx.fill(rounded_rect(bounds, radius), background);
    let border_width = snap_width_to_pixels(ctx, border_width);
    stroke_border(ctx, bounds, radius, border_width, border);
}

fn rounded_rect(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(
        rect,
        radius.min(rect.width().min(rect.height()) * 0.5).max(0.0),
    )
}
