use lyon_path::{PathEvent, iterator::PathIterator};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, StrokeOptions, StrokeTessellator, VertexBuffers,
};

use super::*;

const AA_FLATTEN_TOLERANCE: f32 = 0.1;

#[derive(Debug, Clone)]
pub(super) struct FlattenedContour {
    pub(super) points: Vec<Point>,
    pub(super) closed: bool,
}

#[derive(Debug, Clone, Copy)]
struct AaPathPoint {
    position: Point,
    normal: Vector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeatheredPathType {
    Open,
    Closed,
}

pub(super) fn build_local_fill_mesh(
    path: &LyonPath,
    feather_width: f32,
) -> Result<CachedGlyphMesh> {
    let mut mesh = CachedGlyphMesh::default();
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = FillTessellator::new();
    tessellator
        .tessellate_path(path, &FillOptions::default(), &mut builder)
        .map_err(|error| Error::new(format!("failed to tessellate filled path: {error}")))?;

    for position in &buffers.vertices {
        mesh.push_vertex(Point::new(position[0], position[1]), 1.0);
    }
    mesh.indices.extend(buffers.indices.iter().copied());

    if feather_width > 0.0 {
        let contours = flatten_path_contours(path);
        for contour in &contours {
            if !contour.closed || contour.points.len() < 3 {
                continue;
            }

            let mut aa_points = build_closed_aa_points(&contour.points);
            if !normals_point_to_transparent_side(contour, &contours, feather_width) {
                for point in &mut aa_points {
                    point.normal = negate_vector(point.normal);
                }
            }

            append_local_fill_fringe_for_contour(&mut mesh, &aa_points, feather_width);
        }
    }

    Ok(mesh)
}

pub(super) fn build_local_stroke_mesh(
    path: &LyonPath,
    line_width: f32,
    feather_width: f32,
) -> Result<CachedGlyphMesh> {
    let mut mesh = CachedGlyphMesh::default();
    if line_width <= 0.0 {
        return Ok(mesh);
    }

    if feather_width <= 0.0 {
        append_local_hard_stroked_lyon_path(&mut mesh, path, line_width)?;
        return Ok(mesh);
    }

    let contours = flatten_path_contours(path);

    for contour in contours {
        let path_type = if contour.closed {
            FeatheredPathType::Closed
        } else {
            FeatheredPathType::Open
        };

        let aa_points = if contour.closed {
            build_closed_aa_points(&contour.points)
        } else {
            build_open_aa_points(&contour.points)
        };

        append_local_stroke_contour(&mut mesh, &aa_points, path_type, line_width, feather_width);
    }

    Ok(mesh)
}

pub(super) fn append_stroke_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    stroke: StrokeStyle,
    viewport: Size,
    feather_width: f32,
) {
    if rect.is_empty() {
        return;
    }

    let thickness = stroke
        .width
        .max(1.0)
        .min((rect.width() * 0.5).max(1.0))
        .min((rect.height() * 0.5).max(1.0));

    let top = Rect::new(rect.x(), rect.y(), rect.width(), thickness);
    let bottom = Rect::new(rect.x(), rect.max_y() - thickness, rect.width(), thickness);
    let left = Rect::new(
        rect.x(),
        rect.y() + thickness,
        thickness,
        (rect.height() - (thickness * 2.0)).max(0.0),
    );
    let right = Rect::new(
        rect.max_x() - thickness,
        rect.y() + thickness,
        thickness,
        (rect.height() - (thickness * 2.0)).max(0.0),
    );

    append_painted_rect(vertices, state, top, color, viewport, feather_width);
    append_painted_rect(vertices, state, bottom, color, viewport, feather_width);
    append_painted_rect(vertices, state, left, color, viewport, feather_width);
    append_painted_rect(vertices, state, right, color, viewport, feather_width);
}

pub(super) fn append_painted_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    if let Some(visible) = state.visible_rect(rect) {
        append_feathered_rect(vertices, visible, color, viewport, feather_width);
    }
}

