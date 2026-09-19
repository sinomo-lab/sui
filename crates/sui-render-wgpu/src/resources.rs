use crate::WgpuRenderer;
use crate::draw::ImageBindGroupKey;
use crate::draw::ImageRasterSize;
use crate::draw::PreparedAnalyticPathResources;
use crate::geometry::AnalyticContourGpu;
use crate::geometry::AnalyticPathCpuData;
use crate::geometry::AnalyticPathMetaGpu;
use crate::geometry::AnalyticPointGpu;
use crate::gpu::SharedRenderer;
use crate::gpu::analytic_path_buffer_size;
use crate::gpu::grow_analytic_path_capacity;
use crate::text::TEXT_ATLAS_MAX_PAGES;
use crate::text_engine::TextEngine;
use crate::uploads::{FragmentBuffers, GpuUploads};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use sui_core::Error;
use sui_core::ImageHandle;
use sui_core::Result;
use sui_scene::ImageSampling;
use sui_scene::RegisteredExternalImage;
use sui_scene::RegisteredImage;
use sui_scene::RegisteredImageFormat;
use web_time::Instant;

#[derive(Default)]
pub(crate) struct FrameResources {
    pub(crate) stencil: Option<StencilTarget>,
    pub(crate) analytic_path_arena: AnalyticPathArena,
    pub(crate) uploads: GpuUploads,
    pub(crate) fragments:
        HashMap<sui_core::WindowId, HashMap<crate::retained::RetainedPacketId, FragmentBuffers>>,
    pub(crate) output_transforms: HashMap<sui_core::WindowId, crate::output::CachedOutputTransform>,
}

#[derive(Default)]
pub(crate) struct AnalyticPathArena {
    pub(crate) bind_group: Option<wgpu::BindGroup>,
    pub(crate) meta_buffer: Option<wgpu::Buffer>,
    pub(crate) contour_buffer: Option<wgpu::Buffer>,
    pub(crate) point_buffer: Option<wgpu::Buffer>,
    pub(crate) meta_capacity: usize,
    pub(crate) contour_capacity: usize,
    pub(crate) point_capacity: usize,
    pub(crate) used_slots: usize,
    pub(crate) used_contours: usize,
    pub(crate) used_points: usize,
}

pub(crate) struct StencilTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) size: (u32, u32),
}

pub(crate) const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
pub(crate) const DEFAULT_FEATHER_WIDTH: f32 = 0.0;

