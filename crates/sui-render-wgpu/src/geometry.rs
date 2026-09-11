use crate::feathering;
use crate::gpu::TessellatedPoint;
use crate::gpu::Vertex;
use crate::output::shader_color;
use crate::primitives::to_ndc;
use crate::retained::MAX_ANALYTIC_PATH_CONTOURS;
use crate::retained::MAX_ANALYTIC_PATH_POINTS;
use crate::scene::ANALYTIC_PATH_COORDINATE_PRECISION;
use bytemuck::Pod;
use bytemuck::Zeroable;
use lyon_path::Path as LyonPath;
use lyon_path::builder::PathBuilder as LyonPathBuilder;
use lyon_path::math::point;
use lyon_tessellation::BuffersBuilder;
use lyon_tessellation::FillOptions;
use lyon_tessellation::FillTessellator;
use lyon_tessellation::VertexBuffers;
use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use sui_core::Color;
use sui_core::Error;
use sui_core::Path as ScenePath;
use sui_core::PathElement;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Result;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct AnalyticPathMetaGpu {
    pub(crate) contour_start: u32,
    pub(crate) contour_count: u32,
    pub(crate) point_start: u32,
    pub(crate) mode: u32,
    pub(crate) feather_width: f32,
    pub(crate) stroke_width: f32,
    pub(crate) _pad0: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct AnalyticContourGpu {
    pub(crate) start: u32,
    pub(crate) len: u32,
    pub(crate) flags: u32,
    pub(crate) _pad0: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnalyticPathMode {
    Fill,
    Stroke,
}

impl AnalyticPathMode {
    pub(crate) const fn to_gpu(self) -> u32 {
        match self {
            Self::Fill => 0,
            Self::Stroke => 1,
        }
    }
}

pub(crate) const ANALYTIC_CONTOUR_FLAG_CLOSED: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct AnalyticPointGpu {
    pub(crate) position: [f32; 2],
    pub(crate) _pad: [f32; 2],
}

#[derive(Debug, Clone)]
pub(crate) struct AnalyticPathCpuData {
    pub(crate) resource_signature: u64,
    pub(crate) mode: AnalyticPathMode,
    pub(crate) feather_width: f32,
    pub(crate) stroke_width: f32,
    pub(crate) contours: Vec<AnalyticContourGpu>,
    pub(crate) points: Vec<AnalyticPointGpu>,
}

impl AnalyticPathCpuData {
    pub(crate) fn new(
        mode: AnalyticPathMode,
        feather_width: f32,
        stroke_width: f32,
        contours: Vec<AnalyticContourGpu>,
        points: Vec<AnalyticPointGpu>,
    ) -> Self {
        let mut data = Self {
            resource_signature: 0,
            mode,
            feather_width,
            stroke_width,
            contours,
            points,
        };
        data.resource_signature = data.compute_signature();
        data
    }

    pub(crate) fn meta(&self, contour_start: u32, point_start: u32) -> AnalyticPathMetaGpu {
        AnalyticPathMetaGpu {
            contour_start,
            contour_count: self.contours.len() as u32,
            point_start,
            mode: self.mode.to_gpu(),
            feather_width: self.feather_width,
            stroke_width: self.stroke_width,
            _pad0: [0.0; 2],
        }
    }

    pub(crate) fn byte_size(&self) -> usize {
        std::mem::size_of::<AnalyticPathMetaGpu>()
            + self.contours.len() * std::mem::size_of::<AnalyticContourGpu>()
            + self.points.len() * std::mem::size_of::<AnalyticPointGpu>()
    }

    pub(crate) fn compute_signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.mode.to_gpu().hash(&mut hasher);
        self.feather_width.to_bits().hash(&mut hasher);
        self.stroke_width.to_bits().hash(&mut hasher);
        self.contours.len().hash(&mut hasher);
        self.points.len().hash(&mut hasher);
        for contour in &self.contours {
            contour.start.hash(&mut hasher);
            contour.len.hash(&mut hasher);
            contour.flags.hash(&mut hasher);
        }
        for point in &self.points {
            point.position[0].to_bits().hash(&mut hasher);
            point.position[1].to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }
}

pub(crate) fn hash_transform(hasher: &mut DefaultHasher, transform: Transform) {
    transform.xx.to_bits().hash(hasher);
    transform.yx.to_bits().hash(hasher);
    transform.xy.to_bits().hash(hasher);
    transform.yy.to_bits().hash(hasher);
    transform.dx.to_bits().hash(hasher);
    transform.dy.to_bits().hash(hasher);
}

pub(crate) fn transform_scene_path(path: &ScenePath, transform: Transform) -> ScenePath {
    let mut builder = ScenePath::builder();
    for element in path.elements() {
        match element {
            PathElement::MoveTo(point) => {
                builder.move_to(transform.transform_point(*point));
            }
            PathElement::LineTo(point) => {
                builder.line_to(transform.transform_point(*point));
            }
            PathElement::QuadTo { ctrl, to } => {
                builder.quad_to(
                    transform.transform_point(*ctrl),
                    transform.transform_point(*to),
                );
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                builder.cubic_to(
                    transform.transform_point(*ctrl1),
                    transform.transform_point(*ctrl2),
                    transform.transform_point(*to),
                );
            }
            PathElement::Close => {
                builder.close();
            }
        }
    }
    builder.build()
}

pub(crate) fn hash_rect(hasher: &mut DefaultHasher, rect: Rect) {
    rect.origin.x.to_bits().hash(hasher);
    rect.origin.y.to_bits().hash(hasher);
    rect.size.width.to_bits().hash(hasher);
    rect.size.height.to_bits().hash(hasher);
}

pub(crate) fn hash_point(hasher: &mut DefaultHasher, point: Point) {
    point.x.to_bits().hash(hasher);
    point.y.to_bits().hash(hasher);
}

pub(crate) fn hash_path(path: &ScenePath, transform: Transform) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_transform(&mut hasher, transform);
    hash_rect(&mut hasher, path.bounds());
    for element in path.elements() {
        match element {
            PathElement::MoveTo(point) => {
                0u8.hash(&mut hasher);
                hash_point(&mut hasher, *point);
            }
            PathElement::LineTo(point) => {
                1u8.hash(&mut hasher);
                hash_point(&mut hasher, *point);
            }
            PathElement::QuadTo { ctrl, to } => {
                2u8.hash(&mut hasher);
                hash_point(&mut hasher, *ctrl);
                hash_point(&mut hasher, *to);
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                3u8.hash(&mut hasher);
                hash_point(&mut hasher, *ctrl1);
                hash_point(&mut hasher, *ctrl2);
                hash_point(&mut hasher, *to);
            }
            PathElement::Close => {
                4u8.hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

pub(crate) fn hash_normalized_analytic_path(path: &ScenePath, transform: Transform) -> u64 {
    let (normalized, _) = normalized_analytic_path_transform(path, transform);
    let mut hasher = DefaultHasher::new();
    for element in path.elements() {
        match element {
            PathElement::MoveTo(point) => {
                0u8.hash(&mut hasher);
                hash_quantized_analytic_point(&mut hasher, normalized.transform_point(*point));
            }
            PathElement::LineTo(point) => {
                1u8.hash(&mut hasher);
                hash_quantized_analytic_point(&mut hasher, normalized.transform_point(*point));
            }
            PathElement::QuadTo { ctrl, to } => {
                2u8.hash(&mut hasher);
                hash_quantized_analytic_point(&mut hasher, normalized.transform_point(*ctrl));
                hash_quantized_analytic_point(&mut hasher, normalized.transform_point(*to));
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                3u8.hash(&mut hasher);
                hash_quantized_analytic_point(&mut hasher, normalized.transform_point(*ctrl1));
                hash_quantized_analytic_point(&mut hasher, normalized.transform_point(*ctrl2));
                hash_quantized_analytic_point(&mut hasher, normalized.transform_point(*to));
            }
            PathElement::Close => {
                4u8.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

pub(crate) fn analytic_path_transform_without_translation(
    transform: Transform,
) -> (Transform, Vector) {
    (
        Transform::new(
            transform.xx,
            transform.yx,
            transform.xy,
            transform.yy,
            0.0,
            0.0,
        ),
        Vector::new(transform.dx, transform.dy),
    )
}

pub(crate) fn quantize_analytic_path_coordinate(value: f32) -> f32 {
    (value * ANALYTIC_PATH_COORDINATE_PRECISION).round() / ANALYTIC_PATH_COORDINATE_PRECISION
}

pub(crate) fn hash_quantized_analytic_point(hasher: &mut DefaultHasher, point: Point) {
    quantize_analytic_path_coordinate(point.x)
        .to_bits()
        .hash(hasher);
    quantize_analytic_path_coordinate(point.y)
        .to_bits()
        .hash(hasher);
}

pub(crate) fn normalized_analytic_path_transform(
    path: &ScenePath,
    transform: Transform,
) -> (Transform, Vector) {
    let (linear, translation) = analytic_path_transform_without_translation(transform);
    let local_origin = linear.transform_point(path.bounds().origin).to_vector();
    (
        linear.then(Transform::translation(-local_origin.x, -local_origin.y)),
        translation + local_origin,
    )
}

pub(crate) fn build_analytic_fill_path_data(
    path: &LyonPath,
    feather_width: f32,
) -> Option<AnalyticPathCpuData> {
    let contours = feathering::flatten_path_contours(path);
    if contours.is_empty() || contours.len() > MAX_ANALYTIC_PATH_CONTOURS {
        return None;
    }

    let mut contour_data = Vec::with_capacity(contours.len());
    let mut point_data = Vec::new();

    for contour in contours {
        if !contour.closed || contour.points.len() < 3 {
            return None;
        }

        let start = point_data.len() as u32;
        for point in contour.points {
            point_data.push(AnalyticPointGpu {
                position: [
                    quantize_analytic_path_coordinate(point.x),
                    quantize_analytic_path_coordinate(point.y),
                ],
                _pad: [0.0, 0.0],
            });
            if point_data.len() > MAX_ANALYTIC_PATH_POINTS {
                return None;
            }
        }
        contour_data.push(AnalyticContourGpu {
            start,
            len: (point_data.len() as u32).saturating_sub(start),
            flags: ANALYTIC_CONTOUR_FLAG_CLOSED,
            _pad0: 0,
        });
    }

    if point_data.is_empty() {
        return None;
    }

    Some(AnalyticPathCpuData::new(
        AnalyticPathMode::Fill,
        feather_width.max(0.0),
        0.0,
        contour_data,
        point_data,
    ))
}

pub(crate) fn build_analytic_stroke_path_data(
    path: &LyonPath,
    line_width: f32,
    feather_width: f32,
) -> Option<AnalyticPathCpuData> {
    let contours = feathering::flatten_path_contours(path);
    if contours.is_empty() || contours.len() > MAX_ANALYTIC_PATH_CONTOURS {
        return None;
    }

    let mut contour_data = Vec::with_capacity(contours.len());
    let mut point_data = Vec::new();

    for contour in contours {
        let minimum_points = if contour.closed { 3 } else { 2 };
        if contour.points.len() < minimum_points {
            return None;
        }

        let start = point_data.len() as u32;
        for point in contour.points {
            point_data.push(AnalyticPointGpu {
                position: [
                    quantize_analytic_path_coordinate(point.x),
                    quantize_analytic_path_coordinate(point.y),
                ],
                _pad: [0.0, 0.0],
            });
            if point_data.len() > MAX_ANALYTIC_PATH_POINTS {
                return None;
            }
        }
        contour_data.push(AnalyticContourGpu {
            start,
            len: (point_data.len() as u32).saturating_sub(start),
            flags: if contour.closed {
                ANALYTIC_CONTOUR_FLAG_CLOSED
            } else {
                0
            },
            _pad0: 0,
        });
    }

    if point_data.is_empty() {
        return None;
    }

    Some(AnalyticPathCpuData::new(
        AnalyticPathMode::Stroke,
        feather_width.max(0.0),
        line_width.max(0.5),
        contour_data,
        point_data,
    ))
}

pub(crate) fn append_analytic_path_quad(
    vertices: &mut Vec<Vertex>,
    rect: Rect,
    color: Color,
    viewport: Size,
    path_origin: Vector,
) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let min = to_ndc(rect.x(), rect.y(), viewport);
    let max = to_ndc(rect.max_x(), rect.max_y(), viewport);
    let rgba = shader_color(color);
    let x0 = rect.x();
    let x1 = rect.max_x();
    let y0 = rect.y();
    let y1 = rect.max_y();

    vertices.extend_from_slice(&[
        Vertex::basic(
            [min[0], min[1]],
            rgba,
            [x0 - path_origin.x, y0 - path_origin.y],
            [0.0; 4],
        ),
        Vertex::basic(
            [max[0], min[1]],
            rgba,
            [x1 - path_origin.x, y0 - path_origin.y],
            [0.0; 4],
        ),
        Vertex::basic(
            [min[0], max[1]],
            rgba,
            [x0 - path_origin.x, y1 - path_origin.y],
            [0.0; 4],
        ),
        Vertex::basic(
            [min[0], max[1]],
            rgba,
            [x0 - path_origin.x, y1 - path_origin.y],
            [0.0; 4],
        ),
        Vertex::basic(
            [max[0], min[1]],
            rgba,
            [x1 - path_origin.x, y0 - path_origin.y],
            [0.0; 4],
        ),
        Vertex::basic(
            [max[0], max[1]],
            rgba,
            [x1 - path_origin.x, y1 - path_origin.y],
            [0.0; 4],
        ),
    ]);
}

pub(crate) fn tessellate_filled_lyon_path(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    viewport: Size,
) -> Result<()> {
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = FillTessellator::new();
    tessellator
        .tessellate_path(path, &FillOptions::default(), &mut builder)
        .map_err(|error| Error::new(format!("failed to tessellate filled path: {error}")))?;

    append_indexed_triangles(vertices, &buffers, color, viewport);
    Ok(())
}

pub(crate) fn append_tessellated_filled_lyon_path_vertices(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    viewport: Size,
) -> Result<()> {
    tessellate_filled_lyon_path(vertices, path, Color::rgba(0.0, 0.0, 0.0, 0.0), viewport)
}

pub(crate) fn build_lyon_path(path: &ScenePath, transform: Transform) -> LyonPath {
    let mut builder = LyonPath::builder();
    let mut contour_open = false;

    for element in path.elements() {
        match element {
            PathElement::MoveTo(point_value) => {
                if contour_open {
                    LyonPathBuilder::end(&mut builder, false);
                }
                LyonPathBuilder::begin(
                    &mut builder,
                    transform_path_point(*point_value, transform),
                    &[],
                );
                contour_open = true;
            }
            PathElement::LineTo(point_value) => {
                if contour_open {
                    LyonPathBuilder::line_to(
                        &mut builder,
                        transform_path_point(*point_value, transform),
                        &[],
                    );
                }
            }
            PathElement::QuadTo { ctrl, to } => {
                if contour_open {
                    LyonPathBuilder::quadratic_bezier_to(
                        &mut builder,
                        transform_path_point(*ctrl, transform),
                        transform_path_point(*to, transform),
                        &[],
                    );
                }
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                if contour_open {
                    LyonPathBuilder::cubic_bezier_to(
                        &mut builder,
                        transform_path_point(*ctrl1, transform),
                        transform_path_point(*ctrl2, transform),
                        transform_path_point(*to, transform),
                        &[],
                    );
                }
            }
            PathElement::Close => {
                if contour_open {
                    LyonPathBuilder::end(&mut builder, true);
                    contour_open = false;
                }
            }
        }
    }

    if contour_open {
        LyonPathBuilder::end(&mut builder, false);
    }

    builder.build()
}

pub(crate) fn transform_path_point(
    point_value: Point,
    transform: Transform,
) -> lyon_path::math::Point {
    let scene = transform.transform_point(point_value);
    point(scene.x, scene.y)
}

pub(crate) fn append_indexed_triangles(
    vertices: &mut Vec<Vertex>,
    buffers: &VertexBuffers<[f32; 2], u32>,
    color: Color,
    viewport: Size,
) {
    if viewport.is_empty() {
        return;
    }

    let rgba = shader_color(color);
    for index in &buffers.indices {
        let position = buffers.vertices[*index as usize];
        let ndc = to_ndc(position[0], position[1], viewport);
        vertices.push(Vertex::basic([ndc[0], ndc[1]], rgba, [0.0, 0.0], [0.0; 4]));
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CachedGlyphVertex {
    pub(crate) position: Point,
    pub(crate) coverage: f32,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct CachedGlyphMesh {
    pub(crate) vertices: Vec<CachedGlyphVertex>,
    pub(crate) indices: Vec<u32>,
}

impl CachedGlyphMesh {
    pub(crate) fn push_vertex(&mut self, position: Point, coverage: f32) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(CachedGlyphVertex { position, coverage });
        index
    }

    #[cfg(test)]
    pub(crate) fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}
