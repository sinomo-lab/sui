#![forbid(unsafe_code)]

use std::{collections::HashMap, fmt, sync::Arc};

use bytemuck::{Pod, Zeroable};
use lyon_path::{Path as LyonPath, builder::PathBuilder as LyonPathBuilder, math::point};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeOptions,
    StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};
use sui_core::{
    Color, Error, Path as ScenePath, PathElement, Point, Rect, Result, Size, Transform, WindowId,
};
use sui_scene::{Brush, FontRegistry, SceneCommand, SceneFrame, StrokeStyle, TextRun, TextStyle};
use ttf_parser::GlyphId;
use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCapabilities {
    pub supports_color_management: bool,
    pub supports_offscreen_surfaces: bool,
}

impl Default for RendererCapabilities {
    fn default() -> Self {
        Self {
            supports_color_management: true,
            supports_offscreen_surfaces: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererInterop {
    pub raw_wgpu_enabled: bool,
}

pub struct WgpuRenderer {
    instance: wgpu::Instance,
    frames_rendered: usize,
    capabilities: RendererCapabilities,
    last_frame: Option<SceneFrame>,
    shared: Option<SharedRenderer>,
    text_engine: Option<TextEngine>,
    surfaces: HashMap<WindowId, SurfaceState>,
    offscreen_targets: HashMap<WindowId, OffscreenTarget>,
}

impl WgpuRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_window(&mut self, window_id: WindowId, window: Arc<Window>) -> Result<()> {
        let physical_size = window.inner_size();
        let size = normalize_surface_size(physical_size.width, physical_size.height);
        let state = self.create_surface_state(window, size)?;

        self.surfaces.insert(window_id, state);
        self.offscreen_targets.remove(&window_id);
        Ok(())
    }

    pub fn remove_window(&mut self, window_id: WindowId) {
        self.surfaces.remove(&window_id);
        self.offscreen_targets.remove(&window_id);
    }

    pub fn render(&mut self, frame: &SceneFrame) -> Result<()> {
        let viewport = normalize_viewport(frame.viewport);

        if let Some(size) = viewport {
            if self.surfaces.contains_key(&frame.window_id) {
                self.render_surface(frame, size)?;
            } else {
                self.render_offscreen(frame, size)?;
            }
        }

        self.frames_rendered += 1;
        self.last_frame = Some(frame.clone());
        Ok(())
    }

    pub fn capabilities(&self) -> RendererCapabilities {
        self.capabilities
    }

    pub fn frames_rendered(&self) -> usize {
        self.frames_rendered
    }

    pub fn last_frame(&self) -> Option<&SceneFrame> {
        self.last_frame.as_ref()
    }

    fn text_engine(&mut self) -> Result<&mut TextEngine> {
        if self.text_engine.is_none() {
            self.text_engine = Some(TextEngine::new()?);
        }

        Ok(self
            .text_engine
            .as_mut()
            .expect("text engine initialized before returning mutable reference"))
    }

    fn ensure_shared(&mut self, compatible_surface: Option<&wgpu::Surface<'_>>) -> Result<()> {
        if self.shared.is_some() {
            return Ok(());
        }

        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface,
            }))
            .map_err(|error| Error::new(format!("failed to acquire wgpu adapter: {error}")))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SUI renderer device"),
            ..Default::default()
        }))
        .map_err(|error| Error::new(format!("failed to create wgpu device: {error}")))?;

        self.shared = Some(SharedRenderer {
            adapter,
            device,
            queue,
            pipelines: HashMap::new(),
        });

        Ok(())
    }

    fn render_surface(&mut self, frame: &SceneFrame, size: (u32, u32)) -> Result<()> {
        self.ensure_shared(None)?;
        self.configure_surface_if_needed(frame.window_id, size)?;

        let (frame_texture, suboptimal) = loop {
            let result = {
                let surface = self.surfaces.get(&frame.window_id).ok_or_else(|| {
                    Error::new(format!(
                        "missing surface for window {}",
                        frame.window_id.get()
                    ))
                })?;
                surface.surface.get_current_texture()
            };

            match result {
                wgpu::CurrentSurfaceTexture::Success(texture) => break (texture, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => break (texture, true),
                wgpu::CurrentSurfaceTexture::Outdated => {
                    self.configure_surface(frame.window_id, size)?;
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    self.recreate_surface(frame.window_id, size)?;
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(Error::new(
                        "wgpu surface acquisition triggered a validation error",
                    ));
                }
            }
        };

        let view = frame_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let format = {
            let surface = self.surfaces.get(&frame.window_id).ok_or_else(|| {
                Error::new(format!(
                    "missing surface for window {}",
                    frame.window_id.get()
                ))
            })?;
            surface.config.format
        };

        self.encode_scene(frame, format, &view)?;
        frame_texture.present();

        if suboptimal {
            self.configure_surface(frame.window_id, size)?;
        }

        Ok(())
    }

    fn render_offscreen(&mut self, frame: &SceneFrame, size: (u32, u32)) -> Result<()> {
        self.ensure_shared(None)?;

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let recreate = self
            .offscreen_targets
            .get(&frame.window_id)
            .is_none_or(|target| target.size != size || target.format != format);

        if recreate {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("SUI offscreen frame"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });

            self.offscreen_targets.insert(
                frame.window_id,
                OffscreenTarget {
                    texture,
                    format,
                    size,
                },
            );
        }

        let target = self
            .offscreen_targets
            .get(&frame.window_id)
            .ok_or_else(|| {
                Error::new(format!(
                    "missing offscreen target for window {}",
                    frame.window_id.get()
                ))
            })?;
        let view = target
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.encode_scene(frame, target.format, &view)
    }

    fn configure_surface_if_needed(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let surface = self
            .surfaces
            .get_mut(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

        if surface.config.width == size.0 && surface.config.height == size.1 {
            return Ok(());
        }

        self.configure_surface(window_id, size)
    }

    fn configure_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let surface = self
            .surfaces
            .get_mut(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?;

        let config = configure_surface(&surface.surface, &shared.adapter, &shared.device, size)?;
        surface.config = config;
        Ok(())
    }

    fn recreate_surface(&mut self, window_id: WindowId, size: (u32, u32)) -> Result<()> {
        let window = self
            .surfaces
            .get(&window_id)
            .ok_or_else(|| Error::new(format!("missing surface for window {}", window_id.get())))?
            .window
            .clone();
        let state = self.create_surface_state(window, size)?;
        self.surfaces.insert(window_id, state);
        Ok(())
    }

    fn encode_scene(
        &mut self,
        frame: &SceneFrame,
        target_format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
    ) -> Result<()> {
        let vertices = {
            let text_engine = self.text_engine()?;
            build_vertices(frame, text_engine)?
        };
        let shared = self
            .shared
            .as_mut()
            .expect("renderer shared state initialized");
        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SUI scene encoder"),
            });
        let vertex_buffer = if vertices.is_empty() {
            None
        } else {
            let buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SUI scene vertices"),
                size: std::mem::size_of_val(vertices.as_slice()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            shared
                .queue
                .write_buffer(&buffer, 0, bytemuck::cast_slice(vertices.as_slice()));
            Some(buffer)
        };

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SUI scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            if let Some(buffer) = vertex_buffer.as_ref() {
                let pipeline = shared.pipeline(target_format);
                render_pass.set_pipeline(pipeline);
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..vertices.len() as u32, 0..1);
            }
        }

        shared.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn create_surface_state(
        &mut self,
        window: Arc<Window>,
        size: (u32, u32),
    ) -> Result<SurfaceState> {
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| Error::new(format!("failed to create wgpu surface: {error}")))?;
        self.ensure_shared(Some(&surface))?;

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let config = configure_surface(&surface, &shared.adapter, &shared.device, size)?;

        Ok(SurfaceState {
            window,
            surface,
            config,
        })
    }
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self {
            instance: wgpu::Instance::default(),
            frames_rendered: 0,
            capabilities: RendererCapabilities::default(),
            last_frame: None,
            shared: None,
            text_engine: None,
            surfaces: HashMap::new(),
            offscreen_targets: HashMap::new(),
        }
    }
}