fn append_local_fill_fringe_for_contour(
    mesh: &mut CachedGlyphMesh,
    contour: &[AaPathPoint],
    feather_width: f32,
) {
    if contour.len() < 3 || feather_width <= 0.0 {
        return;
    }

    let base_index = mesh.vertices.len() as u32;
    let mut previous_inner = 0;
    let mut previous_outer = 0;

    for (index, point) in contour.iter().enumerate() {
        let delta = scale_vector(point.normal, 0.5 * feather_width);
        let inner = mesh.push_vertex(offset_point(point.position, negate_vector(delta)), 1.0);
        let outer = mesh.push_vertex(offset_point(point.position, delta), 0.0);

        if index > 0 {
            mesh.add_triangle(inner, previous_inner, previous_outer);
            mesh.add_triangle(previous_outer, outer, inner);
        }

        previous_inner = inner;
        previous_outer = outer;
    }

    let first_inner = base_index;
    let first_outer = base_index + 1;
    mesh.add_triangle(first_inner, previous_inner, previous_outer);
    mesh.add_triangle(previous_outer, first_outer, first_inner);
}

fn append_local_hard_stroked_lyon_path(
    mesh: &mut CachedGlyphMesh,
    path: &LyonPath,
    line_width: f32,
) -> Result<()> {
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = StrokeTessellator::new();
    tessellator
        .tessellate_path(
            path,
            &StrokeOptions::default().with_line_width(line_width),
            &mut builder,
        )
        .map_err(|error| Error::new(format!("failed to tessellate stroked path: {error}")))?;

    for position in &buffers.vertices {
        mesh.push_vertex(Point::new(position[0], position[1]), 1.0);
    }
    mesh.indices.extend(buffers.indices.iter().copied());
    Ok(())
}

