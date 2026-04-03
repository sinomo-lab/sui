#![forbid(unsafe_code)]

use std::{collections::HashMap, fmt, sync::Arc};

use bytemuck::{Pod, Zeroable};
use lyon_path::{
    Path as LyonPath, PathEvent, builder::PathBuilder as LyonPathBuilder, iterator::PathIterator,
    math::point,
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeOptions,
    StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};
use sui_core::{
    Color, Error, ImageHandle, Path as ScenePath, PathElement, Point, Rect, Result, Size,
    Transform, Vector, WindowId,
};
use sui_scene::{
    Brush, RegisteredImage, RegisteredImageFormat, SceneCommand, SceneFrame, StrokeStyle,
};
use sui_text::{
    FontRegistry, ResolvedTextFace, ShapedGlyph as SceneShapedGlyph, ShapedText, TextLayout,
    TextLayoutCacheSnapshot, TextRun, TextStyle, TextSystem,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlyphCacheSnapshot {
    pub entries: usize,
    pub hits: usize,
    pub misses: usize,
}

impl GlyphCacheSnapshot {
    pub const fn requests(self) -> usize {
        self.hits + self.misses
    }

    pub fn hit_rate(self) -> f64 {
        let requests = self.requests();
        if requests == 0 {
            0.0
        } else {
            self.hits as f64 / requests as f64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererTextCacheSnapshot {
    pub layout: TextLayoutCacheSnapshot,
    pub glyph: GlyphCacheSnapshot,
}

pub struct WgpuRenderer {
    instance: wgpu::Instance,
    feather_width: f32,
    frames_rendered: usize,
    capabilities: RendererCapabilities,
    last_frames: HashMap<WindowId, SceneFrame>,
    shared: Option<SharedRenderer>,
    text_engine: Option<TextEngine>,
    image_cache: HashMap<ImageHandle, CachedImageTexture>,
    surfaces: HashMap<WindowId, SurfaceState>,
    offscreen_targets: HashMap<WindowId, OffscreenTarget>,
    frame_resources: FrameResources,
}

#[derive(Default)]
struct FrameResources {
    scene_vertices: Option<DynamicVertexBuffer>,
    clip_vertices: Option<DynamicVertexBuffer>,
    stencil: Option<StencilTarget>,
}

struct DynamicVertexBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
}

struct StencilTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        let expected_len = width as usize * height as usize * 4;
        if pixels.len() != expected_len {
            return Err(Error::new(format!(
                "RGBA image pixel buffer length {} does not match {}x{} image size",
                pixels.len(),
                width,
                height
            )));
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const DEFAULT_FEATHER_WIDTH: f32 = 1.0;
const AA_FLATTEN_TOLERANCE: f32 = 0.1;

impl WgpuRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.set_feather_width(feather_width);
        self
    }

    pub fn feather_width(&self) -> f32 {
        self.feather_width
    }

    pub fn set_feather_width(&mut self, feather_width: f32) {
        self.feather_width = feather_width.max(0.0);
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
        self.last_frames.remove(&window_id);
    }

    pub fn render(&mut self, frame: &SceneFrame) -> Result<()> {
        let viewport = normalize_framebuffer_size(frame.surface_size);

        if let Some(size) = viewport {
            if self.surfaces.contains_key(&frame.window_id) {
                self.render_surface(frame, size)?;
            } else {
                self.render_offscreen(frame, size)?;
            }
        }

        self.frames_rendered += 1;
        self.last_frames.insert(frame.window_id, frame.clone());
        Ok(())
    }

    pub fn capabilities(&self) -> RendererCapabilities {
        self.capabilities
    }

    pub fn frames_rendered(&self) -> usize {
        self.frames_rendered
    }

    pub fn last_frame(&self, window_id: WindowId) -> Option<&SceneFrame> {
        self.last_frames.get(&window_id)
    }

    pub fn text_cache_snapshot(&self) -> RendererTextCacheSnapshot {
        self.text_engine
            .as_ref()
            .map(TextEngine::cache_snapshot)
            .unwrap_or_default()
    }

    pub fn capture_rgba(&self, window_id: WindowId) -> Result<RgbaImage> {
        let shared = self
            .shared
            .as_ref()
            .ok_or_else(|| Error::new("renderer has not initialized a wgpu device yet"))?;
        let target = self.offscreen_targets.get(&window_id).ok_or_else(|| {
            Error::new(format!(
                "window {} does not have an offscreen render target available for screenshot capture",
                window_id.get()
            ))
        })?;

        let bytes_per_row = target.size.0 * 4;
        let padded_bytes_per_row = bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer_size = padded_bytes_per_row as u64 * target.size.1 as u64;
        let buffer = shared.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI screenshot readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SUI screenshot readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(target.size.1),
                },
            },
            wgpu::Extent3d {
                width: target.size.0,
                height: target.size.1,
                depth_or_array_layers: 1,
            },
        );
        shared.queue.submit([encoder.finish()]);

        let (sender, receiver) = std::sync::mpsc::channel();
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        shared
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| {
                Error::new(format!(
                    "failed to poll device for screenshot capture: {error}"
                ))
            })?;
        receiver
            .recv()
            .map_err(|error| {
                Error::new(format!(
                    "failed to receive screenshot readback completion: {error}"
                ))
            })?
            .map_err(|error| {
                Error::new(format!("failed to map screenshot readback buffer: {error}"))
            })?;

        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((target.size.0 * target.size.1 * 4) as usize);
        for row in 0..target.size.1 as usize {
            let start = row * padded_bytes_per_row as usize;
            let row_slice = &mapped[start..start + bytes_per_row as usize];
            for chunk in row_slice.chunks_exact(4) {
                pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
            }
        }
        drop(mapped);
        buffer.unmap();

        RgbaImage::new(target.size.0, target.size.1, pixels)
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

        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI image bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        self.shared = Some(SharedRenderer {
            adapter,
            device,
            queue,
            pipelines: HashMap::new(),
            image_bind_group_layout,
            image_sampler,
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
        let feather_width = self.feather_width;
        let draw_ops = {
            let text_engine = self.text_engine()?;
            build_draw_ops(frame, text_engine, feather_width)?
        };
        let framebuffer_size = normalize_framebuffer_size(frame.surface_size).unwrap_or((1, 1));
        let prepared = prepare_frame_batches(
            batch_draw_ops(draw_ops),
            frame.viewport,
            framebuffer_size,
        );

        let mut image_bind_groups = HashMap::new();
        for pass in &prepared.passes {
            for draw in &pass.draws {
                let DrawOpKind::Image { handle } = draw.kind else {
                    continue;
                };
                if image_bind_groups.contains_key(&handle) {
                    continue;
                }

                let image = frame.image_registry.get(handle).ok_or_else(|| {
                    Error::new(format!("image handle {} is not registered", handle.get()))
                })?;
                image_bind_groups.insert(handle, self.ensure_image_bind_group(handle, image)?);
            }
        }

        {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            self.frame_resources
                .ensure_scene_buffer(&shared.device, prepared.scene_vertices.len() as u64 * VERTEX_SIZE);
            if let Some(buffer) = self.frame_resources.scene_vertices.as_ref() {
                if !prepared.scene_vertices.is_empty() {
                    shared.queue.write_buffer(
                        &buffer.buffer,
                        0,
                        bytemuck::cast_slice(&prepared.scene_vertices),
                    );
                }
            }

            self.frame_resources
                .ensure_clip_buffer(&shared.device, prepared.clip_vertices.len() as u64 * VERTEX_SIZE);
            if let Some(buffer) = self.frame_resources.clip_vertices.as_ref() {
                if !prepared.clip_vertices.is_empty() {
                    shared.queue.write_buffer(
                        &buffer.buffer,
                        0,
                        bytemuck::cast_slice(&prepared.clip_vertices),
                    );
                }
            }

            if prepared.passes.iter().any(|pass| !pass.clip_paths.is_empty()) {
                self.frame_resources
                    .ensure_stencil(&shared.device, framebuffer_size);
            }
        }

        let mut encoder = {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            shared
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SUI scene encoder"),
                })
        };
        if prepared.passes.is_empty() {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SUI scene clear pass"),
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
        } else {
            let shared = self
                .shared
                .as_mut()
                .expect("renderer shared state initialized");
            let scene_buffer = self
                .frame_resources
                .scene_vertices
                .as_ref()
                .expect("scene buffer available when rendering batched passes");
            let clip_buffer = self.frame_resources.clip_vertices.as_ref();
            let stencil_view = self.frame_resources.stencil.as_ref().map(|target| {
                let _ = &target.texture;
                &target.view
            });

            for (index, pass) in prepared.passes.iter().enumerate() {
                let load_op = if index == 0 {
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    })
                } else {
                    wgpu::LoadOp::Load
                };
                let depth_stencil_attachment = if pass.clip_paths.is_empty() {
                    None
                } else {
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view: stencil_view
                            .expect("stencil view available for path-clipped pass"),
                        depth_ops: None,
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0),
                            store: wgpu::StoreOp::Store,
                        }),
                    })
                };
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SUI scene batch pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                });

                if !pass.clip_paths.is_empty() {
                    let clip_pipeline = shared.clip_pipeline(target_format);
                    render_pass.set_pipeline(clip_pipeline);
                    let clip_buffer = clip_buffer
                        .as_ref()
                        .expect("clip buffer available for path-clipped pass");
                    render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1);
                    for (clip_index, clip_path) in pass.clip_paths.iter().enumerate() {
                        render_pass.set_stencil_reference(clip_index as u32);
                        render_pass.set_vertex_buffer(
                            0,
                            vertex_buffer_slice(&clip_buffer.buffer, clip_path.vertices),
                        );
                        render_pass.draw(0..clip_path.vertices.len, 0..1);
                    }
                }

                let mut current_kind = None;
                for draw in &pass.draws {
                    match draw.clip_rect {
                        Some(scissor) => render_pass.set_scissor_rect(
                            scissor.x,
                            scissor.y,
                            scissor.width,
                            scissor.height,
                        ),
                        None => {
                            render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1)
                        }
                    }

                    if current_kind != Some(draw.kind) {
                        let pipeline = match (draw.kind, pass.clip_paths.is_empty()) {
                            (DrawOpKind::Solid, true) => shared.pipeline(target_format),
                            (DrawOpKind::Solid, false) => shared.clipped_pipeline(target_format),
                            (DrawOpKind::Image { .. }, true) => shared.image_pipeline(target_format),
                            (DrawOpKind::Image { .. }, false) => {
                                shared.clipped_image_pipeline(target_format)
                            }
                        };
                        render_pass.set_pipeline(pipeline);
                        current_kind = Some(draw.kind);
                    }

                    if !pass.clip_paths.is_empty() {
                        render_pass.set_stencil_reference(pass.clip_paths.len() as u32);
                    }

                    match draw.kind {
                        DrawOpKind::Solid => {}
                        DrawOpKind::Image { handle } => {
                            let bind_group = image_bind_groups
                                .get(&handle)
                                .expect("image bind group prepared before batched render pass");
                            render_pass.set_bind_group(0, bind_group, &[]);
                        }
                    }

                    render_pass.set_vertex_buffer(
                        0,
                        vertex_buffer_slice(&scene_buffer.buffer, draw.vertices),
                    );
                    render_pass.draw(0..draw.vertices.len, 0..1);
                }
            }
        }

        self.shared
            .as_ref()
            .expect("renderer shared state initialized")
            .queue
            .submit([encoder.finish()]);
        Ok(())
    }

    fn ensure_image_bind_group(
        &mut self,
        handle: ImageHandle,
        image: &RegisteredImage,
    ) -> Result<wgpu::BindGroup> {
        if let Some(cached) = self.image_cache.get(&handle) {
            return Ok(cached.bind_group.clone());
        }

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let texture_format = match image.format() {
            RegisteredImageFormat::Rgba8 => wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI image texture"),
            size: wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        shared.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image.bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width() * 4),
                rows_per_image: Some(image.height()),
            },
            wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI image bind group"),
            layout: &shared.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&shared.image_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });

        self.image_cache.insert(
            handle,
            CachedImageTexture {
                _texture: texture,
                _view: view,
                bind_group: bind_group.clone(),
            },
        );

        Ok(bind_group)
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
            feather_width: DEFAULT_FEATHER_WIDTH,
            frames_rendered: 0,
            capabilities: RendererCapabilities::default(),
            last_frames: HashMap::new(),
            shared: None,
            text_engine: None,
            image_cache: HashMap::new(),
            surfaces: HashMap::new(),
            offscreen_targets: HashMap::new(),
            frame_resources: FrameResources::default(),
        }
    }
}

