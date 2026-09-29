use crate::draw::ImageRasterSize;
use crate::draw::SceneRasterState;
use crate::gpu::Vertex;
use crate::output::shader_color;
use crate::scene::analytic_coverage_outset;
use sui_core::Color;
use sui_core::ColorSpace;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;
use sui_scene::Brush;
use sui_scene::GradientStop;

pub(crate) fn append_image(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    source: &sui_scene::ImageSource,
    image_size: (u32, u32),
    viewport: Size,
) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let transformed = state.current_transform.transform_rect_bbox(rect);
    let Some(visible) = (match state.current_clip_bounds() {
        Some(clip) => transformed.intersection(clip),
        None => Some(transformed),
    }) else {
        return;
    };

    if transformed.width() <= 0.0 || transformed.height() <= 0.0 {
        return;
    }

    let image_width = image_size.0 as f32;
    let image_height = image_size.1 as f32;
    let source_rect = source
        .source_rect
        .unwrap_or(Rect::new(0.0, 0.0, image_width, image_height));
    let source_min_x = source_rect.x().clamp(0.0, image_width);
    let source_min_y = source_rect.y().clamp(0.0, image_height);
    let source_max_x = source_rect.max_x().clamp(source_min_x, image_width);
    let source_max_y = source_rect.max_y().clamp(source_min_y, image_height);
    if source_max_x <= source_min_x || source_max_y <= source_min_y {
        return;
    }

    let u0 = source_min_x / image_width;
    let v0 = source_min_y / image_height;
    let u1 = source_max_x / image_width;
    let v1 = source_max_y / image_height;
    let tint = source.tint.unwrap_or(Color::WHITE).clamped().to_array();

    let axis_aligned = state.current_transform.yx.abs() < 0.0001
        && state.current_transform.xy.abs() < 0.0001
        && state.current_transform.xx >= 0.0
        && state.current_transform.yy >= 0.0;
    if !axis_aligned {
        let transformed_bounds = state.current_transform.transform_rect_bbox(rect);
        let visible = match state.current_clip_bounds() {
            Some(clip) => transformed_bounds.intersection(clip),
            None => Some(transformed_bounds),
        };
        if visible.is_none() {
            return;
        }

        let top_left = state.current_transform.transform_point(rect.origin);
        let top_right = state
            .current_transform
            .transform_point(Point::new(rect.max_x(), rect.y()));
        let bottom_left = state
            .current_transform
            .transform_point(Point::new(rect.x(), rect.max_y()));
        let bottom_right = state
            .current_transform
            .transform_point(Point::new(rect.max_x(), rect.max_y()));
        let top_left = to_ndc(top_left.x, top_left.y, viewport);
        let top_right = to_ndc(top_right.x, top_right.y, viewport);
        let bottom_left = to_ndc(bottom_left.x, bottom_left.y, viewport);
        let bottom_right = to_ndc(bottom_right.x, bottom_right.y, viewport);

        vertices.extend_from_slice(&[
            Vertex::basic(top_left, tint, [u0, v0], [0.0; 4]),
            Vertex::basic(top_right, tint, [u1, v0], [0.0; 4]),
            Vertex::basic(bottom_left, tint, [u0, v1], [0.0; 4]),
            Vertex::basic(bottom_left, tint, [u0, v1], [0.0; 4]),
            Vertex::basic(top_right, tint, [u1, v0], [0.0; 4]),
            Vertex::basic(bottom_right, tint, [u1, v1], [0.0; 4]),
        ]);
        return;
    }

    let left = ((visible.x() - transformed.x()) / transformed.width()).clamp(0.0, 1.0);
    let right = ((visible.max_x() - transformed.x()) / transformed.width()).clamp(0.0, 1.0);
    let top = ((visible.y() - transformed.y()) / transformed.height()).clamp(0.0, 1.0);
    let bottom = ((visible.max_y() - transformed.y()) / transformed.height()).clamp(0.0, 1.0);

    let uv_left = u0 + ((u1 - u0) * left);
    let uv_right = u0 + ((u1 - u0) * right);
    let uv_top = v0 + ((v1 - v0) * top);
    let uv_bottom = v0 + ((v1 - v0) * bottom);
    let min = to_ndc(visible.x(), visible.y(), viewport);
    let max = to_ndc(visible.max_x(), visible.max_y(), viewport);

    vertices.extend_from_slice(&[
        Vertex::basic([min[0], min[1]], tint, [uv_left, uv_top], [0.0; 4]),
        Vertex::basic([max[0], min[1]], tint, [uv_right, uv_top], [0.0; 4]),
        Vertex::basic([min[0], max[1]], tint, [uv_left, uv_bottom], [0.0; 4]),
        Vertex::basic([min[0], max[1]], tint, [uv_left, uv_bottom], [0.0; 4]),
        Vertex::basic([max[0], min[1]], tint, [uv_right, uv_top], [0.0; 4]),
        Vertex::basic([max[0], max[1]], tint, [uv_right, uv_bottom], [0.0; 4]),
    ]);
}

