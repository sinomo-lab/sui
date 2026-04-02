#![forbid(unsafe_code)]

use std::{collections::HashMap, fmt, sync::Arc};

use bytemuck::{Pod, Zeroable};
use sui_core::{Color, Error, Rect, Result, Size, WindowId};
use sui_scene::{Brush, SceneCommand, SceneFrame};
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
        let shared = self
            .shared
            .as_mut()
            .expect("renderer shared state initialized");
        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SUI scene encoder"),
            });
        let vertices = build_vertices(frame);
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

fn build_vertices(frame: &SceneFrame) -> Vec<Vertex> {
    let viewport = frame.viewport;
    let mut vertices = Vec::new();

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
                append_rect(&mut vertices, *rect, *color, viewport);
            }
            SceneCommand::Label { rect, text, color } => {
                append_label(&mut vertices, *rect, text, *color, viewport);
            }
        }
    }

    vertices
}

fn append_label(vertices: &mut Vec<Vertex>, rect: Rect, text: &str, color: Color, viewport: Size) {
    if rect.is_empty() {
        return;
    }

    let visible_chars = text.chars().filter(|ch| !ch.is_whitespace()).count().max(1) as f32;
    let full_width = (visible_chars * 10.0).min(rect.width());
    let primary = Rect::new(
        rect.x(),
        rect.y() + rect.height() * 0.2,
        full_width,
        rect.height() * 0.22,
    );
    let secondary = Rect::new(
        rect.x(),
        rect.y() + rect.height() * 0.58,
        (full_width * 0.7).max(rect.width() * 0.25),
        rect.height() * 0.18,
    );

    append_rect(vertices, primary, color, viewport);
    append_rect(
        vertices,
        secondary,
        color.with_alpha((color.alpha * 0.8).clamp(0.0, 1.0)),
        viewport,
    );
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