fn append_local_stroke_contour(
    mesh: &mut CachedGlyphMesh,
    path: &[AaPathPoint],
    path_type: FeatheredPathType,
    line_width: f32,
    feather_width: f32,
) {
    let n = path.len() as u32;
    if n < 2 || line_width <= 0.0 || feather_width <= 0.0 {
        return;
    }

    // Keep the CPU-side mesh in sync with the shader's thin-stroke branch.
    // When the opaque core collapses to zero width, the three-band thin-line
    // mesh preserves visible ink for 1 px control outlines and separators.
    let thin_line = line_width <= feather_width;
    if thin_line {
        let coverage = (line_width / feather_width).clamp(0.0, 1.0);
        let mut previous_base = 0;

        for (index, point) in path.iter().enumerate() {
            let outer = mesh.push_vertex(
                offset_point(point.position, scale_vector(point.normal, feather_width)),
                0.0,
            );
            mesh.push_vertex(point.position, coverage);
            mesh.push_vertex(
                offset_point(point.position, scale_vector(point.normal, -feather_width)),
                0.0,
            );

            if path_type == FeatheredPathType::Closed || index > 0 {
                mesh.add_triangle(previous_base + 0, previous_base + 1, outer);
                mesh.add_triangle(previous_base + 1, outer, outer + 1);
                mesh.add_triangle(previous_base + 1, previous_base + 2, outer + 1);
                mesh.add_triangle(previous_base + 2, outer + 1, outer + 2);
            }

            previous_base = outer;
        }

        if path_type == FeatheredPathType::Closed {
            mesh.add_triangle(previous_base + 0, previous_base + 1, 0);
            mesh.add_triangle(previous_base + 1, 0, 1);
            mesh.add_triangle(previous_base + 1, previous_base + 2, 1);
            mesh.add_triangle(previous_base + 2, 1, 2);
        }
        return;
    }

    let inner_radius = 0.5 * (line_width - feather_width);
    let outer_radius = 0.5 * (line_width + feather_width);

    match path_type {
        FeatheredPathType::Closed => {
            let mut previous_base = 0;

            for (index, point) in path.iter().enumerate() {
                let outer_pos = mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, outer_radius)),
                    0.0,
                );
                mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, inner_radius)),
                    1.0,
                );
                mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, -inner_radius)),
                    1.0,
                );
                mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, -outer_radius)),
                    0.0,
                );

                if index > 0 {
                    mesh.add_triangle(previous_base + 0, previous_base + 1, outer_pos);
                    mesh.add_triangle(previous_base + 1, outer_pos, outer_pos + 1);
                    mesh.add_triangle(previous_base + 1, previous_base + 2, outer_pos + 1);
                    mesh.add_triangle(previous_base + 2, outer_pos + 1, outer_pos + 2);
                    mesh.add_triangle(previous_base + 2, previous_base + 3, outer_pos + 2);
                    mesh.add_triangle(previous_base + 3, outer_pos + 2, outer_pos + 3);
                }

                previous_base = outer_pos;
            }

            mesh.add_triangle(previous_base + 0, previous_base + 1, 0);
            mesh.add_triangle(previous_base + 1, 0, 1);
            mesh.add_triangle(previous_base + 1, previous_base + 2, 1);
            mesh.add_triangle(previous_base + 2, 1, 2);
            mesh.add_triangle(previous_base + 2, previous_base + 3, 2);
            mesh.add_triangle(previous_base + 3, 2, 3);
        }
        FeatheredPathType::Open => {
            let first = path[0];
            let first_extrude = scale_vector(vector_rot90(first.normal), feather_width);
            let first_base = mesh.push_vertex(
                offset_point(
                    offset_point(first.position, scale_vector(first.normal, outer_radius)),
                    first_extrude,
                ),
                0.0,
            );
            mesh.push_vertex(
                offset_point(first.position, scale_vector(first.normal, inner_radius)),
                1.0,
            );
            mesh.push_vertex(
                offset_point(first.position, scale_vector(first.normal, -inner_radius)),
                1.0,
            );
            mesh.push_vertex(
                offset_point(
                    offset_point(first.position, scale_vector(first.normal, -outer_radius)),
                    first_extrude,
                ),
                0.0,
            );
            mesh.add_triangle(first_base + 0, first_base + 1, first_base + 2);
            mesh.add_triangle(first_base + 0, first_base + 2, first_base + 3);

            let mut previous_base = first_base;
            for point in path.iter().skip(1).take(path.len().saturating_sub(2)) {
                let base = mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, outer_radius)),
                    0.0,
                );
                mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, inner_radius)),
                    1.0,
                );
                mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, -inner_radius)),
                    1.0,
                );
                mesh.push_vertex(
                    offset_point(point.position, scale_vector(point.normal, -outer_radius)),
                    0.0,
                );

                mesh.add_triangle(previous_base + 0, previous_base + 1, base + 0);
                mesh.add_triangle(previous_base + 1, base + 0, base + 1);
                mesh.add_triangle(previous_base + 1, previous_base + 2, base + 1);
                mesh.add_triangle(previous_base + 2, base + 1, base + 2);
                mesh.add_triangle(previous_base + 2, previous_base + 3, base + 2);
                mesh.add_triangle(previous_base + 3, base + 2, base + 3);

                previous_base = base;
            }

            let last = path[path.len() - 1];
            let last_extrude = scale_vector(vector_rot90(last.normal), -feather_width);
            let last_base = mesh.push_vertex(
                offset_point(
                    offset_point(last.position, scale_vector(last.normal, outer_radius)),
                    last_extrude,
                ),
                0.0,
            );
            mesh.push_vertex(
                offset_point(last.position, scale_vector(last.normal, inner_radius)),
                1.0,
            );
            mesh.push_vertex(
                offset_point(last.position, scale_vector(last.normal, -inner_radius)),
                1.0,
            );
            mesh.push_vertex(
                offset_point(
                    offset_point(last.position, scale_vector(last.normal, -outer_radius)),
                    last_extrude,
                ),
                0.0,
            );

            mesh.add_triangle(previous_base + 0, previous_base + 1, last_base + 0);
            mesh.add_triangle(previous_base + 1, last_base + 0, last_base + 1);
            mesh.add_triangle(previous_base + 1, previous_base + 2, last_base + 1);
            mesh.add_triangle(previous_base + 2, last_base + 1, last_base + 2);
            mesh.add_triangle(previous_base + 2, previous_base + 3, last_base + 2);
            mesh.add_triangle(previous_base + 3, last_base + 2, last_base + 3);
            mesh.add_triangle(last_base + 0, last_base + 1, last_base + 2);
            mesh.add_triangle(last_base + 0, last_base + 2, last_base + 3);
        }
    }
}