pub(crate) fn append_image_quad(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    points: [Point; 4],
    source: &sui_scene::ImageSource,
    image_size: (u32, u32),
    viewport: Size,
) {
    if viewport.is_empty() {
        return;
    }

    let points = points.map(|point| state.current_transform.transform_point(point));
    let bounds = points_bounds(&points);
    let Some(_) = (match state.current_clip_bounds() {
        Some(clip) => bounds.intersection(clip),
        None => Some(bounds),
    }) else {
        return;
    };

    let image_width = image_size.0 as f32;
    let image_height = image_size.1 as f32;
    let source_rect = source
        .source_rect
        .unwrap_or(Rect::new(0.0, 0.0, image_width, image_height));
    let source_min_x = source_rect.x().clamp(0.0, image_width);
    let source_min_y = source_rect.y().clamp(0.0, image_height);
    let source_max_x = source_rect.max_x().clamp(source_min_x, image_width);
    let source_max_y = source_rect.max_y().clamp(source_min_y, image_height);
    if source_max_x <= source_min_x || source_max_y <= source_min_y {
        return;
    }

    let u0 = source_min_x / image_width;
    let v0 = source_min_y / image_height;
    let u1 = source_max_x / image_width;
    let v1 = source_max_y / image_height;
    let tint = source.tint.unwrap_or(Color::WHITE).clamped().to_array();
    let top_left = to_ndc(points[0].x, points[0].y, viewport);
    let top_right = to_ndc(points[1].x, points[1].y, viewport);
    let bottom_left = to_ndc(points[2].x, points[2].y, viewport);
    let bottom_right = to_ndc(points[3].x, points[3].y, viewport);

    vertices.extend_from_slice(&[
        Vertex::basic(top_left, tint, [u0, v0], [0.0; 4]),
        Vertex::basic(top_right, tint, [u1, v0], [0.0; 4]),
        Vertex::basic(bottom_left, tint, [u0, v1], [0.0; 4]),
        Vertex::basic(bottom_left, tint, [u0, v1], [0.0; 4]),
        Vertex::basic(top_right, tint, [u1, v0], [0.0; 4]),
        Vertex::basic(bottom_right, tint, [u1, v1], [0.0; 4]),
    ]);
}

pub(crate) fn image_rect_raster_metadata(
    transform: Transform,
    rect: Rect,
    source_rect: Option<Rect>,
    image_size: (u32, u32),
    viewport: Size,
    surface_size: Size,
) -> (Rect, ImageRasterSize) {
    let points = [
        transform.transform_point(rect.origin),
        transform.transform_point(Point::new(rect.max_x(), rect.y())),
        transform.transform_point(Point::new(rect.x(), rect.max_y())),
        transform.transform_point(Point::new(rect.max_x(), rect.max_y())),
    ];
    image_points_raster_metadata(points, source_rect, image_size, viewport, surface_size)
}