pub(crate) struct ImageMipLevel {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

pub(crate) fn image_mip_level_count(width: u32, height: u32) -> u32 {
    width.max(height).max(1).ilog2() + 1
}

pub(crate) fn image_mip_chain(image: &RegisteredImage, mipmapped: bool) -> Vec<ImageMipLevel> {
    let mut levels = vec![ImageMipLevel {
        width: image.width(),
        height: image.height(),
        pixels: image.bytes().to_vec(),
    }];
    if !mipmapped {
        return levels;
    }

    while let Some(previous) = levels.last() {
        if previous.width == 1 && previous.height == 1 {
            break;
        }
        levels.push(downsample_rgba8_srgb(previous));
    }
    levels
}

pub(crate) fn downsample_rgba8_srgb(source: &ImageMipLevel) -> ImageMipLevel {
    let width = (source.width / 2).max(1);
    let height = (source.height / 2).max(1);
    let mut pixels = vec![0; width as usize * height as usize * 4];

    for y in 0..height {
        for x in 0..width {
            let mut premultiplied = [0.0f32; 3];
            let mut alpha_sum = 0.0f32;
            let mut sample_count = 0.0f32;
            let source_y_start = y * source.height / height;
            let source_y_end = ((y + 1) * source.height / height).max(source_y_start + 1);
            let source_x_start = x * source.width / width;
            let source_x_end = ((x + 1) * source.width / width).max(source_x_start + 1);
            for source_y in source_y_start..source_y_end {
                for source_x in source_x_start..source_x_end {
                    let offset = ((source_y * source.width + source_x) * 4) as usize;
                    let alpha = source.pixels[offset + 3] as f32 / 255.0;
                    for (channel, premultiplied_channel) in premultiplied.iter_mut().enumerate() {
                        *premultiplied_channel +=
                            srgb_u8_to_linear(source.pixels[offset + channel]) * alpha;
                    }
                    alpha_sum += alpha;
                    sample_count += 1.0;
                }
            }

            let output = ((y * width + x) * 4) as usize;
            let alpha = alpha_sum / sample_count.max(1.0);
            if alpha_sum > f32::EPSILON {
                for (channel, premultiplied_channel) in premultiplied.iter().enumerate() {
                    pixels[output + channel] =
                        linear_to_srgb_u8(*premultiplied_channel / alpha_sum);
                }
            }
            pixels[output + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    ImageMipLevel {
        width,
        height,
        pixels,
    }
}

pub(crate) fn srgb_u8_to_linear(value: u8) -> f32 {
    let value = value as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn linear_to_srgb_u8(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Bind group layout for the multi-page glyph atlas: a filtering sampler plus a
/// `texture_2d_array` (one layer per atlas page). Distinct from the image layout, which is `D2`.
pub(crate) fn create_text_atlas_array_bind_group_layout(
    device: &wgpu::Device,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("SUI text atlas array bind group layout"),
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
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}
impl WgpuRenderer {
    pub(crate) fn registered_image_data_identity_eq(
        left: &RegisteredImage,
        right: &RegisteredImage,
    ) -> bool {
        left.width() == right.width()
            && left.height() == right.height()
            && left.format() == right.format()
            && left.bytes().len() == right.bytes().len()
            && std::ptr::addr_eq(left.bytes().as_ptr(), right.bytes().as_ptr())
    }

    pub(crate) fn write_registered_image_texture(
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        image: &RegisteredImage,
        mipmapped: bool,
    ) {
        if !mipmapped {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
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
            return;
        }

        let mip_chain = image_mip_chain(image, mipmapped);
        for (mip_level, level) in mip_chain.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: mip_level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level.width * 4),
                    rows_per_image: Some(level.height),
                },
                wgpu::Extent3d {
                    width: level.width,
                    height: level.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub(crate) fn ensure_image_bind_group(
        &mut self,
        key: ImageBindGroupKey,
        image: &RegisteredImage,
    ) -> Result<wgpu::BindGroup> {
        let mipmapped = !image.is_svg() && image.mipmaps_enabled();
        let texture_key = ImageTextureCacheKey {
            handle: key.handle,
            raster_size: image.is_svg().then_some(key.raster_size),
            mipmapped,
        };
        if let Some(cached) = self.image_cache.get_mut(&texture_key) {
            cached.last_used_frame = self.frames_rendered;
            if Self::registered_image_data_identity_eq(&cached.image, image) {
                return Ok(Self::image_bind_group_for_sampling(cached, key.sampling));
            }
        }

        let rasterized;
        let upload_image = if image.is_svg() {
            rasterized = image
                .rasterize_svg_at_size(key.raster_size.width, key.raster_size.height)?
                .ok_or_else(|| Error::new("registered SVG lost its retained vector source"))?;
            &rasterized
        } else {
            image
        };

        if let Some(cached) = self.image_cache.get_mut(&texture_key)
            && cached.texture.width() == upload_image.width()
            && cached.texture.height() == upload_image.height()
            && cached.image.format() == image.format()
        {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            Self::write_registered_image_texture(
                &shared.queue,
                &cached.texture,
                upload_image,
                mipmapped,
            );
            cached.image = image.clone();
            return Ok(Self::image_bind_group_for_sampling(cached, key.sampling));
        }

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let texture_format = match upload_image.format() {
            RegisteredImageFormat::Rgba8 => wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        let mip_level_count = if mipmapped {
            image_mip_level_count(upload_image.width(), upload_image.height())
        } else {
            1
        };
        let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI image texture"),
            size: wgpu::Extent3d {
                width: upload_image.width(),
                height: upload_image.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        Self::write_registered_image_texture(&shared.queue, &texture, upload_image, mipmapped);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let linear_bind_group =
            Self::create_image_bind_group(shared, &view, &shared.image_linear_sampler);
        let nearest_bind_group =
            Self::create_image_bind_group(shared, &view, &shared.image_nearest_sampler);
        let bind_group = match key.sampling {
            ImageSampling::Nearest => nearest_bind_group.clone(),
            ImageSampling::Linear => linear_bind_group.clone(),
        };

        self.image_cache.insert(
            texture_key,
            CachedImageTexture {
                texture,
                _view: view,
                linear_bind_group,
                nearest_bind_group,
                image: image.clone(),
                last_used_frame: self.frames_rendered,
            },
        );

        Ok(bind_group)
    }

    pub(crate) fn ensure_external_image_bind_group(
        &mut self,
        handle: ImageHandle,
        sampling: ImageSampling,
        descriptor: RegisteredExternalImage,
    ) -> Result<wgpu::BindGroup> {
        let entry = self
            .external_texture_registry
            .as_ref()
            .and_then(|registry| registry.resolve(handle))
            .ok_or_else(|| {
                Error::new(format!(
                    "external image handle {} has no WGPU texture registered",
                    handle.get()
                ))
            })?;
        if entry.descriptor != descriptor {
            return Err(Error::new(format!(
                "external image handle {} scene dimensions {}x{} do not match registered texture dimensions {}x{}",
                handle.get(),
                descriptor.width(),
                descriptor.height(),
                entry.descriptor.width(),
                entry.descriptor.height()
            )));
        }

        if let Some(cached) = self.external_image_cache.get(&handle)
            && cached.binding_revision == entry.binding_revision
        {
            return Ok(Self::external_image_bind_group_for_sampling(
                cached, sampling,
            ));
        }

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let linear_bind_group =
            Self::create_image_bind_group(shared, &entry.view, &shared.image_linear_sampler);
        let nearest_bind_group =
            Self::create_image_bind_group(shared, &entry.view, &shared.image_nearest_sampler);
        let bind_group = match sampling {
            ImageSampling::Nearest => nearest_bind_group.clone(),
            ImageSampling::Linear => linear_bind_group.clone(),
        };
        self.external_image_cache.insert(
            handle,
            CachedExternalTextureBindGroup {
                binding_revision: entry.binding_revision,
                linear_bind_group,
                nearest_bind_group,
            },
        );
        Ok(bind_group)
    }

    pub(crate) fn image_bind_group_for_sampling(
        cached: &CachedImageTexture,
        sampling: ImageSampling,
    ) -> wgpu::BindGroup {
        match sampling {
            ImageSampling::Nearest => cached.nearest_bind_group.clone(),
            ImageSampling::Linear => cached.linear_bind_group.clone(),
        }
    }

    pub(crate) fn external_image_bind_group_for_sampling(
        cached: &CachedExternalTextureBindGroup,
        sampling: ImageSampling,
    ) -> wgpu::BindGroup {
        match sampling {
            ImageSampling::Nearest => cached.nearest_bind_group.clone(),
            ImageSampling::Linear => cached.linear_bind_group.clone(),
        }
    }

    pub(crate) fn create_image_bind_group(
        shared: &SharedRenderer,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI image bind group"),
            layout: &shared.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
            ],
        })
    }

    pub(crate) fn ensure_text_atlas_bind_group(
        &mut self,
        text_engine: &mut TextEngine,
        collect_stats: bool,
    ) -> Result<(wgpu::BindGroup, TextAtlasBindGroupStats)> {
        let total_started = collect_stats.then(Instant::now);
        let upload_copy_started = collect_stats.then(Instant::now);
        let uploads = text_engine.take_atlas_uploads();
        let page_size = text_engine.atlas.page_size();
        let page_count = text_engine.atlas.page_count() as u32;
        let mut stats = TextAtlasBindGroupStats {
            upload_copy_time_us: upload_copy_started
                .map(|started| started.elapsed().as_micros() as u64)
                .unwrap_or(0),
            upload_bytes: if collect_stats {
                uploads
                    .iter()
                    .map(|(_, upload)| upload.pixels.len() as u64)
                    .sum()
            } else {
                0
            },
            ..TextAtlasBindGroupStats::default()
        };

        // One persistent texture array; each atlas page is a layer. Dirty rects are written
        // directly to their layer -- no ring rotation, no full-texture forward-copy.
        self.ensure_text_atlas_array(page_size, page_count)?;

        if !uploads.is_empty() {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            let cached = self
                .text_atlas_array
                .as_ref()
                .expect("text atlas array created above");
            let upload_write_started = collect_stats.then(Instant::now);
            clear_text_atlas_pages(
                &shared.device,
                &shared.queue,
                &cached.texture,
                uploads
                    .iter()
                    .filter(|(_, upload)| upload.clear_texture)
                    .map(|(page, _)| *page as u32),
            );
            for (page_index, upload) in &uploads {
                if upload.pixels.is_empty() {
                    continue;
                }
                shared.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &cached.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: upload.offset.0,
                            y: upload.offset.1,
                            z: *page_index as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &upload.pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(upload.extent.0 * 4),
                        rows_per_image: Some(upload.extent.1),
                    },
                    wgpu::Extent3d {
                        width: upload.extent.0,
                        height: upload.extent.1,
                        depth_or_array_layers: 1,
                    },
                );
            }
            stats.upload_write_time_us = upload_write_started
                .map(|started| started.elapsed().as_micros() as u64)
                .unwrap_or(0);
        }

        let bind_group = self
            .text_atlas_array
            .as_ref()
            .map(|cached| cached.bind_group.clone())
            .ok_or_else(|| Error::new("text atlas bind group requested before any atlas upload"))?;
        stats.total_time_us = total_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        Ok((bind_group, stats))
    }

    pub(crate) fn ensure_text_atlas_array(
        &mut self,
        page_size: (u32, u32),
        required_layers: u32,
    ) -> Result<()> {
        // Allocate only as many layers as there are live pages, growing on demand up to the page
        // budget. This keeps the common single-page case at one 16 MB layer instead of committing
        // the whole budget up front.
        let required_layers = required_layers.clamp(1, TEXT_ATLAS_MAX_PAGES as u32);
        if self
            .text_atlas_array
            .as_ref()
            .is_some_and(|cached| cached.size == page_size && cached.layers >= required_layers)
        {
            return Ok(());
        }

        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized before text atlas texture setup");

        let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI text atlas array texture"),
            size: wgpu::Extent3d {
                width: page_size.0,
                height: page_size.1,
                depth_or_array_layers: required_layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let existing_layers = self
            .text_atlas_array
            .as_ref()
            .filter(|old| old.size == page_size)
            .map_or(0, |old| old.layers);
        clear_text_atlas_pages(
            &shared.device,
            &shared.queue,
            &texture,
            existing_layers..required_layers,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let bind_group = shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI text atlas array bind group"),
            layout: &shared.text_atlas_array_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&shared.text_atlas_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });

        // Growing an existing array of the same page size: copy the already-populated layers
        // forward so their glyphs survive (their CPU dirty state was cleared after first upload).
        if let Some(old) = self.text_atlas_array.as_ref()
            && old.size == page_size
        {
            let mut encoder =
                shared
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("SUI text atlas array grow copy"),
                    });
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &old.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: page_size.0,
                    height: page_size.1,
                    depth_or_array_layers: old.layers,
                },
            );
            shared.queue.submit([encoder.finish()]);
        }

