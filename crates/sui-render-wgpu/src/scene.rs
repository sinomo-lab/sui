use std::time::Instant;

use super::*;

#[derive(Debug, Default, Clone)]
pub(crate) struct SceneMesh {
    pub(crate) vertices: Vec<MeshVertex>,
    pub(crate) indices: Vec<u32>,
}

impl SceneMesh {
    pub(crate) fn colored_vertex(&mut self, position: Point, color: Color) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(MeshVertex { position, color });
        index
    }

    pub(crate) fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

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
    Image { handle: ImageHandle },
    TextAtlas,
    AnalyticPath { id: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct DrawOp {
    pub(crate) kind: DrawOpKind,
    pub(crate) vertices: PreparedVertices,
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) clip_state_index: usize,
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
    Image { handle: ImageHandle },
    TextAtlas,
    AnalyticPath { resource_signature: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedDrawPipelineKind {
    Solid,
    Image,
    TextAtlas,
    AnalyticPath,
}

impl PreparedDrawKind {
    const fn pipeline_kind(self) -> PreparedDrawPipelineKind {
        match self {
            Self::Solid => PreparedDrawPipelineKind::Solid,
            Self::Image { .. } => PreparedDrawPipelineKind::Image,
            Self::TextAtlas => PreparedDrawPipelineKind::TextAtlas,
            Self::AnalyticPath { .. } => PreparedDrawPipelineKind::AnalyticPath,
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
    pub(crate) image_bind_groups: HashMap<ImageHandle, wgpu::BindGroup>,
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
        .enumerate()
        .map(|(_, pass)| PreparedPassBatch {
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
        DrawOpKind::Image { handle } => PreparedDrawKind::Image { handle },
        DrawOpKind::TextAtlas => PreparedDrawKind::TextAtlas,
        DrawOpKind::AnalyticPath { id } => PreparedDrawKind::AnalyticPath {
            resource_signature: draw_ops.analytic_paths[&id].resource_signature,
        },
    }
}

pub(crate) fn collect_draw_op_resources(
    draw_ops: &DrawOpArena,
    analytic_paths: &mut HashMap<u64, Arc<AnalyticPathCpuData>>,
    image_handles: &mut HashSet<ImageHandle>,
) -> bool {
    let mut uses_text_atlas = false;
    for draw in &draw_ops.draw_ops {
        match draw.kind {
            DrawOpKind::Solid => {}
            DrawOpKind::Image { handle } => {
                image_handles.insert(handle);
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
    image_bind_groups: &HashMap<ImageHandle, wgpu::BindGroup>,
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
    image_bind_groups: &HashMap<ImageHandle, wgpu::BindGroup>,
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
    image_bind_groups: &HashMap<ImageHandle, wgpu::BindGroup>,
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
    image_bind_groups: &HashMap<ImageHandle, wgpu::BindGroup>,
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
            PreparedDrawKind::Image { handle } => {
                let bind_group = image_bind_groups
                    .get(&handle)
                    .expect("image bind group prepared before retained render pass");
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            PreparedDrawKind::TextAtlas => {
                let bind_group = text_atlas_bind_group
                    .expect("text atlas bind group prepared before retained render pass");
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            PreparedDrawKind::AnalyticPath { .. } => {}
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
            PreparedDrawKind::AnalyticPath { resource_signature } => {
                let scene_buffer = scene_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a scene vertex buffer")
                })?;
                render_pass.set_vertex_buffer(0, vertex_buffer_slice(scene_buffer, draw.vertices));
                let slot = analytic_path_resources
                    .expect("analytic path resources prepared before retained render pass")
                    .slots
                    .get(&resource_signature)
                    .copied()
                    .expect("analytic path slot prepared before retained render pass");
                (0..draw.vertices.len, slot..slot + 1)
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
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                append_painted_rect(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    *color,
                    viewport,
                    self.feather_width,
                );
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::StrokeRect {
                rect,
                brush,
                stroke,
            } => {
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                append_stroke_rect(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    *color,
                    *stroke,
                    viewport,
                    self.feather_width,
                );
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::FillPath { path, brush } => {
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                self.overlay_scratch_vertices.clear();
                let render_mode = append_painted_path(
                    &mut self.scratch_vertices,
                    &mut self.overlay_scratch_vertices,
                    draw_ops,
                    state,
                    path,
                    *color,
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
                let Brush::Solid(color) = brush;
                self.scratch_vertices.clear();
                self.overlay_scratch_vertices.clear();
                let analytic_id = append_stroked_path(
                    &mut self.scratch_vertices,
                    &mut self.overlay_scratch_vertices,
                    draw_ops,
                    state,
                    path,
                    *color,
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
                let image = self.frame.image_registry.get(source.image).ok_or_else(|| {
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
                    image,
                    viewport,
                );
                push_draw_op(
                    draw_ops,
                    DrawOpKind::Image {
                        handle: source.image,
                    },
                    &self.scratch_vertices,
                    state,
                );
                diagnostics.image_command_count += 1;
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
            SceneCommand::Layer(layer) => {
                Err(Error::new(format!(
                    "retained direct packet compiler encountered nested layer {}",
                    layer.layer_id().get()
                )))
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
            SceneCommand::DrawImage { .. } => {
                diagnostics.image_command_time_ms += elapsed_ms;
            }
            SceneCommand::PushClipPath { .. } => {
                diagnostics.clip_path_command_time_ms += elapsed_ms;
            }
            SceneCommand::PopClip
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PopTransform
            | SceneCommand::Layer(_) => {}
        }

        result
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SceneRasterState {
    pub(crate) current_transform: Transform,
    pub(crate) transform_stack: Vec<Transform>,
    clip_stack: Vec<ClipPrimitive>,
    pub(crate) path_clip_state_id: u64,
    pub(crate) active_path_clips: Vec<PreparedVertices>,
    pub(crate) clip_state_index: usize,
}

impl SceneRasterState {
    pub(crate) fn new(draw_ops: &mut DrawOpArena) -> Self {
        let clip_state_index = draw_ops.push_clip_state(&[]);
        Self {
            current_transform: Transform::IDENTITY,
            transform_stack: Vec::new(),
            clip_stack: Vec::new(),
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
        state.transform_stack.clear();
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
    pub(crate) atlas: TextAtlas,
    swash_scale_context: SwashScaleContext,
    pub(crate) text_render_mode: TextRenderMode,
    pub(crate) coverage_policy: TextCoveragePolicy,
    pub(crate) glyph_pixel_alignment_enabled: bool,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) glyph_cache_hits: usize,
    pub(crate) glyph_cache_misses: usize,
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
            atlas: TextAtlas::default(),
            swash_scale_context: SwashScaleContext::new(),
            text_render_mode: TextRenderMode::default(),
            coverage_policy: TextCoveragePolicy::default(),
            glyph_pixel_alignment_enabled: true,
            diagnostics_enabled: true,
            glyph_cache_hits: 0,
            glyph_cache_misses: 0,
            #[cfg(test)]
            swash_face_parse_count: 0,
            frame_stats: TextFrameStats::default(),
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

    pub(crate) fn set_text_coverage_policy(&mut self, policy: TextCoveragePolicy) {
        self.coverage_policy = policy.normalized();
    }

    pub(crate) fn set_glyph_pixel_alignment_enabled(&mut self, enabled: bool) {
        self.glyph_pixel_alignment_enabled = enabled;
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_stats = TextFrameStats::default();
    }

    pub(crate) fn frame_stats(&self) -> TextFrameStats {
        self.frame_stats
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
        viewport: Size,
        raster_scale_factor: f32,
    ) -> Result<()>
    where
        I: IntoIterator<Item = sui_text::TextGlyphInstance<'a>>,
    {

        let mut active_face_index = None;
        let mut swash_face = None;
        let glyph_pixel_alignment_enabled = self.glyph_pixel_alignment_enabled;

        for glyph in glyphs {
            let face_index = glyph.glyph.face_index;
            if active_face_index != Some(face_index) {
                active_face_index = Some(face_index);
                swash_face = None;
            }

            let glyph_face = glyph.face;
            let face_key = GlyphFaceCacheKey::new(glyph_face);
            let glyph_style = glyph.style;
            let coverage_policy = self
                .coverage_policy
                .resolved_for_text_color(glyph_style.color);
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
                coverage_policy,
            )? {
                if let Some(instance) = build_text_atlas_instance(
                    atlas,
                    &translated_glyph,
                    glyph_style.color,
                    state.current_transform,
                    viewport,
                    raster_scale_factor,
                    glyph_pixel_alignment_enabled,
                ) {
                    atlas_instances.push(instance);
                    if self.diagnostics_enabled {
                        self.frame_stats.glyph_instances += 1;
                        self.frame_stats.glyph_upload_bytes += TEXT_ATLAS_INSTANCE_SIZE;
                    }
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
        coverage_policy: TextCoveragePolicy,
    ) -> Result<Option<&CachedGlyphAtlas>> {
        let atlas_physical_scale = glyph_scale * raster_scale_factor.max(1.0);
        let scale_bucket = glyph_scale_bucket(atlas_physical_scale);
        let key = GlyphCacheKey::new(
            face_key,
            glyph_id,
            scale_bucket,
            self.text_render_mode,
            coverage_policy,
        );
        match self.glyph_cache.entry(key) {
            Entry::Occupied(entry) => {
                if self.diagnostics_enabled {
                    self.glyph_cache_hits += 1;
                }
                Ok(Some(&*entry.into_mut()))
            }
            Entry::Vacant(entry) => {
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
                let primitive = if let Some(atlas) = build_cached_glyph_atlas(
                    &mut self.atlas,
                    &mut self.swash_scale_context,
                    swash_face,
                    glyph_id,
                    swash_face.ppem_for_scale(bucketed_physical_scale),
                    raster_scale_factor.max(1.0),
                    bucketed_logical_scale,
                    self.text_render_mode,
                    coverage_policy,
                )? {
                    if let Some(started) = atlas_miss_started {
                        self.frame_stats.atlas_miss_count += 1;
                        self.frame_stats.atlas_miss_time_us += started.elapsed().as_micros() as u64;
                    }
                    atlas
                } else {
                    if let Some(started) = atlas_miss_started {
                        self.frame_stats.atlas_miss_count += 1;
                        self.frame_stats.atlas_miss_time_us += started.elapsed().as_micros() as u64;
                    }
                    return Ok(None);
                };
                Ok(Some(&*entry.insert(primitive)))
            }
        }
    }

    pub(crate) fn take_atlas_upload(&mut self) -> Option<TextAtlasUpload> {
        self.atlas.take_upload()
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
    atlas: &mut TextAtlas,
    scale_context: &mut SwashScaleContext,
    face: &SwashFaceState<'_>,
    glyph_id: u16,
    font_size_physical: f32,
    raster_scale_factor: f32,
    glyph_scale_logical: f32,
    text_render_mode: TextRenderMode,
    coverage_policy: TextCoveragePolicy,
) -> Result<Option<CachedGlyphAtlas>> {
    let sources = [
        SwashSource::ColorOutline(0),
        SwashSource::ColorBitmap(SwashStrikeWith::BestFit),
        SwashSource::Outline,
    ];
    let mut scaler = scale_context
        .builder_with_id(face.font_ref, face.font_id)
        .size(font_size_physical)
        .hint(false)
        .build();
    let mut renderer = SwashRender::new(&sources);
    renderer.format(match text_render_mode {
        TextRenderMode::Grayscale => SwashFormat::Alpha,
        TextRenderMode::LcdSubpixel => SwashFormat::Subpixel,
    });
    let Some(image) = renderer.render(&mut scaler, glyph_id) else {
        return Ok(None);
    };

    let logical_offset = glyph_raster_offset(&image.placement, raster_scale_factor);

    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let Some(rasterized) = swash_image_to_rgba(&image, text_render_mode, coverage_policy) else {
        return Ok(None);
    };

    if width == 0 || height == 0 {
        return Ok(Some(CachedGlyphAtlas {
            scale: glyph_scale_logical,
            offset: logical_offset,
            size: Size::ZERO,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            color_mode: TextAtlasColorMode::from(text_render_mode),
            is_color: rasterized.is_color,
        }));
    }

    let placement = match atlas.insert_rgba(width, height, &rasterized.pixels) {
        Ok(placement) => placement,
        Err(TextAtlasInsertError::TooLarge) => return Ok(None),
        Err(TextAtlasInsertError::Full) => return Err(text_atlas_retry_error()),
    };

    let atlas_size = atlas.size();
    let inv_width = 1.0 / atlas_size.0 as f32;
    let inv_height = 1.0 / atlas_size.1 as f32;
    let logical_uv_min_x = placement.x as f32;
    let logical_uv_min_y = placement.y as f32;
    let logical_uv_max_x = logical_uv_min_x + image.placement.width as f32;
    let logical_uv_max_y = logical_uv_min_y + image.placement.height as f32;
    Ok(Some(CachedGlyphAtlas {
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
    }))
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
    text_render_mode: TextRenderMode,
    coverage_policy: TextCoveragePolicy,
) -> Option<SwashRasterizedGlyph> {
    let width = usize::try_from(image.placement.width).ok()?;
    let height = usize::try_from(image.placement.height).ok()?;
    let pixel_count = width.checked_mul(height)?;

    match image.content {
        SwashImageContent::Mask => {
            let mut coverage = vec![0; pixel_count];
            if image.data.len() < pixel_count {
                return None;
            }
            coverage.copy_from_slice(&image.data[..pixel_count]);

            if coverage.iter().all(|value| *value == 0 || *value == 255) {
                coverage = soften_binary_coverage(&coverage, width, height);
            }

            let mut pixels = vec![0; pixel_count.checked_mul(4)?];
            for (coverage, pixel) in coverage.into_iter().zip(pixels.chunks_exact_mut(4)) {
                let alpha = (coverage_policy.apply(coverage as f32 / 255.0) * 255.0).round() as u8;
                pixel[0] = 255;
                pixel[1] = 255;
                pixel[2] = 255;
                pixel[3] = alpha;
            }

            Some(SwashRasterizedGlyph {
                pixels,
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
                    coverage_policy,
                ));
            }

            Some(SwashRasterizedGlyph {
                pixels,
                is_color: false,
            })
        }
        SwashImageContent::Color => {
            if image.data.len() < pixel_count.checked_mul(4)? {
                return None;
            }

            let mut pixels = vec![0; pixel_count.checked_mul(4)?];
            for (source, pixel) in image.data.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
                pixel[0] = linearized_color_unorm(source[0]);
                pixel[1] = linearized_color_unorm(source[1]);
                pixel[2] = linearized_color_unorm(source[2]);
                pixel[3] = source[3];
            }

            Some(SwashRasterizedGlyph {
                pixels,
                is_color: true,
            })
        }
    }
}

pub(crate) fn linearized_color_unorm(channel: u8) -> u8 {
    (srgb_transfer_to_linear(channel as f32 / 255.0) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

pub(crate) fn convert_subpixel_texel_for_mode(
    source: [u8; 4],
    text_render_mode: TextRenderMode,
    coverage_policy: TextCoveragePolicy,
) -> [u8; 4] {
    match text_render_mode {
        TextRenderMode::Grayscale => {
            let coverage = ((u16::from(source[0]) + u16::from(source[1]) + u16::from(source[2])) / 3) as u8;
            let alpha = (coverage_policy.apply(coverage as f32 / 255.0) * 255.0).round() as u8;
            [255, 255, 255, alpha]
        }
        TextRenderMode::LcdSubpixel => {
            let red = (coverage_policy.apply(source[0] as f32 / 255.0) * 255.0).round() as u8;
            let green = (coverage_policy.apply(source[1] as f32 / 255.0) * 255.0).round() as u8;
            let blue = (coverage_policy.apply(source[2] as f32 / 255.0) * 255.0).round() as u8;
            let alpha = red.max(green).max(blue);
            [red, green, blue, alpha]
        }
    }
}

fn soften_binary_coverage(coverage: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut softened = vec![0; coverage.len()];

    for y in 0..height {
        let y_start = y.saturating_sub(1);
        let y_end = (y + 1).min(height.saturating_sub(1));
        for x in 0..width {
            let x_start = x.saturating_sub(1);
            let x_end = (x + 1).min(width.saturating_sub(1));
            let mut sum = 0u32;
            let mut samples = 0u32;

            for sample_y in y_start..=y_end {
                for sample_x in x_start..=x_end {
                    sum += u32::from(coverage[sample_y * width + sample_x]);
                    samples += 1;
                }
            }

            softened[y * width + x] = (sum / samples.max(1)) as u8;
        }
    }

    softened
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
    glyph_pixel_alignment_enabled: bool,
) {
    if let Some(instance) = build_text_atlas_instance(
        atlas,
        glyph,
        color,
        transform,
        viewport,
        raster_scale_factor,
        glyph_pixel_alignment_enabled,
    ) {
        append_text_instance_vertices(vertices, std::slice::from_ref(&instance));
    }
}

fn build_text_atlas_instance(
    atlas: &CachedGlyphAtlas,
    glyph: &SceneShapedGlyph,
    color: Color,
    transform: Transform,
    viewport: Size,
    raster_scale_factor: f32,
    glyph_pixel_alignment_enabled: bool,
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
        Rect::new(left, top, width, height),
        raster_scale_factor,
        glyph_pixel_alignment_enabled,
    );

    let top_left = to_ndc(top_left.x, top_left.y, viewport);
    let top_right = to_ndc(top_right.x, top_right.y, viewport);
    let bottom_left = to_ndc(bottom_left.x, bottom_left.y, viewport);
    let _bottom_right = to_ndc(bottom_right.x, bottom_right.y, viewport);

    let atlas_contains_lcd_subpixels = matches!(atlas.color_mode, TextAtlasColorMode::LcdSubpixel);

    Some(TextAtlasInstance {
        top_left,
        x_axis: [top_right[0] - top_left[0], top_right[1] - top_left[1]],
        y_axis: [bottom_left[0] - top_left[0], bottom_left[1] - top_left[1]],
        uv_min: atlas.uv_min,
        uv_max: atlas.uv_max,
        color: rgba,
        metadata: [
            (atlas_contains_lcd_subpixels
                && allows_lcd_text(transform, glyph_pixel_alignment_enabled)) as u8 as f32,
            atlas_contains_lcd_subpixels as u8 as f32,
        ],
    })
}

pub(crate) fn allows_lcd_text(transform: Transform, glyph_pixel_alignment_enabled: bool) -> bool {
    glyph_pixel_alignment_enabled && transform_is_lcd_safe(transform)
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
            Vertex {
                position: top_left,
                color: instance.color,
                tex_coords: instance.uv_min,
            },
            Vertex {
                position: top_right,
                color: instance.color,
                tex_coords: [instance.uv_max[0], instance.uv_min[1]],
            },
            Vertex {
                position: bottom_left,
                color: instance.color,
                tex_coords: [instance.uv_min[0], instance.uv_max[1]],
            },
            Vertex {
                position: bottom_left,
                color: instance.color,
                tex_coords: [instance.uv_min[0], instance.uv_max[1]],
            },
            Vertex {
                position: top_right,
                color: instance.color,
                tex_coords: [instance.uv_max[0], instance.uv_min[1]],
            },
            Vertex {
                position: bottom_right,
                color: instance.color,
                tex_coords: instance.uv_max,
            },
        ]);
    }
}

fn snapped_glyph_quad(
    transform: Transform,
    rect: Rect,
    raster_scale_factor: f32,
    glyph_pixel_alignment_enabled: bool,
) -> (Point, Point, Point, Point) {
    let top_left = transform.transform_point(rect.origin);
    let top_right = transform.transform_point(Point::new(rect.max_x(), rect.y()));
    let bottom_left = transform.transform_point(Point::new(rect.x(), rect.max_y()));
    let bottom_right = transform.transform_point(Point::new(rect.max_x(), rect.max_y()));

    if !glyph_pixel_alignment_enabled
        || !transform_is_axis_aligned(transform)
        || raster_scale_factor <= 0.0
    {
        return (top_left, top_right, bottom_left, bottom_right);
    }

    let snapped_left = snap_to_physical_pixel(top_left.x, raster_scale_factor);
    let snapped_top = snap_to_physical_pixel(top_left.y, raster_scale_factor);
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
    transform_is_axis_aligned(transform)
        && transform.xx > 0.0
        && transform.yy > 0.0
}

fn snap_to_physical_pixel(value: f32, raster_scale_factor: f32) -> f32 {
    ((value * raster_scale_factor).round()) / raster_scale_factor
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
        vertices.push(Vertex {
            position: ndc,
            color: [rgba[0], rgba[1], rgba[2], rgba[3] * vertex.coverage],
            tex_coords: [0.0, 0.0],
        });
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

    if state.visible_rect(path.bounds()).is_none() {
        return Ok(FillPathRenderMode::SolidOnly);
    }

    let transformed_bounds = state.current_transform.transform_rect_bbox(path.bounds());
    if feather_width > 0.0 {
        let lyon_path = build_lyon_path(path, state.current_transform);
        if let Some(data) = build_analytic_fill_path_data(&lyon_path, feather_width) {
            tessellate_filled_lyon_path(vertices, &lyon_path, color, viewport)?;
            append_analytic_path_quad(
                overlay_vertices,
                transformed_bounds.inflate(feather_width, feather_width),
                color,
                viewport,
            );
            let id = draw_ops.insert_analytic_path(data);
            return Ok(FillPathRenderMode::SolidPlusAnalytic { id });
        }
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

    let line_width = stroke.width.max(1.0);
    if state
        .visible_rect(path.bounds().inflate(
            (line_width + feather_width) * 0.5,
            (line_width + feather_width) * 0.5,
        ))
        .is_none()
    {
        return Ok(None);
    }

    let transformed_bounds = state.current_transform.transform_rect_bbox(path.bounds());
    // The analytic stroke path is efficient for broad strokes, but very thin UI
    // strokes can lose most of their visible ink once feathering and clipping
    // are combined. Route thin strokes through the cached mesh path instead.
    if feather_width > 0.0 && line_width > feather_width * 2.0 {
        let lyon_path = build_lyon_path(path, state.current_transform);
        if let Some(data) = build_analytic_stroke_path_data(&lyon_path, line_width, feather_width) {
            append_analytic_path_quad(
                overlay_vertices,
                transformed_bounds.inflate(
                    (line_width + feather_width) * 0.5,
                    (line_width + feather_width) * 0.5,
                ),
                color,
                viewport,
            );
            let id = draw_ops.insert_analytic_path(data);
            return Ok(Some(id));
        }
    }

    let mesh =
        path_cache.cached_stroke_mesh(path, state.current_transform, line_width, feather_width)?;
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
        feather_width.max(0.5),
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
        feather_width.max(0.5),
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
        Vertex {
            position: [min[0], min[1]],
            color: rgba,
            tex_coords: [x0, y0],
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
            tex_coords: [x1, y0],
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
            tex_coords: [x0, y1],
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
            tex_coords: [x0, y1],
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
            tex_coords: [x1, y0],
        },
        Vertex {
            position: [max[0], max[1]],
            color: rgba,
            tex_coords: [x1, y1],
        },
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
        vertices.push(Vertex {
            position: [ndc[0], ndc[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        });
    }
}

fn append_image(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    source: &sui_scene::ImageSource,
    image: &RegisteredImage,
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

    let image_width = image.width() as f32;
    let image_height = image.height() as f32;
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
    let tint = source.tint.unwrap_or(Color::WHITE).clamped().to_array();

    vertices.extend_from_slice(&[
        Vertex {
            position: [min[0], min[1]],
            color: tint,
            tex_coords: [uv_left, uv_top],
        },
        Vertex {
            position: [max[0], min[1]],
            color: tint,
            tex_coords: [uv_right, uv_top],
        },
        Vertex {
            position: [min[0], max[1]],
            color: tint,
            tex_coords: [uv_left, uv_bottom],
        },
        Vertex {
            position: [min[0], max[1]],
            color: tint,
            tex_coords: [uv_left, uv_bottom],
        },
        Vertex {
            position: [max[0], min[1]],
            color: tint,
            tex_coords: [uv_right, uv_top],
        },
        Vertex {
            position: [max[0], max[1]],
            color: tint,
            tex_coords: [uv_right, uv_bottom],
        },
    ]);
}

fn append_stroke_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    stroke: StrokeStyle,
    viewport: Size,
    feather_width: f32,
) {
    feathering::append_stroke_rect(
        vertices,
        state,
        rect,
        color,
        stroke,
        viewport,
        feather_width,
    );
}

fn append_painted_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    feathering::append_painted_rect(vertices, state, rect, color, viewport, feather_width);
}

pub(crate) fn append_rect(vertices: &mut Vec<Vertex>, rect: Rect, color: Color, viewport: Size) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let min = to_ndc(rect.x(), rect.y(), viewport);
    let max = to_ndc(rect.max_x(), rect.max_y(), viewport);
    let rgba = shader_color(color);

    vertices.extend_from_slice(&[
        Vertex {
            position: [min[0], min[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [max[0], max[1]],
            color: rgba,
            tex_coords: [0.0, 0.0],
        },
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
        }
        for path in self.analytic_paths.values_mut() {
            Arc::make_mut(path).translate(translation);
        }
    }

    pub(crate) fn append_composed_fragment(
        &mut self,
        fragment: &DrawOpArena,
        translation: Vector,
        external_clips: &[ResolvedClipPrimitive],
        viewport: Size,
    ) -> Result<()> {
        if translation == Vector::ZERO && external_clips.is_empty() {
            self.append_fragment(fragment);
            return Ok(());
        }

        let mut transformed = fragment.clone();
        transformed.translate_in_place(translation, viewport);

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
            self.draw_ops
                .extend(transformed.draw_ops.iter().cloned().filter_map(|mut draw_op| {
                    draw_op.vertices = match draw_op.kind {
                        DrawOpKind::TextAtlas => draw_op.vertices.offset(text_delta),
                        _ => draw_op.vertices.offset(scene_delta),
                    };
                    draw_op.clip_state_index += clip_state_base;
                    let Some(clip_rect) =
                        resolve_fragment_clip_rect(draw_op.clip_rect, external_clip_rect)
                    else {
                        return None;
                    };
                    draw_op.clip_rect = clip_rect;
                    if let DrawOpKind::AnalyticPath { id } = draw_op.kind {
                        draw_op.kind = DrawOpKind::AnalyticPath {
                            id: analytic_id_map[&id],
                        };
                    }
                    Some(draw_op)
                }));
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

pub(crate) fn append_scene_mesh(vertices: &mut Vec<Vertex>, mesh: &SceneMesh, viewport: Size) {
    for index in &mesh.indices {
        let vertex = mesh.vertices[*index as usize];
        let ndc = to_ndc(vertex.position.x, vertex.position.y, viewport);
        vertices.push(Vertex {
            position: ndc,
            color: shader_color(vertex.color),
            tex_coords: [0.0, 0.0],
        });
    }
}

pub(crate) fn shader_color(color: Color) -> [f32; 4] {
    let color = color.clamped();
    let to_linear = match color.space {
        ColorSpace::LinearSrgb => |channel: f32| channel,
        ColorSpace::Srgb | ColorSpace::DisplayP3 => srgb_transfer_to_linear,
    };

    [
        to_linear(color.red),
        to_linear(color.green),
        to_linear(color.blue),
        color.alpha,
    ]
}

fn srgb_transfer_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn preferred_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
}

pub(crate) fn configure_surface(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    size: (u32, u32),
    vsync_enabled: bool,
) -> Result<wgpu::SurfaceConfiguration> {
    let mut config = surface
        .get_default_config(adapter, size.0, size.1)
        .ok_or_else(|| Error::new("wgpu adapter does not support presenting to this surface"))?;
    config.format = preferred_surface_format(&surface.get_capabilities(adapter).formats)
        .unwrap_or(config.format);
    config.present_mode = if vsync_enabled {
        wgpu::PresentMode::AutoVsync
    } else {
        wgpu::PresentMode::AutoNoVsync
    };
    surface.configure(device, &config);
    Ok(config)
}