pub(crate) fn image_quad_raster_metadata(
    transform: Transform,
    points: [Point; 4],
    source_rect: Option<Rect>,
    image_size: (u32, u32),
    viewport: Size,
    surface_size: Size,
) -> (Rect, ImageRasterSize) {
    image_points_raster_metadata(
        points.map(|point| transform.transform_point(point)),
        source_rect,
        image_size,
        viewport,
        surface_size,
    )
}

pub(crate) fn image_points_raster_metadata(
    points: [Point; 4],
    source_rect: Option<Rect>,
    image_size: (u32, u32),
    viewport: Size,
    surface_size: Size,
) -> (Rect, ImageRasterSize) {
    let bounds = points_bounds(&points);
    let scale_x = if viewport.width > 0.0 {
        surface_size.width / viewport.width
    } else {
        1.0
    };
    let scale_y = if viewport.height > 0.0 {
        surface_size.height / viewport.height
    } else {
        1.0
    };
    let physical_distance = |a: Point, b: Point| {
        let dx = (b.x - a.x) * scale_x;
        let dy = (b.y - a.y) * scale_y;
        (dx.mul_add(dx, dy * dy)).sqrt()
    };
    let draw_width =
        physical_distance(points[0], points[1]).max(physical_distance(points[2], points[3]));
    let draw_height =
        physical_distance(points[0], points[2]).max(physical_distance(points[1], points[3]));
    let image_width = image_size.0.max(1) as f32;
    let image_height = image_size.1.max(1) as f32;
    let source_rect = source_rect.unwrap_or(Rect::new(0.0, 0.0, image_width, image_height));
    let source_min_x = source_rect.x().clamp(0.0, image_width);
    let source_min_y = source_rect.y().clamp(0.0, image_height);
    let source_max_x = source_rect.max_x().clamp(source_min_x, image_width);
    let source_max_y = source_rect.max_y().clamp(source_min_y, image_height);
    let source_width = (source_max_x - source_min_x).max(1.0);
    let source_height = (source_max_y - source_min_y).max(1.0);
    let full_width = draw_width * (image_width / source_width);
    let full_height = draw_height * (image_height / source_height);
    (bounds, ImageRasterSize::new(full_width, full_height))
}

pub(crate) fn append_widget_shader_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    shader: sui_scene::WidgetShader,
    viewport: Size,
) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let transformed = state.current_transform.transform_rect_bbox(rect);
    let Some(visible) = (match state.current_clip_bounds() {
        Some(clip) => transformed.intersection(clip),
        None => Some(transformed),
    }) else {
        return;
    };

    if transformed.width() <= 0.0 || transformed.height() <= 0.0 {
        return;
    }

    let left = ((visible.x() - transformed.x()) / transformed.width()).clamp(0.0, 1.0);
    let right = ((visible.max_x() - transformed.x()) / transformed.width()).clamp(0.0, 1.0);
    let top = ((visible.y() - transformed.y()) / transformed.height()).clamp(0.0, 1.0);
    let bottom = ((visible.max_y() - transformed.y()) / transformed.height()).clamp(0.0, 1.0);
    let min = to_ndc(visible.x(), visible.y(), viewport);
    let max = to_ndc(visible.max_x(), visible.max_y(), viewport);
    let (metadata, params) = widget_shader_metadata(shader);

    vertices.extend_from_slice(&[
        Vertex::basic([min[0], min[1]], metadata, [left, top], params),
        Vertex::basic([max[0], min[1]], metadata, [right, top], params),
        Vertex::basic([min[0], max[1]], metadata, [left, bottom], params),
        Vertex::basic([min[0], max[1]], metadata, [left, bottom], params),
        Vertex::basic([max[0], min[1]], metadata, [right, top], params),
        Vertex::basic([max[0], max[1]], metadata, [right, bottom], params),
    ]);
}