impl fmt::Debug for WgpuRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WgpuRenderer")
            .field("frames_rendered", &self.frames_rendered)
            .field("capabilities", &self.capabilities)
            .field("has_device", &self.shared.is_some())
            .field("surface_count", &self.surfaces.len())
            .finish()
    }
}

struct SharedRenderer {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl SharedRenderer {
    fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipelines.entry(format).or_insert_with(|| {
            let shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("SUI solid scene shader"),
                    source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
                });

            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("SUI solid scene pipeline"),
                    layout: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[Vertex::layout()],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        })
    }
}

struct SurfaceState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct OffscreenTarget {
    texture: wgpu::Texture,
    format: wgpu::TextureFormat,
    size: (u32, u32),
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[derive(Clone, Copy)]
struct TessellatedPoint;

impl FillVertexConstructor<[f32; 2]> for TessellatedPoint {
    fn new_vertex(&mut self, vertex: FillVertex<'_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}

impl StrokeVertexConstructor<[f32; 2]> for TessellatedPoint {
    fn new_vertex(&mut self, vertex: StrokeVertex<'_, '_>) -> [f32; 2] {
        let position = vertex.position();
        [position.x, position.y]
    }
}

fn build_vertices(frame: &SceneFrame, text_engine: &mut TextEngine) -> Result<Vec<Vertex>> {
    let viewport = frame.viewport;
    let mut vertices = Vec::new();
    let mut state = SceneRasterState::default();

    for command in frame.scene.commands() {
        match command {
            SceneCommand::Clear(color) => {
                append_rect(
                    &mut vertices,
                    Rect::new(0.0, 0.0, viewport.width, viewport.height),
                    *color,
                    viewport,
                );
            }
            SceneCommand::FillRect { rect, brush } => {
                let Brush::Solid(color) = brush;
                append_painted_rect(&mut vertices, &state, *rect, *color, viewport);
            }
            SceneCommand::StrokeRect {
                rect,
                brush,
                stroke,
            } => {
                let Brush::Solid(color) = brush;
                append_stroke_rect(&mut vertices, &state, *rect, *color, *stroke, viewport);
            }
            SceneCommand::FillPath { path, brush } => {
                let Brush::Solid(color) = brush;
                append_painted_path(&mut vertices, &state, path, *color, viewport)?;
            }
            SceneCommand::StrokePath {
                path,
                brush,
                stroke,
            } => {
                let Brush::Solid(color) = brush;
                append_stroked_path(&mut vertices, &state, path, *color, *stroke, viewport)?;
            }
            SceneCommand::DrawText(text) => {
                text_engine.append_text_run(
                    &mut vertices,
                    &state,
                    text,
                    frame.font_registry.as_ref(),
                    viewport,
                )?;
            }
            SceneCommand::DrawImage { rect, source } => {
                append_image_placeholder(&mut vertices, &state, *rect, source, viewport);
            }
            SceneCommand::PushClip { rect } => {
                state.push_clip(*rect);
            }
            SceneCommand::PopClip => {
                state.pop_clip();
            }
            SceneCommand::PushTransform { transform } => {
                state.push_transform(*transform);
            }
            SceneCommand::PopTransform => {
                state.pop_transform();
            }
            SceneCommand::Label { rect, text, color } => {
                text_engine.append_text_run(
                    &mut vertices,
                    &state,
                    &TextRun {
                        rect: *rect,
                        text: text.clone(),
                        style: TextStyle::new(*color),
                    },
                    frame.font_registry.as_ref(),
                    viewport,
                )?;
            }
        }
    }

    Ok(vertices)
}

#[derive(Debug, Clone)]
struct SceneRasterState {
    current_transform: Transform,
    transform_stack: Vec<Transform>,
    clip_stack: Vec<Rect>,
}

impl Default for SceneRasterState {
    fn default() -> Self {
        Self {
            current_transform: Transform::IDENTITY,
            transform_stack: Vec::new(),
            clip_stack: Vec::new(),
        }
    }
}

impl SceneRasterState {
    fn push_clip(&mut self, rect: Rect) {
        let transformed = self.current_transform.transform_rect_bbox(rect);
        let clip = match self.current_clip() {
            Some(current) => current.intersection(transformed).unwrap_or(Rect::ZERO),
            None => transformed,
        };
        self.clip_stack.push(clip);
    }

    fn pop_clip(&mut self) {
        let _ = self.clip_stack.pop();
    }

    fn push_transform(&mut self, transform: Transform) {
        self.transform_stack.push(self.current_transform);
        self.current_transform = self.current_transform.then(transform);
    }

    fn pop_transform(&mut self) {
        self.current_transform = self.transform_stack.pop().unwrap_or(Transform::IDENTITY);
    }

    fn current_clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }

    fn visible_rect(&self, rect: Rect) -> Option<Rect> {
        let transformed = self.current_transform.transform_rect_bbox(rect);

        match self.current_clip() {
            Some(clip) => transformed.intersection(clip),
            None => Some(transformed),
        }
    }
}

#[derive(Debug)]
struct TextEngine {
    font_db: fontdb::Database,
    default_font: fontdb::ID,
}

impl TextEngine {
    fn new() -> Result<Self> {
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();

        let families = [fontdb::Family::SansSerif];
        let default_font = font_db
            .query(&fontdb::Query {
                families: &families,
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })
            .or_else(|| font_db.faces().next().map(|face| face.id))
            .ok_or_else(|| Error::new("failed to locate a system font for text rendering"))?;

        Ok(Self {
            font_db,
            default_font,
        })
    }