impl fmt::Debug for WgpuRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WgpuRenderer")
            .field("feather_width", &self.feather_width)
            .field("frames_rendered", &self.frames_rendered)
            .field("capabilities", &self.capabilities)
            .field("last_frame_count", &self.last_frames.len())
            .field("has_device", &self.shared.is_some())
            .field("surface_count", &self.surfaces.len())
            .finish()
    }
}

impl FrameResources {
    fn ensure_scene_buffer(&mut self, device: &wgpu::Device, size: u64) {
        Self::ensure_dynamic_buffer(&mut self.scene_vertices, device, size, "SUI scene vertices");
    }

    fn ensure_clip_buffer(&mut self, device: &wgpu::Device, size: u64) {
        Self::ensure_dynamic_buffer(&mut self.clip_vertices, device, size, "SUI clip vertices");
    }

    fn ensure_dynamic_buffer(
        slot: &mut Option<DynamicVertexBuffer>,
        device: &wgpu::Device,
        required_size: u64,
        label: &str,
    ) {
        if required_size == 0 {
            return;
        }

        let needs_recreate = slot
            .as_ref()
            .is_none_or(|buffer| buffer.capacity < required_size);
        if !needs_recreate {
            return;
        }

        let capacity = next_dynamic_buffer_capacity(required_size);
        *slot = Some(DynamicVertexBuffer {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
        });
    }

