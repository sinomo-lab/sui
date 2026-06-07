use super::*;

pub(crate) fn grow_analytic_path_capacity(current: usize, required: usize) -> usize {
    if required == 0 {
        return current;
    }

    let target = required.max(16);
    if current >= target {
        current
    } else {
        target.checked_next_power_of_two().unwrap_or(target)
    }
}

pub(crate) fn analytic_path_buffer_size<T>(capacity: usize) -> u64 {
    capacity.max(1) as u64 * std::mem::size_of::<T>() as u64
}

pub(crate) struct SharedRenderer {
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) pipelines: HashMap<(wgpu::TextureFormat, PipelineKind), wgpu::RenderPipeline>,
    pub(crate) image_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) text_atlas_array_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) analytic_path_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) output_transform_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) image_linear_sampler: wgpu::Sampler,
    pub(crate) image_nearest_sampler: wgpu::Sampler,
    pub(crate) text_atlas_sampler: wgpu::Sampler,
    pub(crate) text_quad_buffer: wgpu::Buffer,
    pub(crate) dual_source_blending_enabled: bool,
}

impl SharedRenderer {
    pub(crate) fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Solid)
    }

    pub(crate) fn clipped_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Clipped)
    }

    pub(crate) fn clip_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::ClipMask)
    }

    pub(crate) fn image_pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::Textured)
    }

    pub(crate) fn clipped_image_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::TexturedClipped)
    }

    pub(crate) fn text_atlas_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::TextAtlas)
    }

    pub(crate) fn clipped_text_atlas_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::TextAtlasClipped)
    }

    pub(crate) fn analytic_path_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::AnalyticPath)
    }

    pub(crate) fn clipped_analytic_path_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::AnalyticPathClipped)
    }

    pub(crate) fn widget_shader_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::WidgetShader)
    }

    pub(crate) fn clipped_widget_shader_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::WidgetShaderClipped)
    }

    pub(crate) fn rounded_rect_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::RoundedRect)
    }

    pub(crate) fn clipped_rounded_rect_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::RoundedRectClipped)
    }

    pub(crate) fn gradient_rect_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::GradientRect)
    }

    pub(crate) fn clipped_gradient_rect_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::GradientRectClipped)
    }

    pub(crate) fn output_transform_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipeline_for(format, PipelineKind::OutputTransform)
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
                PipelineKind::TextAtlas | PipelineKind::TextAtlasClipped => "SUI text atlas shader",
                PipelineKind::AnalyticPath | PipelineKind::AnalyticPathClipped => {
                    "SUI analytic path shader"
                }
                PipelineKind::WidgetShader | PipelineKind::WidgetShaderClipped => {
                    "SUI widget shader"
                }
                PipelineKind::RoundedRect | PipelineKind::RoundedRectClipped => {
                    "SUI rounded rect shader"
                }
                PipelineKind::GradientRect | PipelineKind::GradientRectClipped => {
                    "SUI gradient rect shader"
                }
                PipelineKind::OutputTransform => "SUI output transform shader",
            };
            let shader_source = match kind {
                PipelineKind::Solid | PipelineKind::Clipped | PipelineKind::ClipMask => {
                    SHADER_SOURCE
                }
                PipelineKind::Textured | PipelineKind::TexturedClipped => TEXTURED_SHADER_SOURCE,
                PipelineKind::TextAtlas | PipelineKind::TextAtlasClipped
                    if self.dual_source_blending_enabled =>
                {
                    TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE
                }
                PipelineKind::TextAtlas | PipelineKind::TextAtlasClipped => {
                    TEXT_ATLAS_SHADER_SOURCE
                }
                PipelineKind::AnalyticPath | PipelineKind::AnalyticPathClipped => {
                    ANALYTIC_PATH_SHADER_SOURCE
                }
                PipelineKind::WidgetShader | PipelineKind::WidgetShaderClipped => {
                    WIDGET_SHADER_SOURCE
                }
                PipelineKind::RoundedRect | PipelineKind::RoundedRectClipped => {
                    ROUNDED_RECT_SHADER_SOURCE
                }
                PipelineKind::GradientRect | PipelineKind::GradientRectClipped => {
                    GRADIENT_RECT_SHADER_SOURCE
                }
                PipelineKind::OutputTransform => OUTPUT_TRANSFORM_SHADER_SOURCE,
            };
            let shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(shader_label),
                    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
                });

            let depth_stencil = match kind {
                PipelineKind::Solid
                | PipelineKind::Textured
                | PipelineKind::TextAtlas
                | PipelineKind::AnalyticPath
                | PipelineKind::WidgetShader
                | PipelineKind::RoundedRect
                | PipelineKind::GradientRect
                | PipelineKind::OutputTransform => None,
                PipelineKind::Clipped
                | PipelineKind::TexturedClipped
                | PipelineKind::TextAtlasClipped
                | PipelineKind::AnalyticPathClipped
                | PipelineKind::WidgetShaderClipped
                | PipelineKind::RoundedRectClipped
                | PipelineKind::GradientRectClipped => Some(wgpu::DepthStencilState {
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
                }),
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
            let blend = match kind {
                PipelineKind::TextAtlas | PipelineKind::TextAtlasClipped
                    if self.dual_source_blending_enabled =>
                {
                    wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Src1,
                            dst_factor: wgpu::BlendFactor::OneMinusSrc1,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }
                }
                PipelineKind::TextAtlas | PipelineKind::TextAtlasClipped => wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                },
                PipelineKind::OutputTransform => wgpu::BlendState::REPLACE,
                PipelineKind::Solid
                | PipelineKind::Clipped
                | PipelineKind::Textured
                | PipelineKind::TexturedClipped
                | PipelineKind::AnalyticPath
                | PipelineKind::AnalyticPathClipped
                | PipelineKind::WidgetShader
                | PipelineKind::WidgetShaderClipped
                | PipelineKind::RoundedRect
                | PipelineKind::RoundedRectClipped
                | PipelineKind::GradientRect
                | PipelineKind::GradientRectClipped
                | PipelineKind::ClipMask => wgpu::BlendState::ALPHA_BLENDING,
            };
            let fragment_targets = [Some(wgpu::ColorTargetState {
                format,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })];
            let layout = match kind {
                PipelineKind::Textured
                | PipelineKind::TexturedClipped
                | PipelineKind::TextAtlas
                | PipelineKind::TextAtlasClipped => {
                    let is_text = matches!(
                        kind,
                        PipelineKind::TextAtlas | PipelineKind::TextAtlasClipped
                    );
                    // Text samples a texture_2d_array (one layer per atlas page); images use a
                    // plain texture_2d. They therefore need different bind group layouts.
                    let bind_group_layout = if is_text {
                        &self.text_atlas_array_bind_group_layout
                    } else {
                        &self.image_bind_group_layout
                    };
                    Some(
                        self.device
                            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                                label: Some(if is_text {
                                    "SUI text atlas pipeline layout"
                                } else {
                                    "SUI textured scene pipeline layout"
                                }),
                                bind_group_layouts: &[Some(bind_group_layout)],
                                immediate_size: 0,
                            }),
                    )
                }
                PipelineKind::AnalyticPath | PipelineKind::AnalyticPathClipped => Some(
                    self.device
                        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("SUI analytic path pipeline layout"),
                            bind_group_layouts: &[Some(&self.analytic_path_bind_group_layout)],
                            immediate_size: 0,
                        }),
                ),
                PipelineKind::OutputTransform => Some(self.device.create_pipeline_layout(
                    &wgpu::PipelineLayoutDescriptor {
                        label: Some("SUI output transform pipeline layout"),
                        bind_group_layouts: &[Some(&self.output_transform_bind_group_layout)],
                        immediate_size: 0,
                    },
                )),
                PipelineKind::Solid
                | PipelineKind::Clipped
                | PipelineKind::WidgetShader
                | PipelineKind::WidgetShaderClipped
                | PipelineKind::RoundedRect
                | PipelineKind::RoundedRectClipped
                | PipelineKind::GradientRect
                | PipelineKind::GradientRectClipped
                | PipelineKind::ClipMask => None,
            };
            let scene_vertex_layouts = [Vertex::layout()];
            let text_vertex_layouts = [TextAtlasQuadVertex::layout(), TextAtlasInstance::layout()];
            let vertex_buffers = match kind {
                PipelineKind::TextAtlas | PipelineKind::TextAtlasClipped => {
                    &text_vertex_layouts[..]
                }
                PipelineKind::OutputTransform => &[][..],
                _ => &scene_vertex_layouts[..],
            };

            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(match kind {
                        PipelineKind::Solid => "SUI solid scene pipeline",
                        PipelineKind::Clipped => "SUI clipped scene pipeline",
                        PipelineKind::Textured => "SUI textured scene pipeline",
                        PipelineKind::TexturedClipped => "SUI clipped textured scene pipeline",
                        PipelineKind::TextAtlas => "SUI text atlas pipeline",
                        PipelineKind::TextAtlasClipped => "SUI clipped text atlas pipeline",
                        PipelineKind::AnalyticPath => "SUI analytic path pipeline",
                        PipelineKind::AnalyticPathClipped => "SUI clipped analytic path pipeline",
                        PipelineKind::WidgetShader => "SUI widget shader pipeline",
                        PipelineKind::WidgetShaderClipped => "SUI clipped widget shader pipeline",
                        PipelineKind::RoundedRect => "SUI rounded rect pipeline",
                        PipelineKind::RoundedRectClipped => "SUI clipped rounded rect pipeline",
                        PipelineKind::GradientRect => "SUI gradient rect pipeline",
                        PipelineKind::GradientRectClipped => "SUI clipped gradient rect pipeline",
                        PipelineKind::ClipMask => "SUI clip mask pipeline",
                        PipelineKind::OutputTransform => "SUI output transform pipeline",
                    }),
                    layout: layout.as_ref(),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: vertex_buffers,
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
                        | PipelineKind::TexturedClipped
                        | PipelineKind::TextAtlas
                        | PipelineKind::TextAtlasClipped
                        | PipelineKind::AnalyticPath
                        | PipelineKind::AnalyticPathClipped
                        | PipelineKind::WidgetShader
                        | PipelineKind::WidgetShaderClipped
                        | PipelineKind::RoundedRect
                        | PipelineKind::RoundedRectClipped
                        | PipelineKind::GradientRect
                        | PipelineKind::GradientRectClipped
                        | PipelineKind::OutputTransform => Some(wgpu::FragmentState {
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
pub(crate) enum PipelineKind {
    Solid,
    Clipped,
    Textured,
    TexturedClipped,
    TextAtlas,
    TextAtlasClipped,
    AnalyticPath,
    AnalyticPathClipped,
    WidgetShader,
    WidgetShaderClipped,
    RoundedRect,
    RoundedRectClipped,
    GradientRect,
    GradientRectClipped,
    ClipMask,
    OutputTransform,
}

pub(crate) struct CachedImageTexture {
    pub(crate) texture: wgpu::Texture,
    pub(crate) _view: wgpu::TextureView,
    pub(crate) linear_bind_group: wgpu::BindGroup,
    pub(crate) nearest_bind_group: wgpu::BindGroup,
    pub(crate) image: sui_scene::RegisteredImage,
}

pub(crate) struct CachedTextAtlasTexture {
    pub(crate) texture: wgpu::Texture,
    pub(crate) _view: wgpu::TextureView,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) size: (u32, u32),
    /// Number of array layers currently allocated (grows on demand up to the page budget).
    pub(crate) layers: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TextAtlasBindGroupStats {
    pub(crate) total_time_us: u64,
    pub(crate) upload_copy_time_us: u64,
    pub(crate) upload_write_time_us: u64,
    pub(crate) upload_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct AnalyticPathBindGroupStats {
    pub(crate) total_time_us: u64,
    pub(crate) upload_bytes: u64,
    pub(crate) miss_count: usize,
}

pub(crate) struct CachedAnalyticPathGpu {
    pub(crate) data: Arc<AnalyticPathCpuData>,
    pub(crate) slot: u32,
    pub(crate) last_used_frame: usize,
}

pub(crate) struct SurfaceState {
    pub(crate) window: Arc<Window>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) display_capabilities: DisplayCapabilities,
    pub(crate) color_management: ColorManagementMode,
    pub(crate) output_strategy: OutputStrategy,
    pub(crate) available_surface_formats: Vec<wgpu::TextureFormat>,
}

pub(crate) struct OffscreenTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) size: (u32, u32),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct Vertex {
    pub(crate) position: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) tex_coords: [f32; 2],
    pub(crate) shader_params: [f32; 4],
    pub(crate) shader_params2: [f32; 4], // radii [tl, tr, br, bl]
    pub(crate) shader_params3: [f32; 4], // [border_w, shadow_sigma, shadow_dx_local, shadow_dy_local]
    pub(crate) shader_params4: [f32; 4], // border_color rgba (linear)
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x4,
        2 => Float32x2,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
    ];

    pub(crate) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub(crate) fn basic(
        position: [f32; 2],
        color: [f32; 4],
        tex_coords: [f32; 2],
        shader_params: [f32; 4],
    ) -> Self {
        Self {
            position,
            color,
            tex_coords,
            shader_params,
            shader_params2: [0.0; 4],
            shader_params3: [0.0; 4],
            shader_params4: [0.0; 4],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct TextAtlasQuadVertex {
    pub(crate) local_pos: [f32; 2],
}

impl TextAtlasQuadVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];

    pub(crate) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub(crate) fn unit_quad() -> [Self; 6] {
        [
            Self {
                local_pos: [0.0, 0.0],
            },
            Self {
                local_pos: [1.0, 0.0],
            },
            Self {
                local_pos: [0.0, 1.0],
            },
            Self {
                local_pos: [0.0, 1.0],
            },
            Self {
                local_pos: [1.0, 0.0],
            },
            Self {
                local_pos: [1.0, 1.0],
            },
        ]
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct TextAtlasInstance {
    pub(crate) top_left: [f32; 2],
    pub(crate) x_axis: [f32; 2],
    pub(crate) y_axis: [f32; 2],
    pub(crate) uv_min: [f32; 2],
    pub(crate) uv_max: [f32; 2],
    pub(crate) color: [f32; 4],
    pub(crate) metadata: [f32; 2],
    /// Atlas page == texture-array layer this glyph lives on. Sampled in the fragment shader
    /// once the multi-page texture array goes live (Phase 3).
    pub(crate) layer: u32,
}

impl TextAtlasInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x2,
        5 => Float32x2,
        6 => Float32x4,
        7 => Float32x2,
        8 => Uint32
    ];

    pub(crate) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TessellatedPoint;

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