    fn append_text_run(
        &mut self,
        vertices: &mut Vec<Vertex>,
        state: &SceneRasterState,
        text: &TextRun,
        font_registry: &FontRegistry,
        viewport: Size,
    ) -> Result<()> {
        if text.rect.is_empty() || text.text.is_empty() || viewport.is_empty() {
            return Ok(());
        }

        let shaped = self.shape_text_run(text, font_registry)?;
        if shaped.measurement.width <= 0.0 || shaped.measurement.height <= 0.0 {
            return Ok(());
        }
        if state.visible_rect(shaped.measurement.bounds).is_none() {
            return Ok(());
        }

        self.with_text_face(text, font_registry, |face| {
            for glyph in &shaped.glyphs {
                if let Some(bounds) = glyph.bounds {
                    if bounds.intersection(text.rect).is_none() {
                        continue;
                    }

                    if let Some(clip) = state.current_clip() {
                        let transformed = state.current_transform.transform_rect_bbox(bounds);
                        if transformed.intersection(clip).is_none() {
                            continue;
                        }
                    }
                }

                tessellate_glyph(
                    vertices,
                    &face,
                    glyph,
                    text.style.color,
                    state.current_transform,
                    viewport,
                )?;
            }

            Ok(())
        })?;

        Ok(())
    }