        self.text_atlas_array = Some(CachedTextAtlasTexture {
            texture,
            _view: view,
            bind_group,
            size: page_size,
            layers: required_layers,
        });

        Ok(())
    }

    pub(crate) fn prepare_analytic_path_resources(
        &mut self,
        analytic_paths: HashMap<u64, Arc<AnalyticPathCpuData>>,
        collect_stats: bool,
    ) -> Result<(
        Option<PreparedAnalyticPathResources>,
        AnalyticPathBindGroupStats,
    )> {
        if analytic_paths.is_empty() {
            return Ok((None, AnalyticPathBindGroupStats::default()));
        }

        let total_started = collect_stats.then(Instant::now);
        let shared = self
            .shared
            .as_ref()
            .expect("renderer shared state initialized");
        let mut slots = HashMap::with_capacity(analytic_paths.len());
        let mut pending = Vec::new();
        let mut visible_signatures = Vec::with_capacity(analytic_paths.len());
        let mut sorted_paths: Vec<_> = analytic_paths.into_iter().collect();
        sorted_paths.sort_unstable_by_key(|(signature, _)| *signature);

        for (signature, path) in sorted_paths {
            visible_signatures.push(signature);
            if let Some(cached) = self.analytic_path_cache.get_mut(&signature) {
                cached.last_used_frame = self.frames_rendered;
                slots.insert(signature, cached.slot);
            } else {
                pending.push((signature, path));
            }
        }

        let mut stats = AnalyticPathBindGroupStats {
            miss_count: if collect_stats { pending.len() } else { 0 },
            ..AnalyticPathBindGroupStats::default()
        };
        let needs_rebuild = if self
            .frame_resources
            .analytic_path_arena
            .bind_group
            .is_none()
        {
            true
        } else if pending.is_empty() {
            false
        } else {
            let required_slots =
                self.frame_resources.analytic_path_arena.used_slots + pending.len();
            let required_contours = self.frame_resources.analytic_path_arena.used_contours
                + pending
                    .iter()
                    .map(|(_, path)| path.contours.len())
                    .sum::<usize>();
            let required_points = self.frame_resources.analytic_path_arena.used_points
                + pending
                    .iter()
                    .map(|(_, path)| path.points.len())
                    .sum::<usize>();
            !self.frame_resources.analytic_path_arena.has_capacity(
                required_slots,
                required_contours,
                required_points,
            )
        };

        if needs_rebuild {
            let mut cached_entries: Vec<_> = self
                .analytic_path_cache
                .iter()
                .map(|(signature, entry)| {
                    (
                        *signature,
                        entry.slot,
                        entry.last_used_frame,
                        entry.data.clone(),
                    )
                })
                .collect();
            cached_entries.sort_unstable_by_key(|(_, slot, _, _)| *slot);

            let total_slots = cached_entries.len() + pending.len();
            let total_contours = cached_entries
                .iter()
                .map(|(_, _, _, data)| data.contours.len())
                .sum::<usize>()
                + pending
                    .iter()
                    .map(|(_, data)| data.contours.len())
                    .sum::<usize>();
            let total_points = cached_entries
                .iter()
                .map(|(_, _, _, data)| data.points.len())
                .sum::<usize>()
                + pending
                    .iter()
                    .map(|(_, data)| data.points.len())
                    .sum::<usize>();

            self.frame_resources.analytic_path_arena.ensure_capacity(
                &shared.device,
                &shared.analytic_path_bind_group_layout,
                total_slots,
                total_contours,
                total_points,
            );

            let mut meta_data = Vec::with_capacity(total_slots);
            let mut contour_data = Vec::with_capacity(total_contours);
            let mut point_data = Vec::with_capacity(total_points);
            let mut rebuilt_cache = HashMap::with_capacity(total_slots);

            for (signature, _, last_used_frame, data) in cached_entries {
                let slot = meta_data.len() as u32;
                let contour_start = contour_data.len() as u32;
                let point_start = point_data.len() as u32;
                meta_data.push(data.meta(contour_start, point_start));
                contour_data.extend_from_slice(&data.contours);
                point_data.extend_from_slice(&data.points);
                rebuilt_cache.insert(
                    signature,
                    CachedAnalyticPathGpu {
                        data,
                        slot,
                        last_used_frame,
                    },
                );
            }

            for (signature, data) in pending {
                let slot = meta_data.len() as u32;
                let contour_start = contour_data.len() as u32;
                let point_start = point_data.len() as u32;
                meta_data.push(data.meta(contour_start, point_start));
                contour_data.extend_from_slice(&data.contours);
                point_data.extend_from_slice(&data.points);
                rebuilt_cache.insert(
                    signature,
                    CachedAnalyticPathGpu {
                        data,
                        slot,
                        last_used_frame: self.frames_rendered,
                    },
                );
            }

            let meta_buffer = self
                .frame_resources
                .analytic_path_arena
                .meta_buffer
                .as_ref()
                .expect("analytic path arena metadata buffer initialized");
            let contour_buffer = self
                .frame_resources
                .analytic_path_arena
                .contour_buffer
                .as_ref()
                .expect("analytic path arena contour buffer initialized");
            let point_buffer = self
                .frame_resources
                .analytic_path_arena
                .point_buffer
                .as_ref()
                .expect("analytic path arena point buffer initialized");
            if !meta_data.is_empty() {
                self.frame_resources.uploads.write_buffer(
                    &shared.device,
                    meta_buffer,
                    0,
                    bytemuck::cast_slice(&meta_data),
                );
            }
            if !contour_data.is_empty() {
                self.frame_resources.uploads.write_buffer(
                    &shared.device,
                    contour_buffer,
                    0,
                    bytemuck::cast_slice(&contour_data),
                );
            }
            if !point_data.is_empty() {
                self.frame_resources.uploads.write_buffer(
                    &shared.device,
                    point_buffer,
                    0,
                    bytemuck::cast_slice(&point_data),
                );
            }

            if collect_stats {
                stats.upload_bytes = (meta_data.len() * std::mem::size_of::<AnalyticPathMetaGpu>()
                    + contour_data.len() * std::mem::size_of::<AnalyticContourGpu>()
                    + point_data.len() * std::mem::size_of::<AnalyticPointGpu>())
                    as u64;
            }

            self.analytic_path_cache = rebuilt_cache;
            self.frame_resources.analytic_path_arena.used_slots = meta_data.len();
            self.frame_resources.analytic_path_arena.used_contours = contour_data.len();
            self.frame_resources.analytic_path_arena.used_points = point_data.len();

            for signature in visible_signatures {
                let slot = self
                    .analytic_path_cache
                    .get(&signature)
                    .expect("visible analytic path cached after arena rebuild")
                    .slot;
                slots.insert(signature, slot);
            }
        } else if !pending.is_empty() {
            let meta_buffer = self
                .frame_resources
                .analytic_path_arena
                .meta_buffer
                .as_ref()
                .expect("analytic path arena metadata buffer initialized");
            let contour_buffer = self
                .frame_resources
                .analytic_path_arena
                .contour_buffer
                .as_ref()
                .expect("analytic path arena contour buffer initialized");
            let point_buffer = self
                .frame_resources
                .analytic_path_arena
                .point_buffer
                .as_ref()
                .expect("analytic path arena point buffer initialized");
            let base_slot = self.frame_resources.analytic_path_arena.used_slots as u32;
            let base_contour = self.frame_resources.analytic_path_arena.used_contours as u32;
            let base_point = self.frame_resources.analytic_path_arena.used_points as u32;
            let total_contours = pending
                .iter()
                .map(|(_, data)| data.contours.len())
                .sum::<usize>();
            let total_points = pending
                .iter()
                .map(|(_, data)| data.points.len())
                .sum::<usize>();
            let mut meta_data = Vec::with_capacity(pending.len());
            let mut contour_data = Vec::with_capacity(total_contours);
            let mut point_data = Vec::with_capacity(total_points);

            for (signature, data) in pending {
                let slot = base_slot + meta_data.len() as u32;
                let contour_start = base_contour + contour_data.len() as u32;
                let point_start = base_point + point_data.len() as u32;
                meta_data.push(data.meta(contour_start, point_start));
                contour_data.extend_from_slice(&data.contours);
                point_data.extend_from_slice(&data.points);
                if collect_stats {
                    stats.upload_bytes += data.byte_size() as u64;
                }
                slots.insert(signature, slot);
                self.analytic_path_cache.insert(
                    signature,
                    CachedAnalyticPathGpu {
                        data,
                        slot,
                        last_used_frame: self.frames_rendered,
                    },
                );
            }

            if !meta_data.is_empty() {
                let meta_offset =
                    base_slot as u64 * std::mem::size_of::<AnalyticPathMetaGpu>() as u64;
                self.frame_resources.uploads.write_buffer(
                    &shared.device,
                    meta_buffer,
                    meta_offset,
                    bytemuck::cast_slice(&meta_data),
                );
            }
            if !contour_data.is_empty() {
                let contour_offset =
                    base_contour as u64 * std::mem::size_of::<AnalyticContourGpu>() as u64;
                self.frame_resources.uploads.write_buffer(
                    &shared.device,
                    contour_buffer,
                    contour_offset,
                    bytemuck::cast_slice(&contour_data),
                );
            }
            if !point_data.is_empty() {
                let point_offset =
                    base_point as u64 * std::mem::size_of::<AnalyticPointGpu>() as u64;
                self.frame_resources.uploads.write_buffer(
                    &shared.device,
                    point_buffer,
                    point_offset,
                    bytemuck::cast_slice(&point_data),
                );
            }

            self.frame_resources.analytic_path_arena.used_slots += meta_data.len();
            self.frame_resources.analytic_path_arena.used_contours += contour_data.len();
            self.frame_resources.analytic_path_arena.used_points += point_data.len();
        }

        let bind_group = self
            .frame_resources
            .analytic_path_arena
            .bind_group
            .as_ref()
            .expect("analytic path arena bind group initialized")
            .clone();
        stats.total_time_us = total_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        Ok((
            Some(PreparedAnalyticPathResources { bind_group, slots }),
            stats,
        ))
    }
}

