#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_variables))]
#![allow(clippy::too_many_arguments)]

use super::*;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_surface;

#[cfg(test)]
pub(crate) fn build_vertices(
    frame: &SceneFrame,
    text_engine: &mut TextEngine,
) -> Result<Vec<Vertex>> {
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = compositor.prepare_frame(frame, text_engine, DEFAULT_FEATHER_WIDTH)?;
    let mut vertices = Vec::new();
    for op in &draw_ops.draw_ops {
        match op.kind {
            DrawOpKind::TextAtlas => {
                append_text_instance_vertices(&mut vertices, draw_ops.text_instances(op.vertices));
            }
            _ => vertices.extend_from_slice(draw_ops.scene_vertices(op.vertices)),
        }
    }
    Ok(vertices)
}

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
    fn new(width: f32, height: f32) -> Self {
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
enum AnalyticPathMode {
    Fill,
    Stroke,
}

impl AnalyticPathMode {
    const fn to_gpu(self) -> u32 {
        match self {
            Self::Fill => 0,
            Self::Stroke => 1,
        }
    }
}

const ANALYTIC_CONTOUR_FLAG_CLOSED: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct AnalyticPointGpu {
    pub(crate) position: [f32; 2],
    pub(crate) _pad: [f32; 2],
}

#[derive(Debug, Clone)]
pub(crate) struct AnalyticPathCpuData {
    pub(crate) resource_signature: u64,
    mode: AnalyticPathMode,
    pub(crate) feather_width: f32,
    pub(crate) stroke_width: f32,
    pub(crate) contours: Vec<AnalyticContourGpu>,
    pub(crate) points: Vec<AnalyticPointGpu>,
}

impl AnalyticPathCpuData {
    fn new(
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

    pub(crate) fn translate(&mut self, delta: Vector) {
        if delta == Vector::ZERO {
            return;
        }

        for point in &mut self.points {
            point.position[0] += delta.x;
            point.position[1] += delta.y;
        }
        self.resource_signature = self.compute_signature();
    }

    pub(crate) fn byte_size(&self) -> usize {
        std::mem::size_of::<AnalyticPathMetaGpu>()
            + self.contours.len() * std::mem::size_of::<AnalyticContourGpu>()
            + self.points.len() * std::mem::size_of::<AnalyticPointGpu>()
    }

    fn compute_signature(&self) -> u64 {
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

#[derive(Debug, Clone)]
pub(crate) struct ClipState {
    pub(crate) clip_paths: Vec<PreparedVertices>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedFrameBatches {
    pub(crate) scene_vertices: Vec<Vertex>,
    pub(crate) clip_vertices: Vec<Vertex>,
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
enum PreparedDrawPipelineKind {
    Solid,
    Image,
    TextAtlas,
    AnalyticPath,
    WidgetShader,
    RoundedRect,
    GradientRect,
}

impl PreparedDrawKind {
    const fn pipeline_kind(self) -> PreparedDrawPipelineKind {
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
    pub(crate) scene_buffer: Option<wgpu::Buffer>,
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
    pub(crate) scene_buffer: Option<wgpu::Buffer>,
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

pub(crate) fn prepare_frame_batches(
    draw_ops: DrawOpArena,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> PreparedFrameBatches {
    let passes = batch_draw_ops(&draw_ops, viewport, framebuffer_size);
    PreparedFrameBatches {
        scene_vertices: draw_ops.scene_vertices,
        clip_vertices: draw_ops.clip_vertices,
        text_instances: draw_ops.text_instances,
        passes,
    }
}

pub(crate) fn batch_draw_ops(
    draw_ops: &DrawOpArena,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> Vec<PreparedPassBatch> {
    let cached_passes = cache_draw_ops(draw_ops);
    prepare_cached_passes(
        &cached_passes,
        viewport,
        framebuffer_size,
        Vector::ZERO,
        None,
        0,
        0,
        0,
    )
}

pub(crate) fn cache_draw_ops(draw_ops: &DrawOpArena) -> Vec<CachedPassBatch> {
    let mut passes = Vec::new();

    for op in &draw_ops.draw_ops {
        let share_pass = passes.last().is_some_and(|pass: &CachedPassBatch| {
            let op_clip = &draw_ops.clip_states[op.clip_state_index];
            pass.clip_paths.len() == op_clip.clip_paths.len()
                && pass
                    .clip_paths
                    .iter()
                    .zip(op_clip.clip_paths.iter())
                    .all(|(a, b)| a.vertices.start == b.start && a.vertices.len == b.len)
        });
        if !share_pass {
            let clip_state = &draw_ops.clip_states[op.clip_state_index];
            passes.push(CachedPassBatch {
                clip_paths: clip_state
                    .clip_paths
                    .iter()
                    .copied()
                    .map(|vertices| PreparedClipPath { vertices })
                    .collect(),
                draws: Vec::new(),
            });
        }

        let pass = passes
            .last_mut()
            .expect("cached pass created before draw insertion");
        let kind = prepared_draw_kind(draw_ops, op);
        let clip_rect = op.clip_rect;
        if let Some(previous) = pass.draws.last_mut() {
            let previous_end = previous.vertices.start + previous.vertices.len;
            if previous.kind == kind
                && previous.clip_rect == clip_rect
                && previous_end == op.vertices.start
            {
                previous.vertices.len += op.vertices.len;
                continue;
            }
        }

        pass.draws.push(CachedDrawBatch {
            kind,
            clip_rect,
            vertices: op.vertices,
        });
    }

    passes
}

pub(crate) fn prepare_cached_passes(
    cached_passes: &[CachedPassBatch],
    viewport: Size,
    framebuffer_size: (u32, u32),
    translation: Vector,
    external_clip_rect: Option<Rect>,
    scene_vertex_offset: u32,
    clip_vertex_offset: u32,
    text_instance_offset: u32,
) -> Vec<PreparedPassBatch> {
    cached_passes
        .iter()
        .map(|pass| PreparedPassBatch {
            clip_paths: pass
                .clip_paths
                .iter()
                .copied()
                .map(|clip_path| PreparedClipPath {
                    vertices: clip_path.vertices.offset(clip_vertex_offset),
                })
                .collect(),
            draws: pass
                .draws
                .iter()
                .filter_map(|draw| {
                    let clip_rect = resolve_submission_clip_rect(
                        draw.clip_rect.map(|rect| rect.translate(translation)),
                        external_clip_rect,
                    )?;
                    let clip_rect = match clip_rect {
                        Some(rect) => {
                            if rect.is_empty() {
                                return None;
                            }
                            rect_to_scissor(rect, viewport, framebuffer_size)
                        }
                        None => None,
                    };
                    Some(PreparedDrawBatch {
                        kind: draw.kind,
                        clip_rect,
                        vertices: match draw.kind {
                            PreparedDrawKind::TextAtlas => {
                                draw.vertices.offset(text_instance_offset)
                            }
                            _ => draw.vertices.offset(scene_vertex_offset),
                        },
                    })
                })
                .collect(),
        })
        .collect()
}

fn prepared_draw_kind(draw_ops: &DrawOpArena, op: &DrawOp) -> PreparedDrawKind {
    match op.kind {
        DrawOpKind::Solid => PreparedDrawKind::Solid,
        DrawOpKind::Image {
            handle,
            sampling,
            raster_size,
        } => PreparedDrawKind::Image {
            handle,
            sampling,
            raster_size,
        },
        DrawOpKind::TextAtlas => PreparedDrawKind::TextAtlas,
        DrawOpKind::AnalyticPath { id } => PreparedDrawKind::AnalyticPath {
            resource_signature: draw_ops.analytic_paths[&id].resource_signature,
        },
        DrawOpKind::WidgetShader => PreparedDrawKind::WidgetShader,
        DrawOpKind::RoundedRect => PreparedDrawKind::RoundedRect,
        DrawOpKind::GradientRect => PreparedDrawKind::GradientRect,
    }
}

pub(crate) fn collect_draw_op_resources(
    draw_ops: &DrawOpArena,
    analytic_paths: &mut HashMap<u64, Arc<AnalyticPathCpuData>>,
    image_resources: &mut HashSet<ImageBindGroupKey>,
) -> bool {
    let mut uses_text_atlas = false;
    for draw in &draw_ops.draw_ops {
        match draw.kind {
            DrawOpKind::Solid => {}
            DrawOpKind::Image {
                handle,
                sampling,
                raster_size,
            } => {
                image_resources.insert(ImageBindGroupKey {
                    handle,
                    sampling,
                    raster_size,
                });
            }
            DrawOpKind::TextAtlas => {
                uses_text_atlas = true;
            }
            DrawOpKind::AnalyticPath { id } => {
                let path = &draw_ops.analytic_paths[&id];
                analytic_paths
                    .entry(path.resource_signature)
                    .or_insert_with(|| path.clone());
            }
            DrawOpKind::WidgetShader => {}
            DrawOpKind::RoundedRect => {}
            DrawOpKind::GradientRect => {}
        }
    }
    uses_text_atlas
}

pub(crate) fn prepared_batch_counts(passes: &[PreparedPassBatch]) -> (usize, usize) {
    (
        passes.len(),
        passes
            .iter()
            .map(|pass| pass.clip_paths.len() + pass.draws.len())
            .sum(),
    )
}

pub(crate) fn stamp_analytic_path_slots(
    vertices: &mut [Vertex],
    passes: &[PreparedPassBatch],
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
) {
    let Some(resources) = analytic_path_resources else {
        return;
    };

    for pass in passes {
        for draw in &pass.draws {
            let PreparedDrawKind::AnalyticPath { resource_signature } = draw.kind else {
                continue;
            };
            let Some(slot) = resources.slots.get(&resource_signature).copied() else {
                continue;
            };
            let start = draw.vertices.start as usize;
            let end = start + draw.vertices.len as usize;
            for vertex in &mut vertices[start..end] {
                vertex.shader_params[0] = slot as f32;
            }
        }
    }
}

pub(crate) fn create_static_vertex_buffer(
    device: &wgpu::Device,
    label: &str,
    vertices: &[Vertex],
) -> Option<wgpu::Buffer> {
    if vertices.is_empty() {
        return None;
    }

    Some(
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        }),
    )
}

pub(crate) fn create_static_text_instance_buffer(
    device: &wgpu::Device,
    label: &str,
    instances: &[TextAtlasInstance],
) -> Option<wgpu::Buffer> {
    if instances.is_empty() {
        return None;
    }

    Some(
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        }),
    )
}

pub(crate) fn flatten_fragment_passes(
    fragments: &[PreparedFragmentSubmission],
) -> Vec<EncodablePassBatch> {
    let mut flattened = Vec::new();
    for fragment in fragments {
        for pass in &fragment.passes {
            flattened.push(EncodablePassBatch {
                pass: pass.clone(),
                scene_buffer: fragment.scene_buffer.clone(),
                clip_buffer: fragment.clip_buffer.clone(),
                text_instance_buffer: fragment.text_instance_buffer.clone(),
                translation: fragment.translation,
            });
        }
    }
    flattened
}

pub(crate) fn encode_fragment_passes(
    shared: &mut SharedRenderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    passes: &[EncodablePassBatch],
    stencil_view: Option<&wgpu::TextureView>,
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
) -> Result<usize> {
    let mut cleared = false;
    let mut index = 0;
    let mut render_pass_count = 0;

    while index < passes.len() {
        if passes[index].pass.clip_paths.is_empty() {
            let start = index;
            while index < passes.len() && passes[index].pass.clip_paths.is_empty() {
                index += 1;
            }
            encode_unclipped_pass_run(
                shared,
                encoder,
                view,
                target_format,
                viewport,
                framebuffer_size,
                &passes[start..index],
                image_bind_groups,
                text_atlas_bind_group,
                analytic_path_resources,
                &mut cleared,
            )?;
            render_pass_count += 1;
        } else {
            encode_clipped_pass(
                shared,
                encoder,
                view,
                target_format,
                viewport,
                framebuffer_size,
                &passes[index],
                stencil_view,
                image_bind_groups,
                text_atlas_bind_group,
                analytic_path_resources,
                &mut cleared,
            )?;
            render_pass_count += 1;
            index += 1;
        }
    }

    Ok(render_pass_count)
}

fn encode_unclipped_pass_run(
    shared: &mut SharedRenderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    passes: &[EncodablePassBatch],
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
    cleared: &mut bool,
) -> Result<()> {
    let load_op = next_pass_load_op(cleared);
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SUI scene unclipped batch pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: load_op,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });
    let mut current_kind = None;
    for batch in passes {
        encode_draws_for_pass(
            &mut render_pass,
            shared,
            target_format,
            viewport,
            framebuffer_size,
            &batch.pass,
            batch.scene_buffer.as_ref(),
            batch.text_instance_buffer.as_ref(),
            batch.translation,
            false,
            image_bind_groups,
            text_atlas_bind_group,
            analytic_path_resources,
            &mut current_kind,
        )?;
    }

    Ok(())
}

fn encode_clipped_pass(
    shared: &mut SharedRenderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    batch: &EncodablePassBatch,
    stencil_view: Option<&wgpu::TextureView>,
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
    cleared: &mut bool,
) -> Result<()> {
    let load_op = next_pass_load_op(cleared);
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SUI scene clipped batch pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: load_op,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: stencil_view.expect("stencil view available for path-clipped pass"),
            depth_ops: None,
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0),
                store: wgpu::StoreOp::Store,
            }),
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });

    let clip_pipeline = shared.clip_pipeline(target_format);
    render_pass.set_pipeline(clip_pipeline);
    render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1);
    let (viewport_x, viewport_y) =
        translation_to_viewport_origin(batch.translation, viewport, framebuffer_size);
    render_pass.set_viewport(
        viewport_x,
        viewport_y,
        framebuffer_size.0 as f32,
        framebuffer_size.1 as f32,
        0.0,
        1.0,
    );
    let clip_buffer = batch
        .clip_buffer
        .as_ref()
        .expect("clip buffer available for path-clipped pass");
    for (clip_index, clip_path) in batch.pass.clip_paths.iter().enumerate() {
        render_pass.set_stencil_reference(clip_index as u32);
        render_pass.set_vertex_buffer(0, vertex_buffer_slice(clip_buffer, clip_path.vertices));
        render_pass.draw(0..clip_path.vertices.len, 0..1);
    }

    let mut current_kind = None;
    encode_draws_for_pass(
        &mut render_pass,
        shared,
        target_format,
        viewport,
        framebuffer_size,
        &batch.pass,
        batch.scene_buffer.as_ref(),
        batch.text_instance_buffer.as_ref(),
        batch.translation,
        true,
        image_bind_groups,
        text_atlas_bind_group,
        analytic_path_resources,
        &mut current_kind,
    )?;

    Ok(())
}

