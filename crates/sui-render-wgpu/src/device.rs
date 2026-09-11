use crate::WgpuRenderer;
use crate::gpu::SharedRenderer;
use crate::gpu::TextAtlasQuadVertex;
use crate::resources::create_text_atlas_array_bind_group_layout;
use std::collections::HashMap;
use sui_core::Error;
use sui_core::Result;
use wgpu::util::DeviceExt;

pub(crate) fn optional_renderer_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    let mut features = wgpu::Features::empty();
    if adapter
        .features()
        .contains(wgpu::Features::DUAL_SOURCE_BLENDING)
    {
        features |= wgpu::Features::DUAL_SOURCE_BLENDING;
    }
    features
}

impl SharedRenderer {
    pub(crate) fn new(adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Self {
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
        let text_atlas_array_bind_group_layout = create_text_atlas_array_bind_group_layout(&device);
        let image_linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI linear image sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let image_nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI nearest image sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            lod_max_clamp: 0.0,
            ..Default::default()
        });
        let text_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SUI text atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let text_quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SUI text atlas quad"),
            contents: bytemuck::cast_slice(&TextAtlasQuadVertex::unit_quad()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let analytic_path_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI analytic path bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let output_transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SUI output transform bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let dual_source_blending_enabled = device
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);

        Self {
            adapter,
            device,
            queue,
            pipelines: HashMap::new(),
            image_bind_group_layout,
            text_atlas_array_bind_group_layout,
            analytic_path_bind_group_layout,
            output_transform_bind_group_layout,
            image_linear_sampler,
            image_nearest_sampler,
            text_atlas_sampler,
            text_quad_buffer,
            dual_source_blending_enabled,
        }
    }
}
impl WgpuRenderer {
    pub(crate) fn install_shared(&mut self, shared: SharedRenderer) {
        if let Some(registry) = &self.external_texture_registry {
            registry.attach(
                shared.device.clone(),
                shared.queue.clone(),
                shared.adapter.get_info(),
            );
        }
        self.shared = Some(shared);
    }

    pub(crate) fn ensure_shared(
        &mut self,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<()> {
        if self.shared.is_some() {
            return Ok(());
        }

        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface,
                apply_limit_buckets: false,
            }))
            .map_err(|error| Error::new(format!("failed to acquire wgpu adapter: {error}")))?;

        let required_features = optional_renderer_features(&adapter);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SUI renderer device"),
            required_features,
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|error| Error::new(format!("failed to create wgpu device: {error}")))?;

        self.install_shared(SharedRenderer::new(adapter, device, queue));

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn initialize_async(
        &mut self,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<()> {
        if self.shared.is_some() {
            return Ok(());
        }

        let adapter = self
            .instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|error| Error::new(format!("failed to acquire wgpu adapter: {error}")))?;

        let required_features = optional_renderer_features(&adapter);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("SUI renderer device"),
                required_features,
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|error| Error::new(format!("failed to create wgpu device: {error}")))?;

        self.install_shared(SharedRenderer::new(adapter, device, queue));

        Ok(())
    }
}

/// Build the renderer's wgpu instance with the GL backend left out unless the
/// environment explicitly asks for it (`WGPU_BACKEND=gl`).
///
/// Initializing the GL/WGL backend spins up a device thread per instance, and
/// concurrent initialization — one renderer per test in the headless harness —
/// deadlocks inside some Windows OpenGL drivers (observed as all test threads
/// parked on `wgl::create_instance_device` results while the creator threads
/// sat inside `nvoglv64.dll`; the same churn also produces access-violation
/// crashes). The primary native backends (Vulkan/Metal/DX12) cover every
/// platform SUI supports, so GL is opt-in rather than eagerly probed.
pub(crate) fn default_wgpu_instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    if std::env::var_os("WGPU_BACKEND").is_none() {
        descriptor.backends -= wgpu::Backends::GL;
    }
    wgpu::Instance::new(descriptor)
}