impl FrameResources {
    pub(crate) fn ensure_stencil(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let needs_recreate = self
            .stencil
            .as_ref()
            .is_none_or(|target| target.size != size);
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

impl AnalyticPathArena {
    pub(crate) fn has_capacity(
        &self,
        meta_count: usize,
        contour_count: usize,
        point_count: usize,
    ) -> bool {
        self.bind_group.is_some()
            && self.meta_capacity >= meta_count
            && self.contour_capacity >= contour_count
            && self.point_capacity >= point_count
    }

    pub(crate) fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        meta_count: usize,
        contour_count: usize,
        point_count: usize,
    ) {
        if self.has_capacity(meta_count, contour_count, point_count) {
            return;
        }

        let meta_capacity = grow_analytic_path_capacity(self.meta_capacity, meta_count);
        let contour_capacity = grow_analytic_path_capacity(self.contour_capacity, contour_count);
        let point_capacity = grow_analytic_path_capacity(self.point_capacity, point_count);

        let meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI analytic path metadata arena"),
            size: analytic_path_buffer_size::<AnalyticPathMetaGpu>(meta_capacity),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let contour_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI analytic path contour arena"),
            size: analytic_path_buffer_size::<AnalyticContourGpu>(contour_capacity),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SUI analytic path point arena"),
            size: analytic_path_buffer_size::<AnalyticPointGpu>(point_capacity),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SUI analytic path arena bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: contour_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: point_buffer.as_entire_binding(),
                },
            ],
        });

        self.bind_group = Some(bind_group);
        self.meta_buffer = Some(meta_buffer);
        self.contour_buffer = Some(contour_buffer);
        self.point_buffer = Some(point_buffer);
        self.meta_capacity = meta_capacity;
        self.contour_capacity = contour_capacity;
        self.point_capacity = point_capacity;
    }
}