    fn shape_text_run(
        &self,
        text: &TextRun,
        font_registry: &FontRegistry,
    ) -> Result<ShapedTextLayout> {
        self.with_text_face(text, font_registry, |face| {
            shape_text_run_with_face(&face, text)
        })
    }

    fn with_text_face<T>(
        &self,
        text: &TextRun,
        font_registry: &FontRegistry,
        callback: impl FnOnce(rustybuzz::Face<'_>) -> Result<T>,
    ) -> Result<T> {
        if let Some(handle) = text.style.font {
            let font = font_registry.get(handle).ok_or_else(|| {
                Error::new(format!("font handle {} is not registered", handle.get()))
            })?;
            let face =
                rustybuzz::Face::from_slice(font.bytes(), font.face_index()).ok_or_else(|| {
                    Error::new(format!("failed to parse font handle {}", handle.get()))
                })?;
            return callback(face);
        }

        let font_id = self.default_font;
        self.font_db
            .with_face_data(font_id, |font_data, face_index| -> Result<T> {
                let face = rustybuzz::Face::from_slice(font_data, face_index)
                    .ok_or_else(|| Error::new("failed to parse fallback system font"))?;
                callback(face)
            })
            .transpose()?
            .ok_or_else(|| Error::new("failed to access fallback system font data"))
    }
}

#[derive(Debug, Clone, Copy)]
struct TextMeasurement {
    width: f32,
    height: f32,
    bounds: Rect,
}

#[derive(Debug, Clone, Copy)]
struct ShapedGlyph {
    glyph_id: u16,
    origin_x: f32,
    origin_y: f32,
    scale: f32,
    bounds: Option<Rect>,
}

#[derive(Debug, Clone)]
struct ShapedTextLayout {
    glyphs: Vec<ShapedGlyph>,
    measurement: TextMeasurement,
}

fn shape_text_run_with_face(
    face: &rustybuzz::Face<'_>,
    text: &TextRun,
) -> Result<ShapedTextLayout> {
    let units_per_em = face.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return Err(Error::new(
            "text face reported an invalid units-per-em value",
        ));
    }