/// A single representative color for ops that do not support gradients (stroke rect,
/// fill/stroke path): the solid color, or the first gradient stop, falling back to
/// transparent for an empty stop list. Documented limitation for non-rect gradient use.
pub(crate) fn brush_fallback_color(brush: &Brush) -> Color {
    match brush {
        Brush::Solid(color) => *color,
        Brush::LinearGradient { stops, .. } => {
            stops.first().map(|s| s.color).unwrap_or(Color::TRANSPARENT)
        }
    }
}

/// Emit a 6-vertex (two-triangle) quad for the rounded-rect / gradient pipelines.
///
/// The quad spans `screen_quad` (already transformed + inflated for AA fringe) in NDC,
/// and carries a center-origin rect-local coordinate in attribute 2 so the fragment
/// shader can evaluate a signed-distance field. `center` is the screen-space center of
/// the (un-inflated) rect; `local = corner_screen - center`. The remaining attributes
/// (color, p0, radii, p2, attr6) are constant across the quad.
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_rounded_rect_quad(
    vertices: &mut Vec<Vertex>,
    screen_quad: Rect,
    center: Point,
    viewport: Size,
    color: [f32; 4],
    p0: [f32; 4],
    radii: [f32; 4],
    p2: [f32; 4],
    attr6: [f32; 4],
) {
    if screen_quad.is_empty() || viewport.is_empty() {
        return;
    }

    let min_x = screen_quad.x();
    let min_y = screen_quad.y();
    let max_x = screen_quad.max_x();
    let max_y = screen_quad.max_y();

    let ndc_min = to_ndc(min_x, min_y, viewport);
    let ndc_max = to_ndc(max_x, max_y, viewport);

    let local_min = [min_x - center.x, min_y - center.y];
    let local_max = [max_x - center.x, max_y - center.y];

    let make = |position: [f32; 2], local: [f32; 2]| Vertex {
        position,
        color,
        tex_coords: local,
        shader_params: p0,
        shader_params2: radii,
        shader_params3: p2,
        shader_params4: attr6,
    };

    vertices.extend_from_slice(&[
        make([ndc_min[0], ndc_min[1]], [local_min[0], local_min[1]]),
        make([ndc_max[0], ndc_min[1]], [local_max[0], local_min[1]]),
        make([ndc_min[0], ndc_max[1]], [local_min[0], local_max[1]]),
        make([ndc_min[0], ndc_max[1]], [local_min[0], local_max[1]]),
        make([ndc_max[0], ndc_min[1]], [local_max[0], local_min[1]]),
        make([ndc_max[0], ndc_max[1]], [local_max[0], local_max[1]]),
    ]);
}

pub(crate) fn clipped_screen_quad(state: &SceneRasterState, rect: Rect) -> Option<Rect> {
    match state.current_clip_bounds() {
        Some(clip) => rect.intersection(clip),
        None => Some(rect),
    }
}

/// Clamp per-corner radii to half the smaller rect dimension so the SDF stays valid.
pub(crate) fn clamp_radii(radii: [f32; 4], half_w: f32, half_h: f32) -> [f32; 4] {
    let limit = half_w.min(half_h).max(0.0);
    [
        radii[0].clamp(0.0, limit),
        radii[1].clamp(0.0, limit),
        radii[2].clamp(0.0, limit),
        radii[3].clamp(0.0, limit),
    ]
}