fn next_pass_load_op(cleared: &mut bool) -> wgpu::LoadOp<wgpu::Color> {
    if *cleared {
        wgpu::LoadOp::Load
    } else {
        *cleared = true;
        wgpu::LoadOp::Clear(wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        })
    }
}

fn encode_draws_for_pass(
    render_pass: &mut wgpu::RenderPass<'_>,
    shared: &mut SharedRenderer,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    pass: &PreparedPassBatch,
    scene_buffer: Option<&wgpu::Buffer>,
    text_instance_buffer: Option<&wgpu::Buffer>,
    translation: Vector,
    clipped: bool,
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
    current_kind: &mut Option<PreparedDrawPipelineKind>,
) -> Result<()> {
    let (viewport_x, viewport_y) =
        translation_to_viewport_origin(translation, viewport, framebuffer_size);
    render_pass.set_viewport(
        viewport_x,
        viewport_y,
        framebuffer_size.0 as f32,
        framebuffer_size.1 as f32,
        0.0,
        1.0,
    );

    for draw in &pass.draws {
        match draw.clip_rect {
            Some(scissor) => {
                render_pass.set_scissor_rect(scissor.x, scissor.y, scissor.width, scissor.height)
            }
            None => render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1),
        }

        let pipeline_kind = draw.kind.pipeline_kind();
        if *current_kind != Some(pipeline_kind) {
            let pipeline = match (pipeline_kind, clipped) {
                (PreparedDrawPipelineKind::Solid, true) => shared.clipped_pipeline(target_format),
                (PreparedDrawPipelineKind::Solid, false) => shared.pipeline(target_format),
                (PreparedDrawPipelineKind::Image, true) => {
                    shared.clipped_image_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::Image, false) => shared.image_pipeline(target_format),
                (PreparedDrawPipelineKind::TextAtlas, true) => {
                    shared.clipped_text_atlas_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::TextAtlas, false) => {
                    shared.text_atlas_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::AnalyticPath, true) => {
                    shared.clipped_analytic_path_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::AnalyticPath, false) => {
                    shared.analytic_path_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::WidgetShader, true) => {
                    shared.clipped_widget_shader_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::WidgetShader, false) => {
                    shared.widget_shader_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::RoundedRect, true) => {
                    shared.clipped_rounded_rect_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::RoundedRect, false) => {
                    shared.rounded_rect_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::GradientRect, true) => {
                    shared.clipped_gradient_rect_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::GradientRect, false) => {
                    shared.gradient_rect_pipeline(target_format)
                }
            };
            render_pass.set_pipeline(pipeline);
            if pipeline_kind == PreparedDrawPipelineKind::AnalyticPath {
                let bind_group = &analytic_path_resources
                    .expect("analytic path resources prepared before retained render pass")
                    .bind_group;
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            *current_kind = Some(pipeline_kind);
        }

        if clipped {
            render_pass.set_stencil_reference(pass.clip_paths.len() as u32);
        }

        match draw.kind {
            PreparedDrawKind::Solid => {}
            PreparedDrawKind::Image {
                handle,
                sampling,
                raster_size,
            } => {
                let bind_group = image_bind_groups
                    .get(&ImageBindGroupKey {
                        handle,
                        sampling,
                        raster_size,
                    })
                    .expect("image bind group prepared before retained render pass");
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            PreparedDrawKind::TextAtlas => {
                let bind_group = text_atlas_bind_group
                    .expect("text atlas bind group prepared before retained render pass");
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            PreparedDrawKind::AnalyticPath { .. } => {}
            PreparedDrawKind::WidgetShader => {}
            PreparedDrawKind::RoundedRect => {}
            PreparedDrawKind::GradientRect => {}
        }

        let (vertex_range, instances) = match draw.kind {
            PreparedDrawKind::TextAtlas => {
                let text_instance_buffer = text_instance_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a text instance buffer")
                })?;
                render_pass.set_vertex_buffer(0, shared.text_quad_buffer.slice(..));
                render_pass.set_vertex_buffer(
                    1,
                    text_instance_buffer_slice(text_instance_buffer, draw.vertices),
                );
                (0..6, 0..draw.vertices.len)
            }
            PreparedDrawKind::AnalyticPath { .. } => {
                let scene_buffer = scene_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a scene vertex buffer")
                })?;
                render_pass.set_vertex_buffer(0, vertex_buffer_slice(scene_buffer, draw.vertices));
                (0..draw.vertices.len, 0..1)
            }
            PreparedDrawKind::WidgetShader => {
                let scene_buffer = scene_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a scene vertex buffer")
                })?;
                render_pass.set_vertex_buffer(0, vertex_buffer_slice(scene_buffer, draw.vertices));
                (0..draw.vertices.len, 0..1)
            }
            _ => {
                let scene_buffer = scene_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a scene vertex buffer")
                })?;
                render_pass.set_vertex_buffer(0, vertex_buffer_slice(scene_buffer, draw.vertices));
                (0..draw.vertices.len, 0..1)
            }
        };
        render_pass.draw(vertex_range, instances);
    }

    Ok(())
}