fn append_feathered_rect(
    vertices: &mut Vec<Vertex>,
    rect: Rect,
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    append_rect(vertices, rect, color, viewport);

    if feather_width <= 0.0 {
        return;
    }

    let points = [
        Point::new(rect.x(), rect.y()),
        Point::new(rect.max_x(), rect.y()),
        Point::new(rect.max_x(), rect.max_y()),
        Point::new(rect.x(), rect.max_y()),
    ];
    let aa_points = build_closed_aa_points(&points);
    append_fill_fringe_for_contour(vertices, &aa_points, color, viewport, feather_width);
}

fn append_fill_fringe_for_contour(
    vertices: &mut Vec<Vertex>,
    contour: &[AaPathPoint],
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    if contour.len() < 3 || viewport.is_empty() || feather_width <= 0.0 {
        return;
    }

    let mut mesh = SceneMesh::default();
    let transparent = Color::TRANSPARENT;
    let mut previous_inner = 0;
    let mut previous_outer = 0;

    for (index, point) in contour.iter().enumerate() {
        let delta = scale_vector(point.normal, 0.5 * feather_width);
        let inner = mesh.colored_vertex(offset_point(point.position, negate_vector(delta)), color);
        let outer = mesh.colored_vertex(offset_point(point.position, delta), transparent);

        if index > 0 {
            mesh.add_triangle(inner, previous_inner, previous_outer);
            mesh.add_triangle(previous_outer, outer, inner);
        }

        previous_inner = inner;
        previous_outer = outer;
    }

    let first_inner = 0;
    let first_outer = 1;
    mesh.add_triangle(first_inner, previous_inner, previous_outer);
    mesh.add_triangle(previous_outer, first_outer, first_inner);

    append_scene_mesh(vertices, &mesh, viewport);
}

pub(super) fn flatten_path_contours(path: &LyonPath) -> Vec<FlattenedContour> {
    let mut contours = Vec::new();
    let mut current = Vec::new();

    for event in path.iter().flattened(AA_FLATTEN_TOLERANCE) {
        match event {
            PathEvent::Begin { at } => {
                current.clear();
                current.push(Point::new(at.x, at.y));
            }
            PathEvent::Line { to, .. } => {
                let point = Point::new(to.x, to.y);
                if current
                    .last()
                    .is_none_or(|last| !points_nearly_equal(*last, point))
                {
                    current.push(point);
                }
            }
            PathEvent::End { close, .. } => {
                if close
                    && current.len() > 1
                    && points_nearly_equal(current[0], *current.last().unwrap_or(&current[0]))
                {
                    current.pop();
                }

                if current.len() >= if close { 3 } else { 2 } {
                    contours.push(FlattenedContour {
                        points: std::mem::take(&mut current),
                        closed: close,
                    });
                } else {
                    current.clear();
                }
            }
            PathEvent::Quadratic { .. } | PathEvent::Cubic { .. } => {
                unreachable!("flattened path iteration should not yield curve events")
            }
        }
    }

    contours
}

fn build_open_aa_points(points: &[Point]) -> Vec<AaPathPoint> {
    if points.len() < 2 {
        return Vec::new();
    }

    if points.len() == 2 {
        let normal = vector_rot90(vector_normalize(points[1] - points[0]));
        return vec![
            AaPathPoint {
                position: points[0],
                normal,
            },
            AaPathPoint {
                position: points[1],
                normal,
            },
        ];
    }

    let mut aa_points = Vec::with_capacity(points.len() * 2);
    let mut previous_normal = vector_rot90(vector_normalize(points[1] - points[0]));
    aa_points.push(AaPathPoint {
        position: points[0],
        normal: previous_normal,
    });

    for index in 1..points.len() - 1 {
        let mut next_normal = vector_rot90(vector_normalize(points[index + 1] - points[index]));
        if vector_is_zero(previous_normal) {
            previous_normal = next_normal;
        } else if vector_is_zero(next_normal) {
            next_normal = previous_normal;
        }

        let averaged = scale_vector(previous_normal + next_normal, 0.5);
        let length_sq = vector_length_sq(averaged);
        if length_sq < 0.5 {
            let center_normal = vector_normalize(averaged);
            let previous_cut = scale_vector(previous_normal + center_normal, 0.5);
            let next_cut = scale_vector(next_normal + center_normal, 0.5);
            aa_points.push(AaPathPoint {
                position: points[index],
                normal: scale_vector(
                    previous_cut,
                    1.0 / vector_length_sq(previous_cut).max(1.0e-6),
                ),
            });
            aa_points.push(AaPathPoint {
                position: points[index],
                normal: scale_vector(next_cut, 1.0 / vector_length_sq(next_cut).max(1.0e-6)),
            });
        } else {
            aa_points.push(AaPathPoint {
                position: points[index],
                normal: scale_vector(averaged, 1.0 / length_sq),
            });
        }

        previous_normal = next_normal;
    }

    aa_points.push(AaPathPoint {
        position: points[points.len() - 1],
        normal: vector_rot90(vector_normalize(
            points[points.len() - 1] - points[points.len() - 2],
        )),
    });
    aa_points
}