/// Fill (and optionally border) a rounded rectangle. `mode` in the shader is FILL (0).
pub(crate) fn append_rounded_rect_fill(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    radii: [f32; 4],
    fill: Color,
    border: Option<sui_scene::Border>,
    viewport: Size,
    feather: f32,
) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }
    let transformed = state.current_transform.transform_rect_bbox(rect);
    if transformed.width() <= 0.0 || transformed.height() <= 0.0 {
        return;
    }

    let half_w = transformed.width() * 0.5;
    let half_h = transformed.height() * 0.5;
    let center = Point::new(transformed.x() + half_w, transformed.y() + half_h);
    let fringe = analytic_coverage_outset(feather);
    let Some(screen_quad) = clipped_screen_quad(state, transformed.inflate(fringe, fringe)) else {
        return;
    };
    let radii = clamp_radii(radii, half_w, half_h);

    let (border_w, border_color) = match border {
        Some(border) => (border.width.max(0.0), shader_color(border.color)),
        None => (0.0, [0.0; 4]),
    };

    append_rounded_rect_quad(
        vertices,
        screen_quad,
        center,
        viewport,
        shader_color(fill),
        [half_w, half_h, 0.0, feather],
        radii,
        [border_w, 0.0, 0.0, 0.0],
        border_color,
    );
}

/// Soft drop shadow for a rounded rectangle. `mode` in the shader is SHADOW (1). The
/// shadow quad is the rect inflated by its blur/spread/offset extent and shifted by the
/// offset; the fragment shader re-centers via the local offset in p2.zw.
///
/// CLIP NOTE: the shadow op inherits the active clip just like any other op, so callers
/// that want a shadow to bleed outside a tight self-clip must paint the shadow BEFORE
/// pushing that clip.
pub(crate) fn append_rounded_rect_shadow(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    radii: [f32; 4],
    shadow: sui_scene::ShadowParams,
    viewport: Size,
    feather: f32,
) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }
    let transformed = state.current_transform.transform_rect_bbox(rect);
    if transformed.width() <= 0.0 || transformed.height() <= 0.0 {
        return;
    }

    let half_w = transformed.width() * 0.5;
    let half_h = transformed.height() * 0.5;
    let center = Point::new(transformed.x() + half_w, transformed.y() + half_h);
    let spread = shadow.spread.max(0.0);
    let ext = shadow.extent();
    // The shadow's rounded box is the fill box grown by `spread`; coverage is sampled in
    // the same center-origin local space, offset by the shadow displacement.
    let radii = clamp_radii(radii, half_w + spread, half_h + spread);
    let Some(screen_quad) = clipped_screen_quad(
        state,
        transformed
            .inflate(ext, ext)
            .translate(Vector::new(shadow.offset_x, shadow.offset_y)),
    ) else {
        return;
    };

    append_rounded_rect_quad(
        vertices,
        screen_quad,
        center,
        viewport,
        shader_color(shadow.color),
        [half_w + spread, half_h + spread, 1.0, feather],
        radii,
        [0.0, shadow.blur, shadow.offset_x, shadow.offset_y],
        [0.0; 4],
    );
}

