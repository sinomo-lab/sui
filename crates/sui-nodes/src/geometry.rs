use sui_core::Point;

use crate::{EdgeKind, EdgePathOptions, HandlePosition};

/// Shared by drawing and the spatial index so culling and hit testing cannot
/// reject a curve whose configured bend extends beyond the default curvature.
pub(crate) fn bezier_control_points(
    source: Point,
    source_side: HandlePosition,
    target: Point,
    target_side: HandlePosition,
    kind: EdgeKind,
    options: EdgePathOptions,
) -> (Point, Point) {
    let delta = target - source;
    let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
    let curvature = if kind == EdgeKind::SimpleBezier {
        options.curvature * 0.6
    } else {
        options.curvature
    };
    let bend = (distance * curvature.clamp(0.0, 1.5)).clamp(24.0, 240.0);
    let control = |point: Point, side| match side {
        HandlePosition::Left => Point::new(point.x - bend, point.y),
        HandlePosition::Right => Point::new(point.x + bend, point.y),
        HandlePosition::Top => Point::new(point.x, point.y - bend),
        HandlePosition::Bottom => Point::new(point.x, point.y + bend),
    };
    (control(source, source_side), control(target, target_side))
}