pub(crate) struct CachedImageTexture {
    pub(crate) texture: wgpu::Texture,
    pub(crate) _view: wgpu::TextureView,
    pub(crate) linear_bind_group: wgpu::BindGroup,
    pub(crate) nearest_bind_group: wgpu::BindGroup,
    pub(crate) image: sui_scene::RegisteredImage,
    pub(crate) last_used_frame: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ImageTextureCacheKey {
    pub(crate) handle: sui_core::ImageHandle,
    pub(crate) raster_size: Option<ImageRasterSize>,
    pub(crate) mipmapped: bool,
}

pub(crate) struct CachedExternalTextureBindGroup {
    pub(crate) binding_revision: u64,
    pub(crate) linear_bind_group: wgpu::BindGroup,
    pub(crate) nearest_bind_group: wgpu::BindGroup,
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

pub(crate) struct OffscreenTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) size: (u32, u32),
}

/// Initialize fresh/recycled pages on the GPU, avoiding a full CPU zero-image copy.
fn clear_text_atlas_pages(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    pages: impl Iterator<Item = u32>,
) {
    let mut pages = pages.peekable();
    if pages.peek().is_none() {
        return;
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("SUI clear text atlas pages"),
    });
    for page in pages {
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: page,
            array_layer_count: Some(1),
            ..Default::default()
        });
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SUI clear text atlas page"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    queue.submit([encoder.finish()]);
}