/// Fill a (possibly rounded) rectangle with a linear gradient through every stop. The
/// gradient axis is given by `start`/`end` in scene (pre-transform) coordinates; both
/// are mapped into the same center-origin rect-local space used by the SDF.
///
/// The gradient pipeline draws axis-aligned quads that each blend two colors, so the
/// gradient is split into [`GradientRegion`]s, one per pair of neighboring stops.
/// Every region evaluates the same rounded-rect coverage, so edges and antialiasing
/// match a single fill:
///
/// - A horizontal or vertical gradient cuts the quad into adjacent bands along its
///   axis. Neighbors share bit-identical edges, which the rasterizer tiles without
///   gaps or overlap.
/// - Any other direction cannot be cut into axis-aligned bands, so every region
///   draws the whole quad and the shader keeps only the fragments inside the
///   region's range along the axis (see [`gradient_band_code`]).
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_gradient_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    radii: [f32; 4],
    start: Point,
    end: Point,
    stops: &[GradientStop],
    viewport: Size,
    feather: f32,
) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }
    let transformed = state.current_transform.transform_rect_bbox(rect);
    if transformed.width() <= 0.0 || transformed.height() <= 0.0 {
        return;
    }

    let half_w = transformed.width() * 0.5;
    let half_h = transformed.height() * 0.5;
    let center = Point::new(transformed.x() + half_w, transformed.y() + half_h);
    let fringe = analytic_coverage_outset(feather);
    let Some(screen_quad) = clipped_screen_quad(state, transformed.inflate(fringe, fringe)) else {
        return;
    };
    let radii = clamp_radii(radii, half_w, half_h);
    let params = [half_w, half_h, GRADIENT_BAND_NONE, feather];

    // Gradient axis end-points in screen space, and the point at `offset` along it.
    let start_screen = state.current_transform.transform_point(start);
    let end_screen = state.current_transform.transform_point(end);
    let direction = Vector::new(end_screen.x - start_screen.x, end_screen.y - start_screen.y);
    let along = |offset: f32| {
        Point::new(
            start_screen.x + direction.x * offset,
            start_screen.y + direction.y * offset,
        )
    };
    let local_axis = |from: Point, to: Point| {
        [
            from.x - center.x,
            from.y - center.y,
            to.x - center.x,
            to.y - center.y,
        ]
    };
    let full_axis = local_axis(start_screen, end_screen);

    let regions = gradient_regions(stops);
    let degenerate = direction.x.abs().max(direction.y.abs()) <= 1.0e-4;
    if let [region] = regions.as_slice() {
        // One two-color ramp: map its offsets onto the axis, and clamping pads the ends.
        let axis = if degenerate {
            full_axis
        } else {
            local_axis(along(region.ramp.0), along(region.ramp.1))
        };
        append_rounded_rect_quad(
            vertices,
            screen_quad,
            center,
            viewport,
            shader_color(region.from),
            params,
            radii,
            axis,
            shader_color(region.to),
        );
        return;
    }
    if degenerate {
        // A zero-length axis has no direction to spread stops along; like the shader
        // with a single ramp, it shows the first stop.
        let first = regions[0].from;
        append_rounded_rect_quad(
            vertices,
            screen_quad,
            center,
            viewport,
            shader_color(first),
            params,
            radii,
            full_axis,
            shader_color(first),
        );
        return;
    }

    let horizontal = direction.y.abs() <= direction.x.abs() * 1.0e-4;
    let vertical = direction.x.abs() <= direction.y.abs() * 1.0e-4;
    if horizontal || vertical {
        // Band edges come from one computation per boundary, so neighbors share them
        // exactly and the rasterizer covers each pixel once.
        let (quad_min, quad_max) = if horizontal {
            (screen_quad.x(), screen_quad.max_x())
        } else {
            (screen_quad.y(), screen_quad.max_y())
        };
        let edge = |offset: Option<f32>, open: f32| match offset {
            Some(offset) => {
                let point = along(offset);
                (if horizontal { point.x } else { point.y }).clamp(quad_min, quad_max)
            }
            None => open,
        };
        let backwards = if horizontal {
            direction.x < 0.0
        } else {
            direction.y < 0.0
        };
        for region in &regions {
            let (open_start, open_end) = if backwards {
                (quad_max, quad_min)
            } else {
                (quad_min, quad_max)
            };
            let a = edge(region.start, open_start);
            let b = edge(region.end, open_end);
            let (low, high) = (a.min(b), a.max(b));
            if high <= low {
                continue;
            }
            let band = if horizontal {
                Rect::new(low, screen_quad.y(), high - low, screen_quad.height())
            } else {
                Rect::new(screen_quad.x(), low, screen_quad.width(), high - low)
            };
            append_rounded_rect_quad(
                vertices,
                band,
                center,
                viewport,
                shader_color(region.from),
                params,
                radii,
                local_axis(along(region.ramp.0), along(region.ramp.1)),
                shader_color(region.to),
            );
        }
        return;
    }

    // Diagonal: every region covers the whole quad, so each fragment evaluates the
    // same position along the axis in every region and lands in exactly one.
    for region in &regions {
        let mut params = params;
        params[2] = gradient_band_code(region);
        append_rounded_rect_quad(
            vertices,
            screen_quad,
            center,
            viewport,
            shader_color(region.from),
            params,
            radii,
            full_axis,
            shader_color(region.to),
        );
    }
}