    fn ensure_stencil(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let needs_recreate = self.stencil.as_ref().is_none_or(|target| target.size != size);
        if !needs_recreate {
            return;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI scene stencil"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.stencil = Some(StencilTarget {
            texture,
            view,
            size,
        });
    }
}

fn next_dynamic_buffer_capacity(required_size: u64) -> u64 {
    required_size.max(4096).next_power_of_two()
}

struct SharedRenderer {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: HashMap<(wgpu::TextureFormat, PipelineKind), wgpu::RenderPipeline>,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
}

impl SharedRenderer {
    fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Solid)
    }

    fn clipped_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Clipped)
    }

    fn clip_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::ClipMask)
    }

    fn image_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Textured)
    }

    fn clipped_image_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::TexturedClipped)
    }

    fn pipeline_for(
        &mut self,
        format: wgpu::TextureFormat,
        kind: PipelineKind,
    ) -> &wgpu::RenderPipeline {
        self.pipelines.entry((format, kind)).or_insert_with(|| {
            let shader_label = match kind {
                PipelineKind::Solid | PipelineKind::Clipped | PipelineKind::ClipMask => {
                    "SUI solid scene shader"
                }
                PipelineKind::Textured | PipelineKind::TexturedClipped => {
                    "SUI textured scene shader"
                }
            };
            let shader_source = match kind {
                PipelineKind::Solid | PipelineKind::Clipped | PipelineKind::ClipMask => {
                    SHADER_SOURCE
                }
                PipelineKind::Textured | PipelineKind::TexturedClipped => TEXTURED_SHADER_SOURCE,
            };
            let shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(shader_label),
                    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
                });

            let depth_stencil = match kind {
                PipelineKind::Solid | PipelineKind::Textured => None,
                PipelineKind::Clipped | PipelineKind::TexturedClipped => {
                    Some(wgpu::DepthStencilState {
                        format: STENCIL_FORMAT,
                        depth_write_enabled: Some(false),
                        depth_compare: Some(wgpu::CompareFunction::Always),
                        stencil: wgpu::StencilState {
                            front: wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::Equal,
                                fail_op: wgpu::StencilOperation::Keep,
                                depth_fail_op: wgpu::StencilOperation::Keep,
                                pass_op: wgpu::StencilOperation::Keep,
                            },
                            back: wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::Equal,
                                fail_op: wgpu::StencilOperation::Keep,
                                depth_fail_op: wgpu::StencilOperation::Keep,
                                pass_op: wgpu::StencilOperation::Keep,
                            },
                            read_mask: u32::MAX,
                            write_mask: 0,
                        },
                        bias: wgpu::DepthBiasState::default(),
                    })
                }
                PipelineKind::ClipMask => Some(wgpu::DepthStencilState {
                    format: STENCIL_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Equal,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::IncrementClamp,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Equal,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::IncrementClamp,
                        },
                        read_mask: u32::MAX,
                        write_mask: u32::MAX,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
            };
            let fragment_targets = [Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })];
            let layout = match kind {
                PipelineKind::Textured | PipelineKind::TexturedClipped => Some(
                    self.device
                        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("SUI textured scene pipeline layout"),
                            bind_group_layouts: &[Some(&self.image_bind_group_layout)],
                            immediate_size: 0,
                        }),
                ),
                PipelineKind::Solid | PipelineKind::Clipped | PipelineKind::ClipMask => None,
            };

            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(match kind {
                        PipelineKind::Solid => "SUI solid scene pipeline",
                        PipelineKind::Clipped => "SUI clipped scene pipeline",
                        PipelineKind::Textured => "SUI textured scene pipeline",
                        PipelineKind::TexturedClipped => "SUI clipped textured scene pipeline",
                        PipelineKind::ClipMask => "SUI clip mask pipeline",
                    }),
                    layout: layout.as_ref(),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[Vertex::layout()],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: match kind {
                        PipelineKind::ClipMask => None,
                        PipelineKind::Solid
                        | PipelineKind::Clipped
                        | PipelineKind::Textured
                        | PipelineKind::TexturedClipped => Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs_main"),
                            targets: &fragment_targets,
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                        }),
                    },
                    multiview_mask: None,
                    cache: None,
                })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PipelineKind {
    Solid,
    Clipped,
    Textured,
    TexturedClipped,
    ClipMask,
}

struct CachedImageTexture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
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
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
    tex_coords: [f32; 2],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32x2];

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