    let scale = text.style.font_size / units_per_em;
    let ascent = f32::from(face.ascender()) * scale;
    let descent = f32::from(face.descender().abs()) * scale;
    let natural_line_height = f32::from(face.height().abs()) * scale;
    let line_height = text
        .style
        .line_height
        .max(natural_line_height)
        .max(text.style.font_size);
    let lines: Vec<&str> = text.text.split('\n').collect();
    let line_count = lines.len().max(1);
    let block_height = line_height * line_count as f32;
    let block_top = text.rect.y() + ((text.rect.height() - block_height).max(0.0) * 0.5);

    let mut glyphs = Vec::new();
    let mut measured_width: f32 = 0.0;
    let mut measured_bounds: Option<(f32, f32, f32, f32)> = None;

    for (line_index, line) in lines.iter().enumerate() {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(line);
        buffer.guess_segment_properties();
        let direction = buffer.direction();
        let shaped = rustybuzz::shape(face, &[], buffer);
        let glyph_infos = shaped.glyph_infos();
        let glyph_positions = shaped.glyph_positions();
        let line_width = glyph_positions
            .iter()
            .map(|position| position.x_advance as f32 * scale)
            .sum::<f32>()
            .abs();

        let mut pen_x = match direction {
            rustybuzz::Direction::RightToLeft => text.rect.max_x() - line_width,
            _ => text.rect.x(),
        };
        let mut pen_y = block_top + ascent + (line_index as f32 * line_height);
        measured_width = measured_width.max(line_width);

        for (info, position) in glyph_infos.iter().zip(glyph_positions.iter()) {
            let glyph_id = match u16::try_from(info.glyph_id) {
                Ok(glyph_id) => glyph_id,
                Err(_) => continue,
            };
            let origin_x = pen_x + (position.x_offset as f32 * scale);
            let origin_y = pen_y - (position.y_offset as f32 * scale);
            let bounds = face.glyph_bounding_box(GlyphId(glyph_id)).map(|bbox| {
                let min_x = origin_x + (f32::from(bbox.x_min) * scale);
                let max_x = origin_x + (f32::from(bbox.x_max) * scale);
                let min_y = origin_y - (f32::from(bbox.y_max) * scale);
                let max_y = origin_y - (f32::from(bbox.y_min) * scale);
                Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
            });

            if let Some(bounds) = bounds {
                measured_bounds = Some(match measured_bounds {
                    Some((min_x, min_y, max_x, max_y)) => (
                        min_x.min(bounds.x()),
                        min_y.min(bounds.y()),
                        max_x.max(bounds.max_x()),
                        max_y.max(bounds.max_y()),
                    ),
                    None => (bounds.x(), bounds.y(), bounds.max_x(), bounds.max_y()),
                });
            }

            glyphs.push(ShapedGlyph {
                glyph_id,
                origin_x,
                origin_y,
                scale,
                bounds,
            });

            pen_x += position.x_advance as f32 * scale;
            pen_y -= position.y_advance as f32 * scale;
        }
    }

    let measured_bounds = measured_bounds.unwrap_or_else(|| {
        (
            text.rect.x(),
            block_top,
            text.rect.x() + measured_width,
            block_top + block_height,
        )
    });
    let bounds = Rect::new(
        measured_bounds.0,
        measured_bounds.1,
        (measured_bounds.2 - measured_bounds.0).max(0.0),
        (measured_bounds.3 - measured_bounds.1).max(0.0),
    );

