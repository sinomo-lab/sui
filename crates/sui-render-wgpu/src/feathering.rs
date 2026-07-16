use lyon_path::{PathEvent, iterator::PathIterator};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, VertexBuffers,
};

use super::*;

const AA_FLATTEN_TOLERANCE: f32 = 0.1;

#[derive(Debug, Clone)]
pub(super) struct FlattenedContour {
    pub(super) points: Vec<Point>,
    pub(super) closed: bool,
}

pub(super) fn build_local_fill_mesh(
    path: &LyonPath,
    _feather_width: f32,
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

    Ok(mesh)
}

pub(super) fn build_local_stroke_mesh(
    path: &LyonPath,
    stroke: StrokeStyle,
    _feather_width: f32,
) -> Result<CachedGlyphMesh> {
    let mut mesh = CachedGlyphMesh::default();
    if stroke.width <= 0.0 {
        return Ok(mesh);
    }

    append_local_hard_stroked_lyon_path(&mut mesh, path, stroke)?;
    Ok(mesh)
}

fn append_local_hard_stroked_lyon_path(
    mesh: &mut CachedGlyphMesh,
    path: &LyonPath,
    stroke: StrokeStyle,
) -> Result<()> {
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = StrokeTessellator::new();
    let cap = match stroke.cap {
        sui_scene::StrokeCap::Butt => LineCap::Butt,
        sui_scene::StrokeCap::Round => LineCap::Round,
        sui_scene::StrokeCap::Square => LineCap::Square,
    };
    let join = match stroke.join {
        sui_scene::StrokeJoin::Miter => LineJoin::Miter,
        sui_scene::StrokeJoin::Round => LineJoin::Round,
        sui_scene::StrokeJoin::Bevel => LineJoin::Bevel,
    };
    tessellator
        .tessellate_path(
            path,
            &StrokeOptions::default()
                .with_line_width(stroke.width)
                .with_line_cap(cap)
                .with_line_join(join),
            &mut builder,
        )
        .map_err(|error| Error::new(format!("failed to tessellate stroked path: {error}")))?;

    for position in &buffers.vertices {
        mesh.push_vertex(Point::new(position[0], position[1]), 1.0);
    }
    mesh.indices.extend(buffers.indices.iter().copied());
    Ok(())
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

fn points_nearly_equal(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= 1.0e-4 && (a.y - b.y).abs() <= 1.0e-4
}