#[derive(Debug, Clone, Copy)]
struct MeshVertex {
    position: Point,
    color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphFaceCacheKey {
    data_ptr: usize,
    data_len: usize,
    face_index: u32,
}

impl GlyphFaceCacheKey {
    fn new(face: &ResolvedTextFace) -> Self {
        let data = face.shared_bytes();
        Self {
            data_ptr: data.as_ptr() as usize,
            data_len: data.len(),
            face_index: face.face_index(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    face: GlyphFaceCacheKey,
    glyph_id: u16,
    feather_width_bits: u32,
}

impl GlyphCacheKey {
    fn new(face: GlyphFaceCacheKey, glyph_id: u16, feather_width: f32) -> Self {
        Self {
            face,
            glyph_id,
            feather_width_bits: feather_width.to_bits(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CachedGlyphVertex {
    position: Point,
    coverage: f32,
}

#[derive(Debug, Default, Clone)]
struct CachedGlyphMesh {
    vertices: Vec<CachedGlyphVertex>,
    indices: Vec<u32>,
}

impl CachedGlyphMesh {
    fn push_vertex(&mut self, position: Point, coverage: f32) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(CachedGlyphVertex { position, coverage });
        index
    }

    fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

#[derive(Debug, Default, Clone)]
struct SceneMesh {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

impl SceneMesh {
    fn colored_vertex(&mut self, position: Point, color: Color) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(MeshVertex { position, color });
        index
    }

    fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

#[derive(Debug, Clone)]
struct FlattenedContour {
    points: Vec<Point>,
    closed: bool,
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

#[cfg(test)]
fn build_vertices(frame: &SceneFrame, text_engine: &mut TextEngine) -> Result<Vec<Vertex>> {
    let draw_ops = build_draw_ops(frame, text_engine, DEFAULT_FEATHER_WIDTH)?;
    let mut vertices = Vec::new();
    for op in draw_ops {
        vertices.extend(op.vertices);
    }
    Ok(vertices)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawOpKind {
    Solid,
    Image { handle: ImageHandle },
}

#[derive(Debug, Clone)]
struct DrawOp {
    kind: DrawOpKind,
    vertices: Vec<Vertex>,
    clip_rect: Option<Rect>,
    path_clip_state_id: u64,
    clip_paths: Vec<Vec<Vertex>>,
}

#[derive(Debug, Clone)]
struct DrawPassBatch {
    path_clip_state_id: u64,
    clip_paths: Vec<Vec<Vertex>>,
    draws: Vec<DrawBatch>,
}

#[derive(Debug, Clone)]
struct DrawBatch {
    kind: DrawOpKind,
    clip_rect: Option<Rect>,
    vertices: Vec<Vertex>,
}

#[derive(Debug, Clone)]
struct PreparedFrameBatches {
    scene_vertices: Vec<Vertex>,
    clip_vertices: Vec<Vertex>,
    passes: Vec<PreparedPassBatch>,
}

#[derive(Debug, Clone)]
struct PreparedPassBatch {
    clip_paths: Vec<PreparedClipPath>,
    draws: Vec<PreparedDrawBatch>,
}

#[derive(Debug, Clone, Copy)]
struct PreparedClipPath {
    vertices: PreparedVertices,
}

#[derive(Debug, Clone, Copy)]
struct PreparedDrawBatch {
    kind: DrawOpKind,
    clip_rect: Option<ScissorRect>,
    vertices: PreparedVertices,
}

#[derive(Debug, Clone, Copy)]
struct PreparedVertices {
    start: u32,
    len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScissorRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn batch_draw_ops(draw_ops: Vec<DrawOp>) -> Vec<DrawPassBatch> {
    let mut passes = Vec::new();

    for op in draw_ops {
        let share_pass = passes.last().is_some_and(|pass| can_share_pass(pass, &op));
        if !share_pass {
            passes.push(DrawPassBatch {
                path_clip_state_id: op.path_clip_state_id,
                clip_paths: op.clip_paths.clone(),
                draws: Vec::new(),
            });
        }

        let pass = passes
            .last_mut()
            .expect("pass batch created before draw batch insertion");
        if let Some(previous) = pass.draws.last_mut() {
            if previous.kind == op.kind && previous.clip_rect == op.clip_rect {
                previous.vertices.extend(op.vertices);
                continue;
            }
        }

        pass.draws.push(DrawBatch {
            kind: op.kind,
            clip_rect: op.clip_rect,
            vertices: op.vertices,
        });
    }

    passes
}

fn can_share_pass(pass: &DrawPassBatch, op: &DrawOp) -> bool {
    if pass.clip_paths.is_empty() && op.clip_paths.is_empty() {
        true
    } else {
        pass.path_clip_state_id == op.path_clip_state_id
    }
}

fn prepare_frame_batches(
    pass_batches: Vec<DrawPassBatch>,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> PreparedFrameBatches {
    let mut prepared = PreparedFrameBatches {
        scene_vertices: Vec::new(),
        clip_vertices: Vec::new(),
        passes: Vec::with_capacity(pass_batches.len()),
    };

    for pass in pass_batches {
        let mut prepared_pass = PreparedPassBatch {
            clip_paths: Vec::with_capacity(pass.clip_paths.len()),
            draws: Vec::with_capacity(pass.draws.len()),
        };

        for clip_path in pass.clip_paths {
            let start = prepared.clip_vertices.len() as u32;
            let len = clip_path.len() as u32;
            prepared.clip_vertices.extend(clip_path);
            prepared_pass.clip_paths.push(PreparedClipPath {
                vertices: PreparedVertices { start, len },
            });
        }

        for draw in pass.draws {
            let start = prepared.scene_vertices.len() as u32;
            let len = draw.vertices.len() as u32;
            prepared.scene_vertices.extend(draw.vertices);
            prepared_pass.draws.push(PreparedDrawBatch {
                kind: draw.kind,
                clip_rect: draw
                    .clip_rect
                    .and_then(|rect| rect_to_scissor(rect, viewport, framebuffer_size)),
                vertices: PreparedVertices { start, len },
            });
        }

        prepared.passes.push(prepared_pass);
    }

    prepared
}

fn build_draw_ops(
    frame: &SceneFrame,
    text_engine: &mut TextEngine,
    feather_width: f32,
) -> Result<Vec<DrawOp>> {
    let viewport = frame.viewport;
    let mut draw_ops = Vec::new();
    let mut state = SceneRasterState::default();

    for command in frame.scene.commands() {
        match command {
            SceneCommand::Clear(color) => {
                let mut vertices = Vec::new();
                append_rect(
                    &mut vertices,
                    Rect::new(0.0, 0.0, viewport.width, viewport.height),
                    *color,
                    viewport,
                );
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
            SceneCommand::FillRect { rect, brush } => {
                let Brush::Solid(color) = brush;
                let mut vertices = Vec::new();
                append_painted_rect(
                    &mut vertices,
                    &state,
                    *rect,
                    *color,
                    viewport,
                    feather_width,
                );
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
            SceneCommand::StrokeRect {
                rect,
                brush,
                stroke,
            } => {
                let Brush::Solid(color) = brush;
                let mut vertices = Vec::new();
                append_stroke_rect(
                    &mut vertices,
                    &state,
                    *rect,
                    *color,
                    *stroke,
                    viewport,
                    feather_width,
                );
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
            SceneCommand::FillPath { path, brush } => {
                let Brush::Solid(color) = brush;
                let mut vertices = Vec::new();
                append_painted_path(&mut vertices, &state, path, *color, viewport, feather_width)?;
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
            SceneCommand::StrokePath {
                path,
                brush,
                stroke,
            } => {
                let Brush::Solid(color) = brush;
                let mut vertices = Vec::new();
                append_stroked_path(
                    &mut vertices,
                    &state,
                    path,
                    *color,
                    *stroke,
                    viewport,
                    feather_width,
                )?;
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
            SceneCommand::DrawText(text) => {
                let mut vertices = Vec::new();
                text_engine.append_text_run(
                    &mut vertices,
                    &state,
                    text,
                    frame.font_registry.as_ref(),
                    viewport,
                    feather_width,
                )?;
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
            SceneCommand::DrawShapedText(text) => {
                let mut vertices = Vec::new();
                text_engine.append_shaped_text(
                    &mut vertices,
                    &state,
                    text,
                    viewport,
                    feather_width,
                )?;
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
            SceneCommand::DrawImage { rect, source } => {
                let mut vertices = Vec::new();
                let image = frame.image_registry.get(source.image).ok_or_else(|| {
                    Error::new(format!(
                        "image handle {} is not registered",
                        source.image.get()
                    ))
                })?;
                append_image(&mut vertices, &state, *rect, source, image, viewport);
                push_draw_op(
                    &mut draw_ops,
                    DrawOpKind::Image {
                        handle: source.image,
                    },
                    vertices,
                    &state,
                );
            }
            SceneCommand::PushClip { rect } => {
                state.push_clip(*rect);
            }
            SceneCommand::PushClipPath { path } => {
                state.push_clip_path(path, viewport)?;
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
                let mut vertices = Vec::new();
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
                    feather_width,
                )?;
                push_draw_op(&mut draw_ops, DrawOpKind::Solid, vertices, &state);
            }
        }
    }

    Ok(draw_ops)
}

#[derive(Debug, Clone)]
struct SceneRasterState {
    current_transform: Transform,
    transform_stack: Vec<Transform>,
    clip_stack: Vec<ClipPrimitive>,
    path_clip_state_id: u64,
}

impl Default for SceneRasterState {
    fn default() -> Self {
        Self {
            current_transform: Transform::IDENTITY,
            transform_stack: Vec::new(),
            clip_stack: Vec::new(),
            path_clip_state_id: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum ClipPrimitive {
    Rect(Rect),
    Path { bounds: Rect, vertices: Vec<Vertex> },
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
    fn push_clip(&mut self, rect: Rect) {
        let transformed = self.current_transform.transform_rect_bbox(rect);
        self.clip_stack.push(ClipPrimitive::Rect(transformed));
    }

    fn push_clip_path(&mut self, path: &ScenePath, viewport: Size) -> Result<()> {
        let bounds = self.current_transform.transform_rect_bbox(path.bounds());
        let vertices = if path.is_empty() || viewport.is_empty() {
            Vec::new()
        } else {
            let lyon_path = build_lyon_path(path, self.current_transform);
            tessellate_filled_lyon_path_vertices(&lyon_path, viewport)?
        };
        self.clip_stack
            .push(ClipPrimitive::Path { bounds, vertices });
        self.path_clip_state_id = self.path_clip_state_id.wrapping_add(1);
        Ok(())
    }

    fn pop_clip(&mut self) {
        if matches!(self.clip_stack.pop(), Some(ClipPrimitive::Path { .. })) {
            self.path_clip_state_id = self.path_clip_state_id.wrapping_add(1);
        }
    }

    fn push_transform(&mut self, transform: Transform) {
        self.transform_stack.push(self.current_transform);
        self.current_transform = self.current_transform.then(transform);
    }

    fn pop_transform(&mut self) {
        self.current_transform = self.transform_stack.pop().unwrap_or(Transform::IDENTITY);
    }

    fn current_clip_bounds(&self) -> Option<Rect> {
        let mut clips = self.clip_stack.iter().map(ClipPrimitive::bounds);
        let first = clips.next()?;
        Some(clips.fold(first, |current, clip| {
            current.intersection(clip).unwrap_or(Rect::ZERO)
        }))
    }

    fn current_path_clips(&self) -> Vec<Vec<Vertex>> {
        self.clip_stack
            .iter()
            .filter_map(|clip| match clip {
                ClipPrimitive::Rect(_) => None,
                ClipPrimitive::Path { vertices, .. } => Some(vertices.clone()),
            })
            .collect()
    }

    fn visible_rect(&self, rect: Rect) -> Option<Rect> {
        let transformed = self.current_transform.transform_rect_bbox(rect);

        match self.current_clip_bounds() {
            Some(clip) => transformed.intersection(clip),
            None => Some(transformed),
        }
    }
}

#[derive(Debug, Default)]
struct TextEngine {
    system: TextSystem,
    glyph_cache: HashMap<GlyphCacheKey, CachedGlyphMesh>,
    glyph_cache_hits: usize,
    glyph_cache_misses: usize,
}

impl TextEngine {
    fn new() -> Result<Self> {
        Ok(Self::default())
    }

    fn append_text_run(
        &mut self,
        vertices: &mut Vec<Vertex>,
        state: &SceneRasterState,
        text: &TextRun,
        font_registry: &FontRegistry,
        viewport: Size,
        feather_width: f32,
    ) -> Result<()> {
        if text.rect.is_empty() || text.text.is_empty() || viewport.is_empty() {
            return Ok(());
        }

        let layout = self.shape_text_run(text, font_registry)?;
        self.append_text_layout(
            vertices,
            state,
            Point::new(text.rect.x(), text.rect.y()),
            &layout,
            viewport,
            feather_width,
        )
    }

    fn append_shaped_text(
        &mut self,
        vertices: &mut Vec<Vertex>,
        state: &SceneRasterState,
        text: &ShapedText,
        viewport: Size,
        feather_width: f32,
    ) -> Result<()> {
        if viewport.is_empty() {
            return Ok(());
        }

        self.append_text_layout(
            vertices,
            state,
            text.origin,
            &text.layout,
            viewport,
            feather_width,
        )
    }

    fn append_text_layout(
        &mut self,
        vertices: &mut Vec<Vertex>,
        state: &SceneRasterState,
        origin: Point,
        layout: &TextLayout,
        viewport: Size,
        feather_width: f32,
    ) -> Result<()> {
        if layout.measurement().width <= 0.0 || layout.measurement().height <= 0.0 {
            return Ok(());
        }

        let translated_bounds = layout.measurement().bounds.translate(origin.to_vector());
        if state.visible_rect(translated_bounds).is_none() {
            return Ok(());
        }

        let face = rustybuzz::Face::from_slice(layout.face().bytes(), layout.face().face_index())
            .ok_or_else(|| Error::new("failed to parse shaped text face data"))?;
        let layout_rect = Rect::from_origin_size(origin, layout.box_size());
        let face_key = GlyphFaceCacheKey::new(layout.face());

        for glyph in layout.glyphs() {
            if let Some(bounds) = glyph
                .bounds
                .map(|bounds| bounds.translate(origin.to_vector()))
            {
                if bounds.intersection(layout_rect).is_none() {
                    continue;
                }

                if let Some(clip) = state.current_clip_bounds() {
                    let transformed = state.current_transform.transform_rect_bbox(bounds);
                    if transformed.intersection(clip).is_none() {
                        continue;
                    }
                }
            }

            let mut translated_glyph = glyph.clone();
            translated_glyph.origin_x += origin.x;
            translated_glyph.origin_y += origin.y;
            if let Some(bounds) = translated_glyph.bounds {
                translated_glyph.bounds = Some(bounds.translate(origin.to_vector()));
            }

            if let Some(mesh) = self.cached_glyph_mesh(face_key, &face, glyph.glyph_id, feather_width)? {
                append_cached_glyph_mesh(
                    vertices,
                    mesh,
                    &translated_glyph,
                    layout.style().color,
                    state.current_transform,
                    viewport,
                );
            }
        }

        Ok(())
    }

    fn shape_text_run(&self, text: &TextRun, font_registry: &FontRegistry) -> Result<TextLayout> {
        self.system.shape_text_run(text, font_registry)
    }

    fn cached_glyph_mesh(
        &mut self,
        face_key: GlyphFaceCacheKey,
        face: &rustybuzz::Face<'_>,
        glyph_id: u16,
        feather_width: f32,
    ) -> Result<Option<&CachedGlyphMesh>> {
        let key = GlyphCacheKey::new(face_key, glyph_id, feather_width);
        if self.glyph_cache.contains_key(&key) {
            self.glyph_cache_hits += 1;
            return Ok(self.glyph_cache.get(&key));
        }

        self.glyph_cache_misses += 1;
        let Some(mesh) = build_cached_glyph_mesh(face, glyph_id, feather_width)? else {
            return Ok(None);
        };

        self.glyph_cache.insert(key.clone(), mesh);
        Ok(self.glyph_cache.get(&key))
    }

    #[cfg(test)]
    fn glyph_cache_stats(&self) -> (usize, usize, usize) {
        (
            self.glyph_cache.len(),
            self.glyph_cache_hits,
            self.glyph_cache_misses,
        )
    }

    fn cache_snapshot(&self) -> RendererTextCacheSnapshot {
        RendererTextCacheSnapshot {
            layout: self.system.layout_cache_snapshot(),
            glyph: GlyphCacheSnapshot {
                entries: self.glyph_cache.len(),
                hits: self.glyph_cache_hits,
                misses: self.glyph_cache_misses,
            },
        }
    }
}

fn build_cached_glyph_mesh(
    face: &rustybuzz::Face<'_>,
    glyph_id: u16,
    feather_width: f32,
) -> Result<Option<CachedGlyphMesh>> {
    let mut path_builder = LyonPath::builder();
    {
        let mut outline = CachedGlyphOutlineBuilder {
            builder: &mut path_builder,
            contour_open: false,
        };
        if face.outline_glyph(GlyphId(glyph_id), &mut outline).is_none() {
            return Ok(None);
        }
        outline.finish();
    }

    let path = path_builder.build();
    Ok(Some(build_local_glyph_mesh(&path, feather_width)?))
}

fn build_local_glyph_mesh(path: &LyonPath, feather_width: f32) -> Result<CachedGlyphMesh> {
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

fn append_cached_glyph_mesh(
    vertices: &mut Vec<Vertex>,
    mesh: &CachedGlyphMesh,
    glyph: &SceneShapedGlyph,
    color: Color,
    transform: Transform,
    viewport: Size,
) {
    let color = color.clamped();
    for index in &mesh.indices {
        let vertex = mesh.vertices[*index as usize];
        let positioned = Point::new(
            glyph.origin_x + (vertex.position.x * glyph.scale),
            glyph.origin_y + (vertex.position.y * glyph.scale),
        );
        let transformed = transform.transform_point(positioned);
        let ndc = to_ndc(transformed.x, transformed.y, viewport);
        vertices.push(Vertex {
            position: ndc,
            color: color.with_alpha(color.alpha * vertex.coverage).to_array(),
            tex_coords: [0.0, 0.0],
        });
    }
}

fn append_painted_path(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    viewport: Size,
    feather_width: f32,
) -> Result<()> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(());
    }

    if state.visible_rect(path.bounds()).is_none() {
        return Ok(());
    }

    let lyon_path = build_lyon_path(path, state.current_transform);
    append_filled_aa_lyon_path(vertices, &lyon_path, color, viewport, feather_width)
}

fn append_stroked_path(
    vertices: &mut Vec<Vertex>,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    stroke: StrokeStyle,
    viewport: Size,
    feather_width: f32,
) -> Result<()> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(());
    }

    let line_width = stroke.width.max(1.0);
    if state
        .visible_rect(path.bounds().inflate(
            (line_width + feather_width) * 0.5,
            (line_width + feather_width) * 0.5,
        ))
        .is_none()
    {
        return Ok(());
    }

    let lyon_path = build_lyon_path(path, state.current_transform);
    append_feathered_stroke(
        vertices,
        &lyon_path,
        color,
        line_width,
        viewport,
        feather_width,
    );
    Ok(())
}

fn append_filled_aa_lyon_path(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    viewport: Size,
    feather_width: f32,
) -> Result<()> {
    tessellate_filled_lyon_path(vertices, path, color, viewport)?;
    append_fill_fringe(vertices, path, color, viewport, feather_width);
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

fn tessellate_filled_lyon_path_vertices(path: &LyonPath, viewport: Size) -> Result<Vec<Vertex>> {
    let mut vertices = Vec::new();
    tessellate_filled_lyon_path(
        &mut vertices,
        path,
        Color::rgba(0.0, 0.0, 0.0, 0.0),
        viewport,
    )?;
    Ok(vertices)
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
            tex_coords: [0.0, 0.0],
        });
    }
}

struct CachedGlyphOutlineBuilder<'a, B>
where
    B: LyonPathBuilder,
{
    builder: &'a mut B,
    contour_open: bool,
}

impl<'a, B> CachedGlyphOutlineBuilder<'a, B>
where
    B: LyonPathBuilder,
{
    fn point(&self, x: f32, y: f32) -> lyon_path::math::Point {
        point(x, -y)
    }

    fn finish(&mut self) {
        if self.contour_open {
            LyonPathBuilder::end(self.builder, true);
            self.contour_open = false;
        }
    }
}

impl<B> ttf_parser::OutlineBuilder for CachedGlyphOutlineBuilder<'_, B>
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

fn append_painted_rect(
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
    draw_ops: &mut Vec<DrawOp>,
    kind: DrawOpKind,
    vertices: Vec<Vertex>,
    state: &SceneRasterState,
) {
    if vertices.is_empty() {
        return;
    }

    draw_ops.push(DrawOp {
        kind,
        vertices,
        clip_rect: state.current_clip_bounds(),
        path_clip_state_id: state.path_clip_state_id,
        clip_paths: state.current_path_clips(),
    });
}

const VERTEX_SIZE: u64 = std::mem::size_of::<Vertex>() as u64;

fn vertex_buffer_slice(
    buffer: &wgpu::Buffer,
    vertices: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = vertices.start as u64 * VERTEX_SIZE;
    let end = start + vertices.len as u64 * VERTEX_SIZE;
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

    let min_x = (rect.x().max(0.0) * scale_x)
        .floor()
        .clamp(0.0, framebuffer_width as f32) as u32;
    let min_y = (rect.y().max(0.0) * scale_y)
        .floor()
        .clamp(0.0, framebuffer_height as f32) as u32;
    let max_x = ((rect.x() + rect.width()).min(viewport.width) * scale_x)
        .ceil()
        .clamp(0.0, framebuffer_width as f32) as u32;
    let max_y = ((rect.y() + rect.height()).min(viewport.height) * scale_y)
        .ceil()
        .clamp(0.0, framebuffer_height as f32) as u32;

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

fn to_ndc(x: f32, y: f32, viewport: Size) -> [f32; 2] {
    [
        ((x / viewport.width) * 2.0) - 1.0,
        1.0 - ((y / viewport.height) * 2.0),
    ]
}

fn normalize_framebuffer_size(size: Size) -> Option<(u32, u32)> {
    if size.is_empty() {
        None
    } else {
        Some(normalize_surface_size(
            size.width.round() as u32,
            size.height.round() as u32,
        ))
    }
}

fn normalize_surface_size(width: u32, height: u32) -> (u32, u32) {
    (width.max(1), height.max(1))
}

fn append_fill_fringe(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    viewport: Size,
    feather_width: f32,
) {
    if feather_width <= 0.0 {
        return;
    }

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

        append_fill_fringe_for_contour(vertices, &aa_points, color, viewport, feather_width);
    }
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

fn append_feathered_stroke(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    line_width: f32,
    viewport: Size,
    feather_width: f32,
) {
    if feather_width <= 0.0 {
        append_hard_stroked_lyon_path(vertices, path, color, line_width, viewport);
        return;
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

        append_stroke_contour(
            vertices,
            &aa_points,
            path_type,
            color,
            line_width,
            viewport,
            feather_width,
        );
    }
}

fn append_hard_stroked_lyon_path(
    vertices: &mut Vec<Vertex>,
    path: &LyonPath,
    color: Color,
    line_width: f32,
    viewport: Size,
) {
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut builder = BuffersBuilder::new(&mut buffers, TessellatedPoint);
    let mut tessellator = StrokeTessellator::new();
    if tessellator
        .tessellate_path(
            path,
            &StrokeOptions::default().with_line_width(line_width),
            &mut builder,
        )
        .is_ok()
    {
        append_indexed_triangles(vertices, &buffers, color, viewport);
    }
}

fn append_stroke_contour(
    vertices: &mut Vec<Vertex>,
    path: &[AaPathPoint],
    path_type: FeatheredPathType,
    color: Color,
    line_width: f32,
    viewport: Size,
    feather_width: f32,
) {
    if feather_width <= 0.0 {
        return;
    }

    let n = path.len() as u32;
    if n < 2 || viewport.is_empty() || line_width <= 0.0 {
        return;
    }

    let transparent = Color::TRANSPARENT;
    let mut mesh = SceneMesh::default();

    let thin_line = line_width <= 0.9 * feather_width;
    if thin_line {
        let opacity = (line_width / feather_width).clamp(0.0, 1.0);
        let mid_color = multiply_color_alpha(color, opacity);
        let mut previous_base = 0;

        for (index, point) in path.iter().enumerate() {
            let outer = mesh.colored_vertex(
                offset_point(point.position, scale_vector(point.normal, feather_width)),
                transparent,
            );
            let middle = mesh.colored_vertex(point.position, mid_color);
            let inner = mesh.colored_vertex(
                offset_point(point.position, scale_vector(point.normal, -feather_width)),
                transparent,
            );

            if path_type == FeatheredPathType::Closed || index > 0 {
                mesh.add_triangle(previous_base + 0, previous_base + 1, outer);
                mesh.add_triangle(previous_base + 1, outer, middle);
                mesh.add_triangle(previous_base + 1, previous_base + 2, middle);
                mesh.add_triangle(previous_base + 2, middle, inner);
            }

            previous_base = outer;
        }

        if path_type == FeatheredPathType::Closed {
            mesh.add_triangle(previous_base + 0, previous_base + 1, 0);
            mesh.add_triangle(previous_base + 1, 0, 1);
            mesh.add_triangle(previous_base + 1, previous_base + 2, 1);
            mesh.add_triangle(previous_base + 2, 1, 2);
        }
    } else {
        let inner_radius = 0.5 * (line_width - feather_width);
        let outer_radius = 0.5 * (line_width + feather_width);

        match path_type {
            FeatheredPathType::Closed => {
                let mut previous_base = 0;

                for (index, point) in path.iter().enumerate() {
                    let outer_pos = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, outer_radius)),
                        transparent,
                    );
                    let inner_pos = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, inner_radius)),
                        color,
                    );
                    let inner_neg = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -inner_radius)),
                        color,
                    );
                    let outer_neg = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -outer_radius)),
                        transparent,
                    );

                    if index > 0 {
                        mesh.add_triangle(previous_base + 0, previous_base + 1, outer_pos);
                        mesh.add_triangle(previous_base + 1, outer_pos, inner_pos);
                        mesh.add_triangle(previous_base + 1, previous_base + 2, inner_pos);
                        mesh.add_triangle(previous_base + 2, inner_pos, inner_neg);
                        mesh.add_triangle(previous_base + 2, previous_base + 3, inner_neg);
                        mesh.add_triangle(previous_base + 3, inner_neg, outer_neg);
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
                let first_base = mesh.colored_vertex(
                    offset_point(
                        offset_point(first.position, scale_vector(first.normal, outer_radius)),
                        first_extrude,
                    ),
                    transparent,
                );
                mesh.colored_vertex(
                    offset_point(first.position, scale_vector(first.normal, inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(first.position, scale_vector(first.normal, -inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(
                        offset_point(first.position, scale_vector(first.normal, -outer_radius)),
                        first_extrude,
                    ),
                    transparent,
                );
                mesh.add_triangle(first_base + 0, first_base + 1, first_base + 2);
                mesh.add_triangle(first_base + 0, first_base + 2, first_base + 3);

                let mut previous_base = first_base;
                for point in path.iter().skip(1).take(path.len().saturating_sub(2)) {
                    let base = mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, outer_radius)),
                        transparent,
                    );
                    mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, inner_radius)),
                        color,
                    );
                    mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -inner_radius)),
                        color,
                    );
                    mesh.colored_vertex(
                        offset_point(point.position, scale_vector(point.normal, -outer_radius)),
                        transparent,
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
                let last_base = mesh.colored_vertex(
                    offset_point(
                        offset_point(last.position, scale_vector(last.normal, outer_radius)),
                        last_extrude,
                    ),
                    transparent,
                );
                mesh.colored_vertex(
                    offset_point(last.position, scale_vector(last.normal, inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(last.position, scale_vector(last.normal, -inner_radius)),
                    color,
                );
                mesh.colored_vertex(
                    offset_point(
                        offset_point(last.position, scale_vector(last.normal, -outer_radius)),
                        last_extrude,
                    ),
                    transparent,
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

    append_scene_mesh(vertices, &mesh, viewport);
}

fn append_scene_mesh(vertices: &mut Vec<Vertex>, mesh: &SceneMesh, viewport: Size) {
    for index in &mesh.indices {
        let vertex = mesh.vertices[*index as usize];
        let ndc = to_ndc(vertex.position.x, vertex.position.y, viewport);
        vertices.push(Vertex {
            position: ndc,
            color: vertex.color.clamped().to_array(),
            tex_coords: [0.0, 0.0],
        });
    }
}

fn flatten_path_contours(path: &LyonPath) -> Vec<FlattenedContour> {
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

fn multiply_color_alpha(color: Color, factor: f32) -> Color {
    color.with_alpha(color.alpha * factor)
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

const TEXTURED_SHADER_SOURCE: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
};

@group(0) @binding(0)
var image_sampler: sampler;

@group(0) @binding(1)
var image_texture: texture_2d<f32>;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.tex_coords = tex_coords;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, in.tex_coords) * in.color;
}
"#;

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_FEATHER_WIDTH, DrawBatch, DrawOp, DrawOpKind, DrawPassBatch, ScissorRect,
        TextEngine, Vertex, WgpuRenderer, batch_draw_ops, build_draw_ops, build_vertices,
        prepare_frame_batches, to_ndc,
    };
    use std::sync::Arc;
    use sui_core::{Color, FontHandle, ImageHandle, Path, Point, Rect, Size, Transform, WindowId};
    use sui_scene::{
        ImageRegistry, ImageSource, RegisteredImage, Scene, SceneCommand, SceneFrame, StrokeStyle,
    };
    use sui_text::{FontRegistry, RegisteredFont, ShapedText, TextRun, TextStyle, TextSystem};

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
                surface_size: Size::new(100.0, 100.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        let expected_min = to_ndc(14.0, 8.0, Size::new(100.0, 100.0));
        let expected_max = to_ndc(26.0, 17.0, Size::new(100.0, 100.0));

        assert!(vertices.len() > 6);
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
        assert!(vertices.iter().any(|vertex| vertex.color[3] == 0.0));
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
                surface_size: Size::new(100.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
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
                surface_size: Size::new(80.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
        assert!(vertices.len() >= 12);
    }

    #[test]
    fn build_draw_ops_carries_active_path_clips() {
        let mut clip = Path::builder();
        clip.move_to(Point::new(8.0, 8.0))
            .line_to(Point::new(32.0, 8.0))
            .line_to(Point::new(20.0, 28.0))
            .close();

        let mut scene = Scene::new();
        scene.push(SceneCommand::PushClipPath { path: clip.build() });
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            brush: Color::WHITE.into(),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let ops = build_draw_ops(
            &SceneFrame {
                window_id: WindowId::new(6),
                viewport: Size::new(64.0, 64.0),
                surface_size: Size::new(64.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
            DEFAULT_FEATHER_WIDTH,
        )
        .unwrap();

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].path_clip_state_id, 1);
        assert_eq!(ops[0].clip_paths.len(), 1);
        assert!(!ops[0].clip_paths[0].is_empty());
        assert_eq!(ops[0].clip_rect, Some(Rect::new(8.0, 8.0, 24.0, 20.0)));
    }

    #[test]
    fn batch_draw_ops_merges_consecutive_matching_state() {
        let vertices = vec![
            Vertex {
                position: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                tex_coords: [0.0, 0.0],
            };
            3
        ];

        let passes = batch_draw_ops(vec![
            DrawOp {
                kind: DrawOpKind::Solid,
                vertices: vertices.clone(),
                clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                path_clip_state_id: 0,
                clip_paths: Vec::new(),
            },
            DrawOp {
                kind: DrawOpKind::Solid,
                vertices,
                clip_rect: Some(Rect::new(2.0, 4.0, 20.0, 10.0)),
                path_clip_state_id: 0,
                clip_paths: Vec::new(),
            },
        ]);

        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].draws.len(), 1);
        assert_eq!(passes[0].draws[0].vertices.len(), 6);
    }

    #[test]
    fn prepare_frame_batches_converts_clip_rects_to_scissors() {
        let prepared = prepare_frame_batches(
            vec![DrawPassBatch {
                path_clip_state_id: 0,
                clip_paths: Vec::new(),
                draws: vec![DrawBatch {
                    kind: DrawOpKind::Solid,
                    clip_rect: Some(Rect::new(5.0, 8.0, 20.0, 10.0)),
                    vertices: vec![
                        Vertex {
                            position: [0.0, 0.0],
                            color: [1.0, 1.0, 1.0, 1.0],
                            tex_coords: [0.0, 0.0],
                        };
                        6
                    ],
                }],
            }],
            Size::new(50.0, 40.0),
            (100, 80),
        );

        assert_eq!(prepared.passes.len(), 1);
        assert_eq!(
            prepared.passes[0].draws[0].clip_rect,
            Some(ScissorRect {
                x: 10,
                y: 16,
                width: 40,
                height: 20,
            })
        );
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

        assert!(!layout.glyphs().is_empty());
        assert!(layout.measurement().width > 0.0);
        assert!(layout.measurement().height >= text.style.font_size);
    }

    #[test]
    fn build_vertices_supports_pre_shaped_text() {
        let layout = TextSystem::new()
            .shape_text(
                "scene",
                Size::new(80.0, 24.0),
                TextStyle::new(Color::WHITE),
                &FontRegistry::new(),
            )
            .unwrap();

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawShapedText(ShapedText {
            origin: Point::new(4.0, 6.0),
            layout,
        }));

        let mut text_engine = TextEngine::new().unwrap();
        let vertices = build_vertices(
            &SceneFrame {
                window_id: WindowId::new(11),
                viewport: Size::new(100.0, 80.0),
                surface_size: Size::new(100.0, 80.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
        )
        .unwrap();

        assert!(!vertices.is_empty());
    }

    #[test]
    fn text_engine_reuses_cached_glyph_meshes_across_repeated_builds() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(4.0, 6.0, 120.0, 28.0),
            text: "abc".to_string(),
            style: TextStyle::new(Color::WHITE),
        }));

        let frame = SceneFrame {
            window_id: WindowId::new(12),
            viewport: Size::new(160.0, 60.0),
            surface_size: Size::new(160.0, 60.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
        };

        let mut text_engine = TextEngine::new().unwrap();
        let first = build_vertices(&frame, &mut text_engine).unwrap();
        assert!(!first.is_empty());
        assert_eq!(text_engine.glyph_cache_stats(), (3, 0, 3));

        let second = build_vertices(&frame, &mut text_engine).unwrap();
        assert_eq!(first.len(), second.len());
        assert!(first.iter().zip(&second).all(|(left, right)| {
            left.position == right.position
                && left.color == right.color
                && left.tex_coords == right.tex_coords
        }));
        assert_eq!(text_engine.glyph_cache_stats(), (3, 3, 3));
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
                surface_size: Size::new(160.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(fonts),
                image_registry: Arc::new(ImageRegistry::new()),
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
                surface_size: Size::new(160.0, 60.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
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

    #[test]
    fn build_draw_ops_uses_registered_image_handle() {
        let handle = ImageHandle::new(23);
        let mut images = ImageRegistry::new();
        images.insert(
            handle,
            RegisteredImage::from_rgba8(
                2,
                2,
                vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                ],
            )
            .unwrap(),
        );

        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawImage {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            source: ImageSource::new(handle),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let ops = build_draw_ops(
            &SceneFrame {
                window_id: WindowId::new(7),
                viewport: Size::new(96.0, 64.0),
                surface_size: Size::new(96.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(images),
            },
            &mut text_engine,
            DEFAULT_FEATHER_WIDTH,
        )
        .unwrap();

        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0].kind, DrawOpKind::Image { handle: value } if value == handle));
        assert_eq!(ops[0].vertices.len(), 6);
    }

    #[test]
    fn build_draw_ops_errors_for_unregistered_image_handle() {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawImage {
            rect: Rect::new(4.0, 6.0, 32.0, 24.0),
            source: ImageSource::new(ImageHandle::new(88)),
        });

        let mut text_engine = TextEngine::new().unwrap();
        let error = build_draw_ops(
            &SceneFrame {
                window_id: WindowId::new(8),
                viewport: Size::new(96.0, 64.0),
                surface_size: Size::new(96.0, 64.0),
                scale_factor: 1.0,
                dirty_regions: Vec::new(),
                scene,
                font_registry: Arc::new(FontRegistry::new()),
                image_registry: Arc::new(ImageRegistry::new()),
            },
            &mut text_engine,
            DEFAULT_FEATHER_WIDTH,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("image handle 88 is not registered")
        );
    }

    #[test]
    fn renderer_feather_width_is_configurable() {
        let mut renderer = WgpuRenderer::new().with_feather_width(2.5);

        assert_eq!(renderer.feather_width(), 2.5);

        renderer.set_feather_width(-3.0);

        assert_eq!(renderer.feather_width(), 0.0);
    }
}
