use crate::diagnostics::RendererFrameStats;
use crate::geometry::AnalyticPathCpuData;
use crate::geometry::append_tessellated_filled_lyon_path_vertices;
use crate::geometry::build_lyon_path;
use crate::gpu::AnalyticQuadInstance;
use crate::gpu::CompactVertex;
use crate::gpu::ExtendedQuadInstance;
use crate::gpu::SolidVertex;
use crate::gpu::TextAtlasInstance;
use crate::gpu::Vertex;
use crate::primitives::to_ndc;
use crate::retained::ResolvedClipPrimitive;
use crate::retained::ResolvedRasterState;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use sui_core::ImageHandle;
use sui_core::Path as ScenePath;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Result;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;
use sui_scene::ImageSampling;
use sui_scene::TextRenderPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawOpKind {
    Solid,
    Image {
        handle: ImageHandle,
        sampling: ImageSampling,
        raster_size: ImageRasterSize,
    },
    TextAtlas,
    AnalyticPath {
        id: u64,
    },
    WidgetShader,
    RoundedRect,
    GradientRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ImageRasterSize {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl ImageRasterSize {
    pub(crate) fn new(width: f32, height: f32) -> Self {
        Self {
            width: width.round().max(1.0) as u32,
            height: height.round().max(1.0) as u32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ImageBindGroupKey {
    pub(crate) handle: ImageHandle,
    pub(crate) sampling: ImageSampling,
    pub(crate) raster_size: ImageRasterSize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageDrawMetadata {
    pub(crate) bounds: Rect,
    pub(crate) pixel_snap: sui_scene::ImagePixelSnap,
}

#[derive(Debug, Clone)]
pub(crate) struct DrawOp {
    pub(crate) kind: DrawOpKind,
    pub(crate) vertices: PreparedVertices,
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) clip_state_index: usize,
    pub(crate) image: Option<ImageDrawMetadata>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct DrawOpArena {
    pub(crate) scene_vertices: Vec<Vertex>,
    pub(crate) clip_vertices: Vec<Vertex>,
    pub(crate) text_instances: Vec<TextAtlasInstance>,
    pub(crate) clip_states: Vec<ClipState>,
    pub(crate) draw_ops: Vec<DrawOp>,
    pub(crate) analytic_paths: HashMap<u64, Arc<AnalyticPathCpuData>>,
    pub(crate) next_analytic_path_id: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ClipState {
    pub(crate) clip_paths: Vec<PreparedVertices>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedFrameBatches {
    pub(crate) solid_vertices: Vec<SolidVertex>,
    pub(crate) scene_vertices: Vec<CompactVertex>,
    pub(crate) analytic_vertices: Vec<AnalyticQuadInstance>,
    pub(crate) extended_vertices: Vec<ExtendedQuadInstance>,
    pub(crate) clip_vertices: Vec<SolidVertex>,
    pub(crate) text_instances: Vec<TextAtlasInstance>,
    pub(crate) passes: Vec<PreparedPassBatch>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedPassBatch {
    pub(crate) clip_paths: Vec<PreparedClipPath>,
    pub(crate) draws: Vec<PreparedDrawBatch>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedClipPath {
    pub(crate) vertices: PreparedVertices,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparedDrawKind {
    Solid,
    Image {
        handle: ImageHandle,
        sampling: ImageSampling,
        raster_size: ImageRasterSize,
    },
    TextAtlas,
    AnalyticPath {
        resource_signature: u64,
    },
    WidgetShader,
    RoundedRect,
    GradientRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparedDrawPipelineKind {
    Solid,
    Image,
    TextAtlas,
    AnalyticPath,
    WidgetShader,
    RoundedRect,
    GradientRect,
}

impl PreparedDrawKind {
    pub(crate) const fn pipeline_kind(self) -> PreparedDrawPipelineKind {
        match self {
            Self::Solid => PreparedDrawPipelineKind::Solid,
            Self::Image { .. } => PreparedDrawPipelineKind::Image,
            Self::TextAtlas => PreparedDrawPipelineKind::TextAtlas,
            Self::AnalyticPath { .. } => PreparedDrawPipelineKind::AnalyticPath,
            Self::WidgetShader => PreparedDrawPipelineKind::WidgetShader,
            Self::RoundedRect => PreparedDrawPipelineKind::RoundedRect,
            Self::GradientRect => PreparedDrawPipelineKind::GradientRect,
        }
    }

    pub(crate) const fn uses_extended_vertices(self) -> bool {
        matches!(self, Self::RoundedRect | Self::GradientRect)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CachedPassBatch {
    pub(crate) clip_paths: Vec<PreparedClipPath>,
    pub(crate) draws: Vec<CachedDrawBatch>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CachedDrawBatch {
    pub(crate) kind: PreparedDrawKind,
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) vertices: PreparedVertices,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedDrawBatch {
    pub(crate) kind: PreparedDrawKind,
    pub(crate) clip_rect: Option<ScissorRect>,
    pub(crate) vertices: PreparedVertices,
}

pub(crate) struct PreparedFragmentSubmission {
    pub(crate) passes: Vec<PreparedPassBatch>,
    pub(crate) solid_buffer: Option<wgpu::Buffer>,
    pub(crate) scene_buffer: Option<wgpu::Buffer>,
    pub(crate) analytic_buffer: Option<wgpu::Buffer>,
    pub(crate) extended_buffer: Option<wgpu::Buffer>,
    pub(crate) clip_buffer: Option<wgpu::Buffer>,
    pub(crate) text_instance_buffer: Option<wgpu::Buffer>,
    pub(crate) translation: Vector,
}

pub(crate) struct PreparedSceneSubmission {
    pub(crate) viewport: Size,
    pub(crate) framebuffer_size: (u32, u32),
    pub(crate) encodable_passes: Vec<EncodablePassBatch>,
    pub(crate) image_bind_groups: HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    pub(crate) text_atlas_bind_group: Option<wgpu::BindGroup>,
    pub(crate) analytic_path_resources: Option<PreparedAnalyticPathResources>,
    pub(crate) frame_stats: RendererFrameStats,
}

pub(crate) struct PreparedAnalyticPathResources {
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) slots: HashMap<u64, u32>,
}

pub(crate) struct EncodablePassBatch {
    pub(crate) pass: PreparedPassBatch,
    pub(crate) solid_buffer: Option<wgpu::Buffer>,
    pub(crate) scene_buffer: Option<wgpu::Buffer>,
    pub(crate) analytic_buffer: Option<wgpu::Buffer>,
    pub(crate) extended_buffer: Option<wgpu::Buffer>,
    pub(crate) clip_buffer: Option<wgpu::Buffer>,
    pub(crate) text_instance_buffer: Option<wgpu::Buffer>,
    pub(crate) translation: Vector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedVertices {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl PreparedVertices {
    pub(crate) fn offset(self, delta: u32) -> Self {
        Self {
            start: self.start + delta,
            len: self.len,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScissorRect {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneRasterState {
    pub(crate) current_transform: Transform,
    pub(crate) pixel_snap_offset: Vector,
    pub(crate) transform_stack: Vec<Transform>,
    pub(crate) clip_stack: Vec<ClipPrimitive>,
    pub(crate) text_render_policy: Option<TextRenderPolicy>,
    pub(crate) text_render_policy_stack: Vec<Option<TextRenderPolicy>>,
    pub(crate) path_clip_state_id: u64,
    pub(crate) active_path_clips: Vec<PreparedVertices>,
    pub(crate) clip_state_index: usize,
}

impl SceneRasterState {
    pub(crate) fn new(draw_ops: &mut DrawOpArena) -> Self {
        let clip_state_index = draw_ops.push_clip_state(&[]);
        Self {
            current_transform: Transform::IDENTITY,
            pixel_snap_offset: Vector::ZERO,
            transform_stack: Vec::new(),
            clip_stack: Vec::new(),
            text_render_policy: None,
            text_render_policy_stack: Vec::new(),
            path_clip_state_id: 0,
            active_path_clips: Vec::new(),
            clip_state_index,
        }
    }

    pub(crate) fn from_resolved(
        resolved: &ResolvedRasterState,
        draw_ops: &mut DrawOpArena,
        viewport: Size,
    ) -> Result<Self> {
        let mut state = Self::new(draw_ops);
        state.current_transform = resolved.current_transform;
        state.pixel_snap_offset = resolved.pixel_snap_offset;
        state.transform_stack.clear();
        state.text_render_policy = None;
        state.text_render_policy_stack.clear();
        state.path_clip_state_id = 0;
        state.active_path_clips.clear();
        state.clip_stack.clear();

        for clip in &resolved.clip_stack {
            match clip {
                ResolvedClipPrimitive::Rect(rect) => {
                    state.clip_stack.push(ClipPrimitive::Rect(*rect));
                }
                ResolvedClipPrimitive::Path { path, bounds, .. } => {
                    let mut scratch = Vec::new();
                    if !path.is_empty() && !viewport.is_empty() {
                        let lyon_path = build_lyon_path(path, Transform::IDENTITY);
                        append_tessellated_filled_lyon_path_vertices(
                            &mut scratch,
                            &lyon_path,
                            viewport,
                        )?;
                    }
                    let vertices = draw_ops.push_clip_vertices(&scratch);
                    state.active_path_clips.push(vertices);
                    state
                        .clip_stack
                        .push(ClipPrimitive::Path { bounds: *bounds });
                }
            }
        }

        state.clip_state_index = draw_ops.push_clip_state(&state.active_path_clips);
        Ok(state)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ClipPrimitive {
    Rect(Rect),
    Path { bounds: Rect },
}

impl ClipPrimitive {
    pub(crate) fn bounds(&self) -> Rect {
        match self {
            Self::Rect(rect) => *rect,
            Self::Path { bounds, .. } => *bounds,
        }
    }
}

impl SceneRasterState {
    pub(crate) fn push_clip(&mut self, rect: Rect) {
        let transformed = self.current_transform.transform_rect_bbox(rect);
        self.clip_stack.push(ClipPrimitive::Rect(transformed));
    }

    pub(crate) fn push_clip_path(
        &mut self,
        path: &ScenePath,
        viewport: Size,
        draw_ops: &mut DrawOpArena,
        scratch_vertices: &mut Vec<Vertex>,
    ) -> Result<()> {
        let bounds = self.current_transform.transform_rect_bbox(path.bounds());
        scratch_vertices.clear();
        if !path.is_empty() && !viewport.is_empty() {
            let lyon_path = build_lyon_path(path, self.current_transform);
            append_tessellated_filled_lyon_path_vertices(scratch_vertices, &lyon_path, viewport)?;
        }
        let vertices = draw_ops.push_clip_vertices(scratch_vertices);
        self.clip_stack.push(ClipPrimitive::Path { bounds });
        self.active_path_clips.push(vertices);
        self.path_clip_state_id = self.path_clip_state_id.wrapping_add(1);
        self.clip_state_index = draw_ops.push_clip_state(&self.active_path_clips);
        Ok(())
    }

    pub(crate) fn pop_clip(&mut self, draw_ops: &mut DrawOpArena) {
        if matches!(self.clip_stack.pop(), Some(ClipPrimitive::Path { .. })) {
            let _ = self.active_path_clips.pop();
            self.path_clip_state_id = self.path_clip_state_id.wrapping_add(1);
            self.clip_state_index = draw_ops.push_clip_state(&self.active_path_clips);
        }
    }

    pub(crate) fn push_transform(&mut self, transform: Transform) {
        self.transform_stack.push(self.current_transform);
        self.current_transform = self.current_transform.then(transform);
    }

    pub(crate) fn pop_transform(&mut self) {
        self.current_transform = self.transform_stack.pop().unwrap_or(Transform::IDENTITY);
    }

    pub(crate) fn push_text_render_policy(&mut self, policy: TextRenderPolicy) {
        self.text_render_policy_stack.push(self.text_render_policy);
        self.text_render_policy = Some(policy.normalized());
    }

    pub(crate) fn pop_text_render_policy(&mut self) {
        self.text_render_policy = self.text_render_policy_stack.pop().unwrap_or(None);
    }

    pub(crate) fn active_text_render_policy(&self) -> Option<TextRenderPolicy> {
        self.text_render_policy
    }

    pub(crate) fn current_clip_bounds(&self) -> Option<Rect> {
        let mut clips = self.clip_stack.iter().map(ClipPrimitive::bounds);
        let first = clips.next()?;
        Some(clips.fold(first, |current, clip| {
            current.intersection(clip).unwrap_or(Rect::ZERO)
        }))
    }

    pub(crate) fn visible_rect(&self, rect: Rect) -> Option<Rect> {
        let transformed = self.current_transform.transform_rect_bbox(rect);

        match self.current_clip_bounds() {
            Some(clip) => transformed.intersection(clip),
            None => Some(transformed),
        }
    }
}

pub(crate) fn push_draw_op(
    draw_ops: &mut DrawOpArena,
    kind: DrawOpKind,
    vertices: &[Vertex],
    state: &SceneRasterState,
) {
    if vertices.is_empty() {
        return;
    }

    let vertex_span = draw_ops.push_scene_vertices(vertices);
    draw_ops.draw_ops.push(DrawOp {
        kind,
        vertices: vertex_span,
        clip_rect: state.current_clip_bounds(),
        clip_state_index: state.clip_state_index,
        image: None,
    });
}

pub(crate) fn push_image_draw_op(
    draw_ops: &mut DrawOpArena,
    kind: DrawOpKind,
    vertices: &[Vertex],
    state: &SceneRasterState,
    image: ImageDrawMetadata,
) {
    if vertices.is_empty() {
        return;
    }

    let vertex_span = draw_ops.push_scene_vertices(vertices);
    draw_ops.draw_ops.push(DrawOp {
        kind,
        vertices: vertex_span,
        clip_rect: state.current_clip_bounds(),
        clip_state_index: state.clip_state_index,
        image: Some(image),
    });
}

pub(crate) fn push_text_draw_op(
    draw_ops: &mut DrawOpArena,
    instances: &[TextAtlasInstance],
    state: &SceneRasterState,
) {
    if instances.is_empty() {
        return;
    }

    let instance_span = draw_ops.push_text_instances(instances);
    draw_ops.draw_ops.push(DrawOp {
        kind: DrawOpKind::TextAtlas,
        vertices: instance_span,
        clip_rect: state.current_clip_bounds(),
        clip_state_index: state.clip_state_index,
        image: None,
    });
}

impl DrawOpArena {
    #[cfg(test)]
    pub(crate) fn insert_analytic_path(&mut self, data: AnalyticPathCpuData) -> u64 {
        self.insert_analytic_path_arc(Arc::new(data))
    }

    pub(crate) fn insert_analytic_path_arc(&mut self, data: Arc<AnalyticPathCpuData>) -> u64 {
        let id = self.next_analytic_path_id;
        self.next_analytic_path_id = self.next_analytic_path_id.wrapping_add(1);
        self.analytic_paths.insert(id, data);
        id
    }

    pub(crate) fn import_analytic_paths(&mut self, fragment: &DrawOpArena) -> HashMap<u64, u64> {
        let mut id_map = HashMap::new();
        for (old_id, data) in &fragment.analytic_paths {
            let new_id = self.next_analytic_path_id;
            self.next_analytic_path_id = self.next_analytic_path_id.wrapping_add(1);
            self.analytic_paths.insert(new_id, Arc::clone(data));
            id_map.insert(*old_id, new_id);
        }
        id_map
    }

    pub(crate) fn transform_in_place(&mut self, transform: Transform, viewport: Size) {
        if transform.is_identity() || viewport.is_empty() {
            return;
        }

        let transform_position = |position: [f32; 2]| {
            let logical = Point::new(
                ((position[0] + 1.0) * 0.5) * viewport.width,
                ((1.0 - position[1]) * 0.5) * viewport.height,
            );
            let transformed = transform.transform_point(logical);
            to_ndc(transformed.x, transformed.y, viewport)
        };
        for vertex in &mut self.scene_vertices {
            vertex.position = transform_position(vertex.position);
        }
        for vertex in &mut self.clip_vertices {
            vertex.position = transform_position(vertex.position);
        }
        for instance in &mut self.text_instances {
            instance.top_left = transform_position(instance.top_left);
            let x_axis = transform.transform_vector(Vector::new(
                instance.x_axis[0] * viewport.width * 0.5,
                -instance.x_axis[1] * viewport.height * 0.5,
            ));
            let y_axis = transform.transform_vector(Vector::new(
                instance.y_axis[0] * viewport.width * 0.5,
                -instance.y_axis[1] * viewport.height * 0.5,
            ));
            instance.x_axis = [
                (x_axis.x / viewport.width) * 2.0,
                -((x_axis.y / viewport.height) * 2.0),
            ];
            instance.y_axis = [
                (y_axis.x / viewport.width) * 2.0,
                -((y_axis.y / viewport.height) * 2.0),
            ];
        }
        for draw_op in &mut self.draw_ops {
            draw_op.clip_rect = draw_op
                .clip_rect
                .map(|rect| transform.transform_rect_bbox(rect));
            if let Some(image) = &mut draw_op.image {
                image.bounds = transform.transform_rect_bbox(image.bounds);
            }
        }
    }

    pub(crate) fn finalize_image_draws(&mut self, viewport: Size, surface_size: Size) {
        if viewport.is_empty() || surface_size.is_empty() {
            return;
        }

        let scale_x = surface_size.width / viewport.width;
        let scale_y = surface_size.height / viewport.height;
        if scale_x <= 0.0 || scale_y <= 0.0 {
            return;
        }

        for draw_op in &mut self.draw_ops {
            let Some(mut image) = draw_op.image else {
                continue;
            };
            if image.pixel_snap != sui_scene::ImagePixelSnap::Physical || image.bounds.is_empty() {
                continue;
            }

            let old = image.bounds;
            let left_px = (old.x() * scale_x).round();
            let top_px = (old.y() * scale_y).round();
            let mut right_px = (old.max_x() * scale_x).round();
            let mut bottom_px = (old.max_y() * scale_y).round();
            if right_px <= left_px {
                right_px = left_px + 1.0;
            }
            if bottom_px <= top_px {
                bottom_px = top_px + 1.0;
            }
            let snapped = Rect::new(
                left_px / scale_x,
                top_px / scale_y,
                (right_px - left_px) / scale_x,
                (bottom_px - top_px) / scale_y,
            );
            if snapped == old {
                continue;
            }

            let start = draw_op.vertices.start as usize;
            let end = start + draw_op.vertices.len as usize;
            for vertex in &mut self.scene_vertices[start..end] {
                let logical_x = ((vertex.position[0] + 1.0) * 0.5) * viewport.width;
                let logical_y = ((1.0 - vertex.position[1]) * 0.5) * viewport.height;
                let relative_x = (logical_x - old.x()) / old.width();
                let relative_y = (logical_y - old.y()) / old.height();
                let snapped_x = snapped.x() + relative_x * snapped.width();
                let snapped_y = snapped.y() + relative_y * snapped.height();
                vertex.position = to_ndc(snapped_x, snapped_y, viewport);
            }
            image.bounds = snapped;
            draw_op.image = Some(image);
        }
    }

    pub(crate) fn apply_opacity(&mut self, opacity: f32) {
        if opacity == 1.0 {
            return;
        }

        for vertex in &mut self.scene_vertices {
            vertex.color[3] *= opacity;
        }
        for instance in &mut self.text_instances {
            instance.color[3] *= opacity;
        }
    }

    pub(crate) fn append_transformed_fragment(
        &mut self,
        fragment: &DrawOpArena,
        transform: Transform,
        opacity: f32,
        external_clips: &[ResolvedClipPrimitive],
        viewport: Size,
    ) -> Result<()> {
        if transform.is_identity() && external_clips.is_empty() && opacity == 1.0 {
            self.append_fragment(fragment);
            return Ok(());
        }

        let mut transformed = fragment.clone();
        transformed.transform_in_place(transform, viewport);
        transformed.apply_opacity(opacity);

        let scene_delta = self.scene_vertices.len() as u32;
        let clip_delta = self.clip_vertices.len() as u32;
        let text_delta = self.text_instances.len() as u32;
        let analytic_id_map = self.import_analytic_paths(&transformed);
        self.scene_vertices
            .extend_from_slice(&transformed.scene_vertices);
        self.clip_vertices
            .extend_from_slice(&transformed.clip_vertices);
        self.text_instances
            .extend_from_slice(&transformed.text_instances);

        let external_clip_rect = external_clips.iter().fold(None::<Rect>, |current, clip| {
            let bounds = clip.bounds();
            Some(match current {
                Some(existing) => existing.intersection(bounds).unwrap_or(Rect::ZERO),
                None => bounds,
            })
        });

        let mut external_path_clips = Vec::new();
        for clip in external_clips {
            if let ResolvedClipPrimitive::Path { path, .. } = clip {
                let mut vertices = Vec::new();
                if !path.is_empty() && !viewport.is_empty() {
                    let lyon_path = build_lyon_path(path, Transform::IDENTITY);
                    append_tessellated_filled_lyon_path_vertices(
                        &mut vertices,
                        &lyon_path,
                        viewport,
                    )?;
                }
                external_path_clips.push(self.push_clip_vertices(&vertices));
            }
        }

        let clip_state_base = self.clip_states.len();
        if external_path_clips.is_empty() {
            self.clip_states
                .extend(transformed.clip_states.iter().map(|clip_state| {
                    ClipState {
                        clip_paths: clip_state
                            .clip_paths
                            .iter()
                            .copied()
                            .map(|vertices| vertices.offset(clip_delta))
                            .collect(),
                    }
                }));
            self.draw_ops.extend(
                transformed
                    .draw_ops
                    .iter()
                    .cloned()
                    .filter_map(|mut draw_op| {
                        draw_op.vertices = match draw_op.kind {
                            DrawOpKind::TextAtlas => draw_op.vertices.offset(text_delta),
                            _ => draw_op.vertices.offset(scene_delta),
                        };
                        draw_op.clip_state_index += clip_state_base;
                        let clip_rect =
                            resolve_fragment_clip_rect(draw_op.clip_rect, external_clip_rect)?;
                        draw_op.clip_rect = clip_rect;
                        if let DrawOpKind::AnalyticPath { id } = draw_op.kind {
                            draw_op.kind = DrawOpKind::AnalyticPath {
                                id: analytic_id_map[&id],
                            };
                        }
                        Some(draw_op)
                    }),
            );
            return Ok(());
        }

        let mut clip_state_map = HashMap::new();
        for draw_op in transformed.draw_ops.iter().cloned() {
            let merged_clip_state = *clip_state_map
                .entry(draw_op.clip_state_index)
                .or_insert_with(|| {
                    let mut clip_paths = transformed.clip_states[draw_op.clip_state_index]
                        .clip_paths
                        .iter()
                        .copied()
                        .map(|vertices| vertices.offset(clip_delta))
                        .collect::<Vec<_>>();
                    clip_paths.extend(external_path_clips.iter().copied());
                    self.push_clip_state(&clip_paths)
                });

            let Some(clip_rect) = resolve_fragment_clip_rect(draw_op.clip_rect, external_clip_rect)
            else {
                continue;
            };

            self.draw_ops.push(DrawOp {
                kind: draw_op.kind,
                vertices: match draw_op.kind {
                    DrawOpKind::TextAtlas => draw_op.vertices.offset(text_delta),
                    _ => draw_op.vertices.offset(scene_delta),
                },
                clip_rect,
                clip_state_index: merged_clip_state,
                image: draw_op.image,
            });
            if let DrawOpKind::AnalyticPath { id } = draw_op.kind {
                let last = self.draw_ops.last_mut().expect("analytic draw op inserted");
                last.kind = DrawOpKind::AnalyticPath {
                    id: analytic_id_map[&id],
                };
            }
        }

        Ok(())
    }

    pub(crate) fn append_fragment(&mut self, fragment: &DrawOpArena) {
        let scene_delta = self.scene_vertices.len() as u32;
        let clip_delta = self.clip_vertices.len() as u32;
        let text_delta = self.text_instances.len() as u32;
        let clip_state_delta = self.clip_states.len();
        let analytic_id_map = self.import_analytic_paths(fragment);

        self.scene_vertices
            .extend_from_slice(&fragment.scene_vertices);
        self.clip_vertices
            .extend_from_slice(&fragment.clip_vertices);
        self.text_instances
            .extend_from_slice(&fragment.text_instances);
        self.clip_states
            .extend(fragment.clip_states.iter().map(|clip_state| {
                ClipState {
                    clip_paths: clip_state
                        .clip_paths
                        .iter()
                        .copied()
                        .map(|vertices| vertices.offset(clip_delta))
                        .collect(),
                }
            }));
        self.draw_ops
            .extend(fragment.draw_ops.iter().cloned().map(|mut draw_op| {
                draw_op.vertices = match draw_op.kind {
                    DrawOpKind::TextAtlas => draw_op.vertices.offset(text_delta),
                    _ => draw_op.vertices.offset(scene_delta),
                };
                draw_op.clip_state_index += clip_state_delta;
                if let DrawOpKind::AnalyticPath { id } = draw_op.kind {
                    draw_op.kind = DrawOpKind::AnalyticPath {
                        id: analytic_id_map[&id],
                    };
                }
                draw_op
            }));
    }

    pub(crate) fn push_scene_vertices(&mut self, vertices: &[Vertex]) -> PreparedVertices {
        let start = self.scene_vertices.len() as u32;
        self.scene_vertices.extend_from_slice(vertices);
        PreparedVertices {
            start,
            len: vertices.len() as u32,
        }
    }

    pub(crate) fn push_text_instances(
        &mut self,
        instances: &[TextAtlasInstance],
    ) -> PreparedVertices {
        let start = self.text_instances.len() as u32;
        self.text_instances.extend_from_slice(instances);
        PreparedVertices {
            start,
            len: instances.len() as u32,
        }
    }

    pub(crate) fn push_clip_vertices(&mut self, vertices: &[Vertex]) -> PreparedVertices {
        let start = self.clip_vertices.len() as u32;
        self.clip_vertices.extend_from_slice(vertices);
        PreparedVertices {
            start,
            len: vertices.len() as u32,
        }
    }

    pub(crate) fn push_clip_state(&mut self, clip_paths: &[PreparedVertices]) -> usize {
        self.clip_states.push(ClipState {
            clip_paths: clip_paths.to_vec(),
        });
        self.clip_states.len() - 1
    }

    #[cfg(test)]
    pub(crate) fn scene_vertices(&self, span: PreparedVertices) -> &[Vertex] {
        &self.scene_vertices[span.start as usize..(span.start + span.len) as usize]
    }

    #[cfg(test)]
    pub(crate) fn text_instances(&self, span: PreparedVertices) -> &[TextAtlasInstance] {
        &self.text_instances[span.start as usize..(span.start + span.len) as usize]
    }
}

pub(crate) fn resolve_submission_clip_rect(
    current: Option<Rect>,
    next: Option<Rect>,
) -> Option<Option<Rect>> {
    match (current, next) {
        (Some(current), Some(next)) => current.intersection(next).map(Some),
        (Some(current), None) => Some(Some(current)),
        (None, Some(next)) => Some(Some(next)),
        (None, None) => Some(None),
    }
}

pub(crate) fn resolve_fragment_clip_rect(
    current: Option<Rect>,
    next: Option<Rect>,
) -> Option<Option<Rect>> {
    match (current, next) {
        (Some(current), Some(next)) => current.intersection(next).map(Some),
        (Some(current), None) => Some(Some(current)),
        (None, Some(next)) => Some(Some(next)),
        (None, None) => Some(None),
    }
}