    Ok(ShapedTextLayout {
        glyphs,
        measurement: TextMeasurement {
            width: measured_width,
            height: block_height.max(ascent + descent),
            bounds,
        },
    })
}

fn tessellate_glyph(
    vertices: &mut Vec<Vertex>,
    face: &rustybuzz::Face<'_>,
    glyph: &ShapedGlyph,
    color: Color,
    transform: Transform,
    viewport: Size,
) -> Result<()> {
    let mut path_builder = LyonPath::builder();
    {
        let mut outline = GlyphOutlineBuilder {
            builder: &mut path_builder,
            transform,
            glyph: *glyph,
            contour_open: false,
        };
        if face
            .outline_glyph(GlyphId(glyph.glyph_id), &mut outline)
            .is_none()
        {
            return Ok(());
        }
        outline.finish();
    }

    let path = path_builder.build();
    tessellate_filled_lyon_path(vertices, &path, color, viewport)
}

fn append_painted_path(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    viewport: Size,
) -> Result<()> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(());
    }

    if state.visible_rect(path.bounds()).is_none() {
        return Ok(());
    }

    let lyon_path = build_lyon_path(path, state.current_transform);
    tessellate_filled_lyon_path(vertices, &lyon_path, color, viewport)
}

fn append_stroked_path(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    stroke: StrokeStyle,
    viewport: Size,
) -> Result<()> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(());
    }

    let line_width = stroke.width.max(1.0);
    if state
        .visible_rect(path.bounds().inflate(line_width * 0.5, line_width * 0.5))
        .is_none()
    {
        return Ok(());
    }

    let lyon_path = build_lyon_path(path, state.current_transform);
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = StrokeTessellator::new();
    tessellator
        .tessellate_path(
            &lyon_path,
            &StrokeOptions::default().with_line_width(line_width),
            &mut builder,
        )
        .map_err(|error| Error::new(format!("failed to tessellate stroked path: {error}")))?;

    append_indexed_triangles(vertices, &buffers, color, viewport);
    Ok(())
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

fn build_lyon_path(path: &ScenePath, transform: Transform) -> LyonPath {
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

    let rgba = color.clamped().to_array();
    for index in &buffers.indices {
        let position = buffers.vertices[*index as usize];
        let ndc = to_ndc(position[0], position[1], viewport);
        vertices.push(Vertex {
            position: [ndc[0], ndc[1]],
            color: rgba,
        });
    }
}

struct GlyphOutlineBuilder<'a, B>
where
    B: LyonPathBuilder,
{
    builder: &'a mut B,
    transform: Transform,
    glyph: ShapedGlyph,
    contour_open: bool,
}

impl<'a, B> GlyphOutlineBuilder<'a, B>
where
    B: LyonPathBuilder,
{
    fn point(&self, x: f32, y: f32) -> lyon_path::math::Point {
        let scene = self.transform.transform_point(Point::new(
            self.glyph.origin_x + (x * self.glyph.scale),
            self.glyph.origin_y - (y * self.glyph.scale),
        ));
        point(scene.x, scene.y)
    }

    fn finish(&mut self) {
        if self.contour_open {
            LyonPathBuilder::end(self.builder, true);
            self.contour_open = false;
        }
    }
}

impl<B> ttf_parser::OutlineBuilder for GlyphOutlineBuilder<'_, B>
where
    B: LyonPathBuilder,
{
    fn move_to(&mut self, x: f32, y: f32) {
        if self.contour_open {
            LyonPathBuilder::end(self.builder, true);
        }
        LyonPathBuilder::begin(self.builder, self.point(x, y), &[]);
        self.contour_open = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        LyonPathBuilder::line_to(self.builder, self.point(x, y), &[]);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        LyonPathBuilder::quadratic_bezier_to(
            self.builder,
            self.point(x1, y1),
            self.point(x, y),
            &[],
        );
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        LyonPathBuilder::cubic_bezier_to(
            self.builder,
            self.point(x1, y1),
            self.point(x2, y2),
            self.point(x, y),
            &[],
        );
    }

    fn close(&mut self) {
        self.finish();
    }
}