fn translation_to_viewport_origin(
    translation: Vector,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> (f32, f32) {
    if translation == Vector::ZERO || viewport.is_empty() {
        return (0.0, 0.0);
    }

    let scale_x = framebuffer_size.0 as f32 / viewport.width.max(1.0);
    let scale_y = framebuffer_size.1 as f32 / viewport.height.max(1.0);
    (translation.x * scale_x, translation.y * scale_y)
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DirectPacketBuildDiagnostics {
    pub(crate) command_count: usize,
    pub(crate) text_command_count: usize,
    pub(crate) path_command_count: usize,
    pub(crate) clip_path_command_count: usize,
    pub(crate) image_command_count: usize,
    pub(crate) rect_command_count: usize,
    pub(crate) raster_state_init_time_ms: f64,
    pub(crate) scene_build_time_ms: f64,
    pub(crate) text_command_time_ms: f64,
    pub(crate) path_command_time_ms: f64,
    pub(crate) clip_path_command_time_ms: f64,
    pub(crate) image_command_time_ms: f64,
    pub(crate) rect_command_time_ms: f64,
}

pub(crate) fn build_direct_packet_with_diagnostics(
    frame: &SceneFrame,
    scene: &Scene,
    initial_state: &ResolvedRasterState,
    text_engine: &mut TextEngine,
    path_cache: &mut PathMeshCache,
    feather_width: f32,
) -> Result<(DrawOpArena, DirectPacketBuildDiagnostics)> {
    let mut diagnostics = DirectPacketBuildDiagnostics::default();
    let mut draw_ops = DrawOpArena::default();
    let state_init_started = Instant::now();
    let mut state = SceneRasterState::from_resolved(initial_state, &mut draw_ops, frame.viewport)?;
    diagnostics.raster_state_init_time_ms = state_init_started.elapsed().as_secs_f64() * 1000.0;
    let mut builder = SceneDrawOpBuilder {
        frame,
        text_engine,
        path_cache,
        feather_width,
        scratch_vertices: Vec::new(),
        scratch_text_instances: Vec::new(),
        overlay_scratch_vertices: Vec::new(),
        clip_scratch_vertices: Vec::new(),
    };
    let scene_build_started = Instant::now();
    builder.build_scene(scene, &mut draw_ops, &mut state, &mut diagnostics)?;
    diagnostics.scene_build_time_ms = scene_build_started.elapsed().as_secs_f64() * 1000.0;
    Ok((draw_ops, diagnostics))
}

struct SceneDrawOpBuilder<'a> {
    frame: &'a SceneFrame,
    text_engine: &'a mut TextEngine,
    path_cache: &'a mut PathMeshCache,
    feather_width: f32,
    scratch_vertices: Vec<Vertex>,
    scratch_text_instances: Vec<TextAtlasInstance>,
    overlay_scratch_vertices: Vec<Vertex>,
    clip_scratch_vertices: Vec<Vertex>,
}

enum FillPathRenderMode {
    SolidOnly,
    SolidPlusAnalytic { id: u64 },
}

const ANALYTIC_AA_OUTSET: f32 = 1.0;

fn analytic_coverage_outset(soft_width: f32) -> f32 {
    (soft_width.max(0.0) * 0.5).max(ANALYTIC_AA_OUTSET)
}

impl SceneDrawOpBuilder<'_> {
    fn build_scene(
        &mut self,
        scene: &Scene,
        draw_ops: &mut DrawOpArena,
        state: &mut SceneRasterState,
        diagnostics: &mut DirectPacketBuildDiagnostics,
    ) -> Result<()> {
        for command in scene.commands() {
            self.build_command(command, draw_ops, state, diagnostics)?;
        }

        Ok(())
    }

    fn build_command(
        &mut self,
        command: &SceneCommand,
        draw_ops: &mut DrawOpArena,
        state: &mut SceneRasterState,
        diagnostics: &mut DirectPacketBuildDiagnostics,
    ) -> Result<()> {
        let viewport = self.frame.viewport;
        diagnostics.command_count += 1;
        let command_started = Instant::now();

        let result = match command {
            SceneCommand::Clear(color) => {
                self.scratch_vertices.clear();
                append_rect(
                    &mut self.scratch_vertices,
                    Rect::new(0.0, 0.0, viewport.width, viewport.height),
                    *color,
                    viewport,
                );
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::FillRect { rect, brush } => {
                self.scratch_vertices.clear();
                match brush {
                    Brush::Solid(color) => {
                        append_rounded_rect_fill(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            [0.0; 4],
                            *color,
                            None,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::RoundedRect,
                            &self.scratch_vertices,
                            state,
                        );
                    }
                    Brush::LinearGradient { start, end, stops } => {
                        let stop0 = stops.first().map(|s| s.color).unwrap_or(Color::TRANSPARENT);
                        let stop1 = stops.last().map(|s| s.color).unwrap_or(stop0);
                        append_gradient_rect(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            [0.0; 4],
                            *start,
                            *end,
                            stop0,
                            stop1,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::GradientRect,
                            &self.scratch_vertices,
                            state,
                        );
                    }
                }
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::StrokeRect {
                rect,
                brush,
                stroke,
            } => {
                let color = brush_fallback_color(brush);
                self.scratch_vertices.clear();
                append_rounded_rect_fill(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    [0.0; 4],
                    Color::TRANSPARENT,
                    Some(sui_scene::Border {
                        width: stroke.width,
                        color,
                    }),
                    viewport,
                    self.feather_width,
                );
                push_draw_op(
                    draw_ops,
                    DrawOpKind::RoundedRect,
                    &self.scratch_vertices,
                    state,
                );
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::FillPath { path, brush } => {
                let color = brush_fallback_color(brush);
                self.scratch_vertices.clear();
                self.overlay_scratch_vertices.clear();
                let render_mode = append_painted_path(
                    &mut self.scratch_vertices,
                    &mut self.overlay_scratch_vertices,
                    draw_ops,
                    state,
                    path,
                    color,
                    self.path_cache,
                    viewport,
                    self.feather_width,
                )?;
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                if let FillPathRenderMode::SolidPlusAnalytic { id } = render_mode {
                    push_draw_op(
                        draw_ops,
                        DrawOpKind::AnalyticPath { id },
                        &self.overlay_scratch_vertices,
                        state,
                    );
                }
                diagnostics.path_command_count += 1;
                Ok(())
            }
            SceneCommand::StrokePath {
                path,
                brush,
                stroke,
            } => {
                let color = brush_fallback_color(brush);
                self.scratch_vertices.clear();
                self.overlay_scratch_vertices.clear();
                let analytic_id = append_stroked_path(
                    &mut self.scratch_vertices,
                    &mut self.overlay_scratch_vertices,
                    draw_ops,
                    state,
                    path,
                    color,
                    *stroke,
                    self.path_cache,
                    viewport,
                    self.feather_width,
                )?;
                if !self.scratch_vertices.is_empty() {
                    push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                }
                if let Some(id) = analytic_id {
                    push_draw_op(
                        draw_ops,
                        DrawOpKind::AnalyticPath { id },
                        &self.overlay_scratch_vertices,
                        state,
                    );
                }
                diagnostics.path_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawText(text) => {
                self.scratch_text_instances.clear();
                self.text_engine.append_text_run(
                    &mut self.scratch_text_instances,
                    state,
                    text,
                    self.frame.font_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawShapedText(text) => {
                self.scratch_text_instances.clear();
                self.text_engine.append_shaped_text(
                    &mut self.scratch_text_instances,
                    state,
                    text,
                    self.frame.text_layout_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawShapedTextWindow(text) => {
                self.scratch_text_instances.clear();
                self.text_engine.append_shaped_text_window(
                    &mut self.scratch_text_instances,
                    state,
                    text,
                    self.frame.text_layout_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawImage { rect, source } => {
                self.scratch_vertices.clear();
                let image_size = self
                    .frame
                    .image_registry
                    .dimensions(source.image)
                    .ok_or_else(|| {
                        Error::new(format!(
                            "image handle {} is not registered",
                            source.image.get()
                        ))
                    })?;
                append_image(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    source,
                    image_size,
                    viewport,
                );
                let (bounds, raster_size) = image_rect_raster_metadata(
                    state.current_transform,
                    *rect,
                    source.source_rect,
                    image_size,
                    viewport,
                    self.frame.surface_size,
                );
                push_image_draw_op(
                    draw_ops,
                    DrawOpKind::Image {
                        handle: source.image,
                        sampling: source.sampling,
                        raster_size,
                    },
                    &self.scratch_vertices,
                    state,
                    ImageDrawMetadata {
                        bounds,
                        pixel_snap: if state.current_transform.xy.abs() < 0.0001
                            && state.current_transform.yx.abs() < 0.0001
                            && state.current_transform.xx >= 0.0
                            && state.current_transform.yy >= 0.0
                        {
                            source.pixel_snap
                        } else {
                            sui_scene::ImagePixelSnap::None
                        },
                    },
                );
                diagnostics.image_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawImageQuad { points, source } => {
                self.scratch_vertices.clear();
                let image_size = self
                    .frame
                    .image_registry
                    .dimensions(source.image)
                    .ok_or_else(|| {
                        Error::new(format!(
                            "image handle {} is not registered",
                            source.image.get()
                        ))
                    })?;
                append_image_quad(
                    &mut self.scratch_vertices,
                    state,
                    *points,
                    source,
                    image_size,
                    viewport,
                );
                let (bounds, raster_size) = image_quad_raster_metadata(
                    state.current_transform,
                    *points,
                    source.source_rect,
                    image_size,
                    viewport,
                    self.frame.surface_size,
                );
                push_image_draw_op(
                    draw_ops,
                    DrawOpKind::Image {
                        handle: source.image,
                        sampling: source.sampling,
                        raster_size,
                    },
                    &self.scratch_vertices,
                    state,
                    ImageDrawMetadata {
                        bounds,
                        pixel_snap: sui_scene::ImagePixelSnap::None,
                    },
                );
                diagnostics.image_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawShaderRect { rect, shader } => {
                self.scratch_vertices.clear();
                append_widget_shader_rect(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    *shader,
                    viewport,
                );
                push_draw_op(
                    draw_ops,
                    DrawOpKind::WidgetShader,
                    &self.scratch_vertices,
                    state,
                );
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::PushClip { rect } => {
                state.push_clip(*rect);
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::PushClipPath { path } => {
                state.push_clip_path(path, viewport, draw_ops, &mut self.clip_scratch_vertices)?;
                diagnostics.clip_path_command_count += 1;
                Ok(())
            }
            SceneCommand::PopClip => {
                state.pop_clip(draw_ops);
                Ok(())
            }
            SceneCommand::PushTransform { transform } => {
                state.push_transform(*transform);
                Ok(())
            }
            SceneCommand::PopTransform => {
                state.pop_transform();
                Ok(())
            }
            SceneCommand::PushTextRenderPolicy { policy } => {
                state.push_text_render_policy(*policy);
                Ok(())
            }
            SceneCommand::PopTextRenderPolicy => {
                state.pop_text_render_policy();
                Ok(())
            }
            SceneCommand::Layer(layer) => Err(Error::new(format!(
                "retained direct packet compiler encountered nested layer {}",
                layer.layer_id().get()
            ))),
            SceneCommand::FillRoundedRect {
                rect,
                radii,
                brush,
                border,
                shadow,
            } => {
                // Submission order is z-order: paint the soft shadow first so the fill
                // (and its border) draw on top of it.
                if let Some(shadow) = shadow {
                    self.scratch_vertices.clear();
                    append_rounded_rect_shadow(
                        &mut self.scratch_vertices,
                        state,
                        *rect,
                        *radii,
                        *shadow,
                        viewport,
                        self.feather_width,
                    );
                    push_draw_op(
                        draw_ops,
                        DrawOpKind::RoundedRect,
                        &self.scratch_vertices,
                        state,
                    );
                }

                self.scratch_vertices.clear();
                match brush {
                    Brush::Solid(color) => {
                        append_rounded_rect_fill(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            *radii,
                            *color,
                            *border,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::RoundedRect,
                            &self.scratch_vertices,
                            state,
                        );
                    }
                    Brush::LinearGradient { start, end, stops } => {
                        let stop0 = stops.first().map(|s| s.color).unwrap_or(Color::TRANSPARENT);
                        let stop1 = stops.last().map(|s| s.color).unwrap_or(stop0);
                        append_gradient_rect(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            *radii,
                            *start,
                            *end,
                            stop0,
                            stop1,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::GradientRect,
                            &self.scratch_vertices,
                            state,
                        );
                        // A gradient fill ignores any border here (documented limitation);
                        // borders are only honored for solid rounded-rect fills.
                        let _ = border;
                    }
                }
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::Label { rect, text, color } => {
                self.scratch_text_instances.clear();
                self.text_engine.append_text_run(
                    &mut self.scratch_text_instances,
                    state,
                    &TextRun {
                        rect: *rect,
                        text: text.clone(),
                        style: TextStyle::new(*color),
                    },
                    self.frame.font_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
        };

        let elapsed_ms = command_started.elapsed().as_secs_f64() * 1000.0;
        match command {
            SceneCommand::Clear(_)
            | SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::DrawShaderRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::PushClip { .. } => {
                diagnostics.rect_command_time_ms += elapsed_ms;
            }
            SceneCommand::FillPath { .. } | SceneCommand::StrokePath { .. } => {
                diagnostics.path_command_time_ms += elapsed_ms;
            }
            SceneCommand::DrawText(_)
            | SceneCommand::DrawShapedText(_)
            | SceneCommand::DrawShapedTextWindow(_)
            | SceneCommand::Label { .. } => {
                diagnostics.text_command_time_ms += elapsed_ms;
            }
            SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                diagnostics.image_command_time_ms += elapsed_ms;
            }
            SceneCommand::PushClipPath { .. } => {
                diagnostics.clip_path_command_time_ms += elapsed_ms;
            }
            SceneCommand::PopClip
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PopTransform
            | SceneCommand::PushTextRenderPolicy { .. }
            | SceneCommand::PopTextRenderPolicy
            | SceneCommand::Layer(_) => {}
        }

        result
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SceneRasterState {
    pub(crate) current_transform: Transform,
    pub(crate) pixel_snap_offset: Vector,
    pub(crate) transform_stack: Vec<Transform>,
    clip_stack: Vec<ClipPrimitive>,
    text_render_policy: Option<TextRenderPolicy>,
    text_render_policy_stack: Vec<Option<TextRenderPolicy>>,
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
enum ClipPrimitive {
    Rect(Rect),
    Path { bounds: Rect },
}

impl ClipPrimitive {
    fn bounds(&self) -> Rect {
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

pub(crate) struct TextEngine {
    pub(crate) system: TextSystem,
    pub(crate) glyph_cache: HashMap<GlyphCacheKey, CachedGlyphAtlas>,
    pub(crate) atlas: TextAtlasPages,
    swash_scale_context: SwashScaleContext,
    pub(crate) text_render_mode: TextRenderMode,
    pub(crate) text_subpixel_order: TextSubpixelOrder,
    pub(crate) text_hinting: TextHinting,
    pub(crate) stem_darkening: StemDarkening,
    pub(crate) coverage_policy: TextCoveragePolicy,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) glyph_cache_hits: usize,
    pub(crate) glyph_cache_misses: usize,
    /// Monotonic per-frame counter used to stamp atlas pages for LRU eviction.
    frame_counter: u64,
    #[cfg(test)]
    swash_face_parse_count: usize,
    pub(crate) frame_stats: TextFrameStats,
}

#[derive(Clone, Copy)]
struct SwashFaceState<'a> {
    font_ref: SwashFontRef<'a>,
    font_id: [u64; 2],
    units_per_em: f32,
}

impl<'a> SwashFaceState<'a> {
    fn new(face: &'a ResolvedTextFace, face_key: GlyphFaceCacheKey) -> Result<Self> {
        let face_index = usize::try_from(face.face_index())
            .map_err(|_| Error::new("text face index does not fit into usize"))?;
        let font_ref = SwashFontRef::from_index(face.bytes(), face_index)
            .ok_or_else(|| Error::new("failed to parse shaped text face data for swash"))?;
        let units_per_em = f32::from(font_ref.metrics(&[]).units_per_em.max(1));
        Ok(Self {
            font_ref,
            font_id: swash_font_id(face_key),
            units_per_em,
        })
    }

    fn ppem_for_scale(self, glyph_scale: f32) -> f32 {
        (glyph_scale * self.units_per_em).max(f32::EPSILON)
    }
}

fn swash_font_id(face_key: GlyphFaceCacheKey) -> [u64; 2] {
    let mut hasher = DefaultHasher::new();
    face_key.hash(&mut hasher);
    let primary = hasher.finish();
    let secondary = (face_key.data_ptr as u64).rotate_left(17)
        ^ (face_key.data_len as u64).rotate_left(7)
        ^ u64::from(face_key.face_index).rotate_left(31);
    [primary, secondary]
}

impl Default for TextEngine {
    fn default() -> Self {
        Self {
            system: TextSystem::new(),
            glyph_cache: HashMap::new(),
            atlas: TextAtlasPages::new(TEXT_ATLAS_WIDTH, TEXT_ATLAS_HEIGHT, TEXT_ATLAS_MAX_PAGES),
            swash_scale_context: SwashScaleContext::new(),
            text_render_mode: TextRenderMode::default(),
            text_subpixel_order: TextSubpixelOrder::default(),
            text_hinting: TextHinting::default(),
            stem_darkening: StemDarkening::default(),
            coverage_policy: TextCoveragePolicy::default(),
            diagnostics_enabled: true,
            glyph_cache_hits: 0,
            glyph_cache_misses: 0,
            frame_counter: 0,
            #[cfg(test)]
            swash_face_parse_count: 0,
            frame_stats: TextFrameStats::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedTextRenderPolicy {
    render_mode: TextRenderMode,
    subpixel_order: TextSubpixelOrder,
    hinting: TextHinting,
    stem_darkening: StemDarkening,
    coverage_policy: TextCoveragePolicy,
}

fn map_scene_text_render_mode(mode: sui_scene::TextRenderMode) -> TextRenderMode {
    match mode {
        sui_scene::TextRenderMode::Grayscale => TextRenderMode::Grayscale,
        sui_scene::TextRenderMode::LcdSubpixel => TextRenderMode::LcdSubpixel,
    }
}

fn map_scene_text_subpixel_order(order: sui_scene::TextSubpixelOrder) -> TextSubpixelOrder {
    match order {
        sui_scene::TextSubpixelOrder::None => TextSubpixelOrder::None,
        sui_scene::TextSubpixelOrder::Rgb => TextSubpixelOrder::Rgb,
        sui_scene::TextSubpixelOrder::Bgr => TextSubpixelOrder::Bgr,
    }
}

fn map_text_render_hinting(hinting: TextRenderHinting) -> TextHinting {
    match hinting.normalized() {
        TextRenderHinting::None => TextHinting::None,
        TextRenderHinting::Slight { max_ppem } => TextHinting::Slight { max_ppem },
    }
}

fn map_text_render_stem_darkening(darkening: TextRenderStemDarkening) -> StemDarkening {
    match darkening.normalized() {
        TextRenderStemDarkening::None => StemDarkening::None,
        TextRenderStemDarkening::Enabled { max_ppem, amount } => {
            StemDarkening::Enabled { max_ppem, amount }
        }
    }
}

fn map_text_render_coverage_policy(policy: TextRenderCoveragePolicy) -> TextCoveragePolicy {
    match policy.normalized() {
        TextRenderCoveragePolicy::Perceptual => TextCoveragePolicy::Perceptual,
        TextRenderCoveragePolicy::Linear => TextCoveragePolicy::Linear,
        TextRenderCoveragePolicy::Gamma(gamma) => TextCoveragePolicy::Gamma(gamma),
        TextRenderCoveragePolicy::CoverageBoost(amount) => {
            TextCoveragePolicy::CoverageBoost(amount)
        }
        TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq => {
            TextCoveragePolicy::TwoCoverageMinusCoverageSq
        }
    }
}

impl TextEngine {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self::default())
    }

    pub(crate) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
        if !enabled {
            self.frame_stats = TextFrameStats::default();
        }
    }

    pub(crate) fn set_text_render_mode(&mut self, mode: TextRenderMode) {
        self.text_render_mode = mode;
    }

    pub(crate) fn set_text_subpixel_order(&mut self, order: TextSubpixelOrder) {
        self.text_subpixel_order = order;
    }

    pub(crate) fn set_text_hinting(&mut self, hinting: TextHinting) {
        self.text_hinting = hinting.normalized();
    }

    pub(crate) fn set_stem_darkening(&mut self, darkening: StemDarkening) {
        self.stem_darkening = darkening.normalized();
    }

    pub(crate) fn set_text_coverage_policy(&mut self, policy: TextCoveragePolicy) {
        self.coverage_policy = policy.normalized();
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_stats = TextFrameStats::default();
        // New frame: advance the clock used to age atlas pages for LRU eviction.
        self.frame_counter = self.frame_counter.wrapping_add(1);
    }

    pub(crate) fn frame_stats(&self) -> TextFrameStats {
        self.frame_stats
    }

    fn resolved_text_render_policy(
        &self,
        override_policy: Option<TextRenderPolicy>,
    ) -> ResolvedTextRenderPolicy {
        let Some(policy) = override_policy.map(TextRenderPolicy::normalized) else {
            return ResolvedTextRenderPolicy {
                render_mode: self.text_render_mode,
                subpixel_order: self.text_subpixel_order,
                hinting: self.text_hinting,
                stem_darkening: self.stem_darkening,
                coverage_policy: self.coverage_policy,
            };
        };

        ResolvedTextRenderPolicy {
            render_mode: policy
                .render_mode
                .map(map_scene_text_render_mode)
                .unwrap_or(self.text_render_mode),
            subpixel_order: policy
                .subpixel_order
                .map(map_scene_text_subpixel_order)
                .unwrap_or(self.text_subpixel_order),
            hinting: policy
                .hinting
                .map(map_text_render_hinting)
                .unwrap_or(self.text_hinting),
            stem_darkening: policy
                .stem_darkening
                .map(map_text_render_stem_darkening)
                .unwrap_or(self.stem_darkening),
            coverage_policy: policy
                .coverage_policy
                .map(map_text_render_coverage_policy)
                .unwrap_or(self.coverage_policy),
        }
    }

    pub(crate) fn append_text_run(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        text: &TextRun,
        font_registry: &FontRegistry,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if text.rect.is_empty() || text.text.is_empty() || viewport.is_empty() {
            return Ok(());
        }

        let layout = self.shape_text_run(text, font_registry)?;
        self.append_text_layout(
            atlas_instances,
            state,
            Point::new(text.rect.x(), text.rect.y()),
            &layout,
            None,
            viewport,
            raster_scale_factor,
        )
    }

    pub(crate) fn append_shaped_text(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        text: &ShapedText,
        text_layout_registry: &sui_text::TextLayoutRegistry,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if viewport.is_empty() {
            return Ok(());
        }

        let layout = text.resolve(text_layout_registry).ok_or_else(|| {
            Error::new(format!(
                "text layout handle {} version {} is not available in the frame registry",
                text.layout_handle.get(),
                text.layout_version.get(),
            ))
        })?;

        self.append_text_layout(
            atlas_instances,
            state,
            text.origin,
            layout,
            text.color_override,
            viewport,
            raster_scale_factor,
        )
    }

    pub(crate) fn append_shaped_text_window(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        text: &sui_text::ShapedTextWindow,
        text_layout_registry: &sui_text::TextLayoutRegistry,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if viewport.is_empty() {
            return Ok(());
        }

        let layout = text.resolve(text_layout_registry).ok_or_else(|| {
            Error::new(format!(
                "text layout handle {} version {} is not available in the frame registry",
                text.layout_handle.get(),
                text.layout_version.get(),
            ))
        })?;

        self.append_text_layout_window(
            atlas_instances,
            state,
            text.origin,
            layout,
            text.line_range.clone(),
            text.color_override,
            viewport,
            raster_scale_factor,
        )
    }

    fn append_text_layout(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        origin: Point,
        layout: &TextLayout,
        color_override: Option<Color>,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        if layout.measurement().width <= 0.0 || layout.measurement().height <= 0.0 {
            return Ok(());
        }

        let translated_bounds = layout.measurement().bounds.translate(origin.to_vector());
        if state.visible_rect(translated_bounds).is_none() {
            return Ok(());
        }

        self.append_layout_glyphs(
            atlas_instances,
            state,
            origin,
            layout.glyph_instances(),
            color_override,
            viewport,
            raster_scale_factor,
        )
    }

    fn append_text_layout_window(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        origin: Point,
        layout: &TextLayout,
        line_range: std::ops::Range<usize>,
        color_override: Option<Color>,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()> {
        let line_window = layout.line_window(line_range);
        if line_window.line_range.is_empty() {
            return Ok(());
        }

        let translated_bounds = line_window.bounds().translate(origin.to_vector());
        if translated_bounds.width() <= 0.0 || translated_bounds.height() <= 0.0 {
            return Ok(());
        }

        if state.visible_rect(translated_bounds).is_none() {
            return Ok(());
        }

        self.append_layout_glyphs(
            atlas_instances,
            state,
            origin,
            line_window.glyph_instances(),
            color_override,
            viewport,
            raster_scale_factor,
        )
    }

    fn append_layout_glyphs<'a, I>(
        &mut self,
        atlas_instances: &mut Vec<TextAtlasInstance>,
        state: &SceneRasterState,
        origin: Point,
        glyphs: I,
        color_override: Option<Color>,
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()>
    where
        I: IntoIterator<Item = sui_text::TextGlyphInstance<'a>>,
    {
        let mut active_face_index = None;
        let mut swash_face = None;
        let text_policy = self.resolved_text_render_policy(state.active_text_render_policy());
        for glyph in glyphs {
            let face_index = glyph.glyph.face_index;
            if active_face_index != Some(face_index) {
                active_face_index = Some(face_index);
                swash_face = None;
            }

            let glyph_face = glyph.face;
            let face_key = GlyphFaceCacheKey::new(glyph_face);
            let glyph_style = glyph.style;
            let glyph_color = color_override.unwrap_or(glyph_style.color);
            let coverage_policy = text_policy
                .coverage_policy
                .resolved_for_text_color(glyph_color);
            let render_mode = if matches!(text_policy.render_mode, TextRenderMode::LcdSubpixel)
                && (matches!(text_policy.subpixel_order, TextSubpixelOrder::None)
                    || !allows_lcd_text(state.current_transform))
            {
                TextRenderMode::Grayscale
            } else {
                text_policy.render_mode
            };
            let subpixel_order = match render_mode {
                TextRenderMode::Grayscale => TextSubpixelOrder::None,
                TextRenderMode::LcdSubpixel => text_policy.subpixel_order,
            };
            let mut translated_glyph = glyph.glyph.clone();
            translated_glyph.origin_x += origin.x;
            translated_glyph.origin_y += origin.y;
            if let Some(bounds) = translated_glyph.bounds {
                translated_glyph.bounds = Some(bounds.translate(origin.to_vector()));
            }

            if let Some(atlas) = self.cached_glyph_primitive(
                glyph_face,
                &mut swash_face,
                face_key,
                glyph.glyph.glyph_id,
                glyph.glyph.scale,
                raster_scale_factor,
                glyph_subpixel_offset(
                    state.current_transform,
                    state.pixel_snap_offset,
                    &translated_glyph,
                    raster_scale_factor,
                ),
                render_mode,
                subpixel_order,
                text_policy.hinting,
                text_policy.stem_darkening,
                glyph_style.weight.value(),
            )? && let Some(instance) = build_text_atlas_instance(
                atlas,
                &translated_glyph,
                glyph_color,
                coverage_policy,
                state.current_transform,
                state.pixel_snap_offset,
                viewport,
                raster_scale_factor,
            ) {
                atlas_instances.push(instance);
                if self.diagnostics_enabled {
                    self.frame_stats.glyph_instances += 1;
                    self.frame_stats.glyph_upload_bytes += TEXT_ATLAS_INSTANCE_SIZE;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn shape_text_run(
        &self,
        text: &TextRun,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.system.shape_text_run(text, font_registry)
    }

    fn cached_glyph_primitive<'face>(
        &mut self,
        face: &'face ResolvedTextFace,
        swash_face: &mut Option<SwashFaceState<'face>>,
        face_key: GlyphFaceCacheKey,
        glyph_id: u16,
        glyph_scale: f32,
        raster_scale_factor: f32,
        subpixel_offset: GlyphSubpixelOffsetKey,
        text_render_mode: TextRenderMode,
        text_subpixel_order: TextSubpixelOrder,
        text_hinting: TextHinting,
        stem_darkening: StemDarkening,
        weight: u16,
    ) -> Result<Option<&CachedGlyphAtlas>> {
        let atlas_physical_scale = glyph_scale * raster_scale_factor.max(1.0);
        let scale_bucket = glyph_scale_bucket(atlas_physical_scale);
        let key = GlyphCacheKey::new(
            face_key,
            glyph_id,
            scale_bucket,
            subpixel_offset,
            text_render_mode,
            text_subpixel_order,
            text_hinting,
            stem_darkening,
            weight,
        );
        // Hit: stamp the glyph's page as used this frame (for LRU) and return the cached entry.
        if self.glyph_cache.contains_key(&key) {
            if self.diagnostics_enabled {
                self.glyph_cache_hits += 1;
            }
            let page_index = self.glyph_cache[&key].page_index;
            self.atlas.touch_page(page_index, self.frame_counter);
            return Ok(self.glyph_cache.get(&key));
        }

        // Miss: rasterize and insert into the atlas (which may evict an LRU page).
        if self.diagnostics_enabled {
            self.glyph_cache_misses += 1;
        }
        if swash_face.is_none() {
            #[cfg(test)]
            {
                self.swash_face_parse_count += 1;
            }
            *swash_face = Some(SwashFaceState::new(face, face_key)?);
        }
        let swash_face = swash_face
            .as_ref()
            .expect("swash text face should be cached after initialization");
        let atlas_miss_started = self.diagnostics_enabled.then(Instant::now);
        let bucketed_physical_scale = glyph_scale_from_bucket(scale_bucket);
        let bucketed_logical_scale = bucketed_physical_scale / raster_scale_factor.max(1.0);
        let built = build_cached_glyph_atlas(
            &mut self.atlas,
            &mut self.swash_scale_context,
            swash_face,
            glyph_id,
            swash_face.ppem_for_scale(bucketed_physical_scale),
            raster_scale_factor.max(1.0),
            bucketed_logical_scale,
            subpixel_offset,
            text_render_mode,
            text_subpixel_order,
            text_hinting,
            stem_darkening,
            weight,
            self.frame_counter,
        )?;
        if let Some(started) = atlas_miss_started {
            self.frame_stats.atlas_miss_count += 1;
            self.frame_stats.atlas_miss_time_us += started.elapsed().as_micros() as u64;
        }
        let Some((primitive, evicted_page)) = built else {
            return Ok(None);
        };
        // If a page was recycled, drop every glyph that pointed into it: its atlas region -- and
        // therefore the UVs cached here -- are no longer valid.
        if let Some(evicted) = evicted_page {
            self.glyph_cache
                .retain(|_, cached| cached.page_index != evicted);
        }
        self.glyph_cache.insert(key.clone(), primitive);
        Ok(self.glyph_cache.get(&key))
    }

    /// Drain pending per-page atlas uploads, each tagged with its texture-array layer index.
    pub(crate) fn take_atlas_uploads(&mut self) -> Vec<(usize, TextAtlasUpload)> {
        self.atlas.take_uploads()
    }

    #[cfg(test)]
    pub(crate) fn glyph_cache_stats(&self) -> (usize, usize, usize) {
        (
            self.glyph_cache.len(),
            self.glyph_cache_hits,
            self.glyph_cache_misses,
        )
    }

    #[cfg(test)]
    pub(crate) fn swash_face_parse_count(&self) -> usize {
        self.swash_face_parse_count
    }

    pub(crate) fn cache_snapshot(&self) -> RendererTextCacheSnapshot {
        RendererTextCacheSnapshot {
            layout: self.system.layout_cache_snapshot(),
            glyph: GlyphCacheSnapshot {
                entries: self.glyph_cache.len(),
                hits: self.glyph_cache_hits,
                misses: self.glyph_cache_misses,
            },
            path: GlyphCacheSnapshot::default(),
        }
    }
}

fn build_cached_glyph_atlas(
    pages: &mut TextAtlasPages,
    scale_context: &mut SwashScaleContext,
    face: &SwashFaceState<'_>,
    glyph_id: u16,
    font_size_physical: f32,
    raster_scale_factor: f32,
    glyph_scale_logical: f32,
    subpixel_offset: GlyphSubpixelOffsetKey,
    text_render_mode: TextRenderMode,
    text_subpixel_order: TextSubpixelOrder,
    text_hinting: TextHinting,
    stem_darkening: StemDarkening,
    weight: u16,
    frame: u64,
) -> Result<Option<(CachedGlyphAtlas, Option<usize>)>> {
    let sources = [
        SwashSource::ColorOutline(0),
        SwashSource::ColorBitmap(SwashStrikeWith::BestFit),
        SwashSource::Outline,
    ];
    let mut scaler = scale_context
        .builder_with_id(face.font_ref, face.font_id)
        .size(font_size_physical)
        // Rasterize the requested weight instance to match cosmic-text's shaped advances.
        // No-op on static fonts (no `wght` axis).
        .variations([("wght", f32::from(weight))])
        .hint(text_hinting.should_hint(font_size_physical))
        .build();
    let mut renderer = SwashRender::new(&sources);
    renderer.format(match text_render_mode {
        TextRenderMode::Grayscale => SwashFormat::Alpha,
        TextRenderMode::LcdSubpixel => SwashFormat::subpixel_bgra(),
    });
    renderer.offset(subpixel_offset.as_swash_offset());
    let Some(image) = renderer.render(&mut scaler, glyph_id) else {
        return Ok(None);
    };

    let logical_offset = glyph_raster_offset(&image.placement, raster_scale_factor);

    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let pixel_count = width.saturating_mul(height);

    // A grayscale outline that rendered to a pure binary mask (jaggy) is re-rendered with
    // oversampling for true anti-aliased coverage; everything else uses the direct render.
    let needs_oversample = matches!(text_render_mode, TextRenderMode::Grayscale)
        && matches!(image.content, SwashImageContent::Mask)
        && pixel_count > 0
        && image.data.len() >= pixel_count
        && coverage_needs_oversampling(&image.data[..pixel_count], font_size_physical);

    let rasterized = if needs_oversample {
        let placement = image.placement;
        let oversampled = oversampled_mask_coverage(
            scale_context,
            face,
            glyph_id,
            font_size_physical,
            subpixel_offset,
            weight,
            &placement,
        )
        .map(|coverage| SwashRasterizedGlyph {
            pixels: mask_coverage_to_rgba(
                &coverage,
                stem_darkening.effective_amount(font_size_physical),
            ),
            is_color: false,
        });
        match oversampled.or_else(|| {
            swash_image_to_rgba(
                &image,
                font_size_physical,
                text_render_mode,
                text_subpixel_order,
                stem_darkening,
            )
        }) {
            Some(rasterized) => rasterized,
            None => return Ok(None),
        }
    } else {
        match swash_image_to_rgba(
            &image,
            font_size_physical,
            text_render_mode,
            text_subpixel_order,
            stem_darkening,
        ) {
            Some(rasterized) => rasterized,
            None => return Ok(None),
        }
    };

    if width == 0 || height == 0 {
        return Ok(Some((
            CachedGlyphAtlas {
                scale: glyph_scale_logical,
                offset: logical_offset,
                size: Size::ZERO,
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                color_mode: TextAtlasColorMode::from(text_render_mode),
                is_color: rasterized.is_color,
                page_index: 0,
            },
            None,
        )));
    }

    let insertion = match pages.insert_rgba(width, height, &rasterized.pixels, frame) {
        Ok(insertion) => insertion,
        // Too large for any page, or every page is already hot this frame: drop the glyph for
        // this frame. With on-demand page growth + LRU eviction there is no atlas-full cliff.
        Err(TextAtlasInsertError::TooLarge) | Err(TextAtlasInsertError::Full) => return Ok(None),
    };
    let page_index = insertion.page_index;
    let placement = insertion.placement;

    let atlas_size = pages.page_size();
    let inv_width = 1.0 / atlas_size.0 as f32;
    let inv_height = 1.0 / atlas_size.1 as f32;
    let logical_uv_min_x = placement.x as f32;
    let logical_uv_min_y = placement.y as f32;
    let logical_uv_max_x = logical_uv_min_x + image.placement.width as f32;
    let logical_uv_max_y = logical_uv_min_y + image.placement.height as f32;
    Ok(Some((
        CachedGlyphAtlas {
            scale: glyph_scale_logical,
            offset: logical_offset,
            size: Size::new(
                image.placement.width as f32 / raster_scale_factor,
                image.placement.height as f32 / raster_scale_factor,
            ),
            uv_min: [logical_uv_min_x * inv_width, logical_uv_min_y * inv_height],
            uv_max: [logical_uv_max_x * inv_width, logical_uv_max_y * inv_height],
            color_mode: TextAtlasColorMode::from(text_render_mode),
            is_color: rasterized.is_color,
            page_index,
        },
        insertion.evicted_page,
    )))
}

pub(crate) fn glyph_raster_offset(
    placement: &swash::zeno::Placement,
    raster_scale_factor: f32,
) -> Vector {
    Vector::new(
        placement.left as f32 / raster_scale_factor,
        -(placement.top as f32) / raster_scale_factor,
    )
}

pub(crate) struct SwashRasterizedGlyph {
    pub(crate) pixels: Vec<u8>,
    pub(crate) is_color: bool,
}

pub(crate) fn swash_image_to_rgba(
    image: &swash::scale::image::Image,
    ppem: f32,
    text_render_mode: TextRenderMode,
    text_subpixel_order: TextSubpixelOrder,
    stem_darkening: StemDarkening,
) -> Option<SwashRasterizedGlyph> {
    let width = usize::try_from(image.placement.width).ok()?;
    let height = usize::try_from(image.placement.height).ok()?;
    let pixel_count = width.checked_mul(height)?;

    let stem_darkening_amount = stem_darkening.effective_amount(ppem);

    match image.content {
        SwashImageContent::Mask => {
            if image.data.len() < pixel_count {
                return None;
            }
            // Coverage is already anti-aliased here; the binary-mask case is intercepted earlier
            // (build_cached_glyph_atlas) and re-rendered with oversampling for true coverage.
            Some(SwashRasterizedGlyph {
                pixels: mask_coverage_to_rgba(&image.data[..pixel_count], stem_darkening_amount),
                is_color: false,
            })
        }
        SwashImageContent::SubpixelMask => {
            if image.data.len() < pixel_count.checked_mul(4)? {
                return None;
            }

            let mut pixels = vec![0; pixel_count.checked_mul(4)?];
            for (source, pixel) in image.data.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
                pixel.copy_from_slice(&convert_subpixel_texel_for_mode(
                    [source[0], source[1], source[2], source[3]],
                    text_render_mode,
                    text_subpixel_order,
                    stem_darkening_amount,
                ));
            }

            Some(SwashRasterizedGlyph {
                pixels,
                is_color: false,
            })
        }
        SwashImageContent::Color => {
            let bytes = pixel_count.checked_mul(4)?;
            if image.data.len() < bytes {
                return None;
            }
            // Store the glyph's sRGB color verbatim; the fragment shader linearizes at full float
            // precision, avoiding the dark-tone banding of 8-bit linear storage.
            Some(SwashRasterizedGlyph {
                pixels: image.data[..bytes].to_vec(),
                is_color: true,
            })
        }
    }
}

pub(crate) fn convert_subpixel_texel_for_mode(
    source: [u8; 4],
    text_render_mode: TextRenderMode,
    text_subpixel_order: TextSubpixelOrder,
    stem_darkening_amount: f32,
) -> [u8; 4] {
    match text_render_mode {
        TextRenderMode::Grayscale => {
            let coverage =
                ((u16::from(source[0]) + u16::from(source[1]) + u16::from(source[2])) / 3) as u8;
            let coverage = apply_stem_darkening_to_coverage(coverage, stem_darkening_amount);
            [255, 255, 255, coverage]
        }
        TextRenderMode::LcdSubpixel => {
            let [red_source, green_source, blue_source] = match text_subpixel_order {
                TextSubpixelOrder::None => {
                    let coverage =
                        ((u16::from(source[0]) + u16::from(source[1]) + u16::from(source[2])) / 3)
                            as u8;
                    let coverage =
                        apply_stem_darkening_to_coverage(coverage, stem_darkening_amount);
                    return [255, 255, 255, coverage];
                }
                TextSubpixelOrder::Rgb => [source[2], source[1], source[0]],
                TextSubpixelOrder::Bgr => [source[0], source[1], source[2]],
            };
            let red = apply_stem_darkening_to_coverage(red_source, stem_darkening_amount);
            let green = apply_stem_darkening_to_coverage(green_source, stem_darkening_amount);
            let blue = apply_stem_darkening_to_coverage(blue_source, stem_darkening_amount);
            let alpha = red.max(green).max(blue);
            [red, green, blue, alpha]
        }
    }
}

/// Boost the coverage of partially-covered pixels (thin stems, antialiased edges) so small text
/// reads heavier. The boost is gated by `coverage` itself, so a fully-transparent pixel
/// (coverage 0) stays fully transparent and a fully-covered pixel (coverage 1) stays solid;
/// only the partial-coverage range in between is lifted. Without the `coverage` factor the old
/// formula mapped 0 -> `amount`, flooding every glyph cell's transparent background with
/// `amount` opacity and painting a gray box behind each glyph.
pub(crate) fn apply_stem_darkening_to_coverage(coverage: u8, amount: f32) -> u8 {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= f32::EPSILON {
        return coverage;
    }

    let coverage = coverage as f32 / 255.0;
    let darkened = coverage + (coverage * (1.0 - coverage) * amount);
    (darkened.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn mask_coverage_to_rgba(coverage: &[u8], stem_darkening_amount: f32) -> Vec<u8> {
    let mut pixels = vec![0u8; coverage.len() * 4];
    for (value, pixel) in coverage.iter().zip(pixels.chunks_exact_mut(4)) {
        let value = apply_stem_darkening_to_coverage(*value, stem_darkening_amount);
        pixel[0] = 255;
        pixel[1] = 255;
        pixel[2] = 255;
        pixel[3] = value;
    }
    pixels
}

fn is_binary_coverage(data: &[u8]) -> bool {
    !data.is_empty() && data.iter().all(|&value| value == 0 || value == 255)
}

fn coverage_needs_oversampling(data: &[u8], ppem: f32) -> bool {
    if is_binary_coverage(data) {
        return true;
    }
    if data.is_empty() || !ppem.is_finite() || ppem > 18.0 {
        return false;
    }

    let mut partial_values = [false; 256];
    let mut partial_value_count = 0usize;
    let mut partial_pixel_count = 0usize;
    let mut covered_pixel_count = 0usize;
    for &value in data {
        if value > 0 {
            covered_pixel_count += 1;
        }
        if value > 0 && value < 255 {
            partial_pixel_count += 1;
            let slot = &mut partial_values[value as usize];
            if !*slot {
                *slot = true;
                partial_value_count += 1;
            }
        }
    }

    covered_pixel_count >= 4
        && partial_pixel_count > 0
        && partial_value_count <= 2
        && partial_pixel_count * 5 <= covered_pixel_count * 2
}

/// Area-average a `factor`x oversampled coverage mask (`src`) down onto the 1x pixel grid defined
/// by the target placement, preserving the target's exact origin and dimensions. Oversample
/// samples that fall outside `src` count as zero (transparent outside the glyph), so each output
/// pixel is the true fractional coverage over its `factor` x `factor` footprint.
fn downsample_to_target(
    src: &[u8],
    src_width: usize,
    src_height: usize,
    src_left: i32,
    src_top: i32,
    dst_width: usize,
    dst_height: usize,
    dst_left: i32,
    dst_top: i32,
    factor: i32,
) -> Vec<u8> {
    let mut out = vec![0u8; dst_width * dst_height];
    // The glyph sits at factor x the 1x position, so 1x pixel p maps to oversample pixels
    // [p*factor, (p+1)*factor); align that span into src-local coordinates.
    let delta_x = dst_left * factor - src_left;
    let delta_y = dst_top * factor - src_top;
    let samples_per_pixel = (factor * factor).max(1) as u32;
    for j in 0..dst_height {
        let base_y = delta_y + j as i32 * factor;
        for i in 0..dst_width {
            let base_x = delta_x + i as i32 * factor;
            let mut sum = 0u32;
            for sy in 0..factor {
                let oy = base_y + sy;
                if oy < 0 || oy as usize >= src_height {
                    continue;
                }
                let row = oy as usize * src_width;
                for sx in 0..factor {
                    let ox = base_x + sx;
                    if ox < 0 || ox as usize >= src_width {
                        continue;
                    }
                    sum += u32::from(src[row + ox as usize]);
                }
            }
            out[j * dst_width + i] = (sum / samples_per_pixel) as u8;
        }
    }
    out
}

/// Re-render a glyph's coverage at OVERSAMPLE x and area-average it back onto the 1x grid defined
/// by `target`. Recovers true anti-aliased coverage for glyphs whose 1x render came out as a pure
/// binary mask (jaggy small or heavily-hinted outlines). Returns coverage at `target`'s exact
/// dimensions so glyph placement is unchanged.
fn oversampled_mask_coverage(
    scale_context: &mut SwashScaleContext,
    face: &SwashFaceState<'_>,
    glyph_id: u16,
    font_size_physical: f32,
    subpixel_offset: GlyphSubpixelOffsetKey,
    weight: u16,
    target: &swash::zeno::Placement,
) -> Option<Vec<u8>> {
    const OVERSAMPLE: i32 = 4;
    let sources = [SwashSource::Outline];
    let mut scaler = scale_context
        .builder_with_id(face.font_ref, face.font_id)
        .size(font_size_physical * OVERSAMPLE as f32)
        .variations([("wght", f32::from(weight))])
        .hint(false)
        .build();
    let mut renderer = SwashRender::new(&sources);
    renderer.format(SwashFormat::Alpha);
    // Same fractional sub-pixel position as the 1x render, expressed in oversample pixels.
    let offset = subpixel_offset.as_swash_offset();
    renderer.offset(swash::zeno::Vector::new(
        offset.x * OVERSAMPLE as f32,
        offset.y * OVERSAMPLE as f32,
    ));
    let image = renderer.render(&mut scaler, glyph_id)?;
    if !matches!(image.content, SwashImageContent::Mask) {
        return None;
    }
    Some(downsample_to_target(
        &image.data,
        image.placement.width as usize,
        image.placement.height as usize,
        image.placement.left,
        image.placement.top,
        target.width as usize,
        target.height as usize,
        target.left,
        target.top,
        OVERSAMPLE,
    ))
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn downsample_all_covered_is_full() {
        // A 4x4 fully-covered oversample block becomes one fully-covered 1x pixel.
        let src = vec![255u8; 16];
        let out = downsample_to_target(&src, 4, 4, 0, 0, 1, 1, 0, 0, 4);
        assert_eq!(out, vec![255]);
    }

    #[test]
    fn downsample_half_covered_is_half() {
        // Top two oversample rows covered, bottom two empty -> ~50% coverage.
        let mut src = vec![0u8; 16];
        for value in src.iter_mut().take(8) {
            *value = 255;
        }
        let out = downsample_to_target(&src, 4, 4, 0, 0, 1, 1, 0, 0, 4);
        assert_eq!(out, vec![127]); // 8*255 / 16 = 127 (integer division)
    }

    #[test]
    fn downsample_preserves_target_dimensions() {
        // 8x4 oversample -> 2x1 target; only the left 4x4 block is covered.
        let mut src = vec![0u8; 32];
        for y in 0..4 {
            for x in 0..4 {
                src[y * 8 + x] = 255;
            }
        }
        let out = downsample_to_target(&src, 8, 4, 0, 0, 2, 1, 0, 0, 4);
        assert_eq!(out, vec![255, 0]);
    }

    #[test]
    fn downsample_aligns_via_placement_delta() {
        // Target pixel at 1x x=1 covers oversample [4,8); src begins at global x=4, so its local
        // [0,4) maps exactly onto that pixel. The delta term must absorb src_left vs dst_left*4.
        let src = vec![255u8; 16];
        let out = downsample_to_target(&src, 4, 4, 4, 0, 1, 1, 1, 0, 4);
        assert_eq!(out, vec![255]);
    }

    #[test]
    fn binary_coverage_detection() {
        assert!(is_binary_coverage(&[0, 255, 0, 255]));
        assert!(!is_binary_coverage(&[0, 128, 255]));
        assert!(!is_binary_coverage(&[]));
    }

    #[test]
    fn oversampling_detection_includes_sparse_small_ppem_partial_masks() {
        assert!(coverage_needs_oversampling(
            &[0, 255, 255, 255, 64, 255, 0, 0],
            12.0
        ));
    }

    #[test]
    fn oversampling_detection_keeps_rich_antialiasing_direct() {
        assert!(!coverage_needs_oversampling(
            &[0, 32, 64, 96, 128, 160, 192, 224, 255],
            12.0
        ));
    }

    #[test]
    fn oversampling_detection_keeps_large_ppem_partial_masks_direct() {
        assert!(!coverage_needs_oversampling(
            &[0, 255, 255, 255, 64, 255, 0, 0],
            24.0
        ));
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlyphRasterBounds {
    pub(crate) logical_min_x: f32,
    pub(crate) logical_min_y: f32,
    pub(crate) logical_width: f32,
    pub(crate) logical_height: f32,
    pub(crate) raster_min_x: f32,
    pub(crate) raster_min_y: f32,
    pub(crate) raster_width: usize,
    pub(crate) raster_height: usize,
}

#[cfg(test)]
pub(crate) fn glyph_raster_bounds(path: &tiny_skia::Path) -> Option<GlyphRasterBounds> {
    let bounds = path.bounds().to_non_zero_rect()?;
    let logical_min_x = bounds.x();
    let logical_min_y = bounds.y();
    let logical_width = bounds.width();
    let logical_height = bounds.height();
    let raster_min_x = logical_min_x.floor();
    let raster_min_y = logical_min_y.floor();
    let raster_max_x = (logical_min_x + logical_width).ceil();
    let raster_max_y = (logical_min_y + logical_height).ceil();
    let raster_width = (raster_max_x - raster_min_x).max(0.0) as usize;
    let raster_height = (raster_max_y - raster_min_y).max(0.0) as usize;
    Some(GlyphRasterBounds {
        logical_min_x,
        logical_min_y,
        logical_width,
        logical_height,
        raster_min_x,
        raster_min_y,
        raster_width,
        raster_height,
    })
}

#[cfg(test)]
pub(crate) fn append_cached_glyph_atlas(
    vertices: &mut Vec<Vertex>,
    atlas: &CachedGlyphAtlas,
    glyph: &SceneShapedGlyph,
    color: Color,
    transform: Transform,
    viewport: Size,
    raster_scale_factor: f32,
) {
    if let Some(instance) = build_text_atlas_instance(
        atlas,
        glyph,
        color,
        TextCoveragePolicy::Linear,
        transform,
        Vector::ZERO,
        viewport,
        raster_scale_factor,
    ) {
        append_text_instance_vertices(vertices, std::slice::from_ref(&instance));
    }
}

fn build_text_atlas_instance(
    atlas: &CachedGlyphAtlas,
    glyph: &SceneShapedGlyph,
    color: Color,
    coverage_policy: TextCoveragePolicy,
    transform: Transform,
    pixel_snap_offset: Vector,
    viewport: Size,
    raster_scale_factor: f32,
) -> Option<TextAtlasInstance> {
    if atlas.size.is_empty() || viewport.is_empty() {
        return None;
    }

    let rgba = if atlas.is_color {
        [1.0, 1.0, 1.0, -color.clamped().alpha]
    } else {
        shader_color(color)
    };
    let residual_scale = glyph.scale / atlas.scale.max(f32::EPSILON);
    let left = glyph.origin_x + (atlas.offset.x * residual_scale);
    let top = glyph.origin_y + (atlas.offset.y * residual_scale);
    let width = atlas.size.width * residual_scale;
    let height = atlas.size.height * residual_scale;
    let (top_left, top_right, bottom_left, bottom_right) = snapped_glyph_quad(
        transform,
        pixel_snap_offset,
        Point::new(glyph.origin_x, glyph.origin_y),
        Rect::new(left, top, width, height),
        raster_scale_factor,
    );

    let top_left = to_ndc(top_left.x, top_left.y, viewport);
    let top_right = to_ndc(top_right.x, top_right.y, viewport);
    let bottom_left = to_ndc(bottom_left.x, bottom_left.y, viewport);
    let _bottom_right = to_ndc(bottom_right.x, bottom_right.y, viewport);

    let atlas_contains_lcd_subpixels = matches!(atlas.color_mode, TextAtlasColorMode::LcdSubpixel);
    let (coverage_policy_kind, coverage_policy_parameter) =
        coverage_policy_shader_metadata(coverage_policy);

    Some(TextAtlasInstance {
        top_left,
        x_axis: [top_right[0] - top_left[0], top_right[1] - top_left[1]],
        y_axis: [bottom_left[0] - top_left[0], bottom_left[1] - top_left[1]],
        uv_min: atlas.uv_min,
        uv_max: atlas.uv_max,
        color: rgba,
        metadata: [
            (atlas_contains_lcd_subpixels && allows_lcd_text(transform)) as u8 as f32,
            atlas_contains_lcd_subpixels as u8 as f32,
            coverage_policy_kind,
            coverage_policy_parameter,
        ],
        layer: atlas.page_index as u32,
    })
}

fn coverage_policy_shader_metadata(policy: TextCoveragePolicy) -> (f32, f32) {
    match policy.normalized() {
        TextCoveragePolicy::Perceptual => coverage_policy_shader_metadata(
            TextCoveragePolicy::CoverageBoost(perceptual_text_coverage_boost(Color::BLACK)),
        ),
        TextCoveragePolicy::Linear => (0.0, 0.0),
        TextCoveragePolicy::Gamma(gamma) => (1.0, gamma),
        TextCoveragePolicy::CoverageBoost(amount) => (2.0, amount),
        TextCoveragePolicy::TwoCoverageMinusCoverageSq => (3.0, 0.0),
    }
}

pub(crate) fn allows_lcd_text(transform: Transform) -> bool {
    transform_is_lcd_safe(transform)
}

pub(crate) fn glyph_subpixel_offset(
    transform: Transform,
    pixel_snap_offset: Vector,
    glyph: &SceneShapedGlyph,
    raster_scale_factor: f32,
) -> GlyphSubpixelOffsetKey {
    if !transform_is_axis_aligned(transform) || raster_scale_factor <= 0.0 {
        return GlyphSubpixelOffsetKey::default();
    }

    let origin =
        transform.transform_point(Point::new(glyph.origin_x, glyph.origin_y)) + pixel_snap_offset;
    GlyphSubpixelOffsetKey::new(
        physical_pixel_phase(origin.x * raster_scale_factor, GLYPH_SUBPIXEL_VARIANTS_X).variant,
        physical_pixel_phase(origin.y * raster_scale_factor, GLYPH_SUBPIXEL_VARIANTS_Y).variant,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PhysicalPixelPhase {
    integer: f32,
    variant: u8,
}

fn physical_pixel_phase(physical_position: f32, variants: u8) -> PhysicalPixelPhase {
    if variants <= 1 {
        return PhysicalPixelPhase {
            integer: physical_position.round(),
            variant: 0,
        };
    }

    let variants_i32 = i32::from(variants);
    let rounded = (physical_position * f32::from(variants)).round() as i32;
    let variant = rounded.rem_euclid(variants_i32) as u8;
    let integer = (rounded - i32::from(variant)) as f32 / f32::from(variants);

    PhysicalPixelPhase { integer, variant }
}

#[cfg(test)]
fn append_text_instance_vertices(vertices: &mut Vec<Vertex>, instances: &[TextAtlasInstance]) {
    for instance in instances {
        let top_left = instance.top_left;
        let top_right = [
            instance.top_left[0] + instance.x_axis[0],
            instance.top_left[1] + instance.x_axis[1],
        ];
        let bottom_left = [
            instance.top_left[0] + instance.y_axis[0],
            instance.top_left[1] + instance.y_axis[1],
        ];
        let bottom_right = [
            top_right[0] + instance.y_axis[0],
            top_right[1] + instance.y_axis[1],
        ];
        vertices.extend_from_slice(&[
            Vertex::basic(top_left, instance.color, instance.uv_min, [0.0; 4]),
            Vertex::basic(
                top_right,
                instance.color,
                [instance.uv_max[0], instance.uv_min[1]],
                [0.0; 4],
            ),
            Vertex::basic(
                bottom_left,
                instance.color,
                [instance.uv_min[0], instance.uv_max[1]],
                [0.0; 4],
            ),
            Vertex::basic(
                bottom_left,
                instance.color,
                [instance.uv_min[0], instance.uv_max[1]],
                [0.0; 4],
            ),
            Vertex::basic(
                top_right,
                instance.color,
                [instance.uv_max[0], instance.uv_min[1]],
                [0.0; 4],
            ),
            Vertex::basic(bottom_right, instance.color, instance.uv_max, [0.0; 4]),
        ]);
    }
}

fn snapped_glyph_quad(
    transform: Transform,
    pixel_snap_offset: Vector,
    glyph_origin: Point,
    rect: Rect,
    raster_scale_factor: f32,
) -> (Point, Point, Point, Point) {
    let transformed_origin = transform.transform_point(glyph_origin);
    let top_left = transform.transform_point(rect.origin);
    let top_right = transform.transform_point(Point::new(rect.max_x(), rect.y()));
    let bottom_left = transform.transform_point(Point::new(rect.x(), rect.max_y()));
    let bottom_right = transform.transform_point(Point::new(rect.max_x(), rect.max_y()));

    if !transform_is_axis_aligned(transform) || raster_scale_factor <= 0.0 {
        return (top_left, top_right, bottom_left, bottom_right);
    }

    let snap_origin = transformed_origin + pixel_snap_offset;
    let snapped_origin_x = physical_pixel_phase(
        snap_origin.x * raster_scale_factor,
        GLYPH_SUBPIXEL_VARIANTS_X,
    )
    .integer
        / raster_scale_factor
        - pixel_snap_offset.x;
    let snapped_origin_y = physical_pixel_phase(
        snap_origin.y * raster_scale_factor,
        GLYPH_SUBPIXEL_VARIANTS_Y,
    )
    .integer
        / raster_scale_factor
        - pixel_snap_offset.y;
    let snapped_left = snapped_origin_x + (top_left.x - transformed_origin.x);
    let snapped_top = snapped_origin_y + (top_left.y - transformed_origin.y);
    let width = top_right.x - top_left.x;
    let height = bottom_left.y - top_left.y;

    (
        Point::new(snapped_left, snapped_top),
        Point::new(snapped_left + width, snapped_top),
        Point::new(snapped_left, snapped_top + height),
        Point::new(snapped_left + width, snapped_top + height),
    )
}

fn transform_is_axis_aligned(transform: Transform) -> bool {
    transform.xy.abs() <= f32::EPSILON && transform.yx.abs() <= f32::EPSILON
}

fn transform_is_lcd_safe(transform: Transform) -> bool {
    transform_is_axis_aligned(transform) && transform.xx > 0.0 && transform.yy > 0.0
}

pub(crate) fn append_cached_path_mesh(
    vertices: &mut Vec<Vertex>,
    mesh: &CachedGlyphMesh,
    color: Color,
    viewport: Size,
) {
    if viewport.is_empty() {
        return;
    }

    let rgba = shader_color(color);
    for index in &mesh.indices {
        let vertex = mesh.vertices[*index as usize];
        let ndc = to_ndc(vertex.position.x, vertex.position.y, viewport);
        vertices.push(Vertex::basic(
            ndc,
            [rgba[0], rgba[1], rgba[2], rgba[3] * vertex.coverage],
            [0.0, 0.0],
            [0.0; 4],
        ));
    }
}
fn append_painted_path(
    vertices: &mut Vec<Vertex>,
    overlay_vertices: &mut Vec<Vertex>,
    draw_ops: &mut DrawOpArena,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    path_cache: &mut PathMeshCache,
    viewport: Size,
    feather_width: f32,
) -> Result<FillPathRenderMode> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(FillPathRenderMode::SolidOnly);
    }

    let coverage_outset = analytic_coverage_outset(feather_width);
    if state
        .visible_rect(path.bounds().inflate(coverage_outset, coverage_outset))
        .is_none()
    {
        return Ok(FillPathRenderMode::SolidOnly);
    }

    let transformed_bounds = state.current_transform.transform_rect_bbox(path.bounds());
    let lyon_path = build_lyon_path(path, state.current_transform);
    if let Some(data) = build_analytic_fill_path_data(&lyon_path, feather_width) {
        append_analytic_path_quad(
            overlay_vertices,
            transformed_bounds.inflate(coverage_outset, coverage_outset),
            color,
            viewport,
        );
        let id = draw_ops.insert_analytic_path(data);
        return Ok(FillPathRenderMode::SolidPlusAnalytic { id });
    }

    let mesh = path_cache.cached_fill_mesh(path, state.current_transform, feather_width)?;
    append_cached_path_mesh(vertices, mesh, color, viewport);
    Ok(FillPathRenderMode::SolidOnly)
}

fn append_stroked_path(
    vertices: &mut Vec<Vertex>,
    overlay_vertices: &mut Vec<Vertex>,
    draw_ops: &mut DrawOpArena,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    stroke: StrokeStyle,
    path_cache: &mut PathMeshCache,
    viewport: Size,
    feather_width: f32,
) -> Result<Option<u64>> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(None);
    }

    let stroke = StrokeStyle {
        width: stroke.width.max(1.0),
        ..stroke
    };
    let line_width = stroke.width;
    let coverage_outset = analytic_coverage_outset(feather_width);
    let stroke_outset = (line_width * 0.5) + coverage_outset;
    if state
        .visible_rect(path.bounds().inflate(stroke_outset, stroke_outset))
        .is_none()
    {
        return Ok(None);
    }

    let analytic_stroke_supported = matches!(
        (stroke.cap, stroke.join),
        (sui_scene::StrokeCap::Butt, sui_scene::StrokeJoin::Miter)
            | (sui_scene::StrokeCap::Round, sui_scene::StrokeJoin::Round)
    );
    if analytic_stroke_supported {
        let transformed_bounds = state.current_transform.transform_rect_bbox(path.bounds());
        let lyon_path = build_lyon_path(path, state.current_transform);
        if let Some(data) = build_analytic_stroke_path_data(&lyon_path, line_width, feather_width) {
            append_analytic_path_quad(
                overlay_vertices,
                transformed_bounds.inflate(stroke_outset, stroke_outset),
                color,
                viewport,
            );
            let id = draw_ops.insert_analytic_path(data);
            return Ok(Some(id));
        }
    }

    let mesh =
        path_cache.cached_stroke_mesh(path, state.current_transform, stroke, feather_width)?;
    append_cached_path_mesh(vertices, mesh, color, viewport);
    Ok(None)
}

fn build_analytic_fill_path_data(
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
                position: [point.x, point.y],
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

fn build_analytic_stroke_path_data(
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
                position: [point.x, point.y],
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

fn append_analytic_path_quad(vertices: &mut Vec<Vertex>, rect: Rect, color: Color, viewport: Size) {
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
        Vertex::basic([min[0], min[1]], rgba, [x0, y0], [0.0; 4]),
        Vertex::basic([max[0], min[1]], rgba, [x1, y0], [0.0; 4]),
        Vertex::basic([min[0], max[1]], rgba, [x0, y1], [0.0; 4]),
        Vertex::basic([min[0], max[1]], rgba, [x0, y1], [0.0; 4]),
        Vertex::basic([max[0], min[1]], rgba, [x1, y0], [0.0; 4]),
        Vertex::basic([max[0], max[1]], rgba, [x1, y1], [0.0; 4]),
    ]);
}

fn tessellate_filled_lyon_path(
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

fn append_tessellated_filled_lyon_path_vertices(
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

fn transform_path_point(point_value: Point, transform: Transform) -> lyon_path::math::Point {
    let scene = transform.transform_point(point_value);
    point(scene.x, scene.y)
}

fn append_indexed_triangles(
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

fn append_image(
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

fn append_image_quad(
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

fn image_rect_raster_metadata(
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

fn image_quad_raster_metadata(
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

fn image_points_raster_metadata(
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

fn append_widget_shader_rect(
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
fn brush_fallback_color(brush: &Brush) -> Color {
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
fn append_rounded_rect_quad(
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

fn clipped_screen_quad(state: &SceneRasterState, rect: Rect) -> Option<Rect> {
    match state.current_clip_bounds() {
        Some(clip) => rect.intersection(clip),
        None => Some(rect),
    }
}

/// Clamp per-corner radii to half the smaller rect dimension so the SDF stays valid.
fn clamp_radii(radii: [f32; 4], half_w: f32, half_h: f32) -> [f32; 4] {
    let limit = half_w.min(half_h).max(0.0);
    [
        radii[0].clamp(0.0, limit),
        radii[1].clamp(0.0, limit),
        radii[2].clamp(0.0, limit),
        radii[3].clamp(0.0, limit),
    ]
}

/// Fill (and optionally border) a rounded rectangle. `mode` in the shader is FILL (0).
fn append_rounded_rect_fill(
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
fn append_rounded_rect_shadow(
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

/// Fill a (possibly rounded) rectangle with a 2-stop linear gradient. The gradient axis
/// is given by `start`/`end` in scene (pre-transform) coordinates; both are mapped into
/// the same center-origin rect-local space used by the SDF. Stops beyond the first two
/// are ignored (documented limitation of the bind-group-free packing).
#[allow(clippy::too_many_arguments)]
fn append_gradient_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    radii: [f32; 4],
    start: Point,
    end: Point,
    stop0: Color,
    stop1: Color,
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

    // Gradient axis end-points in center-origin local space (matching the SDF space).
    let start_screen = state.current_transform.transform_point(start);
    let end_screen = state.current_transform.transform_point(end);
    let axis = [
        start_screen.x - center.x,
        start_screen.y - center.y,
        end_screen.x - center.x,
        end_screen.y - center.y,
    ];

    append_rounded_rect_quad(
        vertices,
        screen_quad,
        center,
        viewport,
        shader_color(stop0),
        [half_w, half_h, 0.0, feather],
        radii,
        axis,
        shader_color(stop1),
    );
}

fn points_bounds(points: &[Point]) -> Rect {
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

fn widget_shader_metadata(shader: sui_scene::WidgetShader) -> ([f32; 4], [f32; 4]) {
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

fn shader_color_space(space: ColorSpace) -> f32 {
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

fn push_draw_op(
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

fn push_image_draw_op(
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

fn push_text_draw_op(
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
    pub(crate) fn insert_analytic_path(&mut self, data: AnalyticPathCpuData) -> u64 {
        let id = self.next_analytic_path_id;
        self.next_analytic_path_id = self.next_analytic_path_id.wrapping_add(1);
        self.analytic_paths.insert(id, Arc::new(data));
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

    pub(crate) fn translate_in_place(&mut self, translation: Vector, viewport: Size) {
        if translation == Vector::ZERO || viewport.is_empty() {
            return;
        }

        let delta_x = (translation.x / viewport.width) * 2.0;
        let delta_y = -((translation.y / viewport.height) * 2.0);

        for vertex in &mut self.scene_vertices {
            vertex.position[0] += delta_x;
            vertex.position[1] += delta_y;
        }
        for draw_op in &self.draw_ops {
            if !matches!(draw_op.kind, DrawOpKind::AnalyticPath { .. }) {
                continue;
            }

            let start = draw_op.vertices.start as usize;
            let end = start + draw_op.vertices.len as usize;
            for vertex in &mut self.scene_vertices[start..end] {
                vertex.tex_coords[0] += translation.x;
                vertex.tex_coords[1] += translation.y;
            }
        }
        for vertex in &mut self.clip_vertices {
            vertex.position[0] += delta_x;
            vertex.position[1] += delta_y;
        }
        for instance in &mut self.text_instances {
            instance.top_left[0] += delta_x;
            instance.top_left[1] += delta_y;
        }
        for draw_op in &mut self.draw_ops {
            draw_op.clip_rect = draw_op.clip_rect.map(|rect| rect.translate(translation));
            if let Some(image) = &mut draw_op.image {
                image.bounds = image.bounds.translate(translation);
            }
        }
        for path in self.analytic_paths.values_mut() {
            Arc::make_mut(path).translate(translation);
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

    pub(crate) fn append_composed_fragment(
        &mut self,
        fragment: &DrawOpArena,
        translation: Vector,
        opacity: f32,
        external_clips: &[ResolvedClipPrimitive],
        viewport: Size,
    ) -> Result<()> {
        if translation == Vector::ZERO && external_clips.is_empty() && opacity == 1.0 {
            self.append_fragment(fragment);
            return Ok(());
        }

        let mut transformed = fragment.clone();
        transformed.translate_in_place(translation, viewport);
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
    fn scene_vertices(&self, span: PreparedVertices) -> &[Vertex] {
        &self.scene_vertices[span.start as usize..(span.start + span.len) as usize]
    }

    #[cfg(test)]
    fn text_instances(&self, span: PreparedVertices) -> &[TextAtlasInstance] {
        &self.text_instances[span.start as usize..(span.start + span.len) as usize]
    }
}

fn resolve_submission_clip_rect(current: Option<Rect>, next: Option<Rect>) -> Option<Option<Rect>> {
    match (current, next) {
        (Some(current), Some(next)) => current.intersection(next).map(Some),
        (Some(current), None) => Some(Some(current)),
        (None, Some(next)) => Some(Some(next)),
        (None, None) => Some(None),
    }
}

fn resolve_fragment_clip_rect(current: Option<Rect>, next: Option<Rect>) -> Option<Option<Rect>> {
    match (current, next) {
        (Some(current), Some(next)) => current.intersection(next).map(Some),
        (Some(current), None) => Some(Some(current)),
        (None, Some(next)) => Some(Some(next)),
        (None, None) => Some(None),
    }
}

pub(crate) const VERTEX_SIZE: u64 = std::mem::size_of::<Vertex>() as u64;
pub(crate) const TEXT_ATLAS_INSTANCE_SIZE: u64 = std::mem::size_of::<TextAtlasInstance>() as u64;

fn vertex_buffer_slice(buffer: &wgpu::Buffer, vertices: PreparedVertices) -> wgpu::BufferSlice<'_> {
    let start = vertices.start as u64 * VERTEX_SIZE;
    let end = start + vertices.len as u64 * VERTEX_SIZE;
    buffer.slice(start..end)
}

fn text_instance_buffer_slice(
    buffer: &wgpu::Buffer,
    instances: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = instances.start as u64 * TEXT_ATLAS_INSTANCE_SIZE;
    let end = start + instances.len as u64 * TEXT_ATLAS_INSTANCE_SIZE;
    buffer.slice(start..end)
}

fn rect_to_scissor(
    rect: Rect,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> Option<ScissorRect> {
    if rect.is_empty() || viewport.is_empty() {
        return None;
    }

    let framebuffer_width = framebuffer_size.0.max(1);
    let framebuffer_height = framebuffer_size.1.max(1);
    let scale_x = framebuffer_width as f32 / viewport.width.max(1.0);
    let scale_y = framebuffer_height as f32 / viewport.height.max(1.0);

    let min_x = quantize_scissor_edge(rect.x().max(0.0) * scale_x, framebuffer_width);
    let min_y = quantize_scissor_edge(rect.y().max(0.0) * scale_y, framebuffer_height);
    let max_x = quantize_scissor_edge(
        (rect.x() + rect.width()).min(viewport.width) * scale_x,
        framebuffer_width,
    );
    let max_y = quantize_scissor_edge(
        (rect.y() + rect.height()).min(viewport.height) * scale_y,
        framebuffer_height,
    );

    if max_x <= min_x || max_y <= min_y {
        return None;
    }

    let scissor = ScissorRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };
    if scissor.x == 0
        && scissor.y == 0
        && scissor.width == framebuffer_width
        && scissor.height == framebuffer_height
    {
        None
    } else {
        Some(scissor)
    }
}

fn quantize_scissor_edge(edge: f32, limit: u32) -> u32 {
    edge.round().clamp(0.0, limit as f32) as u32
}

pub(crate) fn to_ndc(x: f32, y: f32, viewport: Size) -> [f32; 2] {
    [
        ((x / viewport.width) * 2.0) - 1.0,
        1.0 - ((y / viewport.height) * 2.0),
    ]
}

pub(crate) fn normalize_framebuffer_size(size: Size) -> Option<(u32, u32)> {
    if size.is_empty() {
        None
    } else {
        Some(normalize_surface_size(
            size.width.round() as u32,
            size.height.round() as u32,
        ))
    }
}

pub(crate) fn normalize_surface_size(width: u32, height: u32) -> (u32, u32) {
    (width.max(1), height.max(1))
}

pub(crate) fn shader_color(color: Color) -> [f32; 4] {
    let linear = color.to_linear_srgb();
    [
        linear.red,
        linear.green,
        linear.blue,
        linear.alpha.clamp(0.0, 1.0),
    ]
}

fn preferred_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
}

fn preferred_hdr_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(|format| matches!(format, wgpu::TextureFormat::Rgba16Float))
}

pub(crate) fn output_transform_requires_intermediate(strategy: OutputStrategy) -> bool {
    match strategy {
        OutputStrategy::HdrNativeSurface { .. }
        | OutputStrategy::HdrIntermediateThenToneMap { .. } => true,
        OutputStrategy::WideGamutSurface { format, primaries } => {
            !format.is_srgb() || !matches!(primaries, DisplayColorPrimaries::Srgb)
        }
        OutputStrategy::SdrSurface { format } => !format.is_srgb(),
    }
}

pub(crate) fn output_primaries(strategy: OutputStrategy) -> DisplayColorPrimaries {
    match strategy {
        OutputStrategy::SdrSurface { .. } => DisplayColorPrimaries::Srgb,
        OutputStrategy::WideGamutSurface { primaries, .. }
        | OutputStrategy::HdrNativeSurface { primaries, .. }
        | OutputStrategy::HdrIntermediateThenToneMap { primaries, .. } => primaries,
    }
}

pub(crate) fn output_sdr_content_scale(
    strategy: OutputStrategy,
    brightness_nits: f32,
    _display_sdr_white_nits: Option<f32>,
) -> f32 {
    let sanitized = if brightness_nits.is_finite() && brightness_nits > 0.0 {
        brightness_nits
    } else {
        DEFAULT_SDR_CONTENT_BRIGHTNESS_NITS
    };

    match strategy {
        // Native HDR is presented as linear scRGB: 1.0 is the 80 nit scRGB
        // reference, so SDR reference white must be lifted to the requested
        // SDR-content brightness before presentation.
        OutputStrategy::HdrNativeSurface { .. } => sanitized / SCRGB_REFERENCE_WHITE_NITS,
        OutputStrategy::SdrSurface { .. }
        | OutputStrategy::WideGamutSurface { .. }
        | OutputStrategy::HdrIntermediateThenToneMap { .. } => 1.0,
    }
}

#[cfg(test)]
pub(crate) fn apply_output_transform_for_testing(
    color: [f32; 4],
    strategy: OutputStrategy,
    mode: RequestedToneMappingMode,
    sdr_content_brightness_nits: f32,
    display_sdr_white_nits: Option<f32>,
) -> [f32; 4] {
    let scale = output_sdr_content_scale(
        strategy,
        sdr_content_brightness_nits,
        display_sdr_white_nits,
    );
    let scaled = [
        color[0] * scale,
        color[1] * scale,
        color[2] * scale,
        color[3],
    ];

    let transformed = match strategy {
        OutputStrategy::HdrNativeSurface { .. } => [scaled[0], scaled[1], scaled[2], scaled[3]],
        _ => match mode {
            RequestedToneMappingMode::Automatic => match strategy {
                OutputStrategy::SdrSurface { .. }
                | OutputStrategy::WideGamutSurface { .. }
                | OutputStrategy::HdrIntermediateThenToneMap { .. } => {
                    tone_map_linear_color(scaled, RequestedToneMappingMode::Clamp)
                }
                OutputStrategy::HdrNativeSurface { .. } => unreachable!(),
            },
            RequestedToneMappingMode::Clamp => {
                tone_map_linear_color(scaled, RequestedToneMappingMode::Clamp)
            }
            RequestedToneMappingMode::Reinhard => {
                tone_map_linear_color(scaled, RequestedToneMappingMode::Reinhard)
            }
        },
    };

    let [red, green, blue] =
        linear_srgb_to_output_primaries([transformed[0], transformed[1], transformed[2]], strategy);
    [red, green, blue, transformed[3]]
}

#[cfg(test)]
pub(crate) fn linear_srgb_to_output_primaries(
    color: [f32; 3],
    strategy: OutputStrategy,
) -> [f32; 3] {
    match output_primaries(strategy) {
        DisplayColorPrimaries::Srgb => color,
        DisplayColorPrimaries::DisplayP3 => [
            (0.822_461_96 * color[0]) + (0.177_538_02 * color[1]),
            (0.033_194_2 * color[0]) + (0.966_805_76 * color[1]),
            (0.017_082_63 * color[0]) + (0.072_397_43 * color[1]) + (0.910_519_96 * color[2]),
        ],
    }
}

#[cfg(test)]
pub(crate) fn tone_map_linear_color(color: [f32; 4], mode: RequestedToneMappingMode) -> [f32; 4] {
    let transform = |channel: f32| match mode {
        RequestedToneMappingMode::Automatic | RequestedToneMappingMode::Clamp => {
            channel.clamp(0.0, 1.0)
        }
        RequestedToneMappingMode::Reinhard => {
            let channel = channel.max(0.0);
            channel / (1.0 + channel)
        }
    };

    [
        transform(color[0]),
        transform(color[1]),
        transform(color[2]),
        color[3].clamp(0.0, 1.0),
    ]
}

fn requested_output_primaries(
    capabilities: DisplayCapabilities,
    requested: ColorManagementMode,
) -> DisplayColorPrimaries {
    match requested.output_primaries {
        RequestedOutputColorPrimaries::Automatic => {
            if capabilities.supports_wide_gamut {
                capabilities.preferred_primaries
            } else {
                DisplayColorPrimaries::Srgb
            }
        }
        RequestedOutputColorPrimaries::Srgb => DisplayColorPrimaries::Srgb,
        RequestedOutputColorPrimaries::DisplayP3 => {
            if capabilities.supports_wide_gamut {
                DisplayColorPrimaries::DisplayP3
            } else {
                DisplayColorPrimaries::Srgb
            }
        }
    }
}

pub(crate) fn select_output_strategy(
    formats: &[wgpu::TextureFormat],
    capabilities: DisplayCapabilities,
    requested: ColorManagementMode,
) -> OutputStrategy {
    let sdr_format =
        preferred_surface_format(formats).unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
    let primaries = requested_output_primaries(capabilities.clone(), requested);
    let native_hdr_format = preferred_hdr_surface_format(formats);
    let native_hdr_available =
        capabilities.native_hdr_presentation_supported && native_hdr_format.is_some();

    if matches!(requested.mode, RequestedColorManagementMode::ForceSdr) {
        return OutputStrategy::SdrSurface { format: sdr_format };
    }

    let wants_hdr = match requested.dynamic_range {
        RequestedDynamicRangeMode::HighDynamicRange => true,
        RequestedDynamicRangeMode::StandardDynamicRange => false,
        RequestedDynamicRangeMode::Automatic => match requested.mode {
            RequestedColorManagementMode::PreferHdr => true,
            RequestedColorManagementMode::Automatic => {
                native_hdr_available
                    || capabilities.supports_hdr
                    || matches!(
                        capabilities.preferred_dynamic_range,
                        DynamicRangeMode::HighDynamicRange
                    )
            }
            RequestedColorManagementMode::ForceSdr
            | RequestedColorManagementMode::PreferWideGamut => false,
        },
    };
    let wants_wide_gamut = match requested.output_primaries {
        RequestedOutputColorPrimaries::DisplayP3 => true,
        RequestedOutputColorPrimaries::Srgb => false,
        RequestedOutputColorPrimaries::Automatic => match requested.mode {
            RequestedColorManagementMode::PreferWideGamut
            | RequestedColorManagementMode::PreferHdr => true,
            RequestedColorManagementMode::Automatic => capabilities.supports_wide_gamut,
            RequestedColorManagementMode::ForceSdr => false,
        },
    };

    if wants_hdr {
        if native_hdr_available && let Some(format) = native_hdr_format {
            let uses_linear_sc_rgb = capabilities.native_hdr_presentation_supported
                && matches!(
                    capabilities.preferred_primaries,
                    DisplayColorPrimaries::Srgb
                );
            let transfer = if uses_linear_sc_rgb {
                DisplayTransferFunction::LinearExtended
            } else {
                DisplayTransferFunction::Srgb
            };
            return OutputStrategy::HdrNativeSurface {
                format,
                primaries: DisplayColorPrimaries::Srgb,
                transfer,
            };
        }

        // If HDR cannot be presented end-to-end, normal presentation stays SDR.
        // HdrIntermediateThenToneMap is reserved for debug captures and explicit
        // diagnostics where seeing HDR scene values on an SDR display is useful.
        return OutputStrategy::SdrSurface { format: sdr_format };
    }

    if wants_wide_gamut && capabilities.supports_wide_gamut {
        return OutputStrategy::WideGamutSurface {
            format: sdr_format,
            primaries,
        };
    }

    OutputStrategy::SdrSurface { format: sdr_format }
}

fn configure_surface_for_strategy(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    size: (u32, u32),
    vsync_enabled: bool,
    strategy: OutputStrategy,
) -> Result<wgpu::SurfaceConfiguration> {
    let mut config = surface
        .get_default_config(adapter, size.0, size.1)
        .ok_or_else(|| Error::new("wgpu adapter does not support presenting to this surface"))?;
    config.format = strategy.surface_format();
    config.present_mode = if vsync_enabled {
        wgpu::PresentMode::AutoVsync
    } else {
        wgpu::PresentMode::AutoNoVsync
    };
    let capabilities = surface.get_capabilities(adapter);
    if capabilities
        .alpha_modes
        .contains(&wgpu::CompositeAlphaMode::Opaque)
    {
        config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
    }
    surface.configure(device, &config);
    configure_native_hdr_surface_color_space(surface, strategy)?;
    Ok(config)
}

fn configure_native_hdr_surface_color_space(
    surface: &wgpu::Surface<'static>,
    strategy: OutputStrategy,
) -> Result<()> {
    if !matches!(strategy, OutputStrategy::HdrNativeSurface { .. }) {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        windows_surface::set_native_hdr_surface_color_space(surface)
            .map_err(|error| Error::new(format!("failed to configure native HDR surface: {error}")))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = surface;
        Ok(())
    }
}

pub(crate) fn configure_surface(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    size: (u32, u32),
    vsync_enabled: bool,
    display_capabilities: DisplayCapabilities,
    color_management: ColorManagementMode,
) -> Result<(wgpu::SurfaceConfiguration, OutputStrategy)> {
    let surface_capabilities = surface.get_capabilities(adapter);
    let strategy = select_output_strategy(
        &surface_capabilities.formats,
        display_capabilities,
        color_management,
    );
    let config =
        configure_surface_for_strategy(surface, adapter, device, size, vsync_enabled, strategy)?;
    Ok((config, strategy))
}