fn build_closed_aa_points(points: &[Point]) -> Vec<AaPathPoint> {
    if points.len() < 3 {
        return Vec::new();
    }

    let mut aa_points = Vec::with_capacity(points.len());
    let mut previous_normal = vector_rot90(vector_normalize(points[0] - points[points.len() - 1]));

    for index in 0..points.len() {
        let next_index = if index + 1 == points.len() {
            0
        } else {
            index + 1
        };
        let mut next_normal = vector_rot90(vector_normalize(points[next_index] - points[index]));
        if vector_is_zero(previous_normal) {
            previous_normal = next_normal;
        } else if vector_is_zero(next_normal) {
            next_normal = previous_normal;
        }

        let averaged = scale_vector(previous_normal + next_normal, 0.5);
        let length_sq = vector_length_sq(averaged).max(1.0e-6);
        aa_points.push(AaPathPoint {
            position: points[index],
            normal: scale_vector(averaged, 1.0 / length_sq),
        });
        previous_normal = next_normal;
    }

    aa_points
}

fn normals_point_to_transparent_side(
    contour: &FlattenedContour,
    contours: &[FlattenedContour],
    feather_width: f32,
) -> bool {
    for window in contour.points.windows(2) {
        let edge = window[1] - window[0];
        let edge_length_sq = vector_length_sq(edge);
        if edge_length_sq <= 1.0e-6 {
            continue;
        }

        let midpoint = Point::new(
            (window[0].x + window[1].x) * 0.5,
            (window[0].y + window[1].y) * 0.5,
        );
        let normal = vector_rot90(vector_normalize(edge));
        let sample = offset_point(midpoint, scale_vector(normal, -0.25 * feather_width));
        return point_in_filled_path(sample, contours);
    }

    true
}

fn point_in_filled_path(point: Point, contours: &[FlattenedContour]) -> bool {
    let mut inside = false;

    for contour in contours {
        if contour.closed && point_in_polygon(point, &contour.points) {
            inside = !inside;
        }
    }

    inside
}

fn point_in_polygon(point: Point, polygon: &[Point]) -> bool {
    let mut inside = false;
    let mut previous = *polygon.last().unwrap_or(&Point::ZERO);

    for current in polygon {
        let intersects = ((current.y > point.y) != (previous.y > point.y))
            && (point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x);
        if intersects {
            inside = !inside;
        }
        previous = *current;
    }

    inside
}

fn points_nearly_equal(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= 1.0e-4 && (a.y - b.y).abs() <= 1.0e-4
}

fn vector_length_sq(vector: Vector) -> f32 {
    vector.x * vector.x + vector.y * vector.y
}

fn vector_is_zero(vector: Vector) -> bool {
    vector_length_sq(vector) <= 1.0e-6
}

fn vector_normalize(vector: Vector) -> Vector {
    let length_sq = vector_length_sq(vector);
    if length_sq <= 1.0e-6 {
        Vector::ZERO
    } else {
        let length = length_sq.sqrt();
        Vector::new(vector.x / length, vector.y / length)
    }
}

fn vector_rot90(vector: Vector) -> Vector {
    Vector::new(vector.y, -vector.x)
}

fn scale_vector(vector: Vector, factor: f32) -> Vector {
    Vector::new(vector.x * factor, vector.y * factor)
}

fn negate_vector(vector: Vector) -> Vector {
    Vector::new(-vector.x, -vector.y)
}

fn offset_point(point: Point, offset: Vector) -> Point {
    Point::new(point.x + offset.x, point.y + offset.y)
}