fn append_image_placeholder(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    source: &sui_scene::ImageSource,
    viewport: Size,
) {
    if rect.is_empty() {
        return;
    }

    let seed = source.image.get() as f32;
    let fallback = Color::rgba(
        ((seed * 0.17).sin() * 0.25) + 0.45,
        ((seed * 0.11).cos() * 0.20) + 0.45,
        ((seed * 0.07).sin() * 0.15) + 0.50,
        0.85,
    )
    .clamped();
    let fill = source
        .tint
        .map(|tint| tint.with_alpha((tint.alpha * 0.35).clamp(0.18, 0.65)))
        .unwrap_or(fallback);
    let border = source.tint.unwrap_or(Color::WHITE).with_alpha(0.95);

    append_painted_rect(vertices, state, rect, fill, viewport);
    append_stroke_rect(
        vertices,
        state,
        rect,
        border,
        StrokeStyle::new(1.5),
        viewport,
    );

    let inset = rect.inflate(-rect.width() * 0.18, -rect.height() * 0.18);
    if !inset.is_empty() {
        append_painted_rect(
            vertices,
            state,
            Rect::new(
                inset.x(),
                inset.y(),
                inset.width(),
                (inset.height() * 0.24).max(1.0),
            ),
            border.with_alpha(0.35),
            viewport,
        );
    }
}

fn append_stroke_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    stroke: StrokeStyle,
    viewport: Size,
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

    append_painted_rect(vertices, state, top, color, viewport);
    append_painted_rect(vertices, state, bottom, color, viewport);
    append_painted_rect(vertices, state, left, color, viewport);
    append_painted_rect(vertices, state, right, color, viewport);
}

fn append_painted_rect(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    rect: Rect,
    color: Color,
    viewport: Size,
) {
    if let Some(visible) = state.visible_rect(rect) {
        append_rect(vertices, visible, color, viewport);
    }
}

fn append_rect(vertices: &mut Vec<Vertex>, rect: Rect, color: Color, viewport: Size) {
    if rect.is_empty() || viewport.is_empty() {
        return;
    }

    let min = to_ndc(rect.x(), rect.y(), viewport);
    let max = to_ndc(rect.max_x(), rect.max_y(), viewport);
    let rgba = color.clamped().to_array();

    vertices.extend_from_slice(&[
        Vertex {
            position: [min[0], min[1]],
            color: rgba,
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
        },
        Vertex {
            position: [min[0], max[1]],
            color: rgba,
        },
        Vertex {
            position: [max[0], min[1]],
            color: rgba,
        },
        Vertex {
            position: [max[0], max[1]],
            color: rgba,
        },
    ]);
}

fn to_ndc(x: f32, y: f32, viewport: Size) -> [f32; 2] {
    [
        ((x / viewport.width) * 2.0) - 1.0,
        1.0 - ((y / viewport.height) * 2.0),
    ]
}

fn normalize_viewport(size: Size) -> Option<(u32, u32)> {
    if size.is_empty() {
        None
    } else {
        Some(normalize_surface_size(
            size.width as u32,
            size.height as u32,
        ))
    }
}

fn normalize_surface_size(width: u32, height: u32) -> (u32, u32) {
    (width.max(1), height.max(1))
}

fn preferred_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
}

fn configure_surface(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    size: (u32, u32),
) -> Result<wgpu::SurfaceConfiguration> {
    let mut config = surface
        .get_default_config(adapter, size.0, size.1)
        .ok_or_else(|| Error::new("wgpu adapter does not support presenting to this surface"))?;
    config.format = preferred_surface_format(&surface.get_capabilities(adapter).formats)
        .unwrap_or(config.format);
    surface.configure(device, &config);
    Ok(config)
}

const SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

#[cfg(test)]
mod tests {
    use super::{TextEngine, build_vertices, to_ndc};
    use std::sync::Arc;
    use sui_core::{Color, FontHandle, Path, Point, Rect, Size, Transform, WindowId};
    use sui_scene::{
        FontRegistry, RegisteredFont, Scene, SceneCommand, SceneFrame, StrokeStyle, TextRun,
        TextStyle,
    };