/// `shader_params.z` of a gradient quad that is not limited to a band.
const GRADIENT_BAND_NONE: f32 = 0.0;
/// Steps a band's offsets are quantized to (11 bits each), so the band code stays an
/// integer that `f32` represents exactly.
const GRADIENT_BAND_STEPS: f32 = 2047.0;

/// Packs a region's range along the axis into `shader_params.z` for the gradient
/// shader. The code is `1 + open_start << 23 + open_end << 22 + low << 11 + high`, with
/// `low` and `high` the ramp's offsets in [`GRADIENT_BAND_STEPS`]; the largest code is
/// `2^24`, which `f32` holds exactly. Neighboring regions quantize their shared
/// boundary identically, so the shader's half-open range test assigns every fragment
/// to one region.
pub(crate) fn gradient_band_code(region: &GradientRegion) -> f32 {
    // A bounded end always equals the matching end of the ramp, so the ramp's
    // offsets serve as both the range and the color mapping.
    let quantize = |offset: f32| (offset.clamp(0.0, 1.0) * GRADIENT_BAND_STEPS).round() as u32;
    let (low, high) = (quantize(region.ramp.0), quantize(region.ramp.1));
    let code = ((region.start.is_none() as u32) << 23)
        | ((region.end.is_none() as u32) << 22)
        | (low << 11)
        | high;
    (code + 1) as f32
}

/// A stretch of a linear gradient between two boundaries along its axis (as offsets
/// from 0 at `start` to 1 at `end`; `None` extends past the rectangle). Inside it the
/// color ramps from `from` at `ramp.0` to `to` at `ramp.1`, clamping outside the ramp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GradientRegion {
    pub(crate) start: Option<f32>,
    pub(crate) end: Option<f32>,
    pub(crate) ramp: (f32, f32),
    pub(crate) from: Color,
    pub(crate) to: Color,
}

impl GradientRegion {
    fn solid(color: Color, start: Option<f32>, end: Option<f32>, at: f32) -> Self {
        Self {
            start,
            end,
            ramp: (at, at),
            from: color,
            to: color,
        }
    }
}

/// Splits `stops` into regions that tile the whole axis. Offsets are clamped to
/// `0..=1` and made non-decreasing, so a stop placed before its predecessor sits on it
/// and makes a hard edge. Before the first stop and after the last, the gradient
/// holds that stop's color.
pub(crate) fn gradient_regions(stops: &[GradientStop]) -> Vec<GradientRegion> {
    let mut previous = 0.0_f32;
    let stops = stops
        .iter()
        .map(|stop| {
            previous = stop.offset.clamp(0.0, 1.0).max(previous);
            (previous, stop.color)
        })
        .collect::<Vec<_>>();
    let (Some(&(first_offset, first_color)), Some(&(last_offset, last_color))) =
        (stops.first(), stops.last())
    else {
        return vec![GradientRegion::solid(Color::TRANSPARENT, None, None, 0.0)];
    };

    let ramps = stops
        .windows(2)
        .filter(|pair| pair[1].0 > pair[0].0)
        .map(|pair| (pair[0], pair[1]))
        .collect::<Vec<_>>();
    let (Some(&((ramp_start, ramp_first), _)), Some(&(_, (ramp_end, ramp_last)))) =
        (ramps.first(), ramps.last())
    else {
        // Every stop shares one offset: a hard edge from the first color to the last.
        if first_color == last_color {
            return vec![GradientRegion::solid(first_color, None, None, first_offset)];
        }
        return vec![
            GradientRegion::solid(first_color, None, Some(first_offset), first_offset),
            GradientRegion::solid(last_color, Some(last_offset), None, last_offset),
        ];
    };

    let mut regions = Vec::with_capacity(ramps.len() + 2);
    // A hard edge at the first ramp starts it from a different color than the pad.
    let leading_pad = ramp_first != first_color;
    if leading_pad {
        regions.push(GradientRegion::solid(
            first_color,
            None,
            Some(ramp_start),
            ramp_start,
        ));
    }
    for (index, &((low, from), (high, to))) in ramps.iter().enumerate() {
        regions.push(GradientRegion {
            start: (index > 0 || leading_pad).then_some(low),
            end: Some(high),
            ramp: (low, high),
            from,
            to,
        });
    }
    if ramp_last != last_color {
        regions.push(GradientRegion::solid(
            last_color,
            Some(ramp_end),
            None,
            ramp_end,
        ));
    } else if let Some(last) = regions.last_mut() {
        last.end = None;
    }
    regions
}

