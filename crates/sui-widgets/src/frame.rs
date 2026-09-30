//! Bordered boxes drawn on the physical pixel grid. A border lies inside its
//! box, as in CSS, and the box's edges are moved to whole physical pixels,
//! so a one-pixel border covers one row of pixels instead of blending across
//! two at half strength.

use crate::ControlMetrics;
use sui_core::{Color, Path, Rect};
use sui_runtime::PaintCtx;
use sui_scene::StrokeStyle;

/// A length given in physical pixels, such as a theme's border or focus ring
/// width, in logical pixels.
pub(crate) fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    ctx.dpi().physical_pixels_to_logical(value)
}

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

/// Draws a border along the inside of the rounded `rect`, both snapped to
/// physical pixels. `width` is in logical pixels and is rounded to whole
/// physical ones.
pub(crate) fn stroke_border(ctx: &mut PaintCtx, rect: Rect, radius: f32, width: f32, color: Color) {
    let width = snap_width_to_pixels(ctx, width);
    if width <= 0.0 {
        return;
    }
    let rect = snap_to_pixels(ctx, rect);
    if radius <= 0.0 {
        // The renderer draws a rectangle's border inside it.
        ctx.stroke_rect(rect, color, StrokeStyle::new(width));
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
    stroke_border(ctx, bounds, radius, border_width, border);
}

/// Draws the theme's focus ring around the rounded box `bounds`, outset and
/// as wide as `metrics` give in physical pixels, with the box snapped to
/// physical pixels so the ring lines up with its border.
pub(crate) fn draw_focus_ring(
    ctx: &mut PaintCtx,
    bounds: Rect,
    radius: f32,
    metrics: ControlMetrics,
    color: Color,
) {
    let bounds = snap_to_pixels(ctx, bounds);
    let outset = physical_pixels(ctx, metrics.focus_ring_outset);
    ctx.stroke(
        rounded_rect(bounds.inflate(outset, outset), radius + outset),
        color,
        StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
    );
}

fn rounded_rect(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(
        rect,
        radius.min(rect.width().min(rect.height()) * 0.5).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_core::{Event, Result, Size, WindowEvent};
    use sui_layout::Constraints;
    use sui_runtime::{Application, MeasureCtx, RenderOutput, Widget, WindowBuilder};
    use sui_scene::SceneCommand;

    /// A box off the pixel grid at every scale below.
    const BOX: Rect = Rect::new(10.5, 20.3, 40.0, 20.0);

    /// Paints one bordered box with a one-pixel border.
    struct Frame {
        radius: f32,
    }

    impl Widget for Frame {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(100.0, 100.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let width = physical_pixels(ctx, 1.0);
            draw_control_shape(ctx, BOX, self.radius, width, Color::WHITE, Color::BLACK);
        }
    }

    fn render(radius: f32, scale_factor: f64) -> Result<RenderOutput> {
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Frame").root(Frame { radius }))
            .build()?;
        let window_id = runtime.window_ids()[0];
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::ScaleFactorChanged {
                scale_factor,
                raw_dpi: None,
                suggested_size: None,
            }),
        )?;
        runtime.render(window_id)
    }

    fn filled(output: &RenderOutput) -> Rect {
        output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::FillPath { path, .. } => Some(path.bounds()),
                _ => None,
            })
            .expect("the box is filled")
    }

    fn edges(rect: Rect) -> [f32; 4] {
        [rect.x(), rect.y(), rect.max_x(), rect.max_y()]
    }

    fn near(left: Rect, right: Rect, tolerance: f32) -> bool {
        edges(left)
            .into_iter()
            .zip(edges(right))
            .all(|(left, right)| (left - right).abs() <= tolerance + 1e-3)
    }

    #[test]
    fn a_box_and_its_border_land_on_whole_physical_pixels() -> Result<()> {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let output = render(4.0, f64::from(scale))?;
            let fill = filled(&output);
            assert!(
                edges(fill)
                    .into_iter()
                    .all(|edge| ((edge * scale) - (edge * scale).round()).abs() < 1e-3),
                "{fill:?} is off the pixel grid at {scale}"
            );
            assert!(
                near(fill, BOX, 0.5 / scale),
                "{fill:?} moved more than half a pixel from {BOX:?} at {scale}"
            );
            let (path, stroke) = output
                .frame
                .scene
                .commands()
                .iter()
                .find_map(|command| match command {
                    SceneCommand::StrokePath { path, stroke, .. } => Some((path.bounds(), *stroke)),
                    _ => None,
                })
                .expect("the border is stroked");
            assert!(
                (stroke.width * scale - 1.0).abs() < 1e-3,
                "{stroke:?} at {scale}"
            );
            // Centered on a path half its width inside the box, the border's
            // outer edge is the box's edge.
            let half = stroke.width * 0.5;
            assert!(
                near(path.inflate(half, half), fill, 0.0),
                "{path:?} in {fill:?} at {scale}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_square_box_draws_its_border_as_a_rect_inside_it() -> Result<()> {
        let output = render(0.0, 1.25)?;
        let fill = filled(&output);
        let (rect, stroke) = output
            .frame
            .scene
            .commands()
            .iter()
            .find_map(|command| match command {
                SceneCommand::StrokeRect { rect, stroke, .. } => Some((*rect, *stroke)),
                _ => None,
            })
            .expect("the border is a rect border");
        assert!(near(rect, fill, 0.0), "{rect:?} is not {fill:?}");
        assert!((stroke.width * 1.25 - 1.0).abs() < 1e-3);
        Ok(())
    }
}