    fn load_test_font() -> RegisteredFont {
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();
        let families = [fontdb::Family::SansSerif];
        let font_id = font_db
            .query(&fontdb::Query {
                families: &families,
                weight: fontdb::Weight::NORMAL,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })
            .or_else(|| font_db.faces().next().map(|face| face.id))
            .expect("system font available for renderer tests");

        font_db
            .with_face_data(font_id, |font_data, face_index| {
                RegisteredFont::from_bytes(font_data.to_vec()).with_face_index(face_index)
            })
            .expect("font data should be readable from system font database")
    }

    #[test]
    fn build_vertices_applies_clip_and_transform_to_fill_rects() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::PushTransform {
            transform: Transform::translation(10.0, 5.0),
        });
        scene.push(SceneCommand::PushClip {
            rect: Rect::new(0.0, 0.0, 16.0, 12.0),
        });
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(4.0, 3.0, 20.0, 10.0),
            brush: Color::WHITE.into(),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(1),
                viewport: Size::new(100.0, 100.0),
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        let expected_min = to_ndc(14.0, 8.0, Size::new(100.0, 100.0));
        let expected_max = to_ndc(26.0, 17.0, Size::new(100.0, 100.0));

        assert_eq!(vertices.len(), 6);
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.position == expected_min)
        );
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.position == expected_max)
        );
    }

    #[test]
    fn build_vertices_supports_text_and_stroke_primitives() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 80.0, 24.0),
            text: "scene".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));
        scene.push(SceneCommand::StrokeRect {
            rect: Rect::new(2.0, 2.0, 20.0, 10.0),
            brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
            stroke: StrokeStyle::new(2.0),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(2),
                viewport: Size::new(100.0, 80.0),
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
        assert!(vertices.len() >= 30);
    }

    #[test]
    fn build_vertices_supports_non_rect_paths() {
        let mut triangle = Path::builder();
        triangle
            .move_to(Point::new(10.0, 10.0))
            .line_to(Point::new(40.0, 10.0))
            .line_to(Point::new(24.0, 36.0))
            .close();

        let mut curve = Path::builder();
        curve
            .move_to(Point::new(8.0, 44.0))
            .quad_to(Point::new(24.0, 24.0), Point::new(48.0, 44.0));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillPath {
            path: triangle.build(),
            brush: Color::rgba(0.2, 0.8, 0.4, 1.0).into(),
        });
        scene.push(SceneCommand::StrokePath {
            path: curve.build(),
            brush: Color::rgba(0.9, 0.4, 0.2, 1.0).into(),
            stroke: StrokeStyle::new(3.0),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(5),
                viewport: Size::new(80.0, 60.0),
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
        assert!(vertices.len() >= 12);
    }

    #[test]
    fn text_engine_shapes_text_with_font_metrics() {
        let text = TextRun {
            rect: Rect::new(8.0, 10.0, 160.0, 32.0),
            text: "office".to_string(),
            style: TextStyle::new(Color::WHITE),
        };

        let text_engine = TextEngine::new().unwrap();
        let layout = text_engine
            .shape_text_run(&text, &FontRegistry::new())
            .unwrap();

        assert!(!layout.glyphs.is_empty());
        assert!(layout.measurement.width > 0.0);
        assert!(layout.measurement.height >= text.style.font_size);
    }

    #[test]
    fn build_vertices_uses_registered_font_handle() {
        let handle = FontHandle::new(17);
        let mut fonts = FontRegistry::new();
        fonts.insert(handle, load_test_font());

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "registered".to_string(),
            style: TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(3),
                viewport: Size::new(160.0, 60.0),
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(fonts),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
    }

    #[test]
    fn build_vertices_errors_for_unregistered_font_handle() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "missing".to_string(),
            style: TextStyle {
                font: Some(FontHandle::new(404)),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let error = match build_vertices(
            &SceneFrame {
                window_id: WindowId::new(4),
                viewport: Size::new(160.0, 60.0),
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
            },
            &mut text_engine,
        ) {
            Ok(_) => panic!("expected missing font handle to fail during shaping"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("font handle 404 is not registered")
        );
    }
}