pub(crate) fn points_bounds(points: &[Point]) -> Rect {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    Rect::from_points(Point::new(min_x, min_y), Point::new(max_x, max_y))
}

pub(crate) fn widget_shader_metadata(shader: sui_scene::WidgetShader) -> ([f32; 4], [f32; 4]) {
    match shader {
        sui_scene::WidgetShader::ColorWheel => ([0.0, 0.0, 0.0, 0.0], [0.0; 4]),
        sui_scene::WidgetShader::ColorPickerHueBar => ([1.0, 0.0, 0.0, 0.0], [0.0; 4]),
        sui_scene::WidgetShader::ColorPickerSaturationValuePlane {
            color_space,
            hue,
            max_value,
        } => (
            [2.0, shader_color_space(color_space), hue, max_value],
            [0.0; 4],
        ),
        sui_scene::WidgetShader::ColorPickerSaturationBar {
            color_space,
            hue,
            value,
        } => ([3.0, shader_color_space(color_space), hue, value], [0.0; 4]),
        sui_scene::WidgetShader::ColorPickerValueBar {
            color_space,
            hue,
            saturation,
            max_value,
        } => (
            [4.0, shader_color_space(color_space), hue, saturation],
            [max_value, 0.0, 0.0, 0.0],
        ),
        sui_scene::WidgetShader::ColorPickerAlphaBar { color } => (
            [5.0, shader_color_space(color.space), 0.0, 0.0],
            color.to_array(),
        ),
        sui_scene::WidgetShader::ColorPickerRgbChannelBar {
            color,
            channel,
            max_value,
        } => (
            [
                6.0,
                shader_color_space(color.space),
                channel as f32,
                max_value,
            ],
            color.to_array(),
        ),
    }
}

pub(crate) fn shader_color_space(space: ColorSpace) -> f32 {
    match space {
        ColorSpace::Srgb => 0.0,
        ColorSpace::LinearSrgb => 1.0,
        ColorSpace::DisplayP3 => 2.0,
        ColorSpace::LinearDisplayP3 => 3.0,
    }
}

pub(crate) fn append_rect(vertices: &mut Vec<Vertex>, rect: Rect, color: Color, viewport: Size) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let min = to_ndc(rect.x(), rect.y(), viewport);
    let max = to_ndc(rect.max_x(), rect.max_y(), viewport);
    let rgba = shader_color(color);

    vertices.extend_from_slice(&[
        Vertex::basic([min[0], min[1]], rgba, [0.0, 0.0], [0.0; 4]),
        Vertex::basic([max[0], min[1]], rgba, [0.0, 0.0], [0.0; 4]),
        Vertex::basic([min[0], max[1]], rgba, [0.0, 0.0], [0.0; 4]),
        Vertex::basic([min[0], max[1]], rgba, [0.0, 0.0], [0.0; 4]),
        Vertex::basic([max[0], min[1]], rgba, [0.0, 0.0], [0.0; 4]),
        Vertex::basic([max[0], max[1]], rgba, [0.0, 0.0], [0.0; 4]),
    ]);
}

pub(crate) fn to_ndc(x: f32, y: f32, viewport: Size) -> [f32; 2] {
    [
        ((x / viewport.width) * 2.0) - 1.0,
        1.0 - ((y / viewport.height) * 2.0),
    ]
}
